//! Task operations and undo/redo (in 0.2 these were covered by the e2e tests).
use fluentcal_core::history::History;
use fluentcal_core::model::{Freq, Group, RepeatRule, Task};
use fluentcal_core::recur::occurrence_on;
use fluentcal_core::tasks::*;
use serde_json::Value;

fn mk(id: &str, start: &str) -> Task {
    Task { id: id.into(), title: id.into(), start: start.into(), end: start.into(), order: 1.0, ..Default::default() }
}

fn doc_with(tasks: Vec<Task>, groups: Vec<Group>) -> Doc {
    Doc { tasks: tasks.into_iter().map(|t| (t.id.clone(), t)).collect(), groups: groups.into_iter().map(|g| (g.id.clone(), g)).collect(), ..Default::default() }
}

/// Applies an edit to the typed document, as the store + Doc::apply would.
fn apply(doc: &mut Doc, e: &Edit) {
    for (c, id, v) in &e.items {
        match (c.as_str(), v) {
            ("tasks", Some(v)) => drop(doc.tasks.insert(id.clone(), serde_json::from_value(v.clone()).unwrap())),
            ("tasks", None) => drop(doc.tasks.remove(id)),
            ("groups", Some(v)) => drop(doc.groups.insert(id.clone(), serde_json::from_value(v.clone()).unwrap())),
            ("groups", None) => drop(doc.groups.remove(id)),
            _ => {}
        }
    }
}

#[test]
fn clears_sample_data_but_keeps_what_the_user_renamed() {
    let g = Group { id: "g".into(), name: "示例项目".into(), sample: Some(true), order: 1.0, ..Default::default() };
    let a = Task { sample: Some(true), group_id: Some("g".into()), ..mk("a", "2026-10-01") };
    let b = Task { sample: Some(true), group_id: Some("g".into()), deps: Some(vec!["a".into()]), ..mk("b", "2026-10-02") };
    let mine = Task { group_id: Some("g".into()), deps: Some(vec!["b".into()]), ..mk("mine", "2026-10-03") };
    let mut doc = doc_with(vec![a, b.clone(), mine], vec![g]);
    // Renaming a sample task makes it the user's own.
    let renamed = save_task(&doc, Task { title: "我的任务".into(), ..b }, 5);
    apply(&mut doc, &renamed);
    assert_eq!(doc.tasks["b"].sample, None);
    assert_eq!(sample_count(&doc), 2); // a and the group
    let (e, n) = clear_sample_data(&doc, 6);
    apply(&mut doc, &e);
    assert_eq!(n, 1);
    assert!(!doc.tasks.contains_key("a") && doc.groups.is_empty());
    assert_eq!(doc.tasks["b"].deps(), &[] as &[String], "predecessor a is gone");
    assert_eq!(doc.tasks["mine"].group_id, None, "the user's task is ungrouped, not deleted");
}

#[test]
fn deletes_several_groups_at_once_and_their_tasks_become_ungrouped() {
    let g = |id: &str| Group { id: id.into(), name: id.into(), order: 1.0, ..Default::default() };
    let a = Task { group_id: Some("g1".into()), ..mk("a", "2026-10-01") };
    let b = Task { group_id: Some("g2".into()), ..mk("b", "2026-10-02") };
    let c = Task { group_id: Some("g3".into()), ..mk("c", "2026-10-03") };
    let mut doc = doc_with(vec![a, b, c, mk("free", "2026-10-04")], vec![g("g1"), g("g2"), g("g3")]);
    // "gone" was deleted meanwhile (sync); a repeated id counts once.
    let ids = ["g1", "gone", "g2", "g1"].map(String::from);
    let e = delete_groups(&doc, &ids, 7);
    let groups: Vec<&str> = e.items.iter().filter(|(c, _, _)| c == "groups").map(|(_, id, _)| id.as_str()).collect();
    assert_eq!(groups, ["g1", "g2"], "one delete per existing group");
    assert_eq!(e.items.len(), 4, "two tasks ungrouped, two groups deleted");
    apply(&mut doc, &e);
    assert_eq!(doc.groups.keys().collect::<Vec<_>>(), ["g3"]);
    assert_eq!((doc.tasks["a"].group_id.as_deref(), doc.tasks["a"].updated_at), (None, 7));
    assert_eq!(doc.tasks["b"].group_id, None);
    assert_eq!(doc.tasks["c"].group_id.as_deref(), Some("g3"), "other groups keep their tasks");
    assert_eq!(doc.tasks["free"].updated_at, 0, "untouched");
    assert!(delete_groups(&doc, &["gone".into()], 8).is_empty());
}

#[test]
fn moving_an_occurrence_detaches_it_and_deleting_cleans_up() {
    let weekly = Task { repeat: Some(RepeatRule { freq: Some(Freq::Weekly), ..Default::default() }), color: Some("teal".into()), ..mk("w", "2026-10-05") };
    let after = Task { deps: Some(vec!["w".into()]), ..mk("after", "2026-10-20") };
    let mut doc = doc_with(vec![weekly, after], vec![]);
    let occ = occurrence_on(&doc.tasks["w"], "2026-10-12").unwrap();
    let e = move_occurrence(&doc, &occ, "2026-10-14", 7);
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["w"].ex_dates.as_deref(), Some(&["2026-10-12".to_owned()][..]));
    let copy = doc.tasks.values().find(|t| t.id != "w" && t.id != "after").unwrap();
    assert_eq!((copy.start.as_str(), copy.repeat.is_none(), copy.color.as_deref()), ("2026-10-14", true, Some("teal")));
    assert_eq!(copy.id.len(), 36, "a new UUID");
    let e = delete_task(&doc, "w", 8);
    apply(&mut doc, &e);
    assert!(!doc.tasks.contains_key("w"));
    assert!(doc.tasks["after"].deps().is_empty(), "removed from successors");
    // Delete "this and later" ends the series the day before.
    let daily = Task { repeat: Some(RepeatRule { freq: Some(Freq::Daily), count: Some(9), ..Default::default() }), ..mk("d", "2026-10-01") };
    let e = delete_occurrence(&doc, &daily, "2026-10-05", DeleteMode::Future, 9);
    let r = serde_json::from_value::<Task>(e.items[0].2.clone().unwrap()).unwrap().repeat.unwrap();
    assert_eq!((r.count, r.until.as_deref()), (None, Some("2026-10-04")));
}

#[test]
fn done_progress_and_auto_postpone() {
    let mut doc = doc_with(vec![mk("t", "2026-10-25"), Task { auto_postpone: Some(true), ..mk("late", "2026-10-25") }], vec![]);
    let e = set_occurrence_done(&doc, &doc.tasks["t"].clone(), "2026-10-25", true, 1);
    apply(&mut doc, &e);
    assert!(doc.tasks["t"].done && doc.tasks["t"].progress == 100.0);
    let e = set_progress(&doc, &doc.tasks["t"].clone(), 40.4, 2);
    apply(&mut doc, &e);
    assert_eq!((doc.tasks["t"].progress, doc.tasks["t"].done), (40.0, false));
    let e = auto_postpone(&doc, "2026-10-29", 3);
    assert_eq!(e.items.len(), 1);
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["late"].start, "2026-10-29");
}

#[test]
fn undo_redo_batches_and_skips_items_changed_since() {
    let mut doc = doc_with(vec![mk("a", "2026-10-01")], vec![]);
    let mut h = History::default();
    let raw = |d: &Doc| { let d = d.clone(); move |c: &str, id: &str| d.raw(c, id) };
    // Two writes within 400 ms are one step.
    let e1 = set_color(&doc, &doc.tasks["a"].clone(), "red", 1000);
    h.record(&e1, raw(&doc), 1000);
    apply(&mut doc, &e1);
    let e2 = move_task(&doc, &doc.tasks["a"].clone(), "2026-10-05", 1100);
    h.record(&e2, raw(&doc), 1100);
    apply(&mut doc, &e2);
    let r = h.undo(raw(&doc));
    apply(&mut doc, &r.edit);
    assert!(r.done && r.skipped == 0);
    assert_eq!((doc.tasks["a"].start.as_str(), doc.tasks["a"].color.as_deref()), ("2026-10-01", None));
    assert!(h.can_redo() && !h.can_undo());
    let r = h.redo(raw(&doc));
    apply(&mut doc, &r.edit);
    assert_eq!((doc.tasks["a"].start.as_str(), doc.tasks["a"].color.as_deref()), ("2026-10-05", Some("red")));

    // A boundary (click) starts a new step; an item changed elsewhere is left alone.
    h.boundary();
    let (e3, added) = add_task(&doc, Task { title: "新".into(), start: "2026-10-09".into(), ..Default::default() }, None, 5000);
    h.record(&e3, raw(&doc), 5000);
    apply(&mut doc, &e3);
    let mut elsewhere = doc.tasks[&added.id].clone();
    elsewhere.title = "另一台电脑改的".into();
    doc.tasks.insert(added.id.clone(), elsewhere);
    let r = h.undo(raw(&doc));
    assert_eq!((r.skipped, r.edit.items.len()), (1, 0));
    assert!(doc.tasks.contains_key(&added.id));
    let _: Value = doc.raw("tasks", "nope");
}

#[test]
fn merge_edit_keeps_changes_made_elsewhere() {
    use fluentcal_core::tasks::merge_edit;
    let base: Task = serde_json::from_value(serde_json::json!({ "id": "a", "title": "写报告", "start": "2026-10-10", "end": "2026-10-12", "progress": 0, "done": false, "order": 1 })).unwrap();
    // The editor changed the title and cleared the end; meanwhile the task was ticked done and
    // moved in the Gantt chart.
    let mut draft = base.clone();
    draft.title = "写年度报告".into();
    let mut current = base.clone();
    current.done = true;
    current.start = "2026-10-11".into();
    current.end = "2026-10-13".into();
    let m = merge_edit(&base, &draft, &current);
    assert_eq!(m.title, "写年度报告");
    assert!(m.done);
    assert_eq!((m.start.as_str(), m.end.as_str()), ("2026-10-11", "2026-10-13"));
    // A field the editor removed is removed; a milestone ends where it starts.
    let mut base2 = base.clone();
    base2.notes = Some("带上 U 盘".into());
    let mut draft2 = base2.clone();
    draft2.notes = None;
    draft2.milestone = Some(true);
    let m2 = merge_edit(&base2, &draft2, &base2);
    assert_eq!(m2.notes, None);
    assert_eq!(m2.end, m2.start);
}
