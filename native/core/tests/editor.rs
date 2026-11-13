//! The task editor's logic (0.2's EditorApp, RepeatEditor, ReminderEditor, DepsEditor).
use fluentcal_core::editor::{Answer, Ask, Editor, Payload};
use fluentcal_core::model::{Freq, Group, RepeatRule, Task};
use fluentcal_core::tasks::*;

const TODAY: &str = "2026-10-14";

fn mk(id: &str, start: &str) -> Task {
    Task { id: id.into(), title: id.into(), start: start.into(), end: start.into(), order: 1.0, ..Default::default() }
}

fn doc_with(tasks: Vec<Task>, groups: Vec<Group>) -> Doc {
    Doc { tasks: tasks.into_iter().map(|t| (t.id.clone(), t)).collect(), groups: groups.into_iter().map(|g| (g.id.clone(), g)).collect(), ..Default::default() }
}

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

fn open(doc: &Doc, p: Payload) -> Editor {
    let mut e = Editor::default();
    e.load(doc, &p, TODAY, 1000);
    e
}

#[test]
fn payloads_read_like_0_2() {
    let p: Payload = serde_json::from_str(r#"{"mode":"edit","taskId":"weekly","date":"2026-10-16"}"#).unwrap();
    assert_eq!(p, Payload::edit("weekly", Some("2026-10-16".into())));
    let p: Payload = serde_json::from_str(r#"{"mode":"new","start":"2026-10-14","groupId":"web","milestone":true}"#).unwrap();
    assert_eq!(p, Payload::New { start: Some("2026-10-14".into()), end: None, group_id: Some("web".into()), title: None, milestone: Some(true) });
}

#[test]
fn new_task_defaults_and_save() {
    let mut doc = doc_with(vec![], vec![]);
    doc.settings.tasks.default_color = "blue".into();
    doc.settings.reminders.default_offsets = vec![15];
    let mut e = open(&doc, Payload::new_on(None));
    let t = e.draft.clone().unwrap();
    assert_eq!((t.start.as_str(), t.end.as_str(), t.color.as_deref(), t.reminders.clone()), (TODAY, TODAY, Some("blue"), Some(vec![15])));
    assert!(e.is_new && !e.dirty());
    assert_eq!(e.window_title(), "新建任务");
    // An empty title can't be saved; the title box gets the focus.
    let fx = e.save(&doc, 2000);
    assert!(fx.focus_title && !fx.close && e.title_error && fx.edit.is_empty());
    e.edit("title", "  写周报 ");
    assert!(e.dirty());
    let fx = e.save(&doc, 2000);
    assert!(fx.close);
    apply(&mut doc, &fx.edit);
    let saved = doc.tasks.values().next().unwrap();
    assert_eq!(saved.title, "写周报");
}

#[test]
fn dates_keep_the_length_and_end_follows() {
    let doc = doc_with(vec![Task { end: "2026-10-12".into(), ..mk("a", "2026-10-10") }], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    e.edit("start", "2026-10-20");
    assert_eq!(e.draft.as_ref().unwrap().end, "2026-10-22");
    e.edit("end", "2026-10-01"); // before the start: the start
    assert_eq!(e.draft.as_ref().unwrap().end, "2026-10-20");
    assert_eq!(e.view(&doc, TODAY).span, "农历九月十一");
    e.edit("end", "2026-10-24");
    assert!(e.view(&doc, TODAY).span.starts_with("共 5 天 · 农历"));
    e.edit("all-day", "0");
    assert_eq!(e.draft.as_ref().unwrap().time.as_deref(), Some("09:00"));
    e.edit("all-day", "1");
    assert_eq!(e.draft.as_ref().unwrap().time, None);
}

#[test]
fn progress_and_done_follow_each_other() {
    let doc = doc_with(vec![mk("a", TODAY)], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    e.edit("progress-done", "100");
    assert!(e.draft.as_ref().unwrap().done);
    e.edit("done", "0");
    assert_eq!(e.draft.as_ref().unwrap().progress, 0.0);
    e.edit("progress", "45");
    e.edit("done", "1");
    assert_eq!(e.draft.as_ref().unwrap().progress, 100.0);
}

#[test]
fn repeat_rules() {
    let doc = doc_with(vec![mk("a", "2026-10-16")], vec![]); // a Friday
    let mut e = open(&doc, Payload::edit("a", None));
    e.edit("freq", "weekly");
    let r = e.draft.as_ref().unwrap().repeat.clone().unwrap();
    assert_eq!((r.freq, r.interval, r.weekdays.clone()), (Some(Freq::Weekly), Some(1), Some(vec![5])));
    let v = e.view(&doc, TODAY);
    assert_eq!((v.unit, v.weekly, v.summary.as_str(), v.show_progress), ("周", true, "每周的周五", false));
    e.toggle_day(5); // the last one stays
    assert_eq!(e.draft.as_ref().unwrap().repeat.as_ref().unwrap().weekdays, Some(vec![5]));
    e.toggle_day(1);
    e.toggle_day(3);
    assert_eq!(e.draft.as_ref().unwrap().repeat.as_ref().unwrap().weekdays, Some(vec![1, 3, 5]));
    e.edit("interval", "120");
    assert_eq!(e.draft.as_ref().unwrap().repeat.as_ref().unwrap().interval, Some(99));
    e.edit("end-mode", "count");
    assert_eq!(e.view(&doc, TODAY).count, 10);
    e.edit("count", "3");
    e.edit("end-mode", "count"); // the same choice again changes nothing
    assert_eq!(e.view(&doc, TODAY).count, 3);
    e.edit("end-mode", "until");
    let r = e.draft.as_ref().unwrap().repeat.clone().unwrap();
    assert_eq!((r.count, r.until.as_deref()), (None, Some("2027-01-16")));
    e.edit("freq", "monthly");
    let v = e.view(&doc, TODAY);
    assert_eq!(v.month_modes.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["每月 16 日", "每月第 3 个周五"]);
    e.edit("freq", "ebbinghaus");
    assert!(!e.view(&doc, TODAY).has_end && e.view(&doc, TODAY).unit.is_empty());
    e.edit("freq", "");
    assert_eq!(e.draft.as_ref().unwrap().repeat, None);
}

/// GitHub #8: 每隔 N 天 is a choice of its own (the rule stays daily + interval), and the
/// lunar year sits next to the solar one.
#[test]
fn every_n_days_and_the_order_of_the_choices() {
    use fluentcal_core::editor::FREQS;
    let labels: Vec<&str> = FREQS.iter().map(|(_, l)| *l).collect();
    assert_eq!(labels[..3], ["不重复", "每天", "每隔 N 天"]);
    let y = labels.iter().position(|l| *l == "每年").unwrap();
    assert_eq!(labels[y + 1], "每年（农历）");

    let doc = doc_with(vec![mk("a", "2026-10-16"), Task { repeat: Some(RepeatRule { freq: Some(Freq::Daily), interval: Some(9), ..Default::default() }), ..mk("b", "2026-10-16") }], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    let rule = |e: &Editor| e.draft.as_ref().unwrap().repeat.clone().unwrap();
    e.edit("freq", "daily");
    let v = e.view(&doc, TODAY);
    assert_eq!((v.freq.as_str(), v.unit, v.summary.as_str()), ("daily", "", "每天"), "每天 has no interval box");
    e.edit("freq", "everyNDays");
    assert_eq!((rule(&e).freq, rule(&e).interval), (Some(Freq::Daily), Some(2)));
    let v = e.view(&doc, TODAY);
    assert_eq!((v.freq.as_str(), v.unit, v.summary.as_str()), ("everyNDays", "天", "每 2 天"));
    e.edit("interval", "9");
    assert_eq!(e.view(&doc, TODAY).summary, "每 9 天");
    e.edit("interval", "1");
    assert_eq!(rule(&e).interval, Some(2), "stays 每隔 N 天");
    e.edit("freq", "everyNDays"); // the same choice again changes nothing
    assert_eq!(rule(&e).interval, Some(2));
    e.edit("freq", "daily");
    assert_eq!((rule(&e).freq, rule(&e).interval), (Some(Freq::Daily), Some(1)));
    // a task saved as every 9 days opens as 每隔 N 天
    let e = open(&doc, Payload::edit("b", None));
    let v = e.view(&doc, TODAY);
    assert_eq!((v.freq.as_str(), v.interval, v.unit), ("everyNDays", 9, "天"));
}

#[test]
fn reminders_offer_what_suits_the_task() {
    let mut doc = doc_with(vec![mk("a", TODAY)], vec![]);
    doc.settings.reminders.all_day_time = "08:30".into();
    let mut e = open(&doc, Payload::edit("a", None));
    let labels: Vec<String> = e.reminder_options(&doc).into_iter().map(|(_, l)| l).collect();
    assert_eq!(labels[0], "当天 08:30");
    assert_eq!(labels[1], "提前 1 天"); // all-day: no "15 minutes before"
    e.add_reminder(&doc);
    e.add_reminder(&doc);
    assert_eq!(e.draft.as_ref().unwrap().reminders, Some(vec![0, 1440]));
    assert_eq!(e.view(&doc, TODAY).reminder_hint, "全天任务在 08:30 提醒");
    e.set_reminder(0, 1440); // duplicates collapse
    assert_eq!(e.draft.as_ref().unwrap().reminders, Some(vec![1440]));
    e.remove_reminder(0);
    assert_eq!(e.draft.as_ref().unwrap().reminders, None);
    doc.settings.reminders.enabled = false;
    assert_eq!(e.view(&doc, TODAY).reminder_hint, "提醒已在设置中关闭");
}

#[test]
fn predecessors_never_make_a_cycle() {
    let a = mk("a", "2026-10-01");
    let b = Task { deps: Some(vec!["a".into()]), ..mk("b", "2026-10-02") };
    let c = Task { deps: Some(vec!["b".into()]), ..mk("c", "2026-10-03") };
    let r = Task { repeat: Some(RepeatRule { freq: Some(Freq::Daily), ..Default::default() }), ..mk("r", "2026-10-04") };
    let d = Task { group_id: Some("g".into()), ..mk("d", "2026-10-09") };
    let doc = doc_with(vec![a, b, c, r, d], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    // b and c depend on a; r repeats: only d.
    assert_eq!(e.dep_options(&doc, TODAY).iter().map(|o| o.value.as_str()).collect::<Vec<_>>(), ["d"]);
    let mut e2 = open(&doc, Payload::edit("c", None));
    e2.edit("group", "g");
    // the same group first, then by start; b is already a predecessor
    assert_eq!(e2.dep_options(&doc, TODAY).iter().map(|o| o.value.as_str()).collect::<Vec<_>>(), ["d", "a"]);
    assert_eq!(e2.dep_options(&doc, TODAY)[1].label, "a（10月1日 周四）");
    e.add_dep("d");
    assert_eq!(e.view(&doc, TODAY).deps[0].title.as_deref(), Some("d"));
    e.remove_dep("d");
    assert_eq!(e.draft.as_ref().unwrap().deps, None);
}

#[test]
fn save_keeps_changes_made_elsewhere_meanwhile() {
    let mut doc = doc_with(vec![Task { end: "2026-10-12".into(), ..mk("a", "2026-10-10") }], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    e.edit("title", "改名");
    // meanwhile: ticked done in the calendar
    let elsewhere = save_task(&doc, Task { done: true, progress: 100.0, ..doc.tasks["a"].clone() }, 1500);
    apply(&mut doc, &elsewhere);
    let fx = e.save(&doc, 2000);
    apply(&mut doc, &fx.edit);
    let t = &doc.tasks["a"];
    assert_eq!((t.title.as_str(), t.done, t.progress), ("改名", true, 100.0));
}

#[test]
fn saving_a_task_deleted_elsewhere_asks_to_recreate() {
    let mut doc = doc_with(vec![mk("a", TODAY)], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    e.edit("title", "还在");
    doc.tasks.clear();
    let fx = e.save(&doc, 2000);
    assert_eq!(fx.ask, Some(Ask::Recreate));
    let fx = e.answer(&doc, Answer::Primary, TODAY, 2100);
    assert!(fx.close);
    apply(&mut doc, &fx.edit);
    assert_eq!(doc.tasks["a"].title, "还在");
}

#[test]
fn deleting_a_repeating_task_this_one_or_all() {
    let rule = RepeatRule { freq: Some(Freq::Weekly), weekdays: Some(vec![5]), ..Default::default() };
    let mut doc = doc_with(vec![Task { repeat: Some(rule), ..mk("w", "2026-10-02") }], vec![]);
    let mut e = open(&doc, Payload::edit("w", Some("2026-10-16".into())));
    e.edit("title", "未保存的改动");
    let fx = e.remove(&doc);
    assert_eq!(fx.ask, Some(Ask::DeleteRepeat { title: "w".into() }));
    let (title, text, p, s, c, danger) = fx.ask.unwrap().texts();
    assert_eq!((title.as_str(), text.as_str(), p, s, c, danger), ("删除重复任务", "“w”是重复任务，要删除哪些？", "全部删除", "仅这一次", "取消", true));
    let fx = e.answer(&doc, Answer::Secondary, TODAY, 2000);
    assert!(fx.close);
    apply(&mut doc, &fx.edit);
    let t = &doc.tasks["w"];
    assert_eq!((t.title.as_str(), t.ex_dates.clone()), ("w", Some(vec!["2026-10-16".into()])));
    let mut e = open(&doc, Payload::edit("w", None));
    e.remove(&doc);
    let fx = e.answer(&doc, Answer::Primary, TODAY, 2000);
    apply(&mut doc, &fx.edit);
    assert!(doc.tasks.is_empty());
}

#[test]
fn deleting_asks_and_cancel_keeps_the_task() {
    let doc = doc_with(vec![mk("a", TODAY)], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    assert_eq!(e.remove(&doc).ask, Some(Ask::Delete { title: "a".into() }));
    let fx = e.answer(&doc, Answer::Close, TODAY, 2000);
    assert!(!fx.close && fx.edit.is_empty() && e.asking.is_none());
    // A new task isn't stored yet: 删除 only closes (0.2 has no button for it).
    let mut n = open(&doc, Payload::new_on(None));
    let fx = n.remove(&doc);
    assert!(fx.close && fx.edit.is_empty());
}

#[test]
fn unsaved_changes_ask_before_closing_or_switching() {
    let doc = doc_with(vec![mk("a", TODAY), mk("b", TODAY)], vec![]);
    let mut e = open(&doc, Payload::edit("a", None));
    assert!(e.cancel().close); // nothing changed
    let mut e = open(&doc, Payload::edit("a", None));
    e.edit("notes", "备注");
    assert_eq!(e.cancel().ask, Some(Ask::Discard));
    let fx = e.answer(&doc, Answer::Close, TODAY, 2000); // 继续编辑
    assert!(!fx.close && e.dirty());
    // opened on b meanwhile: asks, then switches
    let fx = e.take(&doc, Payload::edit("b", None), TODAY, 2000);
    assert_eq!(fx.ask, Some(Ask::Discard));
    let fx = e.answer(&doc, Answer::Primary, TODAY, 2000);
    assert!(fx.reloaded && !fx.close);
    assert_eq!(e.draft.as_ref().unwrap().id, "b");
    // unchanged: switches at once
    let fx = e.take(&doc, Payload::edit("a", None), TODAY, 2000);
    assert!(fx.reloaded && fx.ask.is_none());
    assert_eq!(e.draft.as_ref().unwrap().id, "a");
}

#[test]
fn a_task_deleted_before_opening_shows_missing() {
    let doc = doc_with(vec![], vec![]);
    let e = open(&doc, Payload::edit("gone", None));
    assert!(e.missing && e.draft.is_none() && !e.dirty());
}

#[test]
fn new_group_is_saved_and_picked() {
    let mut doc = doc_with(vec![mk("a", TODAY)], vec![Group { id: "g".into(), name: "工作".into(), order: 3.0, ..Default::default() }]);
    let mut e = open(&doc, Payload::edit("a", None));
    assert!(e.create_group(&doc, "  ").is_none());
    let edit = e.create_group(&doc, "生活").unwrap();
    apply(&mut doc, &edit);
    let gid = e.draft.as_ref().unwrap().group_id.clone().unwrap();
    assert_eq!((doc.groups[&gid].name.as_str(), doc.groups[&gid].color.as_str(), doc.groups[&gid].order), ("生活", "blue", 4.0));
    let labels: Vec<String> = e.view(&doc, TODAY).groups.into_iter().map(|o| o.label).collect();
    assert_eq!(labels, ["未分组", "工作", "生活", "新建分组…"]);
}
