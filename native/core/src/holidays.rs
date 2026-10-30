//! Statutory holiday schedule updates (0.2's src/lib/holidayFetch.ts and stores/holidays.ts):
//! China's schedule incl. 调休 working days, scraped from gov.cn by the holiday-cn project.

use crate::cloud::http::Http;
use crate::date::{is_iso_date, now_ms};
use crate::model::{HolidayDay, HolidayYear};
use crate::store::Store;
use serde_json::{json, Map, Value};
use std::time::Duration;

/// cdn.jsdelivr.net is unreachable on some networks, so try mirrors.
const SOURCES: [&str; 2] = [
    "https://fastly.jsdelivr.net/gh/NateScarlet/holiday-cn@master/{y}.json",
    "https://raw.githubusercontent.com/NateScarlet/holiday-cn/master/{y}.json",
];
/// Checked at most this often unless forced.
pub const INTERVAL_MS: i64 = 3 * 86_400_000;

/// A holiday-cn year file; None when it isn't one. An unpublished year has no days.
pub fn parse_year(v: &Value, year: i32) -> Option<HolidayYear> {
    if v.get("year").and_then(Value::as_i64) != Some(year as i64) {
        return None;
    }
    let days = v.get("days")?.as_array()?;
    let days = days
        .iter()
        .filter_map(|d| {
            let date = d.get("date")?.as_str().filter(|s| is_iso_date(s))?;
            Some(HolidayDay { date: date.into(), name: d.get("name")?.as_str()?.into(), is_off_day: d.get("isOffDay").and_then(Value::as_bool).unwrap_or(false) })
        })
        .collect();
    Some(HolidayYear { year, fetched_at: now_ms(), days })
}

/// Fetches one year; None when every source failed. `proxy` as in the sync settings.
pub fn fetch_year(year: i32, proxy: &str) -> Option<HolidayYear> {
    let http = Http::new(proxy, Duration::from_secs(8)).ok()?;
    SOURCES.iter().find_map(|src| {
        let r = http.send("GET", &src.replace("{y}", &year.to_string()), &[], None).ok()?;
        (r.status == 200).then(|| r.json()).flatten().and_then(|v| parse_year(&v, year))
    })
}

/// Refreshes this and next year's schedule (blocking: call it off the UI thread). Returns
/// (years updated, every source failed). A year not published yet keeps the built-in data.
pub fn update(store: &Store, this_year: i32, force: bool, proxy: &str) -> (Vec<i32>, bool) {
    let auto = store.setting_bool("/holidays/autoUpdate", true);
    let last = store.setting("/holidays/lastCheck").and_then(|v| v.as_i64()).unwrap_or(0);
    if !force && (!auto || now_ms() - last < INTERVAL_MS) {
        return (Vec::new(), false);
    }
    let res: Vec<Option<HolidayYear>> = [this_year, this_year + 1].iter().map(|y| fetch_year(*y, proxy)).collect();
    let mut items = Map::new();
    let mut updated = Vec::new();
    for r in res.iter().flatten().filter(|r| !r.days.is_empty()) {
        items.insert(r.year.to_string(), serde_json::to_value(r).unwrap_or(Value::Null));
        updated.push(r.year);
    }
    if !items.is_empty() {
        let _ = store.write("holidays", "holidays", items);
    }
    if res.iter().any(Option::is_some) {
        let _ = store.update_settings("holidays", &json!({ "holidays": { "lastCheck": now_ms() } }));
    }
    (updated, res.iter().all(Option::is_none))
}
