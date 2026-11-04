//! Repeating tasks (0.2's src/lib/recur.ts): occurrences in a date range, the next occurrence,
//! and the rule in words.

use crate::i18n::{tr, trf};

use crate::date::{add_days, add_months, day_num, days_in_month, diff_days, from_day_num, parts, weekday, ymd, WEEKDAY_SHORT};
use crate::lunar::{day_info, is_legal_workday, lunar_day_name, lunar_month_name, lunar_to_solar};
use crate::model::{Freq, RepeatRule, Task};
use std::collections::HashSet;
use std::ops::ControlFlow;

/// One concrete appearance of a task on the calendar.
#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence<'a> {
    pub task: &'a Task,
    /// Occurrence start (for repeating tasks this identifies the instance).
    pub date: String,
    pub start: String,
    pub end: String,
    pub done: bool,
}

pub const EBBINGHAUS_OFFSETS: [i32; 7] = [0, 1, 2, 4, 7, 15, 30];
const MAX_STEPS: usize = 20_000;

/// The task's end date (the start when missing or before it).
pub fn task_end(t: &Task) -> &str {
    if t.end.is_empty() || t.end < t.start { &t.start } else { &t.end }
}

/// Calls `f` with the occurrence start dates of a rule, in order, from `start`, until it
/// returns Break (or the rule ends).
fn starts(rule: &RepeatRule, start: &str, from: &str, f: &mut dyn FnMut(String) -> ControlFlow<()>) {
    let interval = rule.interval.unwrap_or(1).max(1) as i32;
    let (sy, sm, sd) = parts(start);
    // Jump ahead for unbounded rules so far-future ranges don't iterate from the start.
    let can_skip = rule.count.is_none();
    macro_rules! emit {
        ($e:expr) => {
            if f($e).is_break() {
                return;
            }
        };
    }
    let Some(freq) = rule.freq else { return };
    match freq {
        Freq::Daily => {
            let mut k = if can_skip && from > start { diff_days(start, from) / interval } else { 0 };
            loop {
                emit!(add_days(start, k * interval));
                k += 1;
            }
        }
        Freq::Weekly => {
            let mut days: Vec<u32> = rule.weekdays.clone().filter(|w| !w.is_empty()).unwrap_or_else(|| vec![weekday(start)]);
            days.sort_unstable();
            // Weeks are counted from the Sunday of the start week.
            let week0 = day_num(start) - weekday(start) as i32;
            let mut w = if can_skip && from > start { ((day_num(from) - week0) / 7 / interval - 1).max(0) } else { 0 };
            loop {
                for &wd in &days {
                    let n = week0 + w * interval * 7 + wd as i32;
                    if n >= day_num(start) {
                        emit!(from_day_num(n));
                    }
                }
                w += 1;
            }
        }
        Freq::Monthly => {
            let nth = sd.div_ceil(7); // 1..5, 5 = last
            let wd = weekday(start);
            let last_week = sd + 7 > days_in_month(sy, sm);
            let first = ymd(sy, sm, 1);
            for k in 0.. {
                let (y, m, _) = parts(&add_months(&first, k * interval));
                if rule.month_mode.as_deref() == Some("nthWeekday") {
                    let first_wd = weekday(&ymd(y, m, 1));
                    let mut day = 1 + (wd + 7 - first_wd) % 7 + (nth.min(4) - 1) * 7;
                    if last_week || nth == 5 {
                        while day + 7 <= days_in_month(y, m) {
                            day += 7;
                        }
                    }
                    emit!(ymd(y, m, day));
                } else {
                    emit!(ymd(y, m, sd.min(days_in_month(y, m))));
                }
            }
        }
        Freq::Yearly => {
            for k in 0.. {
                let y = sy + k * interval;
                emit!(ymd(y, sm, sd.min(days_in_month(y, sm))));
            }
        }
        Freq::MonthLastDay => {
            let first = ymd(sy, sm, 1);
            for k in 0.. {
                let (y, m, _) = parts(&add_months(&first, k * interval));
                emit!(ymd(y, m, days_in_month(y, m)));
            }
        }
        Freq::Workday | Freq::LegalWorkday => {
            let mut d = if can_skip && from > start { from.to_owned() } else { start.to_owned() };
            loop {
                let ok = if freq == Freq::Workday { weekday(&d) % 6 != 0 } else { is_legal_workday(&d) };
                if ok {
                    emit!(d.clone());
                }
                d = add_days(&d, 1);
            }
        }
        Freq::LunarYearly => {
            let info = day_info(start);
            for k in 0..=501 {
                if let Some(s) = lunar_to_solar(info.lunar_year + k * interval, info.lunar_month, info.lunar_day) {
                    emit!(s);
                }
            }
        }
        Freq::Ebbinghaus => {
            for off in EBBINGHAUS_OFFSETS {
                emit!(add_days(start, off));
            }
        }
    }
}

/// Expands a task into occurrences overlapping [range_start, range_end] (inclusive).
pub fn occurrences<'a>(task: &'a Task, range_start: &str, range_end: &str, limit: usize) -> Vec<Occurrence<'a>> {
    let end = task_end(task);
    let Some(rule) = &task.repeat else {
        if end < range_start || task.start.as_str() > range_end {
            return Vec::new();
        }
        return vec![Occurrence { task, date: task.start.clone(), start: task.start.clone(), end: end.to_owned(), done: task.done }];
    };
    let span = diff_days(&task.start, end).max(0);
    let from = add_days(range_start, -span);
    let ex: HashSet<&str> = task.ex_dates.iter().flatten().map(String::as_str).collect();
    let done: HashSet<&str> = task.done_dates.iter().flatten().map(String::as_str).collect();
    let mut out = Vec::new();
    let (mut n, mut steps) = (0u32, 0usize);
    starts(rule, &task.start, &from, &mut |s| {
        steps += 1;
        if steps > MAX_STEPS {
            return ControlFlow::Break(());
        }
        if s < task.start {
            return ControlFlow::Continue(());
        }
        if rule.until.as_deref().is_some_and(|u| s.as_str() > u) || s.as_str() > range_end {
            return ControlFlow::Break(());
        }
        n += 1;
        if rule.count.is_some_and(|c| n > c) {
            return ControlFlow::Break(());
        }
        let e = add_days(&s, span);
        if e.as_str() < range_start || ex.contains(s.as_str()) {
            return ControlFlow::Continue(());
        }
        let is_done = done.contains(s.as_str());
        out.push(Occurrence { task, date: s.clone(), start: s, end: e, done: is_done });
        if out.len() >= limit { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    });
    out
}

/// Occurrence of a task that starts on `date`, if any.
pub fn occurrence_on<'a>(task: &'a Task, date: &str) -> Option<Occurrence<'a>> {
    occurrences(task, date, date, 400).into_iter().find(|o| o.date == date)
}

/// Next occurrence start on or after `from`.
pub fn next_occurrence(task: &Task, from: &str) -> Option<String> {
    let horizon = add_days(from, 800);
    occurrences(task, from, &horizon, 1).into_iter().find(|o| o.start.as_str() >= from).map(|o| o.start)
}

/// Human readable rule, e.g. "每 2 周的周一、周三".
pub fn describe_rule(rule: Option<&RepeatRule>, start: &str) -> String {
    let Some(rule) = rule else { return tr("不重复").into() };
    let i = rule.interval.unwrap_or(1).max(1);
    let every = |unit: &str| if i == 1 { trf("每{0}", &[format!("{}", unit)]) } else { trf("每 {0} {1}", &[format!("{}", i), format!("{}", unit)]) };
    let (y, m, d) = parts(start);
    let mut s = match rule.freq {
        None => return tr("不重复").into(),
        Some(Freq::Daily) => if i == 1 { tr("每天").into() } else { trf("每 {0} 天", &[format!("{}", i)]) },
        Some(Freq::Weekly) => {
            let mut days: Vec<u32> = rule.weekdays.clone().filter(|w| !w.is_empty()).unwrap_or_else(|| vec![weekday(start)]);
            days.sort_by_key(|x| (x + 6) % 7);
            let days: Vec<String> = days.iter().map(|x| trf("周{0}", &[format!("{}", crate::i18n::tr(WEEKDAY_SHORT[*x as usize % 7]))])).collect();
            trf("{0}的{1}", &[format!("{}", every(tr("周"))), format!("{}", days.join("、"))])
        }
        Some(Freq::Monthly) => {
            if rule.month_mode.as_deref() == Some("nthWeekday") {
                let nth = d.div_ceil(7);
                let last = nth >= 5 || d + 7 > days_in_month(y, m);
                let which = if last { tr("最后一个").to_owned() } else { trf("第 {0} 个", &[format!("{}", nth)]) };
                trf("{0}的{1}周{2}", &[format!("{}", every(tr("个月"))), format!("{}", which), format!("{}", crate::i18n::tr(WEEKDAY_SHORT[weekday(start) as usize]))])
            } else {
                trf("{0}的 {1} 日", &[format!("{}", every(tr("个月"))), format!("{}", d)])
            }
        }
        Some(Freq::Yearly) => trf("{0}的 {1} 月 {2} 日", &[format!("{}", if i == 1 { tr("每年").to_owned() } else { trf("每 {0} 年", &[i.to_string()]) }), format!("{}", m), format!("{}", d)]),
        Some(Freq::Workday) => tr("每个工作日（周一至周五）").into(),
        Some(Freq::LegalWorkday) => tr("每个法定工作日（含调休）").into(),
        Some(Freq::LunarYearly) => {
            let info = day_info(start);
            let each = if i == 1 { tr("每年").to_owned() } else { trf("每 {0} 年", &[format!("{}", i)]) };
            trf("{0}农历{1}{2}", &[format!("{}", each), format!("{}", lunar_month_name(info.lunar_month)), format!("{}", lunar_day_name(info.lunar_day))])
        }
        Some(Freq::MonthLastDay) => trf("{0}的最后一天", &[format!("{}", every(tr("个月")))]),
        Some(Freq::Ebbinghaus) => return tr("艾宾浩斯记忆曲线（第 1、2、3、5、8、16、31 天复习）").into(),
    };
    if let Some(c) = rule.count.filter(|c| *c > 0) {
        s += &trf("，共 {0} 次", &[format!("{}", c)]);
    } else if let Some(u) = &rule.until {
        s += &trf("，直到 {0}", &[format!("{}", u)]);
    }
    s
}
