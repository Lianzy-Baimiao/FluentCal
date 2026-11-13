//! The list view (0.2's src/lib/agenda.test.ts) and its 所有 mode (GitHub #4).
use fluentcal_core::agenda::{agenda, all_tasks, day_heading, day_sub};
use fluentcal_core::model::{Freq, RepeatRule, Task};

const TODAY: &str = "2026-10-30";

fn t(id: &str, start: &str, end: &str) -> Task {
    Task { id: id.into(), title: id.into(), start: start.into(), end: end.into(), order: 1.0, ..Default::default() }
}

fn ids(items: &[fluentcal_core::recur::Occurrence]) -> Vec<String> {
    items.iter().map(|o| o.task.id.clone()).collect()
}

#[test]
fn groups_by_day_keeps_today_sorts_all_day_first_then_by_time() {
    let late = Task { time: Some("18:00".into()), ..t("late", "2026-10-31", "2026-10-31") };
    let early = Task { time: Some("08:30".into()), ..t("early", "2026-10-31", "2026-10-31") };
    let all_day = t("allday", "2026-10-31", "2026-10-31");
    let tasks = [late, early, all_day];
    let days = agenda(&tasks, TODAY, "2026-11-05", TODAY, true);
    assert_eq!(days.iter().map(|d| d.date.as_str()).collect::<Vec<_>>(), ["2026-10-30", "2026-10-31"]);
    assert!(days[0].items.is_empty());
    assert_eq!(ids(&days[1].items), ["allday", "early", "late"]);
}

#[test]
fn a_running_multi_day_task_is_on_the_first_day_and_repeats_on_each_day() {
    let trip = t("trip", "2026-10-28", "2026-11-02");
    let gym = Task { repeat: Some(RepeatRule { freq: Some(Freq::Weekly), weekdays: Some(vec![2, 4]), ..Default::default() }), ..t("gym", "2026-10-27", "2026-10-27") };
    let tasks = [trip, gym];
    let days = agenda(&tasks, TODAY, "2026-11-06", TODAY, true);
    assert_eq!(ids(&days.iter().find(|d| d.date == TODAY).unwrap().items), ["trip"]);
    let gym_days: Vec<&str> = days.iter().filter(|d| d.items.iter().any(|o| o.task.id == "gym")).map(|d| d.date.as_str()).collect();
    assert_eq!(gym_days, ["2026-11-03", "2026-11-05"]);
}

#[test]
fn can_hide_done_tasks() {
    let tasks = [Task { done: true, ..t("done", TODAY, TODAY) }, t("open", TODAY, TODAY)];
    assert_eq!(ids(&agenda(&tasks, TODAY, TODAY, TODAY, false)[0].items), ["open"]);
    assert_eq!(ids(&agenda(&tasks, TODAY, TODAY, TODAY, true)[0].items), ["open", "done"]);
}

#[test]
fn all_lists_every_task_once_on_its_start_past_ones_too() {
    let daily = Task { repeat: Some(RepeatRule { freq: Some(Freq::Daily), interval: Some(1), ..Default::default() }), ..t("daily", "2026-01-05", "2026-01-05") };
    let birthday = Task { repeat: Some(RepeatRule { freq: Some(Freq::LunarYearly), ..Default::default() }), done_dates: Some(vec!["1992-12-21".into()]), ..t("birthday", "1992-12-21", "1992-12-21") };
    let old = Task { done: true, ..t("old", "2025-03-04", "2025-03-06") };
    let later = t("later", "2027-02-19", "2027-02-19");
    let tasks = [later, daily, old, birthday];
    let days = all_tasks(&tasks, TODAY, true);
    let dates: Vec<&str> = days.iter().map(|d| d.date.as_str()).collect();
    assert_eq!(dates, ["1992-12-21", "2025-03-04", "2026-01-05", TODAY, "2027-02-19"], "by date, today kept");
    assert_eq!(days.iter().map(|d| d.items.len()).sum::<usize>(), 4, "a repeating task only once");
    let old_item = &days[1].items[0];
    assert_eq!((old_item.end.as_str(), old_item.done), ("2025-03-06", true));
    assert!(!days[0].items[0].done, "a series is never done as a whole");
    assert!(days[3].items.is_empty());
    // hiding done tasks hides the done one, not a series
    let days = all_tasks(&tasks, TODAY, false);
    assert!(days.iter().all(|d| d.items.iter().all(|o| o.task.id != "old")));
    assert!(days.iter().any(|d| d.items.iter().any(|o| o.task.id == "birthday")));
}

#[test]
fn names_days_with_the_year_when_it_differs() {
    let names: Vec<String> = [TODAY, "2026-10-31", "2026-10-29", "2026-11-02", "2025-03-04", "2027-01-01"].iter().map(|d| day_heading(d, TODAY)).collect();
    assert_eq!(names, ["今天", "明天", "昨天", "11月2日", "2025年3月4日", "2027年1月1日"]);
    assert_eq!(day_sub("2026-10-01", true), "周四 · 八月廿一 · 国庆节·休");
    assert_eq!(day_sub("2026-10-01", false), "周四 · 八月廿一 · 国庆节");
}
