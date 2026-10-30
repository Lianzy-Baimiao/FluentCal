//! 0.2's src/lib/recur.test.ts.
use fluentcal_core::model::{Freq, RepeatRule, Task};
use fluentcal_core::recur::*;

fn mk(start: &str, end: Option<&str>, repeat: Option<RepeatRule>) -> Task {
    Task { id: format!("t{start}"), title: "任务".into(), start: start.into(), end: end.unwrap_or(start).into(), repeat, ..Default::default() }
}

fn rule(freq: Freq) -> RepeatRule {
    RepeatRule { freq: Some(freq), ..Default::default() }
}

fn dates(t: &Task, a: &str, b: &str) -> Vec<String> {
    occurrences(t, a, b, 400).into_iter().map(|o| o.date).collect()
}

#[test]
fn returns_a_plain_task_when_it_overlaps_the_range() {
    let t = mk("2026-10-28", Some("2026-11-02"), None);
    assert_eq!(dates(&t, "2026-11-01", "2026-11-30"), ["2026-10-28"]);
    assert!(dates(&t, "2026-11-03", "2026-11-30").is_empty());
}

#[test]
fn repeats_daily_with_an_interval() {
    let t = mk("2026-10-01", None, Some(RepeatRule { interval: Some(2), ..rule(Freq::Daily) }));
    assert_eq!(dates(&t, "2026-10-01", "2026-10-10"), ["2026-10-01", "2026-10-03", "2026-10-05", "2026-10-07", "2026-10-09"]);
}

#[test]
fn repeats_weekly_on_chosen_weekdays() {
    let t = mk("2026-10-28", None, Some(RepeatRule { weekdays: Some(vec![1, 3]), ..rule(Freq::Weekly) }));
    assert_eq!(dates(&t, "2026-10-26", "2026-11-08"), ["2026-10-28", "2026-11-02", "2026-11-04"]);
    let biweekly = mk("2026-10-30", None, Some(RepeatRule { interval: Some(2), weekdays: Some(vec![5]), ..rule(Freq::Weekly) }));
    assert_eq!(dates(&biweekly, "2026-10-01", "2026-12-01"), ["2026-10-30", "2026-11-13", "2026-11-27"]);
}

#[test]
fn clamps_monthly_dates_to_the_month_end() {
    let t = mk("2026-01-31", None, Some(rule(Freq::Monthly)));
    assert_eq!(dates(&t, "2026-02-01", "2026-04-30"), ["2026-02-28", "2026-03-31", "2026-04-30"]);
}

#[test]
fn repeats_on_the_nth_or_last_weekday_of_the_month() {
    let nth = |s: &str| mk(s, None, Some(RepeatRule { month_mode: Some("nthWeekday".into()), ..rule(Freq::Monthly) }));
    assert_eq!(dates(&nth("2026-10-13"), "2026-11-01", "2026-12-31"), ["2026-11-10", "2026-12-08"]);
    assert_eq!(dates(&nth("2026-10-27"), "2026-11-01", "2026-12-31"), ["2026-11-24", "2026-12-29"]);
}

#[test]
fn handles_feb_29_yearly() {
    let t = mk("2024-02-29", None, Some(rule(Freq::Yearly)));
    assert_eq!(dates(&t, "2025-01-01", "2028-12-31"), ["2025-02-28", "2026-02-28", "2027-02-28", "2028-02-29"]);
}

#[test]
fn stops_after_count_or_until() {
    let count = mk("2026-10-01", None, Some(RepeatRule { count: Some(3), ..rule(Freq::Daily) }));
    assert_eq!(dates(&count, "2026-10-01", "2026-12-31").len(), 3);
    let until = mk("2026-10-01", None, Some(RepeatRule { until: Some("2026-10-04".into()), ..rule(Freq::Daily) }));
    assert_eq!(dates(&until, "2026-10-01", "2026-12-31").len(), 4);
    // count includes occurrences before the range
    assert_eq!(dates(&count, "2026-10-03", "2026-10-10"), ["2026-10-03"]);
}

#[test]
fn applies_excluded_and_completed_dates() {
    let mut t = mk("2026-10-01", None, Some(rule(Freq::Daily)));
    t.ex_dates = Some(vec!["2026-10-02".into()]);
    t.done_dates = Some(vec!["2026-10-03".into()]);
    let occ = occurrences(&t, "2026-10-01", "2026-10-04", 400);
    assert_eq!(occ.iter().map(|o| o.date.as_str()).collect::<Vec<_>>(), ["2026-10-01", "2026-10-03", "2026-10-04"]);
    assert_eq!(occ.iter().map(|o| o.done).collect::<Vec<_>>(), [false, true, false]);
}

#[test]
fn repeats_on_workdays_and_official_working_days() {
    assert_eq!(dates(&mk("2026-10-30", None, Some(rule(Freq::Workday))), "2026-10-30", "2026-11-03"), ["2026-10-30", "2026-11-02", "2026-11-03"]);
    let legal = dates(&mk("2026-09-28", None, Some(rule(Freq::LegalWorkday))), "2026-10-01", "2026-10-12");
    assert!(legal.contains(&"2026-10-10".to_owned()) && legal.contains(&"2026-10-12".to_owned()));
    assert!(!legal.contains(&"2026-10-01".to_owned()) && !legal.contains(&"2026-10-03".to_owned()));
}

#[test]
fn repeats_on_lunar_dates_and_the_month_end() {
    assert_eq!(dates(&mk("2026-02-17", None, Some(rule(Freq::LunarYearly))), "2027-01-01", "2027-12-31"), ["2027-02-06"]);
    assert_eq!(dates(&mk("2026-10-31", None, Some(rule(Freq::MonthLastDay))), "2026-11-01", "2027-02-28"), ["2026-11-30", "2026-12-31", "2027-01-31", "2027-02-28"]);
}

#[test]
fn follows_the_ebbinghaus_schedule() {
    let t = mk("2026-10-01", None, Some(rule(Freq::Ebbinghaus)));
    assert_eq!(dates(&t, "2026-09-01", "2027-01-01"), ["2026-10-01", "2026-10-02", "2026-10-03", "2026-10-05", "2026-10-08", "2026-10-16", "2026-10-31"]);
}

#[test]
fn includes_multi_day_occurrences_that_started_before_the_range() {
    let t = mk("2026-10-30", Some("2026-10-31"), Some(rule(Freq::Weekly)));
    let o = occurrences(&t, "2026-10-31", "2026-10-31", 400);
    assert_eq!(o.len(), 1);
    assert_eq!((o[0].date.as_str(), o[0].end.as_str()), ("2026-10-30", "2026-10-31"));
}

#[test]
fn stays_fast_for_old_unbounded_rules() {
    let started = std::time::Instant::now();
    assert_eq!(dates(&mk("2000-01-01", None, Some(rule(Freq::Daily))), "2026-10-01", "2026-10-07").len(), 7);
    assert_eq!(dates(&mk("2000-01-03", None, Some(rule(Freq::Weekly))), "2026-10-01", "2026-10-31").len(), 4);
    assert!(started.elapsed().as_millis() < 50, "{:?}", started.elapsed());
}

#[test]
fn finds_the_next_occurrence_and_describes_rules() {
    let t = mk("2026-10-30", None, Some(RepeatRule { weekdays: Some(vec![5]), ..rule(Freq::Weekly) }));
    assert_eq!(next_occurrence(&t, "2026-10-31").as_deref(), Some("2026-11-06"));
    assert_eq!(describe_rule(t.repeat.as_ref(), &t.start), "每周的周五");
    let r = RepeatRule { interval: Some(2), weekdays: Some(vec![3, 1]), count: Some(5), ..rule(Freq::Weekly) };
    assert_eq!(describe_rule(Some(&r), "2026-10-28"), "每 2 周的周一、周三，共 5 次");
    assert_eq!(describe_rule(Some(&rule(Freq::LunarYearly)), "2026-02-17"), "每年农历正月初一");
    assert_eq!(describe_rule(None, "2026-10-01"), "不重复");
}

#[test]
fn tasks_round_trip_without_losing_fields() {
    let v = serde_json::json!({ "id": "a", "title": "x", "start": "2026-10-01", "end": "2026-10-01", "progress": 40, "done": false,
        "order": 3, "createdAt": 1, "updatedAt": 2, "repeat": { "freq": "fortnightly", "interval": 2 }, "futureField": { "k": 1 } });
    let t: Task = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(t.repeat.as_ref().unwrap().freq, None, "an unknown frequency doesn't fail the task");
    let back = t.to_value();
    assert_eq!(back["futureField"], v["futureField"]);
    assert_eq!(back["order"], serde_json::json!(3), "whole numbers stay integers");
    assert_eq!(back["progress"], serde_json::json!(40));
}
