//! Export to CSV and iCalendar, and backup import (0.2's src/lib/export.ts).

use crate::date::{add_days, days_in_month, diff_days, from_day_num, parts, weekday};
use crate::model::{Freq, Group, Task};
use crate::recur::{describe_rule, occurrences, task_end};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------------------
// CSV (opens in Excel; the caller writes it with a UTF-8 BOM)

fn csv_cell(v: &str) -> String {
    // Neutralise spreadsheet formulas (CSV injection).
    let safe = if v.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{v}") } else { v.to_owned() };
    if safe.contains(['"', ',', '\r', '\n']) { format!("\"{}\"", safe.replace('"', "\"\"")) } else { safe }
}

pub fn to_csv(tasks: &[Task], groups: &BTreeMap<String, Group>) -> String {
    let head = ["标题", "开始日期", "结束日期", "时间", "分组", "进度", "状态", "重复", "备注"];
    let mut sorted: Vec<&Task> = tasks.iter().collect();
    sorted.sort_by(|a, b| a.start.cmp(&b.start).then(a.order.total_cmp(&b.order)));
    let mut lines = vec![head.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(",")];
    for t in sorted {
        let group = t.group_id.as_ref().and_then(|g| groups.get(g)).map_or(String::new(), |g| g.name.clone());
        let progress = if t.done { 100.0 } else { t.progress };
        let row = [
            t.title.clone(),
            t.start.clone(),
            t.end.clone(),
            t.time.clone().unwrap_or_default(),
            group,
            format!("{}%", progress as i64),
            if t.done { "已完成" } else { "未完成" }.into(),
            t.repeat.as_ref().map_or(String::new(), |r| describe_rule(Some(r), &t.start)),
            t.notes.clone().unwrap_or_default(),
        ];
        lines.push(row.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(","));
    }
    lines.join("\r\n")
}

// ---------------------------------------------------------------------------------------
// iCalendar (RFC 5545)

fn compact(iso: &str) -> String {
    iso.replace('-', "")
}
const ICS_DAYS: [&str; 7] = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n")
}

/// Folds lines longer than 75 octets (UTF-8) as required by RFC 5545.
fn fold(line: &str) -> String {
    if line.len() <= 75 {
        return line.to_owned();
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in line.chars() {
        let limit = if out.is_empty() { 75 } else { 74 };
        if cur.len() + ch.len_utf8() > limit {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
    }
    out.push(cur);
    out.join("\r\n ")
}

/// UTC time as 20261029T083000Z.
fn stamp(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (date, rest) = (from_day_num(secs.div_euclid(86_400) as i32), secs.rem_euclid(86_400));
    format!("{}T{:02}{:02}{:02}Z", compact(&date), rest / 3600, rest % 3600 / 60, rest % 60)
}

fn rrule(t: &Task) -> Option<String> {
    // Rules give exactly the dates recur.rs gives (same week start, same clamping of the
    // 29th–31st), and DTSTART is the first real occurrence: RFC 5545 always counts DTSTART.
    let r = t.repeat.as_ref()?;
    let i = r.interval.unwrap_or(1).max(1);
    let (y, m, day) = parts(&t.start);
    let mut s = match r.freq? {
        Freq::Daily => format!("FREQ=DAILY;INTERVAL={i}"),
        Freq::Weekly => {
            let wd = r.weekdays.clone().filter(|w| !w.is_empty()).unwrap_or_else(|| vec![weekday(&t.start)]);
            let days: Vec<&str> = wd.iter().map(|d| ICS_DAYS[*d as usize % 7]).collect();
            // recur.rs counts "every N weeks" from the Sunday of the start week.
            format!("FREQ=WEEKLY;INTERVAL={i};WKST=SU;BYDAY={}", days.join(","))
        }
        Freq::Monthly => {
            if r.month_mode.as_deref() == Some("nthWeekday") {
                let nth = day.div_ceil(7);
                let last = nth >= 5 || day + 7 > days_in_month(y, m);
                let n = if last { "-1".to_owned() } else { nth.to_string() };
                format!("FREQ=MONTHLY;INTERVAL={i};BYDAY={n}{}", ICS_DAYS[weekday(&t.start) as usize])
            } else if day > 28 {
                // The 29th–31st fall back to the month's last day, like recur.rs.
                let days: Vec<String> = (28..=day).map(|d| d.to_string()).collect();
                format!("FREQ=MONTHLY;INTERVAL={i};BYMONTHDAY={};BYSETPOS=-1", days.join(","))
            } else {
                format!("FREQ=MONTHLY;INTERVAL={i};BYMONTHDAY={day}")
            }
        }
        Freq::Yearly if m == 2 && day == 29 => format!("FREQ=YEARLY;INTERVAL={i};BYMONTH=2;BYMONTHDAY=28,29;BYSETPOS=-1"),
        Freq::Yearly => format!("FREQ=YEARLY;INTERVAL={i}"),
        Freq::Workday => "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR".into(),
        Freq::MonthLastDay => format!("FREQ=MONTHLY;INTERVAL={i};BYMONTHDAY=-1"),
        // lunar / legal workdays / Ebbinghaus: exported as expanded events
        Freq::LegalWorkday | Freq::LunarYearly | Freq::Ebbinghaus => return None,
    };
    if let Some(c) = r.count.filter(|c| *c > 0) {
        s += &format!(";COUNT={c}");
    } else if let Some(u) = &r.until {
        // UNTIL must have the same value type as DTSTART (floating date-time for timed events).
        s += &format!(";UNTIL={}{}", compact(u), if t.time.is_some() { "T235959" } else { "" });
    }
    Some(s)
}

/// First occurrence of a repeating task, ignoring deleted dates (they become EXDATEs).
fn first_occurrence(t: &Task) -> String {
    let mut c = t.clone();
    c.ex_dates = None;
    occurrences(&c, &t.start, &add_days(&t.start, 800), 1).into_iter().next().map_or(t.start.clone(), |o| o.start)
}

fn event_lines(t: &Task, uid_suffix: &str, start: &str, end: &str, group: Option<&str>, rule: Option<&str>, now_ms: i64) -> Vec<String> {
    let mut l = vec!["BEGIN:VEVENT".to_owned(), format!("UID:{}{uid_suffix}@fluentcal", t.id), format!("DTSTAMP:{}", stamp(now_ms))];
    if let Some(time) = &t.time {
        let hm = time.replace(':', "");
        l.push(format!("DTSTART:{}T{hm}00", compact(start)));
        l.push(if start == end { "DURATION:PT1H".into() } else { format!("DTEND:{}T{hm}00", compact(end)) });
    } else {
        l.push(format!("DTSTART;VALUE=DATE:{}", compact(start)));
        l.push(format!("DTEND;VALUE=DATE:{}", compact(&add_days(end, 1))));
    }
    l.push(format!("SUMMARY:{}", esc(&t.title)));
    if let Some(n) = t.notes.as_deref().filter(|n| !n.is_empty()) {
        l.push(format!("DESCRIPTION:{}", esc(n)));
    }
    if let Some(g) = group {
        l.push(format!("CATEGORIES:{}", esc(g)));
    }
    if let Some(rule) = rule {
        l.push(format!("RRULE:{rule}"));
        if let Some(ex) = t.ex_dates.as_ref().filter(|e| !e.is_empty()) {
            match &t.time {
                Some(time) => {
                    let hm = time.replace(':', "");
                    l.push(format!("EXDATE:{}", ex.iter().map(|d| format!("{}T{hm}00", compact(d))).collect::<Vec<_>>().join(",")));
                }
                None => l.push(format!("EXDATE;VALUE=DATE:{}", ex.iter().map(|d| compact(d)).collect::<Vec<_>>().join(","))),
            }
        }
    }
    l.push("END:VEVENT".into());
    l
}

pub fn to_ics(tasks: &[Task], groups: &BTreeMap<String, Group>, now: &str, now_ms: i64) -> String {
    let mut lines: Vec<String> =
        ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//FluentCal//Desktop Calendar//ZH", "CALSCALE:GREGORIAN", "X-WR-CALNAME:FluentCal"].map(String::from).into();
    for t in tasks {
        let g = t.group_id.as_ref().and_then(|g| groups.get(g)).map(|g| g.name.as_str());
        if t.repeat.is_none() {
            lines.extend(event_lines(t, "", &t.start, task_end(t), g, None, now_ms));
            continue;
        }
        if let Some(rule) = rrule(t) {
            let first = first_occurrence(t);
            let end = add_days(&first, diff_days(&t.start, task_end(t)));
            lines.extend(event_lines(t, "", &first, &end, g, Some(&rule), now_ms));
        } else {
            // Not expressible as RRULE: one event per occurrence, from a year ago to two years
            // ahead (a lunar birthday that started in 1990 still exports this year's date).
            let back = add_days(now, -366);
            let from = if t.start > back { t.start.clone() } else { back };
            for o in occurrences(t, &from, &add_days(now, 732), 800) {
                lines.extend(event_lines(t, &format!("-{}", compact(&o.date)), &o.start, &o.end, g, None, now_ms));
            }
        }
    }
    lines.push("END:VCALENDAR".into());
    lines.iter().map(|l| fold(l)).collect::<Vec<_>>().join("\r\n") + "\r\n"
}

// ---------------------------------------------------------------------------------------
// JSON backup import

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MergePlan {
    pub tasks: Map<String, Value>,
    pub groups: Map<String, Value>,
    pub day_marks: Map<String, Value>,
    pub added: usize,
    pub updated: usize,
    pub kept: usize,
}

fn obj<'a>(v: &'a Value, k: &str) -> Option<&'a Map<String, Value>> {
    v.get(k).and_then(Value::as_object)
}

/// What a "merge" import writes: new items are added; a task on both sides takes whichever
/// version was edited last (updatedAt); existing groups and day colours are kept.
pub fn plan_merge(current: &Value, incoming: &Value) -> MergePlan {
    let empty = Map::new();
    let cur_tasks = obj(current, "tasks").unwrap_or(&empty);
    let mut p = MergePlan::default();
    for (id, t) in obj(incoming, "tasks").unwrap_or(&empty) {
        let at = |v: &Value| v.get("updatedAt").and_then(Value::as_i64).unwrap_or(0);
        match cur_tasks.get(id) {
            None => {
                p.tasks.insert(id.clone(), t.clone());
                p.added += 1;
            }
            Some(cur) if at(t) > at(cur) => {
                p.tasks.insert(id.clone(), t.clone());
                p.updated += 1;
            }
            Some(_) => p.kept += 1,
        }
    }
    for (col, out) in [("groups", &mut p.groups), ("dayMarks", &mut p.day_marks)] {
        let cur = obj(current, col).unwrap_or(&empty);
        for (id, v) in obj(incoming, col).unwrap_or(&empty) {
            if !cur.contains_key(id) {
                out.insert(id.clone(), v.clone());
            }
        }
    }
    p
}

/// Validates a backup file; returns the document or an error message.
pub fn parse_backup(text: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{FEFF}')).map_err(|_| "文件不是有效的 JSON".to_owned())?;
    let Some(tasks) = obj(&v, "tasks") else { return Err("文件中没有任务数据，可能不是 FluentCal 备份".into()) };
    for (id, t) in tasks {
        let ok = ["title", "start", "end"].iter().all(|k| t.get(*k).is_some_and(Value::is_string));
        if !ok {
            return Err(format!("任务 {id} 的格式不正确"));
        }
    }
    let or_empty = |k: &str| v.get(k).filter(|x| x.is_object()).cloned().unwrap_or_else(|| json!({}));
    Ok(json!({
        "version": 1,
        "tasks": tasks,
        "groups": or_empty("groups"),
        "dayMarks": or_empty("dayMarks"),
        "holidays": or_empty("holidays"),
        "settings": or_empty("settings"),
    }))
}
