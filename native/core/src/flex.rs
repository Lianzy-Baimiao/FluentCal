//! 不定日待办 (GitHub #7): things to do without a day. Two kinds (`Task::flex`):
//! - "month": once every month, on whichever day. Each month has one slot ("m:2026-11");
//!   ticking it records the day it was done, and the calendar shows it done on that day. Last
//!   month's slot left open is shown as overdue for one more month.
//! - "someday": once, no date at all (slot "s:"); gone from the list when done.
//!
//! They are ordinary tasks in the data (sync, undo, export and the editor work as for any
//! task): `start` is the first day of the first month (month) or the day it was written down
//! (someday), and `flex_done` maps a slot's key ("2026-11", or "" for someday) to the day it
//! was done. A version that doesn't know `flex` shows them as plain tasks on `start`.

use crate::date::{add_days, add_months, end_of_month, start_of_month};
use crate::model::Task;
use crate::recur::Occurrence;
use std::collections::BTreeSet;

pub const MONTH: &str = "month";
pub const SOMEDAY: &str = "someday";
pub const SOMEDAY_SLOT: &str = "s:";

pub fn is_flex(t: &Task) -> bool {
    kind(t).is_some()
}

/// "month" | "someday"; a kind written by a newer version reads as none.
pub fn kind(t: &Task) -> Option<&str> {
    t.flex.as_deref().filter(|k| *k == MONTH || *k == SOMEDAY)
}

/// "2026-11" of a date.
pub fn month_key(date: &str) -> &str {
    date.get(..7).unwrap_or(date)
}

pub fn month_slot(date: &str) -> String {
    format!("m:{}", month_key(date))
}

/// A slot ("m:2026-11", "s:") rather than a calendar date.
pub fn is_slot(date: &str) -> bool {
    date.starts_with("m:") || date == SOMEDAY_SLOT
}

fn key_of(slot: &str) -> &str {
    slot.split_once(':').map_or(slot, |(_, k)| k)
}

/// The day a slot was done.
pub fn slot_done<'a>(t: &'a Task, slot: &str) -> Option<&'a str> {
    t.flex_done.as_ref()?.get(key_of(slot)).map(String::as_str)
}

/// Ticks a slot (done on `day`) or unticks it.
pub fn set_slot(t: &mut Task, slot: &str, day: Option<&str>) {
    let mut m = t.flex_done.take().unwrap_or_default();
    match day {
        Some(d) => drop(m.insert(key_of(slot).to_owned(), d.to_owned())),
        None => drop(m.remove(key_of(slot))),
    }
    if kind(t) == Some(SOMEDAY) {
        t.done = !m.is_empty();
        t.progress = if t.done { 100.0 } else { 0.0 };
    }
    t.flex_done = (!m.is_empty()).then_some(m);
}

/// Unticks what was done on `day` (the done chip in the calendar was unticked).
pub fn clear_day(t: &mut Task, day: &str) {
    let slots: Vec<String> = t.flex_done.iter().flatten().filter(|(_, d)| d.as_str() == day).map(|(k, _)| k.clone()).collect();
    for k in slots {
        set_slot(t, &k, None);
    }
}

/// What was done on `from` was done on `to` (the done chip was dragged to another day).
pub fn move_day(t: &mut Task, from: &str, to: &str) {
    for d in t.flex_done.iter_mut().flat_map(|m| m.values_mut()).filter(|d| d.as_str() == from) {
        *d = to.to_owned();
    }
}

/// The calendar's view of a flex task: done, on each day one of its slots was done.
pub fn done_occurrences<'a>(t: &'a Task, range_start: &str, range_end: &str) -> Vec<Occurrence<'a>> {
    let days: BTreeSet<&str> = t.flex_done.iter().flatten().map(|(_, d)| d.as_str())
        .filter(|d| crate::date::is_iso_date(d) && *d >= range_start && *d <= range_end).collect();
    days.into_iter().map(|d| Occurrence { task: t, date: d.to_owned(), start: d.to_owned(), end: d.to_owned(), done: true }).collect()
}

/// All dated completion records for the "所有" list. Multiple slots completed on one day
/// share one calendar occurrence, just as they do in the month and multi-week views.
pub fn all_done_occurrences(t: &Task) -> Vec<Occurrence<'_>> {
    let days: BTreeSet<&str> = t.flex_done.iter().flatten().map(|(_, d)| d.as_str()).filter(|d| crate::date::is_iso_date(d)).collect();
    days.into_iter().map(|d| Occurrence { task: t, date: d.to_owned(), start: d.to_owned(), end: d.to_owned(), done: true }).collect()
}

/// The first month a "month" task counts for.
fn first_month(t: &Task) -> &str {
    month_key(&t.start)
}

/// The to-do column beside the calendar.
#[derive(Debug, Default)]
pub struct Side<'a> {
    /// This month's: (task, the day it was done). Done ones stay, struck through.
    pub month: Vec<(&'a Task, Option<&'a str>)>,
    /// Left open last month.
    pub overdue: Vec<&'a Task>,
    /// Without any date, not done.
    pub someday: Vec<&'a Task>,
    pub month_slot: String,
    pub overdue_slot: String,
}

pub fn side<'a>(tasks: impl IntoIterator<Item = &'a Task>, today: &str) -> Side<'a> {
    let this = month_key(today).to_owned();
    let prev = month_key(&add_months(&start_of_month(today), -1)).to_owned();
    let mut s = Side { month_slot: format!("m:{this}"), overdue_slot: format!("m:{prev}"), ..Default::default() };
    for t in tasks {
        match kind(t) {
            Some(MONTH) => {
                if first_month(t) <= this.as_str() {
                    s.month.push((t, slot_done(t, &s.month_slot)));
                }
                if first_month(t) <= prev.as_str() && slot_done(t, &s.overdue_slot).is_none() {
                    s.overdue.push(t);
                }
            }
            Some(SOMEDAY) if slot_done(t, SOMEDAY_SLOT).is_none() => s.someday.push(t),
            _ => {}
        }
    }
    let by_order = |a: &&Task, b: &&Task| a.order.total_cmp(&b.order).then_with(|| a.id.cmp(&b.id));
    s.month.sort_by(|a, b| by_order(&a.0, &b.0));
    s.overdue.sort_by(by_order);
    s.someday.sort_by(by_order);
    s
}

/// Bars of the Gantt chart, as (slot or day, start, end, done): a "month" task still open is
/// a bar over the whole month; what was done is a block on its day.
pub fn bars(t: &Task, range_start: &str, range_end: &str, today: &str) -> Vec<(String, String, String, bool)> {
    let mut out: Vec<_> = done_occurrences(t, range_start, range_end).into_iter().map(|o| (o.date, o.start, o.end, true)).collect();
    if let Some((slot, first, last)) = open_month(t, today) {
        if last.as_str() >= range_start && first.as_str() <= range_end {
            out.push((slot, first, last, false));
        }
    }
    out
}

/// This month's slot of a "month" task while it is open: (slot, first day, last day).
pub fn open_month(t: &Task, today: &str) -> Option<(String, String, String)> {
    if kind(t) != Some(MONTH) || first_month(t) > month_key(today) {
        return None;
    }
    let slot = month_slot(today);
    slot_done(t, &slot).is_none().then(|| (slot, start_of_month(today), end_of_month(today)))
}

/// What reminders count from: the last day of the month, for a "month" task still open in the
/// month of `day` ("以月底为基准，提前 N 天提醒").
pub fn due_occurrences<'a>(t: &'a Task, range_start: &str, range_end: &str) -> Vec<Occurrence<'a>> {
    if kind(t) != Some(MONTH) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut first = start_of_month(range_start);
    while first.as_str() <= range_end {
        let last = end_of_month(&first);
        let slot = month_slot(&first);
        if last.as_str() >= range_start && last.as_str() <= range_end && first_month(t) <= month_key(&first) && slot_done(t, &slot).is_none() {
            out.push(Occurrence { task: t, date: slot, start: last.clone(), end: last.clone(), done: false });
        }
        first = add_days(&last, 1);
    }
    out
}

/// A new flex task written into the column on `today`.
pub fn new_task(kind: &str, title: &str, today: &str) -> Task {
    let start = if kind == MONTH { start_of_month(today) } else { today.to_owned() };
    Task { title: title.to_owned(), end: start.clone(), start, flex: Some(kind.to_owned()), auto_postpone: Some(false), ..Default::default() }
}

/// "每月一次（不定日）" / "无日期".
pub fn describe(t: &Task) -> &'static str {
    match kind(t) {
        Some(MONTH) => crate::i18n::tr("每月一次（不定日）"),
        Some(_) => crate::i18n::tr("无日期"),
        None => "",
    }
}
