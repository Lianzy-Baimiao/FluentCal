//! Sync status texts (0.2's src/lib/syncText.ts).

use crate::i18n::{tr, trf};

use crate::cloud::service::Status;
use crate::date::{local_of_ms, parts};

/// "刚刚", "5 分钟前", "3 小时前", else "10/30 14:05" (local time at UTC offset `offset_min`).
pub fn ago(ms: i64, now: i64, offset_min: i32) -> String {
    let d = (now - ms).max(0);
    if d < 60_000 {
        return tr("刚刚").into();
    }
    if d < 3_600_000 {
        return trf("{0} 分钟前", &[format!("{}", d / 60_000)]);
    }
    if d < 86_400_000 {
        return trf("{0} 小时前", &[format!("{}", d / 3_600_000)]);
    }
    let (date, min) = local_of_ms(ms, offset_min);
    let (_, m, day) = parts(&date);
    format!("{m}/{day} {:02}:{:02}", min / 60, min % 60)
}

/// One line for the status card and the calendar's sync icon.
pub fn status_line(s: &Status, now: i64, offset_min: i32) -> String {
    if !s.enabled {
        return tr("未开启").into();
    }
    if s.syncing {
        return trf("正在同步到 {0}…", &[format!("{}", s.provider)]);
    }
    if let Some(e) = &s.error {
        return if s.halted { trf("已暂停：{0}", &[format!("{}", e)]) } else { trf("同步失败：{0}（稍后自动重试）", &[format!("{}", e)]) };
    }
    let last = s.last_sync.map_or(tr("还没有同步过").to_owned(), |t| trf("上次同步 {0}", &[format!("{}", ago(t, now, offset_min))]));
    let pending = if s.pending { tr("，有修改等待上传") } else { "" };
    trf("已同步到 {0}，{1}{2}", &[format!("{}", s.provider), format!("{}", last), format!("{}", pending)])
}

/// "最后由 OFFICE-PC 于 3 小时前上传"
pub fn uploaded_by(device: Option<&str>, at: Option<i64>, now: i64, offset_min: i32) -> String {
    if device.is_none() && at.is_none() {
        return String::new();
    }
    let when = at.map(|t| trf(" 于 {0}", &[format!("{}", ago(t, now, offset_min))])).unwrap_or_default();
    trf("最后由 {0}{1}上传", &[format!("{}", device.unwrap_or(tr("另一台电脑"))), format!("{}", when)])
}
