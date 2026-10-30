//! Sync status texts (0.2's src/lib/syncText.ts).

use crate::cloud::service::Status;
use crate::date::{local_of_ms, parts};

/// "刚刚", "5 分钟前", "3 小时前", else "10/30 14:05" (local time at UTC offset `offset_min`).
pub fn ago(ms: i64, now: i64, offset_min: i32) -> String {
    let d = (now - ms).max(0);
    if d < 60_000 {
        return "刚刚".into();
    }
    if d < 3_600_000 {
        return format!("{} 分钟前", d / 60_000);
    }
    if d < 86_400_000 {
        return format!("{} 小时前", d / 3_600_000);
    }
    let (date, min) = local_of_ms(ms, offset_min);
    let (_, m, day) = parts(&date);
    format!("{m}/{day} {:02}:{:02}", min / 60, min % 60)
}

/// One line for the status card and the calendar's sync icon.
pub fn status_line(s: &Status, now: i64, offset_min: i32) -> String {
    if !s.enabled {
        return "未开启".into();
    }
    if s.syncing {
        return format!("正在同步到 {}…", s.provider);
    }
    if let Some(e) = &s.error {
        return if s.halted { format!("已暂停：{e}") } else { format!("同步失败：{e}（稍后自动重试）") };
    }
    let last = s.last_sync.map_or("还没有同步过".to_owned(), |t| format!("上次同步 {}", ago(t, now, offset_min)));
    let pending = if s.pending { "，有修改等待上传" } else { "" };
    format!("已同步到 {}，{last}{pending}", s.provider)
}

/// "最后由 OFFICE-PC 于 3 小时前上传"
pub fn uploaded_by(device: Option<&str>, at: Option<i64>, now: i64, offset_min: i32) -> String {
    if device.is_none() && at.is_none() {
        return String::new();
    }
    let when = at.map(|t| format!(" 于 {}", ago(t, now, offset_min))).unwrap_or_default();
    format!("最后由 {}{when}上传", device.unwrap_or("另一台电脑"))
}
