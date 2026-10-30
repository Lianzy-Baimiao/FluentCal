//! 0.2's src/lib/gantt.test.ts and src/lib/layout.test.ts.
use fluentcal_core::gantt::*;
use fluentcal_core::model::{Freq, Group, RepeatRule, Task};
use fluentcal_core::recur::occurrences;
use fluentcal_core::week::layout_week;
use std::collections::BTreeMap;

fn mk(id: &str, start: &str, end: &str) -> Task {
    Task { id: id.into(), title: id.into(), start: start.into(), end: end.into(), ..Default::default() }
}

#[test]
fn builds_group_task_and_add_rows() {
    let g1 = Group { id: "g1".into(), name: "项目 A".into(), color: "blue".into(), order: 2.0, ..Default::default() };
    let g2 = Group { id: "g2".into(), name: "项目 B".into(), color: "green".into(), order: 1.0, collapsed: Some(true), ..Default::default() };
    let a = Task { group_id: Some("g1".into()), progress: 50.0, ..mk("a", "2026-10-05", "2026-10-09") };
    let b = Task { group_id: Some("g1".into()), done: true, ..mk("b", "2026-10-01", "2026-10-02") };
    let c = Task { group_id: Some("g2".into()), ..mk("c", "2026-10-03", "2026-10-03") };
    let lone = mk("lone", "2026-10-04", "2026-10-04");
    let ms = Task { milestone: Some(true), ..mk("ms", "2026-10-20", "2026-10-20") };
    let opts = RowOptions { hide_done: false, hide_single_day: true, group_filter: vec![] };
    let rows = build_rows(&[a.clone(), b.clone(), c, lone, ms], &[g1.clone(), g2.clone()], &opts);
    let keys: Vec<String> = rows.iter().map(Row::key).collect();
    assert_eq!(keys, ["g:g2", "g:g1", "t:b", "t:a", "a:g1", "g:__ungrouped__", "t:ms", "a:__ungrouped__"]);
    let filtered = build_rows(&[a.clone(), b.clone()], &[g1, g2], &RowOptions { hide_done: true, group_filter: vec!["g1".into()], ..opts });
    assert_eq!(filtered.iter().map(Row::key).collect::<Vec<_>>(), ["g:g1", "t:a", "a:g1"]);
    // a: 5 days at 50%, b: 2 days done
    let (s, e, p) = summarize(&[a, b]);
    assert_eq!((s.as_deref(), e.as_deref(), p), (Some("2026-10-01"), Some("2026-10-09"), ((5.0 * 50.0 + 2.0 * 100.0) / 7.0f64).round()));
    assert_eq!(summarize(&[]), (None, None, 0.0));
}

#[test]
fn ranges_pixels_and_headers() {
    let a = mk("a", "2026-10-05", "2026-10-09");
    let ms = Task { milestone: Some(true), ..mk("ms", "2026-10-20", "2026-10-20") };
    let (s, e) = chart_range(&[a, ms], "2026-10-29", "day");
    assert_eq!(s, "2026-09-01");
    assert!(e.ends_with("-31") || e.ends_with("-30"));
    assert!(e.as_str() > "2027-01-01");
    let dw = day_width("day");
    assert_eq!(x_of("2026-10-11", "2026-10-01", dw), 10.0 * dw);
    assert_eq!(date_at(10.0 * dw + dw - 1.0, "2026-10-01", dw), "2026-10-11");
    assert_eq!(date_at(-1.0, "2026-10-01", dw), "2026-09-30");
    let (top, bottom) = header_tiers(("2026-10-01", "2026-11-30"), "day", "2026-10-29", 1);
    assert_eq!(top.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["2026年10月", "2026年11月"]);
    assert_eq!(bottom.len(), 61);
    assert_eq!(bottom.iter().find(|c| c.today).and_then(|c| c.date.as_deref()), Some("2026-10-29"));
    let (top, bottom) = header_tiers(("2026-10-01", "2027-03-31"), "month", "2026-10-29", 1);
    assert_eq!(top.iter().map(|c| c.label.as_str()).collect::<Vec<_>>(), ["2026年", "2027年"]);
    assert_eq!(bottom.len(), 6);
    assert_eq!(top[0].w + top[1].w, 182.0 * day_width("month"));
}

#[test]
fn rejects_dependency_links_that_would_loop_and_routes_arrows() {
    let x = mk("x", "2026-10-01", "2026-10-01");
    let y = Task { deps: Some(vec!["x".into()]), ..mk("y", "2026-10-02", "2026-10-02") };
    let z = Task { deps: Some(vec!["y".into()]), ..mk("z", "2026-10-03", "2026-10-03") };
    let r = Task { repeat: Some(RepeatRule { freq: Some(Freq::Daily), ..Default::default() }), ..mk("r", "2026-10-03", "2026-10-03") };
    let all: BTreeMap<String, Task> = [x, y, z, r].into_iter().map(|t| (t.id.clone(), t)).collect();
    assert!(!can_link(&all, "z", "x")); // x → y → z → x
    assert!(can_link(&all, "x", "z"));
    assert!(!can_link(&all, "x", "y")); // already linked
    assert!(!can_link(&all, "x", "x"));
    assert!(!can_link(&all, "x", "r")); // repeating tasks have no single bar
    assert!(would_cycle(&all, "y", "x"));
    assert_eq!(dep_path(100.0, 16.0, 200.0, 48.0), "M100,16 H108 V48 H200");
    assert!(dep_path(100.0, 16.0, 90.0, 48.0).contains("V32"));
}

const WEEK: &str = "2026-10-26"; // Monday

#[test]
fn week_layout_lanes_overflow_and_clipping() {
    let bar = mk("bar", "2026-10-26", "2026-10-28");
    let mon1 = Task { order: 1.0, ..mk("mon1", "2026-10-26", "2026-10-26") };
    let mon2 = Task { order: 2.0, ..mk("mon2", "2026-10-26", "2026-10-26") };
    let tue = mk("tue", "2026-10-27", "2026-10-27");
    let all = [mon1, tue, bar, mon2];
    let occs = || all.iter().flat_map(|t| occurrences(t, WEEK, "2026-11-01", 400)).collect::<Vec<_>>();
    let l = layout_week(WEEK, occs(), None, true);
    let by = |id: &str| l.chips.iter().find(|c| c.occ.task.id == id).unwrap().clone();
    assert_eq!((by("bar").col, by("bar").span, by("bar").lane, by("bar").multi_day), (0, 3, 0, true));
    assert_eq!(by("mon1").lane, 1);
    assert_eq!(by("mon2").lane, 2);
    assert_eq!((by("tue").col, by("tue").lane), (1, 1));

    let l = layout_week(WEEK, occs(), Some(2), true);
    let by = |id: &str| l.chips.iter().find(|c| c.occ.task.id == id).unwrap().visible;
    assert!(by("bar") && !by("mon1") && !by("mon2") && by("tue"));
    assert_eq!((l.overflow[0], l.overflow_lane[0], l.overflow[1]), (2, 1, 0));

    let long = mk("long", "2026-10-24", "2026-11-03");
    let c = &layout_week(WEEK, occurrences(&long, WEEK, "2026-11-01", 400), None, true).chips[0];
    assert_eq!((c.col, c.span, c.cont_left, c.cont_right), (0, 7, true, true));

    let a = Task { done: true, ..mk("a", "2026-10-29", "2026-10-29") };
    let b = mk("b", "2026-10-29", "2026-10-29");
    let both: Vec<_> = [&a, &b].iter().flat_map(|t| occurrences(t, WEEK, "2026-11-01", 400)).collect();
    let l = layout_week(WEEK, both, None, true);
    assert_eq!(l.chips.iter().map(|c| c.occ.task.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
}
