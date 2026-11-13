//! The list view (0.2's src/lib/agenda.ts; WEB-03, CAL-14): tasks by day. 近期 lists the
//! occurrences from today on; 所有 lists every task once, on its start date, past ones too
//! (GitHub #1, #4).

use crate::i18n::{tr, trf};

use crate::date::{add_days, parts, weekday, WEEKDAY_SHORT};
use crate::lunar::display_info;
use crate::model::Task;
use crate::recur::{occurrences, task_end, Occurrence};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct AgendaDay<'a> {
    pub date: String,
    pub items: Vec<Occurrence<'a>>,
}

/// 近期: tasks from `start` to `end`, by day. A task that began before `start` and is still
/// running is listed on `start`. Days without tasks are left out, except `today`.
pub fn agenda<'a>(tasks: impl IntoIterator<Item = &'a Task>, start: &str, end: &str, today: &str, show_done: bool) -> Vec<AgendaDay<'a>> {
    let mut by_day: BTreeMap<String, Vec<Occurrence<'a>>> = BTreeMap::new();
    for t in tasks {
        for o in occurrences(t, start, end, 200) {
            if !show_done && o.done {
                continue;
            }
            let day = if o.start.as_str() < start { start.to_owned() } else { o.start.clone() };
            if day.as_str() <= end {
                by_day.entry(day).or_default().push(o);
            }
        }
    }
    if today >= start && today <= end {
        by_day.entry(today.to_owned()).or_default();
    }
    sorted(by_day)
}

/// 所有: every task once, on its start date, so what was recorded before can be looked through.
/// A repeating task stands for its series: never done here, its rule says when it comes. Today
/// is always there (the list opens on it).
pub fn all_tasks<'a>(tasks: impl IntoIterator<Item = &'a Task>, today: &str, show_done: bool) -> Vec<AgendaDay<'a>> {
    let mut by_day: BTreeMap<String, Vec<Occurrence<'a>>> = BTreeMap::new();
    for t in tasks {
        let done = t.repeat.is_none() && t.done;
        if (!show_done && done) || t.start.is_empty() {
            continue;
        }
        let o = Occurrence { task: t, date: t.start.clone(), start: t.start.clone(), end: task_end(t).to_owned(), done };
        by_day.entry(t.start.clone()).or_default().push(o);
    }
    by_day.entry(today.to_owned()).or_default();
    sorted(by_day)
}

/// All-day first, then by time; within that open before done, then the calendar's own order.
fn sorted(by_day: BTreeMap<String, Vec<Occurrence<'_>>>) -> Vec<AgendaDay<'_>> {
    by_day
        .into_iter()
        .map(|(date, mut items)| {
            items.sort_by(|a, b| {
                let time = |o: &Occurrence| o.task.time.clone().unwrap_or_default();
                time(a)
                    .cmp(&time(b))
                    .then(a.done.cmp(&b.done))
                    .then(a.task.order.partial_cmp(&b.task.order).unwrap_or(Ordering::Equal))
                    .then_with(|| a.task.title.cmp(&b.task.title))
            });
            AgendaDay { date, items }
        })
        .collect()
}

/// "今天", "明天", "昨天", or e.g. "10月31日"; with the year when it isn't this year's.
pub fn day_heading(date: &str, today: &str) -> String {
    if date == today {
        return tr("今天").into();
    }
    if date == add_days(today, 1) {
        return tr("明天").into();
    }
    if date == add_days(today, -1) {
        return tr("昨天").into();
    }
    let (y, m, d) = parts(date);
    if y == parts(today).0 {
        trf("{0}月{1}日", &[m.to_string(), d.to_string()])
    } else {
        trf("{0}年{1}月{2}日", &[y.to_string(), m.to_string(), d.to_string()])
    }
}

/// What follows the heading: "周四 · 十月初四 · 国庆节·休" (the weekday, the lunar day, a holiday,
/// festival or solar term).
pub fn day_sub(date: &str, show_holidays: bool) -> String {
    let i = display_info(date);
    let extra = match i.legal.as_ref().filter(|_| show_holidays) {
        Some(l) => format!("{}{}", l.name, if l.work { tr("·班") } else { tr("·休") }),
        None => i.festivals.first().cloned().or_else(|| i.term.clone()).unwrap_or_default(),
    };
    let wd = trf("周{0}", &[tr(WEEKDAY_SHORT[weekday(date) as usize]).to_owned()]);
    [wd, i.lunar_label, extra].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}
