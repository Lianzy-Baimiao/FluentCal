//! 0.2's src/lib/misc.test.ts and src/lib/syncText.test.ts.
use fluentcal_core::cloud::service::Status;
use fluentcal_core::countdown::{countdowns, next_holiday};
use fluentcal_core::date::local_ms;
use fluentcal_core::export::{parse_backup, plan_merge, to_csv, to_ics};
use fluentcal_core::model::{Freq, RepeatRule, Task};
use fluentcal_core::reminders::{compute_reminders, GRACE_MS, HORIZON_MS};
use fluentcal_core::search::{search_tasks, SearchOptions};
use fluentcal_core::seed::sample_data;
use fluentcal_core::settings::Settings;
use fluentcal_core::sync_text::{ago, status_line, uploaded_by};
use serde_json::json;
use std::collections::{BTreeMap, HashSet};

const CN: i32 = 480; // Beijing time

fn mk(id: &str, title: &str, start: &str) -> Task {
    Task { id: id.into(), title: title.into(), start: start.into(), end: start.into(), ..Default::default() }
}

#[test]
fn settings_defaults_fill_missing_keys_ignore_wrong_types_keep_extras() {
    let s = Settings::from_value(&json!({ "theme": { "mode": "dark", "accent": 42 }, "desk": { "bounds": { "x": 1, "y": 2, "w": 3, "h": 4 } }, "calendar": { "restDays": [0] } }));
    assert_eq!(s.theme.mode, "dark");
    assert_eq!(s.theme.accent, "system");
    assert_eq!(s.desk.bounds, Some(json!({ "x": 1, "y": 2, "w": 3, "h": 4 })));
    assert_eq!(s.desk.bg_opacity, 72.0);
    assert_eq!(s.calendar.rest_days, [0]);
    assert_eq!(Settings::from_value(&serde_json::Value::Null), Settings::default());
}

#[test]
fn writes_safe_csv() {
    let mut t1 = mk("t1", "周报, \"重要\"", "2026-10-30");
    t1.notes = Some("第一行\n第二行".into());
    let t2 = Task { end: "2026-10-03".into(), ..mk("t2", "=SUM(A1)", "2026-10-01") };
    let csv = to_csv(&[t1, t2], &BTreeMap::new());
    let lines: Vec<&str> = csv.split("\r\n").collect();
    assert_eq!(lines[0], "标题,开始日期,结束日期,时间,分组,进度,状态,重复,备注");
    assert!(lines[1].starts_with("'=SUM(A1),2026-10-01,2026-10-03"));
    assert!(csv.contains("\"周报, \"\"重要\"\"\""));
    assert!(csv.contains("\"第一行\n第二行\""));
}

#[test]
fn writes_valid_icalendar() {
    let t2 = Task { end: "2026-10-03".into(), ..mk("t2", "=SUM(A1)", "2026-10-01") };
    let weekly = Task { time: Some("10:00".into()), repeat: Some(RepeatRule { freq: Some(Freq::Weekly), weekdays: Some(vec![3]), ..Default::default() }), ..mk("w", "例会", "2026-10-28") };
    let lunar = Task { repeat: Some(RepeatRule { freq: Some(Freq::LunarYearly), count: Some(2), ..Default::default() }), ..mk("l", &"生日".repeat(30), "2026-02-17") };
    let ics = to_ics(&[t2, weekly, lunar], &BTreeMap::new(), "2026-10-29", 0);
    assert!(ics.contains("DTSTART;VALUE=DATE:20261001"));
    assert!(ics.contains("DTEND;VALUE=DATE:20261004"));
    assert!(ics.contains("RRULE:FREQ=WEEKLY;INTERVAL=1;WKST=SU;BYDAY=WE"));
    assert!(ics.contains("DTSTART:20261028T100000"));
    assert_eq!(ics.matches("BEGIN:VEVENT").count(), 4); // lunar rule expanded into 2 events
    for line in ics.split("\r\n") {
        assert!(line.len() <= 75, "{line}");
    }
}

#[test]
fn merges_a_backup_without_reverting_newer_edits_and_validates_backups() {
    let cur = json!({ "tasks": { "a": { "id": "a", "title": "本周改过", "start": "2026-10-01", "end": "2026-10-01", "updatedAt": 200 }, "b": { "id": "b", "title": "旧", "start": "2026-10-01", "end": "2026-10-01", "updatedAt": 10 } },
        "groups": { "g": { "id": "g", "name": "现有", "color": "blue", "order": 1 } }, "dayMarks": {} });
    let inc = parse_backup(&json!({
        "tasks": { "a": { "id": "a", "title": "周一的备份", "start": "2026-10-01", "end": "2026-10-01", "updatedAt": 100 }, "b": { "id": "b", "title": "备份里更新", "start": "2026-10-01", "end": "2026-10-01", "updatedAt": 50 }, "c": { "id": "c", "title": "新的", "start": "2026-10-02", "end": "2026-10-02" } },
        "groups": { "g": { "id": "g", "name": "备份名", "color": "red", "order": 1 }, "h": { "id": "h", "name": "新组", "color": "red", "order": 2 } },
    }).to_string()).unwrap();
    let p = plan_merge(&cur, &inc);
    assert_eq!(p.tasks.keys().collect::<Vec<_>>(), ["b", "c"]);
    assert_eq!((p.added, p.updated, p.kept), (1, 1, 1));
    assert_eq!(p.groups.keys().collect::<Vec<_>>(), ["h"]);
    assert!(parse_backup("nope").is_err());
    assert!(parse_backup(r#"{"tasks": {"x": {"title": 1}}}"#).is_err());
    assert!(parse_backup(&json!({ "tasks": { "a": { "id": "a", "title": "x", "start": "2026-10-01", "end": "2026-10-01" } } }).to_string()).is_ok());
}

#[test]
fn reminders_fire_before_timed_tasks_and_at_the_all_day_time() {
    let at = |h: u32, m: u32| local_ms("2026-10-29", h * 60 + m, CN);
    let timed = Task { time: Some("10:00".into()), reminders: Some(vec![15]), ..mk("m", "开会", "2026-10-29") };
    let r = |tasks: &[Task], now: i64| compute_reminders(tasks, now, "09:00", HORIZON_MS, GRACE_MS, CN);
    assert_eq!(r(&[timed.clone()], at(9, 0)).iter().map(|x| x.at_ms).collect::<Vec<_>>(), [at(9, 45)]);
    assert_eq!(r(&[timed.clone()], at(9, 50)).len(), 1); // still within grace
    assert_eq!(r(&[timed], at(10, 0)).len(), 0);
    let all_day = Task { reminders: Some(vec![0, 1440]), ..mk("d", "全天", "2026-10-30") };
    let hours: Vec<i64> = r(&[all_day], at(8, 0)).iter().map(|x| (x.at_ms / 60_000 + CN as i64).rem_euclid(1440) / 60).collect();
    assert_eq!(hours, [9, 9]);
    let daily = Task { time: Some("20:00".into()), reminders: Some(vec![0]), repeat: Some(RepeatRule { freq: Some(Freq::Daily), ..Default::default() }), done_dates: Some(vec!["2026-10-29".into()]), ..mk("x", "每天", "2026-10-29") };
    assert_eq!(r(&[daily], at(12, 0)).iter().map(|x| x.date.as_str()).collect::<Vec<_>>(), ["2026-10-30"]);
}

#[test]
fn search_finds_by_words_status_and_range() {
    let tasks = [mk("a", "交周报", "2026-10-30"), Task { done: true, ..mk("b", "周会纪要", "2025-01-01") }];
    let opts = SearchOptions { q: "周".into(), range: "all".into(), status: "all".into(), color: String::new() };
    let titles = |o: &SearchOptions, t: &[Task]| search_tasks(t, &BTreeMap::new(), o, "2026-10-29").into_iter().map(|h| h.task.title).collect::<Vec<_>>();
    assert_eq!(titles(&opts, &tasks), ["交周报", "周会纪要"]);
    assert_eq!(titles(&SearchOptions { range: "1m".into(), ..opts.clone() }, &tasks).len(), 1);
    assert_eq!(titles(&SearchOptions { status: "done".into(), ..opts.clone() }, &tasks), ["周会纪要"]);
    // A weekly task started in January is found near today and jumps to its nearest occurrence.
    let weekly = Task { repeat: Some(RepeatRule { freq: Some(Freq::Weekly), ..Default::default() }), ..mk("w", "周例会", "2026-01-05") };
    let hit = search_tasks(&[weekly], &BTreeMap::new(), &SearchOptions { q: "例会".into(), range: "1m".into(), ..opts }, "2026-10-29");
    assert_eq!(hit.iter().map(|h| h.date.as_str()).collect::<Vec<_>>(), ["2026-10-26"]);
}

#[test]
fn sample_data_is_linked() {
    let (groups, tasks) = sample_data("2026-10-29", 0);
    assert_eq!(groups.len(), 1);
    let ids: HashSet<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
    assert!(tasks.iter().all(|t| t.deps().iter().all(|d| ids.contains(d.as_str()))));
    assert!(tasks.iter().any(|t| t.is_milestone()));
    assert!(tasks.iter().all(|t| t.is_sample()) && groups[0].sample == Some(true));
}

#[test]
fn counts_down_to_the_next_statutory_holiday() {
    assert_eq!(next_holiday("2026-09-20"), Some(("中秋节".into(), "2026-09-25".into())));
    assert_eq!(next_holiday("2026-09-28"), Some(("国庆节".into(), "2026-10-01".into())));
    // during the holiday it moves on to the next one
    assert!(next_holiday("2026-10-03").is_some_and(|(_, d)| d.as_str() > "2026-10-08"));
    // 2027 schedule unpublished: 元旦 2027-01-01 is the next major festival
    assert_eq!(next_holiday("2026-10-29"), Some(("元旦".into(), "2027-01-01".into())));
}

#[test]
fn countdown_keeps_a_running_multi_day_occurrence_as_today() {
    let trip = Task { end: "2026-10-31".into(), countdown: Some(true), repeat: Some(RepeatRule { freq: Some(Freq::Yearly), ..Default::default() }), ..mk("trip", "出差", "2026-10-27") };
    let once = Task { end: "2026-10-30".into(), countdown: Some(true), ..mk("once", "展会", "2026-10-28") };
    let c = countdowns(&[trip, once.clone()], "2026-10-29", false);
    assert_eq!(c.iter().map(|x| (x.label.as_str(), x.days)).collect::<Vec<_>>(), [("出差", 0), ("展会", 0)]);
    assert!(countdowns(&[once], "2026-10-31", false).is_empty()); // over
}

#[test]
fn sync_texts() {
    let now = 1_793_332_800_000; // 2026-10-30 04:00 UTC
    let base = Status { enabled: true, provider: "WebDAV".into(), last_sync: Some(now - 5 * 60_000), ..Default::default() };
    assert_eq!(ago(now - 10_000, now, CN), "刚刚");
    assert_eq!(ago(now - 5 * 60_000, now, CN), "5 分钟前");
    assert_eq!(ago(now - 3 * 3_600_000, now, CN), "3 小时前");
    assert_eq!(ago(now + 5_000, now, CN), "刚刚"); // clock skew between PCs
    assert_eq!(ago(now - 2 * 86_400_000, now, CN), "10/28 12:00");
    assert_eq!(status_line(&Status { enabled: false, ..base.clone() }, now, CN), "未开启");
    assert_eq!(status_line(&base, now, CN), "已同步到 WebDAV，上次同步 5 分钟前");
    assert!(status_line(&Status { pending: true, ..base.clone() }, now, CN).contains("有修改等待上传"));
    assert_eq!(status_line(&Status { syncing: true, ..base.clone() }, now, CN), "正在同步到 WebDAV…");
    assert_eq!(status_line(&Status { error: Some("网络错误".into()), ..base.clone() }, now, CN), "同步失败：网络错误（稍后自动重试）");
    assert_eq!(status_line(&Status { error: Some("密码不对".into()), halted: true, ..base.clone() }, now, CN), "已暂停：密码不对");
    assert!(status_line(&Status { last_sync: None, ..base }, now, CN).contains("还没有同步过"));
    assert_eq!(uploaded_by(Some("OFFICE-PC"), Some(now - 3_600_000), now, CN), "最后由 OFFICE-PC 于 1 小时前上传");
    assert_eq!(uploaded_by(None, None, now, CN), "");
}
