//! What the Gantt view shows (0.2's gantt/model.ts): rows, bars, summaries, links and the
//! header, for the whole chart; mod.rs hands the visible part to the window.

use fluentcal_core::i18n::{tr, trf};

use crate::theme::color;
use crate::{GBar, GCell, GLink, GRow, GStripe, GSummary};
use fluentcal_core::date::{add_days, diff_days, format_range, parts};
use fluentcal_core::gantt::{build_rows, chart_range, day_width, Row, RowOptions, ROW_HEIGHT, UNGROUPED};
use fluentcal_core::model::{Group, Task};
use fluentcal_core::recur::occurrences;
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::{task_color, Tokens};

/// A bar with the numbers the drag code needs (chart px, unscaled f64).
#[derive(Clone)]
pub struct Bar {
    pub g: GBar,
    pub start: String,
    pub end: String,
    pub x: f64,
    pub w: f64,
}

#[derive(Default)]
pub struct Model {
    pub rows: Vec<GRow>,
    pub bars: Vec<Bar>,
    pub summaries: Vec<GSummary>,
    pub links: Vec<GLink>,
    pub top: Vec<GCell>,
    pub bottom: Vec<GCell>,
    pub stripes: Vec<GStripe>,
    pub month_lines: Vec<f64>,
    /// x of the first week line and the week period (none in the month scale).
    pub week: Option<(f64, f64)>,
    pub range: (String, String),
    pub dw: f64,
    pub chart_w: f64,
    pub today_x: f64,
}

/// Table width: setting (or the splitter being dragged), 0 when the table is hidden.
pub fn table_width(doc: &Doc, live: Option<f64>) -> f64 {
    let g = &doc.settings.gantt;
    if g.show_table { live.unwrap_or(g.table_width).clamp(140.0, 480.0) } else { 0.0 }
}

fn md(d: &str) -> String {
    let (_, m, dd) = parts(d);
    format!("{m}/{dd}")
}

/// Approximate text width in px at 12 px (0.2's charW): whether a title fits inside a bar.
pub fn char_w(s: &str) -> f64 {
    s.chars().map(|c| if c as u32 > 0x2e80 { 12.0 } else { 6.6 }).sum()
}

fn group_of<'a>(doc: &'a Doc, id: Option<&str>) -> Option<&'a Group> {
    id.and_then(|g| doc.groups.get(g))
}

/// `ungrouped_collapsed`: 未分组 is collapsed in this window only (0.2: not saved).
pub fn compute(doc: &Doc, today: &str, t: &Tokens, ungrouped_collapsed: bool) -> Model {
    let g = &doc.settings.gantt;
    let tasks: Vec<Task> = doc.tasks.values().cloned().collect();
    let mut groups: Vec<Group> = doc.groups.values().cloned().collect();
    groups.sort_by(|a, b| a.order.total_cmp(&b.order));
    let opts = RowOptions { hide_done: g.hide_done, hide_single_day: g.hide_single_day, group_filter: g.group_filter.clone() };
    let mut rows = build_rows(&tasks, &groups, &opts);
    if ungrouped_collapsed {
        rows.retain(|r| match r {
            Row::Group { .. } => true,
            Row::Task { group_id, .. } | Row::Add { group_id } => group_id != UNGROUPED,
        });
    }
    let dw = day_width(&g.scale);
    let single: Vec<Task> = tasks.iter().filter(|t| t.repeat.is_none()).cloned().collect();
    let (rs, re) = chart_range(&single, today, &g.scale);
    let (lo, hi) = (add_days(today, -3 * 366), add_days(today, 5 * 366));
    let range = (if rs < lo { lo } else { rs }, if re > hi { hi } else { re });
    let days = (diff_days(&range.0, &range.1) + 1) as f64;
    let x_of = |d: &str| diff_days(&range.0, d) as f64 * dw;
    let mut m = Model { dw, chart_w: days * dw, today_x: x_of(today) + dw / 2.0, ..Default::default() };
    for (i, r) in rows.iter().enumerate() {
        m.rows.push(row(r, t, ungrouped_collapsed));
        match r {
            Row::Group { group, start: Some(s), end: Some(e), progress, .. } => m.summaries.push(GSummary {
                key: format!("g:{}", group.id).into(),
                row: i as i32,
                x: x_of(s) as f32,
                w: ((diff_days(s, e) + 1) as f64 * dw) as f32,
                progress: *progress as f32,
                color: color(task_color(Some(&group.color), t).bar),
            }),
            Row::Task { task, .. } => bars_of(doc, task, i, &range, dw, today, t, &mut m.bars),
            _ => {}
        }
    }
    m.links = super::geom::links(doc, &m.bars);
    let (top, bottom) = super::geom::tiers(doc, &range, today);
    m.month_lines = (if g.scale == "month" { &bottom } else { &top }).iter().map(|c| f64::from(c.x)).filter(|x| *x > 0.0).collect();
    m.top = top;
    m.bottom = bottom;
    m.stripes = super::geom::stripes(doc, &range, dw);
    if g.scale != "month" {
        let wd = fluentcal_core::date::weekday(&range.0);
        let off = ((doc.settings.calendar.week_start as i32 - wd as i32).rem_euclid(7)) as f64 * dw;
        m.week = Some((off, 7.0 * dw));
    }
    m.range = range;
    m
}

fn row(r: &Row, t: &Tokens, ungrouped_collapsed: bool) -> GRow {
    match r {
        Row::Group { group, tasks, start, progress, .. } => GRow {
            kind: "group".into(),
            key: r.key().into(),
            id: group.id.clone().into(),
            name: group.name.clone().into(),
            color: color(task_color(Some(&group.color), t).bar),
            collapsed: if group.id == UNGROUPED { ungrouped_collapsed } else { group.collapsed == Some(true) },
            meta: tasks.len().to_string().into(),
            pct: if start.is_some() { format!("{progress}%").into() } else { "".into() },
            real: group.id != UNGROUPED,
            ..Default::default()
        },
        Row::Task { task, .. } => {
            let c = task_color(task.color.as_deref(), t);
            let dates = if task.start == task.end { md(&task.start) } else { format!("{}–{}", md(&task.start), md(&task.end)) };
            let pct = if task.is_milestone() || task.repeat.is_some() { String::new() } else { format!("{}%", if task.done { 100.0 } else { task.progress }) };
            GRow {
                kind: "task".into(),
                key: r.key().into(),
                id: task.id.clone().into(),
                name: task.title.clone().into(),
                color: color(c.bar),
                check_ring: color(c.bar.mix(0.7, t.text_secondary)),
                meta: dates.into(),
                meta_tip: trf("{0} 天", &[format!("{}", diff_days(&task.start, &task.end) + 1)]).into(),
                pct: pct.into(),
                checked: task.done,
                done: task.done && task.repeat.is_none(),
                repeat: task.repeat.is_some(),
                milestone: task.is_milestone(),
                real: true,
                ..Default::default()
            }
        }
        Row::Add { group_id } => GRow { kind: "add".into(), key: r.key().into(), id: group_id.clone().into(), ..Default::default() },
    }
}

#[allow(clippy::too_many_arguments)]
fn bars_of(doc: &Doc, task: &Task, row: usize, range: &(String, String), dw: f64, today: &str, t: &Tokens, out: &mut Vec<Bar>) {
    let key = task.color.clone().filter(|c| !c.is_empty()).or_else(|| group_of(doc, task.group_id.as_deref()).map(|g| g.color.clone()));
    let c = task_color(key.as_deref(), t);
    let rep = task.repeat.is_some();
    let occ: Vec<(String, String, String, bool)> = if rep {
        occurrences(task, &range.0, &range.1, 3000).into_iter().map(|o| (o.date, o.start, o.end, o.done)).collect()
    } else {
        vec![(task.start.clone(), task.start.clone(), task.end.clone(), task.done)]
    };
    let deps: Vec<String> = task.deps().iter().filter_map(|id| doc.tasks.get(id).map(|d| d.title.clone())).collect();
    for (date, start, end, done) in occ {
        let x = diff_days(&range.0, &start) as f64 * dw;
        let w = (diff_days(&start, &end) + 1) as f64 * dw;
        let progress = if done { 100.0 } else if rep { 0.0 } else { task.progress };
        let status = if task.is_milestone() { tr("里程碑").to_owned() } else if rep { tr("重复任务（双击编辑整个系列）").to_owned() } else { trf("进度 {0}%", &[format!("{}", progress)]) };
        let mut tip = vec![task.title.clone(), trf("{0} · {1} 天", &[format!("{}", format_range(&start, &end, today)), format!("{}", diff_days(&start, &end) + 1)]), status];
        if !deps.is_empty() {
            tip.push(trf("前置：{0}", &[format!("{}", deps.join("、"))]));
        }
        out.push(Bar {
            g: GBar {
                key: if rep { format!("{}@{date}", task.id) } else { task.id.clone() }.into(),
                task_id: task.id.clone().into(),
                date: date.into(),
                row: row as i32,
                x: x as f32,
                w: w as f32,
                progress: progress as f32,
                done,
                rep,
                milestone: task.is_milestone(),
                title: task.title.clone().into(),
                inside: !task.is_milestone() && char_w(&task.title) + 14.0 <= w,
                bar: color(c.bar),
                on_bar: color(c.on_bar),
                text: color(c.text),
                rep_outline: color(c.bar.mix(0.6, t.text_primary)),
                tip: tip.join("\n").into(),
            },
            start,
            end,
            x,
            w,
        });
    }
}

pub const RH: f64 = ROW_HEIGHT;
