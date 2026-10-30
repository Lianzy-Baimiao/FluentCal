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

/// Festivals that are statutory holidays, used before the official schedule is published.
const MAJOR: [&str; 7] = ["元旦", "春节", "清明", "劳动节", "端午节", "中秋节", "国庆节"];

/// First day of the next statutory holiday within ~5 months. When the official schedule for
/// that period is not published yet, falls back to the date of the next major festival.
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

/// Tasks flagged as countdown (next occurrence) plus, optionally, the next holiday.
pub fn countdowns(tasks: &[Task], now: &str, with_holiday: bool) -> Vec<CountdownItem> {
    let mut out = Vec::new();
    if with_holiday {
        if let Some((name, date)) = next_holiday(now) {
            out.push(CountdownItem { key: format!("holiday:{date}"), label: name, days: diff_days(now, &date), date, task_id: None });
        }
    }
    for t in tasks {
        if t.countdown != Some(true) || (t.done && t.repeat.is_none()) {
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
