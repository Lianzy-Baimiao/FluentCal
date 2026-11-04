//! Dates as in 0.2's src/lib/date.ts: ISO strings `YYYY-MM-DD` (local time) in the data, and
//! day numbers (days since 1970-01-01) for arithmetic, so adding days never drifts.

use crate::i18n::{tr, trf};

/// Days since 1970-01-01 (proleptic Gregorian; H. Hinnant's algorithm).
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i32 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i32 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn civil_from_days(z: i32) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i32::from(m <= 2), m, d)
}

pub fn ymd(y: i32, m: u32, d: u32) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// (year, month, day) of a well-formed ISO date; zeros for anything else.
pub fn parts(iso: &str) -> (i32, u32, u32) {
    let n = |r: std::ops::Range<usize>| iso.get(r).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    (n(0..4) as i32, n(5..7), n(8..10))
}

pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        return false;
    }
    let (y, m, d) = parts(s);
    (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m)
}

pub fn day_num(iso: &str) -> i32 {
    let (y, m, d) = parts(iso);
    days_from_civil(y, m, d)
}

pub fn from_day_num(n: i32) -> String {
    let (y, m, d) = civil_from_days(n);
    ymd(y, m, d)
}

pub fn add_days(iso: &str, n: i32) -> String {
    from_day_num(day_num(iso) + n)
}

/// b - a in days
pub fn diff_days(a: &str, b: &str) -> i32 {
    day_num(b) - day_num(a)
}

/// 0 = Sunday … 6 = Saturday
pub fn weekday(iso: &str) -> u32 {
    weekday_of(day_num(iso))
}

pub fn weekday_of(n: i32) -> u32 {
    (n + 4).rem_euclid(7) as u32 // 1970-01-01 was a Thursday
}

pub fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(y) => 29,
        2 => 28,
        _ => 0,
    }
}

pub fn start_of_week(iso: &str, week_start: u32) -> String {
    add_days(iso, -(((weekday(iso) + 7 - week_start) % 7) as i32))
}

pub fn start_of_month(iso: &str) -> String {
    format!("{}01", &iso[..8])
}

pub fn end_of_month(iso: &str) -> String {
    let (y, m, _) = parts(iso);
    ymd(y, m, days_in_month(y, m))
}

/// Adds months, clamping the day (Jan 31 + 1 month = Feb 28/29).
pub fn add_months(iso: &str, n: i32) -> String {
    let (y, m, d) = parts(iso);
    let total = y * 12 + (m as i32 - 1) + n;
    let (ny, nm) = (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
    ymd(ny, nm, d.min(days_in_month(ny, nm)))
}

/// ISO 8601 week number.
pub fn iso_week(iso: &str) -> u32 {
    // The week belongs to the year containing its Thursday.
    let thursday = day_num(iso) - ((weekday(iso) as i32 + 6) % 7) + 3;
    let (year, _, _) = civil_from_days(thursday);
    ((thursday - days_from_civil(year, 1, 1)) / 7 + 1) as u32
}

/// Weeks shown for a month: from the week containing the 1st through the week containing the
/// last day (`six == false`: 4–6 rows) or always 6 rows.
pub fn month_weeks(year: i32, month: u32, week_start: u32, six: bool) -> Vec<String> {
    let first = ymd(year, month, 1);
    let last = ymd(year, month, days_in_month(year, month));
    let start = start_of_week(&first, week_start);
    let count = if six { 6 } else { diff_days(&start, &last) / 7 + 1 };
    (0..count).map(|i| add_days(&start, i * 7)).collect()
}

pub const WEEKDAY_SHORT: [&str; 7] = ["日", "一", "二", "三", "四", "五", "六"];

pub fn format_md(iso: &str) -> String {
    let (_, m, d) = parts(iso);
    trf("{0}月{1}日", &[format!("{}", m), format!("{}", d)])
}

pub fn format_ymd(iso: &str) -> String {
    let (y, m, d) = parts(iso);
    trf("{0}年{1}月{2}日", &[format!("{}", y), format!("{}", m), format!("{}", d)])
}

/// "10月30日 周五", with the year when it differs from `now`'s.
pub fn format_friendly(iso: &str, now: &str) -> String {
    let base = if parts(iso).0 == parts(now).0 { format_md(iso) } else { format_ymd(iso) };
    trf("{0} 周{1}", &[format!("{}", base), format!("{}", crate::i18n::tr(WEEKDAY_SHORT[weekday(iso) as usize]))])
}

pub fn format_range(start: &str, end: &str, now: &str) -> String {
    if start == end {
        return format_friendly(start, now);
    }
    format!("{} – {}", format_friendly(start, now), format_friendly(end, now))
}

pub fn relative_day(iso: &str, now: &str) -> String {
    match diff_days(now, iso) {
        0 => tr("今天").into(),
        1 => tr("明天").into(),
        2 => tr("后天").into(),
        -1 => tr("昨天").into(),
        -2 => tr("前天").into(),
        d if d > 0 => trf("{0} 天后", &[format!("{}", d)]),
        d => trf("{0} 天前", &[format!("{}", -d)]),
    }
}

/// Milliseconds since 1970 (UTC).
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// This PC's offset from UTC now, in minutes (China: 480).
pub fn utc_offset_minutes() -> i32 {
    let (l, u) = unsafe { (windows::Win32::System::SystemInformation::GetLocalTime(), windows::Win32::System::SystemInformation::GetSystemTime()) };
    let mins = |t: &windows::Win32::Foundation::SYSTEMTIME| {
        days_from_civil(t.wYear as i32, t.wMonth as u32, t.wDay as u32) as i64 * 1440 + t.wHour as i64 * 60 + t.wMinute as i64
    };
    let d = mins(&l) - mins(&u);
    ((d as f64 / 15.0).round() * 15.0) as i32
}

/// Epoch ms of a local date and time of day (minutes), at UTC offset `offset_min`.
pub fn local_ms(iso: &str, minute_of_day: u32, offset_min: i32) -> i64 {
    (day_num(iso) as i64 * 1440 + minute_of_day as i64 - offset_min as i64) * 60_000
}

/// Local (date, minute of day) of an epoch time, at UTC offset `offset_min`.
pub fn local_of_ms(ms: i64, offset_min: i32) -> (String, u32) {
    let m = (ms.div_euclid(60_000)) + offset_min as i64;
    (from_day_num(m.div_euclid(1440) as i32), m.rem_euclid(1440) as u32)
}

/// Local date and time now: (ISO date, minutes since midnight, seconds).
pub fn local_now() -> (String, u32, u32) {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    (ymd(t.wYear as i32, t.wMonth as u32, t.wDay as u32), t.wHour as u32 * 60 + t.wMinute as u32, t.wSecond as u32)
}

pub fn today() -> String {
    local_now().0
}
