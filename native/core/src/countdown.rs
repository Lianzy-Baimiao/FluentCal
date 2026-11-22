//! The countdown bar under the calendar (0.2's src/lib/countdown.ts).

use crate::date::{add_days, diff_days};
use crate::lunar::{day_info, legal_holiday};
use crate::model::Task;
use crate::recur::occurrences;

#[derive(Debug, Clone, PartialEq)]
pub struct CountdownItem {
    pub key: String,
    pub label: String,
    pub date: String,
    /// Days from today (0 = today).
    pub days: i32,
    pub task_id: Option<String>,
}

/// Major festivals treated as statutory holidays before an official schedule is published.
const MAJOR: [&str; 7] = ["元旦", "春节", "清明", "劳动节", "端午节", "中秋节", "国庆节"];

/// First day of the next statutory holiday. If the schedule is unpublished, falls back to the
/// next major festival date.
pub fn next_holiday(now: &str) -> Option<(String, String)> {
    // Skip a holiday that is already running so the countdown moves on to the next one.
    let mut start = now.to_owned();
    while legal_holiday(&start).is_some_and(|l| !l.work) {
        start = add_days(&start, 1);
    }
    let mut d = start.clone();
    for _ in 0..150 {
        if let Some(h) = legal_holiday(&d).filter(|h| !h.work) {
            return Some((h.name, d));
        }
        d = add_days(&d, 1);
    }
    let mut d = start;
    for _ in 0..400 {
        if let Some(f) = day_info(&d).festivals.into_iter().find(|x| MAJOR.contains(&x.as_str())) {
            if d.as_str() > now {
                return Some((f, d));
            }
        }
        d = add_days(&d, 1);
    }
    None
}

/// Next civil festival or solar term, optionally skipping a date already shown by another source.
pub fn next_observance(now: &str, term_only: bool) -> Option<(String, String)> {
    let mut date = add_days(now, 1);
    for _ in 0..800 {
        let info = day_info(&date);
        let names: Vec<_> = if term_only {
            info.term.into_iter().collect()
        } else {
            info.festivals.into_iter().filter(|name| !MAJOR.contains(&name.as_str())).collect()
        };
        if !names.is_empty() {
            return Some((names.join("、"), date));
        }
        date = add_days(&date, 1);
    }
    None
}

fn add_source(out: &mut Vec<CountdownItem>, key: &str, name: String, date: String, now: &str) {
    if let Some(existing) = out.iter_mut().find(|item| item.date == date) {
        let normal = |label: &str| label.replace("节假日", "").replace('节', "");
        if !existing.label.split('、').any(|part| normal(part) == normal(&name)) {
            existing.label.push('、');
            existing.label.push_str(&name);
        }
        return;
    }
    out.push(CountdownItem { key: format!("{key}:{date}"), label: name, days: diff_days(now, &date), date, task_id: None });
}

fn add_upcoming_observances(out: &mut Vec<CountdownItem>, now: &str, with_holidays: bool, with_festivals: bool, with_terms: bool) {
    if with_holidays {
        if let Some((name, date)) = next_holiday(now) {
            add_source(out, "holiday", name, date, now);
        }
    }
    // Keep the historical single-holiday countdown as the default display. Enabling other
    // calendar sources turns the picker into a multi-item source union.
    if with_holidays && !with_festivals && !with_terms {
        return;
    }
    let mut date = add_days(now, 1);
    for _ in 0..366 {
        let info = day_info(&date);
        if with_holidays {
            if let Some(holiday) = legal_holiday(&date).filter(|holiday| !holiday.work) {
                let previous = legal_holiday(&add_days(&date, -1));
                let starts_holiday = !previous.is_some_and(|previous| !previous.work && previous.name == holiday.name);
                if starts_holiday {
                    add_source(out, "holiday", holiday.name, date.clone(), now);
                }
            }
        }
        if with_festivals {
            for festival in info.festivals.into_iter().filter(|name| !MAJOR.contains(&name.as_str())) {
                add_source(out, "festival", festival, date.clone(), now);
            }
        }
        if with_terms {
            if let Some(term) = info.term {
                add_source(out, "term", term, date.clone(), now);
            }
        }
        date = add_days(&date, 1);
    }
}

/// Tasks flagged as countdown (next occurrence) plus, optionally, the next holiday.
pub fn countdowns(tasks: &[Task], now: &str, with_holiday: bool) -> Vec<CountdownItem> {
    selected_countdowns(tasks, now, with_holiday, false, false, true, &[])
}

/// Union of marked tasks and selected groups; each task contributes at most one occurrence.
pub fn selected_countdowns(tasks: &[Task], now: &str, with_holiday: bool, with_festivals: bool, with_terms: bool, marked: bool, groups: &[String]) -> Vec<CountdownItem> {
    let mut out = Vec::new();
    add_upcoming_observances(&mut out, now, with_holiday, with_festivals, with_terms);
    for t in tasks {
        let selected = (marked && t.countdown == Some(true)) || groups.iter().any(|g| g == t.group_id.as_deref().unwrap_or(""));
        if !selected || (t.done && t.repeat.is_none()) {
            continue;
        }
        // The running or next occurrence; one that already started counts as "today".
        let Some(occ) = occurrences(t, now, &add_days(now, 800), 1).into_iter().next() else { continue };
        let date = if occ.start.as_str() < now { now.to_owned() } else { occ.start };
        out.push(CountdownItem { key: t.id.clone(), label: t.title.clone(), days: diff_days(now, &date), date, task_id: Some(t.id.clone()) });
    }
    out.sort_by_key(|c| c.days);
    out
}

#[cfg(test)]
mod tests {
    use super::{add_source, add_upcoming_observances};

    #[test]
    fn same_day_observance_sources_are_combined() {
        let mut out = Vec::new();
        add_source(&mut out, "holiday", "清明节假期".into(), "2026-04-05".into(), "2026-04-04");
        add_source(&mut out, "term", "清明".into(), "2026-04-05".into(), "2026-04-04");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].label, "清明节假期、清明");
        assert_eq!(out[0].days, 1);
    }

    #[test]
    fn each_selected_source_lists_upcoming_observances_for_a_year() {
        let mut out = Vec::new();
        add_upcoming_observances(&mut out, "2026-09-24", true, true, true);
        let mid_autumn: Vec<_> = out.iter().filter(|item| item.date == "2026-09-25").collect();
        assert_eq!(mid_autumn.len(), 1);
        assert!(mid_autumn[0].label.contains("中秋"));
        assert!(out.iter().filter(|item| item.key.starts_with("term:")).count() > 2);
        assert!(out.iter().any(|item| item.key.starts_with("festival:") && item.label == "圣诞节"));
    }
}
