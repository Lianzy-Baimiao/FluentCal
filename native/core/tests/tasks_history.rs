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
fn deletes_several_groups_with_their_tasks() {
    let g = |id: &str| Group { id: id.into(), name: id.into(), order: 1.0, ..Default::default() };
    let a = Task { group_id: Some("g1".into()), ..mk("a", "2026-10-01") };
    let b = Task { group_id: Some("g2".into()), deps: Some(vec!["a".into()]), ..mk("b", "2026-10-02") };
    // c (another group) and free (no group) depend on deleted tasks: they lose those links only.
    let c = Task { group_id: Some("g3".into()), deps: Some(vec!["b".into(), "free".into()]), ..mk("c", "2026-10-03") };
    let free = Task { deps: Some(vec!["a".into()]), ..mk("free", "2026-10-04") };
    let mut doc = doc_with(vec![a, b, c, free, mk("other", "2026-10-05")], vec![g("g1"), g("g2"), g("g3")]);
    let ids = ["g1", "gone", "g2"].map(String::from);
    assert_eq!(tasks_in_groups(&doc, &ids), 2);
    let e = delete_groups(&doc, &ids, true, 7);
    apply(&mut doc, &e);
    assert_eq!(doc.groups.keys().collect::<Vec<_>>(), ["g3"]);
    assert_eq!(doc.tasks.keys().collect::<Vec<_>>(), ["c", "free", "other"], "the groups' tasks are gone");
    assert_eq!(doc.tasks["c"].deps(), &["free".to_owned()][..]);
    assert_eq!((doc.tasks["free"].deps().len(), doc.tasks["free"].updated_at), (0, 7));
    assert_eq!(doc.tasks["other"].updated_at, 0, "untouched");
    assert_eq!(e.items.len(), 6, "2 tasks and 2 groups deleted, 2 tasks rewritten");
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
    let e = delete_groups(&doc, &ids, false, 7);
    let groups: Vec<&str> = e.items.iter().filter(|(c, _, _)| c == "groups").map(|(_, id, _)| id.as_str()).collect();
    assert_eq!(groups, ["g1", "g2"], "one delete per existing group");
    assert_eq!(e.items.len(), 4, "two tasks ungrouped, two groups deleted");
    apply(&mut doc, &e);
    assert_eq!(doc.groups.keys().collect::<Vec<_>>(), ["g3"]);
    assert_eq!((doc.tasks["a"].group_id.as_deref(), doc.tasks["a"].updated_at), (None, 7));
    assert_eq!(doc.tasks["b"].group_id, None);
    assert_eq!(doc.tasks["c"].group_id.as_deref(), Some("g3"), "other groups keep their tasks");
    assert_eq!(doc.tasks["free"].updated_at, 0, "untouched");
    assert!(delete_groups(&doc, &["gone".into()], true, 8).is_empty());
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

/// 10:00 on a day, local time.
fn at(day: &str) -> i64 {
    fluentcal_core::date::local_ms(day, 600, fluentcal_core::date::utc_offset_minutes())
}

// GitHub #15
#[test]
fn the_day_a_task_was_ticked_done_is_kept() {
    let visit = Task { end: "2026-11-25".into(), ..mk("visit", "2026-11-23") };
    let mut doc = doc_with(vec![visit], vec![]);
    let tick = |doc: &mut Doc, done: bool, day: &str| {
        let e = set_occurrence_done(doc, &doc.tasks["visit"].clone(), "2026-11-23", done, at(day));
        apply(doc, &e);
    };
    // ticked two days after it ended: that day, not one of its own
    tick(&mut doc, true, "2026-11-27");
    assert_eq!(doc.tasks["visit"].done_on.as_deref(), Some("2026-11-27"));
    assert_eq!(done_day(&doc.tasks["visit"], "2026-11-23"), Some("2026-11-27"));
    assert_eq!(doc.tasks["visit"].to_value()["doneOn"], "2026-11-27");
    // other edits leave it alone (a colour the next day, a move)
    let e = set_color(&doc, &doc.tasks["visit"].clone(), "red", at("2026-11-28"));
    apply(&mut doc, &e);
    let e = move_task(&doc, &doc.tasks["visit"].clone(), "2026-11-24", at("2026-11-28"));
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["visit"].done_on.as_deref(), Some("2026-11-27"));
    // corrected by hand
    let e = set_done_day(&doc, &doc.tasks["visit"].clone(), "2026-11-24", "2026-11-25", at("2026-11-28"));
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["visit"].done_on.as_deref(), Some("2026-11-25"));
    assert!(set_done_day(&doc, &doc.tasks["visit"].clone(), "2026-11-24", "2026-11-25", at("2026-11-28")).is_empty(), "the same day: nothing to write");
    // nothing was done on a day that has not come; today itself is fine
    assert!(set_done_day(&doc, &doc.tasks["visit"].clone(), "2026-11-24", "2026-11-29", at("2026-11-28")).is_empty());
    assert!(set_done_day(&doc, &doc.tasks["visit"].clone(), "2026-11-24", "11月25日", at("2026-11-28")).is_empty(), "not a day at all");
    assert!(!set_done_day(&doc, &doc.tasks["visit"].clone(), "2026-11-24", "2026-11-28", at("2026-11-28")).is_empty());
    // unticked: gone; ticked again: the new day, whatever was set by hand
    tick(&mut doc, false, "2026-11-29");
    assert_eq!(doc.tasks["visit"].done_on, None);
    assert_eq!(doc.tasks["visit"].to_value().get("doneOn"), None, "not written at all");
    assert_eq!(done_day(&doc.tasks["visit"], "2026-11-24"), None);
    assert!(set_done_day(&doc, &doc.tasks["visit"].clone(), "2026-11-24", "2026-11-25", at("2026-11-29")).is_empty(), "not done: no day to correct");
    tick(&mut doc, true, "2026-11-30");
    assert_eq!(doc.tasks["visit"].done_on.as_deref(), Some("2026-11-30"));
    // progress: below 100 % is an untick, 100 % a tick
    let e = set_progress(&doc, &doc.tasks["visit"].clone(), 60.0, at("2026-12-01"));
    apply(&mut doc, &e);
    assert_eq!((doc.tasks["visit"].done, doc.tasks["visit"].done_on.clone()), (false, None));
    let e = set_progress(&doc, &doc.tasks["visit"].clone(), 100.0, at("2026-12-02"));
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["visit"].done_on.as_deref(), Some("2026-12-02"));
}

#[test]
fn a_task_done_before_the_day_was_kept_has_none() {
    let old = Task { done: true, progress: 100.0, ..mk("old", "2026-10-01") };
    let mut doc = doc_with(vec![old], vec![]);
    let e = set_color(&doc, &doc.tasks["old"].clone(), "teal", at("2026-11-27"));
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["old"].done_on, None, "not made up");
    assert_eq!(done_day(&doc.tasks["old"], "2026-10-01"), None);
    // … until the user says when
    let e = set_done_day(&doc, &doc.tasks["old"].clone(), "2026-10-01", "2026-10-02", at("2026-11-27"));
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["old"].done_on.as_deref(), Some("2026-10-02"));
    // a copy was done when the task was; a new task made done is done today, or when it says
    let (_, copy) = duplicate_task(&doc, &doc.tasks["old"].clone(), at("2026-11-28"));
    assert_eq!((copy.done, copy.done_on.as_deref()), (true, Some("2026-10-02")));
    let (_, new) = add_task(&doc, Task { done: true, ..mk("", "2026-11-20") }, None, at("2026-11-28"));
    assert_eq!(new.done_on.as_deref(), Some("2026-11-28"));
    let (_, named) = add_task(&doc, Task { done: true, done_on: Some("2026-11-21".into()), ..mk("", "2026-11-20") }, None, at("2026-11-28"));
    assert_eq!(named.done_on.as_deref(), Some("2026-11-21"));
    // a day on a task that isn't done means nothing: dropped
    let (_, open) = add_task(&doc, Task { done_on: Some("2026-11-21".into()), ..mk("", "2026-11-20") }, None, at("2026-11-28"));
    assert_eq!(open.done_on, None);
    // a version that doesn't know the field unticked the task (the day it left behind is
    // stale): ticking it here is a new day
    let stale = Task { done: false, done_on: Some("2026-10-02".into()), ..doc.tasks["old"].clone() };
    doc.tasks.insert("old".into(), stale);
    let e = set_occurrence_done(&doc, &doc.tasks["old"].clone(), "2026-10-01", true, at("2026-11-29"));
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["old"].done_on.as_deref(), Some("2026-11-29"));
}

#[test]
fn a_repeating_task_keeps_the_day_of_each_time() {
    let weekly = Task { repeat: Some(RepeatRule { freq: Some(Freq::Weekly), ..Default::default() }), end: "2026-11-03".into(), ..mk("w", "2026-11-02") };
    let mut doc = doc_with(vec![weekly], vec![]);
    let tick = |doc: &mut Doc, date: &str, done: bool, day: &str| {
        let e = set_occurrence_done(doc, &doc.tasks["w"].clone(), date, done, at(day));
        apply(doc, &e);
    };
    tick(&mut doc, "2026-11-02", true, "2026-11-04");
    tick(&mut doc, "2026-11-09", true, "2026-11-09");
    let w = doc.tasks["w"].clone();
    assert_eq!((done_day(&w, "2026-11-02"), done_day(&w, "2026-11-09"), done_day(&w, "2026-11-16")), (Some("2026-11-04"), Some("2026-11-09"), None));
    assert_eq!((w.done, w.done_on.clone()), (false, None), "the series itself is never done");
    // by hand, for one of them; one that isn't done has no day to correct
    let e = set_done_day(&doc, &w, "2026-11-02", "2026-11-03", at("2026-11-10"));
    apply(&mut doc, &e);
    assert_eq!(done_day(&doc.tasks["w"], "2026-11-02"), Some("2026-11-03"));
    assert!(set_done_day(&doc, &doc.tasks["w"].clone(), "2026-11-16", "2026-11-16", at("2026-11-10")).is_empty());
    // unticking one forgets its day only
    tick(&mut doc, "2026-11-02", false, "2026-11-11");
    let w = doc.tasks["w"].clone();
    assert_eq!((done_day(&w, "2026-11-02"), done_day(&w, "2026-11-09")), (None, Some("2026-11-09")));
    assert_eq!(w.to_value()["doneOnDates"], serde_json::json!({ "2026-11-09": "2026-11-09" }));
    // an occurrence ticked before the days were kept has none, and ticking another leaves it so
    let mut before = w.clone();
    before.done_dates.as_mut().unwrap().push("2026-11-16".into());
    doc.tasks.insert("w".into(), before);
    tick(&mut doc, "2026-11-23", true, "2026-11-24");
    let w = doc.tasks["w"].clone();
    assert_eq!((done_day(&w, "2026-11-16"), done_day(&w, "2026-11-23")), (None, Some("2026-11-24")));
    // a copy has the same times done, on the days the task has them: a day that isn't known is
    // not made up for the copy either
    let (_, copy) = duplicate_task(&doc, &w, at("2026-11-25"));
    assert_eq!(copy.done_dates, w.done_dates);
    assert_eq!((done_day(&copy, "2026-11-09"), done_day(&copy, "2026-11-16"), done_day(&copy, "2026-11-23")), (Some("2026-11-09"), None, Some("2026-11-24")));
    // dragged out of the series: the task it becomes was done when the occurrence was
    let occ = occurrence_on(&doc.tasks["w"], "2026-11-09").unwrap();
    let e = move_occurrence(&doc, &occ, "2026-11-12", at("2026-11-26"));
    apply(&mut doc, &e);
    let copy = doc.tasks.values().find(|t| t.id != "w").unwrap();
    assert_eq!((copy.done, copy.done_on.as_deref(), copy.done_on_dates.is_none()), (true, Some("2026-11-09"), true));
    let occ = occurrence_on(&doc.tasks["w"], "2026-11-16").unwrap();
    let e = move_occurrence(&doc, &occ, "2026-11-18", at("2026-11-26"));
    let copy: Task = serde_json::from_value(e.items[1].2.clone().unwrap()).unwrap();
    assert_eq!((copy.done, copy.done_on), (true, None), "not known for the occurrence: not made up for the task");
}

#[test]
fn todos_without_a_day_keep_their_own_record_of_the_day() {
    let today = fluentcal_core::date::today();
    let month = Task { flex: Some("month".into()), ..mk("m", "2026-10-01") };
    let mut doc = doc_with(vec![month], vec![]);
    let slot = fluentcal_core::flex::month_slot(&today);
    let e = set_occurrence_done(&doc, &doc.tasks["m"].clone(), &slot, true, at(&today));
    apply(&mut doc, &e);
    let m = doc.tasks["m"].clone();
    assert_eq!(done_day(&m, &slot), Some(today.as_str()));
    assert_eq!((m.done_on.clone(), m.done_on_dates.clone()), (None, None), "flexDone is the record");
    // the day corrected through the slot (the column) and through the day (the calendar)
    let first = fluentcal_core::date::start_of_month(&today);
    let e = set_done_day(&doc, &m, &slot, &first, at(&today));
    apply(&mut doc, &e);
    assert_eq!(done_day(&doc.tasks["m"], &slot), Some(first.as_str()));
    let second = fluentcal_core::date::add_days(&first, 1);
    // (on the 1st the 2nd has not come: the clock is put a day on for this one)
    let e = set_done_day(&doc, &doc.tasks["m"].clone(), &first, &second, at(&today).max(at(&second)));
    apply(&mut doc, &e);
    assert_eq!((done_day(&doc.tasks["m"], &slot), done_day(&doc.tasks["m"], &second), done_day(&doc.tasks["m"], &first)), (Some(second.as_str()), Some(second.as_str()), None));
    // an open slot has no day to correct
    assert!(set_done_day(&doc, &doc.tasks["m"].clone(), "m:2026-09", "2026-09-05", at(&today)).is_empty());
}

// GitHub #14
#[test]
fn preset_groups_are_made_when_first_chosen() {
    let mut doc = doc_with(vec![mk("a", "2026-11-20"), mk("b", "2026-11-21")], vec![Group { id: "g1".into(), name: "工作".into(), color: "teal".into(), order: 1.0, ..Default::default() }]);
    // offered: the ones no group has the name of
    assert_eq!(open_presets(&doc).iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["生日", "还款", "纪念日"]);
    // a task put in one: the group is made with it, in one edit
    let e = set_group(&doc, &doc.tasks["a"].clone(), Some("__preset__:生日"), 5000);
    assert_eq!(e.items.iter().map(|(c, _, _)| c.as_str()).collect::<Vec<_>>(), ["groups", "tasks"]);
    apply(&mut doc, &e);
    let birthday = doc.groups.values().find(|g| g.name == "生日").expect("made").clone();
    assert_eq!((birthday.color.as_str(), birthday.order), ("pink", 2.0));
    assert_eq!(doc.tasks["a"].group_id.as_deref(), Some(birthday.id.as_str()));
    assert_eq!(open_presets(&doc).iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["还款", "纪念日"]);
    // chosen again (the entry was still on a menu): the same group, not a second one
    let e = set_group(&doc, &doc.tasks["b"].clone(), Some("__preset__:生日"), 6000);
    assert_eq!(e.items.len(), 1);
    apply(&mut doc, &e);
    assert_eq!((doc.groups.len(), doc.tasks["b"].group_id.as_deref()), (2, Some(birthday.id.as_str())));
    // the one a group of the user's has the name of is that group
    let e = set_group(&doc, &doc.tasks["b"].clone(), Some("__preset__:工作"), 7000);
    apply(&mut doc, &e);
    assert_eq!((doc.groups.len(), doc.tasks["b"].group_id.as_deref()), (2, Some("g1")));
    // an ordinary group, 未分组, the group it is in already, a preset there is none of
    let e = set_group(&doc, &doc.tasks["b"].clone(), Some(birthday.id.as_str()), 8000);
    apply(&mut doc, &e);
    assert_eq!((doc.tasks["b"].group_id.as_deref(), doc.tasks["b"].updated_at), (Some(birthday.id.as_str()), 8000));
    let e = set_group(&doc, &doc.tasks["b"].clone(), None, 9000);
    apply(&mut doc, &e);
    assert_eq!(doc.tasks["b"].group_id, None);
    assert!(set_group(&doc, &doc.tasks["b"].clone(), None, 9500).is_empty());
    assert!(set_group(&doc, &doc.tasks["b"].clone(), Some("__preset__:旅行"), 9500).is_empty());
    // renamed, a preset is offered again
    let renamed = Group { name: "家人生日".into(), ..birthday };
    let e = save_group(&doc, renamed);
    apply(&mut doc, &e);
    assert_eq!(open_presets(&doc).iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["生日", "还款", "纪念日"]);
    // a group made while the calendar was in another language (or on another PC) is that preset
    // here too: it isn't offered a second time under this language's name
    for (id, name) in [("en", "Birthdays"), ("tw", "還款")] {
        let e = save_group(&doc, Group { id: id.into(), name: name.into(), color: "gray".into(), order: 9.0, ..Default::default() });
        apply(&mut doc, &e);
    }
    assert_eq!(open_presets(&doc).iter().map(|(k, _)| *k).collect::<Vec<_>>(), ["纪念日"]);
    let groups = doc.groups.len();
    let e = set_group(&doc, &doc.tasks["a"].clone(), Some("__preset__:还款"), 9800);
    assert_eq!(e.items.len(), 1, "the task only: the group is there");
    apply(&mut doc, &e);
    assert_eq!((doc.groups.len(), doc.tasks["a"].group_id.as_deref()), (groups, Some("tw")));
}

// GitHub #7: the to-do column's order, by hand
#[test]
fn todo_column_is_ordered_by_hand() {
    use fluentcal_core::flex;
    let todo = |id: &str, kind: &str, order: f64| Task { id: id.into(), order, ..flex::new_task(kind, id, "2026-11-20") };
    let mut doc = doc_with(
        vec![todo("a", flex::SOMEDAY, 1.0), todo("b", flex::SOMEDAY, 2.0), todo("c", flex::SOMEDAY, 3.0), todo("d", flex::SOMEDAY, 4.0), todo("m1", flex::MONTH, 2.5), todo("m2", flex::MONTH, 9.0), mk("dated", "2026-11-20")],
        vec![],
    );
    fn column(doc: &Doc) -> (String, String) {
        let s = fluentcal_core::flex::side(doc.tasks.values(), "2026-11-20");
        (s.someday.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(" "), s.month.iter().map(|(t, _)| t.id.as_str()).collect::<Vec<_>>().join(" "))
    }
    // moves it, and says how many tasks were written
    fn go(doc: &mut Doc, id: &str, after: Option<&str>, before: Option<&str>, now: i64) -> usize {
        let e = reorder_flex(doc, id, after, before, now);
        apply(doc, &e);
        e.items.len()
    }
    assert_eq!(column(&doc), ("a b c d".to_owned(), "m1 m2".to_owned()));
    // between two: a number between theirs, one task written
    assert_eq!(go(&mut doc, "d", Some("a"), Some("b"), 5000), 1);
    assert_eq!((column(&doc).0.as_str(), doc.tasks["d"].order, doc.tasks["d"].updated_at), ("a d b c", 1.5, 5000));
    // to the front, to the end
    assert_eq!(go(&mut doc, "c", None, Some("a"), 5000), 1);
    assert_eq!((column(&doc).0.as_str(), doc.tasks["c"].order), ("c a d b", 0.0));
    assert_eq!(go(&mut doc, "c", Some("b"), None, 5000), 1);
    assert_eq!((column(&doc).0.as_str(), doc.tasks["c"].order), ("a d b c", 3.0));
    assert_eq!(go(&mut doc, "a", Some("c"), None, 5000), 1);
    assert_eq!(go(&mut doc, "a", None, Some("d"), 5000), 1);
    assert_eq!((column(&doc).0.as_str(), doc.tasks["a"].order), ("a d b c", 0.5));
    // where it is already: nothing is written
    assert_eq!(go(&mut doc, "d", Some("a"), Some("b"), 6000), 0);
    assert_eq!(go(&mut doc, "d", None, Some("b"), 6000), 0);
    assert_eq!(go(&mut doc, "d", Some("a"), None, 6000), 0);
    assert_eq!(go(&mut doc, "a", None, Some("d"), 6000), 0);
    assert_eq!(go(&mut doc, "c", Some("b"), None, 6000), 0);
    // not beside one of the other kind, a task with a day, or one that is gone; those don't move either
    assert_eq!(go(&mut doc, "a", None, Some("m1"), 6000), 0);
    assert_eq!(go(&mut doc, "a", Some("dated"), None, 6000), 0);
    assert_eq!(go(&mut doc, "a", Some("gone"), Some("gone"), 6000), 0);
    assert_eq!(go(&mut doc, "dated", None, Some("a"), 6000), 0);
    assert_eq!(go(&mut doc, "gone", None, Some("a"), 6000), 0);
    assert_eq!(column(&doc), ("a d b c".to_owned(), "m1 m2".to_owned()));
    // each kind has its own order
    assert_eq!(go(&mut doc, "m2", None, Some("m1"), 7000), 1);
    assert_eq!((column(&doc), doc.tasks["m2"].order), (("a d b c".to_owned(), "m2 m1".to_owned()), 1.5));

    // the ones the column doesn't show keep their places: `h` (done) stays behind `a`
    let mut doc = doc_with(vec![todo("a", flex::SOMEDAY, 1.0), todo("h", flex::SOMEDAY, 2.0), todo("b", flex::SOMEDAY, 3.0), todo("c", flex::SOMEDAY, 4.0)], vec![]);
    let mut hidden = doc.tasks["h"].clone();
    flex::set_slot(&mut hidden, flex::SOMEDAY_SLOT, Some("2026-11-19"));
    doc.tasks.insert("h".into(), hidden);
    assert_eq!(column(&doc).0, "a b c");
    assert_eq!(go(&mut doc, "c", Some("a"), Some("b"), 8000), 1);
    assert_eq!((column(&doc).0.as_str(), doc.tasks["c"].order), ("a c b", 2.5), "between the hidden one and b");
    assert_eq!(go(&mut doc, "b", None, Some("a"), 8000), 1);
    assert_eq!(go(&mut doc, "b", Some("a"), Some("c"), 8000), 1);
    assert_eq!((column(&doc).0.as_str(), doc.tasks["b"].order), ("a b c", 2.25));

    // the same number (a copy has its original's; they are listed by id then): the ones behind
    // it with that number are spread out with it
    let mut doc = doc_with(vec![todo("x", flex::SOMEDAY, 5.0), todo("y", flex::SOMEDAY, 5.0), todo("z", flex::SOMEDAY, 5.0), todo("w", flex::SOMEDAY, 6.0)], vec![]);
    assert_eq!(column(&doc).0, "x y z w");
    assert_eq!(go(&mut doc, "z", Some("x"), Some("y"), 9000), 2, "z and y");
    assert_eq!(column(&doc).0, "x z y w");
    let n = |doc: &Doc, id: &str| doc.tasks[id].order;
    assert!(n(&doc, "x") < n(&doc, "z") && n(&doc, "z") < n(&doc, "y") && n(&doc, "y") < n(&doc, "w"));
    assert_eq!((n(&doc, "x"), n(&doc, "w"), doc.tasks["x"].updated_at, doc.tasks["y"].updated_at), (5.0, 6.0, 0, 9000));
    // nothing behind them to stay under
    let mut doc = doc_with(vec![todo("x", flex::SOMEDAY, 5.0), todo("y", flex::SOMEDAY, 5.0), todo("z", flex::SOMEDAY, 5.0)], vec![]);
    assert_eq!(go(&mut doc, "z", None, Some("y"), 9000), 2);
    assert_eq!((column(&doc).0.as_str(), n(&doc, "x"), n(&doc, "z"), n(&doc, "y")), ("x z y", 5.0, 6.0, 7.0));
    assert_eq!(go(&mut doc, "y", None, Some("x"), 9000), 1);
    assert_eq!((column(&doc).0.as_str(), n(&doc, "y")), ("y x z", 4.0));

    // no number left between two: all are numbered anew
    let next = f64::from_bits(1.0f64.to_bits() + 1);
    let mut doc = doc_with(vec![todo("p", flex::SOMEDAY, 1.0), todo("q", flex::SOMEDAY, next), todo("r", flex::SOMEDAY, 8.0), todo("s", flex::SOMEDAY, 0.5)], vec![]);
    assert_eq!(column(&doc).0, "s p q r");
    assert_eq!(go(&mut doc, "r", Some("p"), Some("q"), 9500), 4);
    assert_eq!(column(&doc).0, "s p r q");
    assert_eq!((n(&doc, "s"), n(&doc, "p"), n(&doc, "r"), n(&doc, "q")), (1.0, 2.0, 3.0, 4.0));
    // a new one still goes to the end
    let (e, _) = add_task(&doc, flex::new_task(flex::SOMEDAY, "new", "2026-11-20"), None, 9600);
    apply(&mut doc, &e);
    assert!(column(&doc).0.starts_with("s p r q "));
}
