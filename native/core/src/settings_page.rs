//! What the settings window computes (0.2's src/windows/settings/*.vue scripts): texts as
//! `toLocaleString('zh-CN')` formats them, patches from a setting's path (useSetting), the
//! presets (presets.ts), the reminder editor's list (ReminderEditor.vue) and the like.

use crate::date::{local_of_ms, parts, weekday, WEEKDAY_SHORT};
use crate::reminders::{offset_label, REMINDER_OFFSETS};
use crate::settings::Settings;
use serde_json::{json, Map, Value};

/// "2026/10/14 10:30:00" (toLocaleString('zh-CN')).
pub fn zh_datetime(ms: i64, offset_min: i32) -> String {
    let (day, minute) = local_of_ms(ms, offset_min);
    let (y, m, d) = parts(&day);
    let sec = (ms + offset_min as i64 * 60_000).rem_euclid(60_000) / 1000;
    format!("{y}/{m}/{d} {:02}:{:02}:{sec:02}", minute / 60, minute % 60)
}

/// "10/15周四 09:00" ({ month: numeric, day: numeric, weekday: short, hour/minute: 2-digit }).
pub fn zh_reminder_time(ms: i64, offset_min: i32) -> String {
    let (day, minute) = local_of_ms(ms, offset_min);
    let (_, m, d) = parts(&day);
    format!("{m}/{d}周{} {:02}:{:02}", WEEKDAY_SHORT[weekday(&day) as usize], minute / 60, minute % 60)
}

/// "12 KB" (at least 1).
pub fn kb(bytes: u64) -> String {
    format!("{} KB", ((bytes as f64 / 1024.0).round() as u64).max(1))
}

/// { a: { b: value } } from "a.b" (0.2's useSetting setter).
pub fn patch(path: &str, value: Value) -> Value {
    path.rsplit('.').fold(value, |acc, k| {
        let mut m = Map::new();
        m.insert(k.to_owned(), acc);
        Value::Object(m)
    })
}

/// A number as the settings store it: whole numbers as integers.
pub fn num(v: f64) -> Value {
    if v.fract() == 0.0 && v.abs() < 9e15 {
        json!(v as i64)
    } else {
        json!(v)
    }
}

pub struct Preset {
    pub id: &'static str,
    pub bg: f64,
    pub cell: f64,
    pub acrylic: bool,
    shadow: bool,
    text_shadow: &'static str,
    mode: Option<&'static str>,
}

const fn preset(id: &'static str, bg: f64, cell: f64, acrylic: bool, shadow: bool, text_shadow: &'static str, mode: Option<&'static str>) -> Preset {
    Preset { id, bg, cell, acrylic, shadow, text_shadow, mode }
}

/// presets.ts PRESETS.
pub const PRESETS: [Preset; 5] = [
    preset("light", 92.0, 70.0, false, true, "auto", Some("light")),
    preset("dark", 90.0, 60.0, false, true, "auto", Some("dark")),
    preset("acrylic", 45.0, 40.0, true, false, "auto", None),
    preset("glass", 18.0, 38.0, false, false, "auto", None),
    preset("minimal", 0.0, 10.0, false, false, "on", None),
];

/// The settings patch a preset applies.
pub fn preset_patch(id: &str) -> Option<Value> {
    let p = PRESETS.iter().find(|p| p.id == id)?;
    let desk = json!({
        "acrylic": p.acrylic, "bgColor": "auto", "shadow": p.shadow, "textShadow": p.text_shadow,
        "bgOpacity": num(p.bg), "cellOpacity": num(p.cell),
    });
    Some(match p.mode {
        Some(m) => json!({ "theme": { "mode": m }, "desk": desk }),
        None => json!({ "desk": desk }),
    })
}

/// presets.ts matchPreset: the preset whose opacities and acrylic the settings have, or "".
pub fn match_preset(s: &Settings) -> &'static str {
    let d = &s.desk;
    PRESETS.iter().find(|p| p.bg == d.bg_opacity && p.cell == d.cell_opacity && p.acrylic == d.acrylic).map_or("", |p| p.id)
}

/// CalendarPage toggleRest: the rest days with `day` on or off, sorted.
pub fn rest_days(days: &[u32], day: u32, on: bool) -> Vec<u32> {
    let mut v: Vec<u32> = days.iter().copied().filter(|d| *d != day).collect();
    if on {
        v.push(day);
    }
    v.sort_unstable();
    v
}

/// ReminderEditor's options (allDay = false): the standard offsets and any other in the list.
pub fn offset_options(list: &[i64]) -> Vec<(i64, String)> {
    let mut out: Vec<(i64, String)> = REMINDER_OFFSETS.iter().map(|(v, l)| (*v, (*l).to_owned())).collect();
    for v in list {
        if !out.iter().any(|(o, _)| o == v) {
            out.push((*v, offset_label(*v)));
        }
    }
    out
}

/// ReminderEditor set / remove (`value` None): unique and sorted; empty = no default.
pub fn set_offset(list: &[i64], i: usize, value: Option<i64>) -> Vec<i64> {
    let mut next = list.to_vec();
    match value {
        Some(v) if i < next.len() => next[i] = v,
        Some(_) => {}
        None if i < next.len() => {
            next.remove(i);
            return next;
        }
        None => {}
    }
    next.sort_unstable();
    next.dedup();
    next
}

/// ReminderEditor add: the first option not used yet (0 if all are), sorted.
pub fn add_offset(list: &[i64]) -> Vec<i64> {
    let v = offset_options(list).iter().map(|(v, _)| *v).find(|v| !list.contains(v)).unwrap_or(0);
    let mut next = list.to_vec();
    next.push(v);
    next.sort_unstable();
    next
}

/// SyncPage proxyMode: "" = system, "direct", else custom.
pub fn proxy_mode(proxy: &str) -> &'static str {
    match proxy {
        "" => "system",
        "direct" => "direct",
        _ => "custom",
    }
}

/// The proxy setting for a mode (SyncPage proxyMode setter).
pub fn proxy_for_mode(mode: &str) -> &'static str {
    match mode {
        "system" => "",
        "direct" => "direct",
        _ => "http://127.0.0.1:7890",
    }
}

/// GeneralPage pinHint.
pub fn pin_hint(mode: &str) -> &'static str {
    match mode {
        "desktop" => "钉在桌面上：在所有窗口之下，按 Win+D 显示桌面时仍可见，不占任务栏。",
        "top" => "始终显示在其他窗口之上（会挡住其他程序）。",
        "normal" => "和普通程序一样，可出现在任务栏和 Alt+Tab 中。",
        _ => "",
    }
}

/// GeneralPage: the holiday card's description.
pub fn holiday_desc(last_check: i64, years: &[String], offset_min: i32) -> String {
    let last = if last_check > 0 { zh_datetime(last_check, offset_min) } else { "从未".to_owned() };
    let mut y = years.to_vec();
    y.sort();
    let years = if y.is_empty() { "无（使用内置数据）".to_owned() } else { y.join("、") };
    format!("联网获取国务院公布的放假和调休安排。上次检查：{last}；在线数据：{years}")
}

/// GeneralPage checkHolidays: the message after an update round.
pub fn holiday_result(updated: &[i32], failed: bool) -> String {
    if failed {
        "无法连接节假日数据源，请检查网络。".into()
    } else if updated.is_empty() {
        "已是最新，暂无新的放假安排。".into()
    } else {
        let y: Vec<String> = updated.iter().map(i32::to_string).collect();
        format!("已更新 {} 年放假安排。", y.join("、"))
    }
}
