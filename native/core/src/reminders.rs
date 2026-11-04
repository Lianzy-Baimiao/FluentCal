//! Reminder times (0.2's src/lib/reminders.ts).

use crate::i18n::{tr, trf};

use crate::date::{local_ms, local_of_ms, relative_day};
use crate::model::Task;
use crate::recur::occurrences;

pub const REMINDER_OFFSETS: [(i64, &str); 12] = [
    (0, "准时"),
    (5, "提前 5 分钟"),
    (10, "提前 10 分钟"),
    (15, "提前 15 分钟"),
    (30, "提前 30 分钟"),
    (60, "提前 1 小时"),
    (120, "提前 2 小时"),
    (1440, "提前 1 天"),
    (2880, "提前 2 天"),
    (4320, "提前 3 天"),
    (7200, "提前 5 天"),
    (10080, "提前 1 周"),
];

pub fn offset_label(min: i64) -> String {
    if let Some((_, l)) = REMINDER_OFFSETS.iter().find(|(v, _)| *v == min) {
        return tr(l).into();
    }
    if min % 1440 == 0 {
        return trf("提前 {0} 天", &[format!("{}", min / 1440)]);
    }
    if min % 60 == 0 {
        return trf("提前 {0} 小时", &[format!("{}", min / 60)]);
    }
    trf("提前 {0} 分钟", &[format!("{}", min)])
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReminderItem {
    pub key: String,
    pub at_ms: i64,
    pub title: String,
    pub body: String,
    pub task_id: String,
    pub date: String,
}

pub const HORIZON_MS: i64 = 48 * 3_600_000;
pub const GRACE_MS: i64 = 10 * 60_000;

fn hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':').unwrap_or((s, "0"));
    Some(h.trim().parse::<u32>().ok()? * 60 + m.trim().parse::<u32>().ok()?)
}

/// Reminders firing in (now - grace, now + horizon]. All-day tasks fire at `all_day_time` on
/// the occurrence day (minus the offset). Done occurrences are skipped. `offset_min`: the PC's
/// UTC offset (date::utc_offset_minutes).
pub fn compute_reminders(tasks: &[Task], now_ms: i64, all_day_time: &str, horizon_ms: i64, grace_ms: i64, offset_min: i32) -> Vec<ReminderItem> {
    let mut out = Vec::new();
    let from = local_of_ms(now_ms - grace_ms, offset_min).0;
    for t in tasks {
        let Some(rs) = t.reminders.as_ref().filter(|r| !r.is_empty()) else { continue };
        let max_offset = *rs.iter().max().unwrap_or(&0);
        let until = local_of_ms(now_ms + horizon_ms + max_offset * 60_000, offset_min).0;
        let time = t.time.as_deref().filter(|s| !s.is_empty()).unwrap_or(if all_day_time.is_empty() { "09:00" } else { all_day_time });
        let Some(minute) = hhmm(time) else { continue };
        for o in occurrences(t, &from, &until, 200) {
            if o.done {
                continue;
            }
            let base = local_ms(&o.start, minute, offset_min);
            for &off in rs {
                let at = base - off * 60_000;
                if at <= now_ms - grace_ms || at > now_ms + horizon_ms {
                    continue;
                }
                let at_day = local_of_ms(at, offset_min).0;
                let when = format!("{}{}", relative_day(&o.start, &at_day), t.time.as_deref().map(|s| format!(" {s}")).unwrap_or_default());
                let notes = t.notes.as_deref().and_then(|n| n.lines().next()).map(str::trim).unwrap_or("");
                let label = if off != 0 { offset_label(off) } else { String::new() };
                let body: Vec<&str> = [when.as_str(), label.as_str(), notes].into_iter().filter(|s| !s.is_empty()).collect();
                out.push(ReminderItem {
                    // The fire time is part of the key, so moving the task to another time fires again.
                    key: format!("{}@{}#{off}@{at}", t.id, o.date),
                    at_ms: at,
                    title: t.title.clone(),
                    body: body.join(" · "),
                    task_id: t.id.clone(),
                    date: o.date.clone(),
                });
            }
        }
    }
    out.sort_by_key(|r| r.at_ms);
    out
}
