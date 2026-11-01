//! Field edits (EditorApp's computed setters, RepeatEditor, ReminderEditor, DepsEditor).

use super::Editor;
use crate::date::{add_days, add_months, diff_days, weekday};
use crate::model::{Freq, RepeatRule};
use crate::tasks::Doc;

/// The repeat choices (RepeatEditor FREQS), value and label.
pub const FREQS: [(&str, &str); 10] = [
    ("", "不重复"),
    ("daily", "每天"),
    ("weekly", "每周"),
    ("monthly", "每月"),
    ("yearly", "每年"),
    ("workday", "工作日（周一至周五）"),
    ("legalWorkday", "法定工作日（含调休）"),
    ("lunarYearly", "农历每年"),
    ("monthLastDay", "每月最后一天"),
    ("ebbinghaus", "艾宾浩斯记忆曲线"),
];

/// The unit after "每 N" (RepeatEditor UNIT); "" = no interval for this frequency.
pub fn unit_of(f: Option<Freq>) -> &'static str {
    match f {
        Some(Freq::Daily) => "天",
        Some(Freq::Weekly) => "周",
        Some(Freq::Monthly) | Some(Freq::MonthLastDay) => "个月",
        Some(Freq::Yearly) | Some(Freq::LunarYearly) => "年",
        _ => "",
    }
}

pub fn freq_of(s: &str) -> Option<Freq> {
    serde_json::from_value(serde_json::Value::String(s.into())).ok()
}

fn on(v: &str) -> bool {
    v == "1" || v == "true"
}

fn num(v: &str) -> f64 {
    v.trim().parse::<f64>().unwrap_or(0.0)
}

impl Editor {
    /// A field changed in the window. `v` is text; numbers as decimals, booleans "1"/"0".
    pub fn edit(&mut self, key: &str, v: &str) {
        let Some(t) = self.draft.as_mut() else { return };
        match key {
            "title" => t.title = v.into(),
            "start" if !v.is_empty() => {
                // keeps the length (0.2's start setter)
                let len = diff_days(&t.start, &t.end).max(0);
                t.start = v.into();
                t.end = add_days(v, len);
            }
            "end" if !v.is_empty() => t.end = if v < t.start.as_str() { t.start.clone() } else { v.into() },
            "all-day" => t.time = if on(v) { None } else { Some("09:00".into()) },
            "time" => t.time = (!v.is_empty()).then(|| v.into()),
            "notes" => t.notes = (!v.is_empty()).then(|| v.into()),
            "color" => t.color = (!v.is_empty()).then(|| v.into()),
            "group" => t.group_id = (!v.is_empty()).then(|| v.into()),
            "progress" => t.progress = num(v).clamp(0.0, 100.0),
            // the slider's end of drag (0.2's @change): 100 % means done
            "progress-done" => {
                t.progress = num(v).clamp(0.0, 100.0);
                t.done = t.progress >= 100.0;
            }
            "done" => {
                t.done = on(v);
                t.progress = if t.done { 100.0 } else if t.progress == 100.0 { 0.0 } else { t.progress };
            }
            "milestone" => t.milestone = on(v).then_some(true),
            "countdown" => t.countdown = on(v).then_some(true),
            "postpone" => t.auto_postpone = Some(on(v)),
            "freq" => self.set_freq(v),
            "interval" => self.patch_rule(|r| r.interval = Some(num(v).round().clamp(1.0, 99.0) as u32)),
            "month-mode" => self.patch_rule(|r| r.month_mode = Some(v.into())),
            "end-mode" => {
                let start = t.start.clone();
                if v != self.end_mode() {
                    self.patch_rule(|r| {
                        r.count = (v == "count").then_some(10);
                        r.until = (v == "until").then(|| add_months(&start, 3));
                    });
                }
            }
            "count" => self.patch_rule(|r| r.count = Some(num(v).round().clamp(1.0, 999.0) as u32)),
            "until" => self.patch_rule(|r| r.until = (!v.is_empty()).then(|| v.into())),
            _ => {}
        }
    }

    fn set_freq(&mut self, v: &str) {
        let Some(t) = self.draft.as_mut() else { return };
        let cur = t.repeat.as_ref().and_then(|r| r.freq).and_then(|f| serde_json::to_value(f).ok());
        if cur.as_ref().and_then(|c| c.as_str()).unwrap_or("") == v {
            return; // the same choice (FSelect only emits a change)
        }
        t.repeat = freq_of(v).map(|f| RepeatRule {
            freq: Some(f),
            interval: Some(1),
            weekdays: (f == Freq::Weekly).then(|| vec![weekday(&t.start)]),
            ..Default::default()
        });
    }

    fn patch_rule(&mut self, f: impl FnOnce(&mut RepeatRule)) {
        if let Some(r) = self.draft.as_mut().and_then(|t| t.repeat.as_mut()) {
            f(r);
        }
    }

    /// "never" / "count" / "until" (RepeatEditor endMode).
    pub fn end_mode(&self) -> &'static str {
        match self.draft.as_ref().and_then(|t| t.repeat.as_ref()) {
            Some(r) if r.count.is_some_and(|c| c > 0) => "count",
            Some(r) if r.until.as_ref().is_some_and(|u| !u.is_empty()) => "until",
            _ => "never",
        }
    }

    /// A weekday button of the weekly rule (0 = Sunday); the last one stays on.
    pub fn toggle_day(&mut self, d: u32) {
        self.patch_rule(|r| {
            let mut set: Vec<u32> = r.weekdays.clone().unwrap_or_default();
            if set.contains(&d) && set.len() > 1 {
                set.retain(|x| *x != d);
            } else if !set.contains(&d) {
                set.push(d);
            }
            set.sort_unstable();
            set.dedup();
            r.weekdays = Some(set);
        });
    }

    /// ReminderEditor set(i, v): the list stays sorted without duplicates.
    pub fn set_reminder(&mut self, i: usize, v: i64) {
        let Some(t) = self.draft.as_mut() else { return };
        let mut next = t.reminders.clone().unwrap_or_default();
        if i < next.len() {
            next[i] = v;
        }
        next.sort_unstable();
        next.dedup();
        t.reminders = Some(next);
    }

    /// ReminderEditor add(): the first offering not in the list yet (else 准时).
    pub fn add_reminder(&mut self, doc: &Doc) {
        let opts = self.reminder_options(doc);
        let Some(t) = self.draft.as_mut() else { return };
        let mut list = t.reminders.clone().unwrap_or_default();
        let v = opts.iter().map(|(v, _)| *v).find(|v| !list.contains(v)).unwrap_or(0);
        list.push(v);
        list.sort_unstable();
        t.reminders = Some(list);
    }

    pub fn remove_reminder(&mut self, i: usize) {
        let Some(t) = self.draft.as_mut() else { return };
        let mut next = t.reminders.clone().unwrap_or_default();
        if i < next.len() {
            next.remove(i);
        }
        t.reminders = (!next.is_empty()).then_some(next);
    }

    pub fn add_dep(&mut self, id: &str) {
        let Some(t) = self.draft.as_mut() else { return };
        if !id.is_empty() {
            let mut d = t.deps.clone().unwrap_or_default();
            d.push(id.into());
            t.deps = Some(d);
        }
    }

    pub fn remove_dep(&mut self, id: &str) {
        let Some(t) = self.draft.as_mut() else { return };
        let next: Vec<String> = t.deps.clone().unwrap_or_default().into_iter().filter(|d| d != id).collect();
        t.deps = (!next.is_empty()).then_some(next);
    }
}
