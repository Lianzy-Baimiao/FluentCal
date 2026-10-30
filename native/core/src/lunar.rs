//! Lunar calendar, solar terms, festivals and statutory holidays (0.2's src/lib/lunar.ts),
//! with tyme4rs (the Rust port of tyme4ts). Online holiday data (holiday-cn) replaces the
//! built-in schedule for the years it covers.

use crate::date::{parts, weekday, ymd};
use crate::model::HolidayYear;
use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex};
use tyme4rs::tyme::holiday::LegalHoliday;
use tyme4rs::tyme::lunar::{LunarDay, LunarMonth};
use tyme4rs::tyme::solar::SolarDay;
use tyme4rs::tyme::Culture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Lunar,
    Month,
    Term,
    Festival,
}

/// Statutory holiday: off day or adjusted working day (调休).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Legal {
    pub name: String,
    pub work: bool,
}

/// What a day cell shows under the date number, plus tooltip details.
#[derive(Debug, Clone, PartialEq)]
pub struct DayInfo {
    /// Short label: festival, solar term, lunar month (on day 1) or lunar day.
    pub text: String,
    pub kind: Kind,
    pub lunar_year: i32,
    /// Negative = leap month (tyme convention).
    pub lunar_month: i32,
    pub lunar_day: u32,
    /// e.g. "九月二十"
    pub lunar_label: String,
    /// e.g. "丙午年"
    pub ganzhi_year: String,
    pub festivals: Vec<String>,
    pub term: Option<String>,
    pub legal: Option<Legal>,
}

const FESTIVAL_RENAME: [(&str, &str); 3] = [("龙头节", "龙抬头"), ("冬至节", "冬至"), ("清明节", "清明")];
const MONTH_RENAME: [(&str, &str); 2] = [("十一月", "冬月"), ("十二月", "腊月")];
/// Fixed-date observances tyme doesn't list.
const EXTRA_FIXED: [(&str, &str); 4] = [("02-14", "情人节"), ("04-01", "愚人节"), ("12-24", "平安夜"), ("12-25", "圣诞节")];

fn rename(table: &[(&str, &str)], s: String) -> String {
    table.iter().find(|(a, _)| *a == s).map_or(s, |(_, b)| (*b).to_owned())
}

/// Second Sunday of May / third Sunday of June.
fn extra_movable(y: i32, m: u32, d: u32) -> Option<&'static str> {
    if m != 5 && m != 6 {
        return None;
    }
    let first = weekday(&ymd(y, m, 1));
    let first_sunday = 1 + (7 - first) % 7;
    match (m, d) {
        (5, d) if d == first_sunday + 7 => Some("母亲节"),
        (6, d) if d == first_sunday + 14 => Some("父亲节"),
        _ => None,
    }
}

#[derive(Default)]
struct State {
    overrides: BTreeMap<String, HolidayYear>,
    /// Every date listed by any online year file (a year's file can list days of the
    /// previous year, e.g. 2019.json has the 2018-12-29 调休 day).
    override_days: HashMap<String, (String, bool)>,
    info: HashMap<String, DayInfo>,
    legal: HashMap<String, Option<Legal>>,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));

/// Online holiday data (holiday-cn) replaces the built-in schedule for the years it covers.
pub fn set_holiday_overrides(h: BTreeMap<String, HolidayYear>) {
    let mut st = STATE.lock().unwrap();
    st.override_days.clear();
    for y in h.values() {
        for d in &y.days {
            st.override_days.insert(d.date.clone(), (d.name.clone(), d.is_off_day));
        }
    }
    st.overrides = h;
    st.info.clear();
    st.legal.clear();
}

/// Online data is authoritative for a date when its year was fetched; days near New Year
/// also need the neighbouring year's file, which may list them.
fn online_covers(st: &State, y: i32, m: u32, d: u32) -> bool {
    let has = |yy: i32| st.overrides.get(&yy.to_string()).is_some_and(|x| !x.days.is_empty());
    if !has(y) {
        return false;
    }
    // A year's file may also list the last days of the previous year, so late December is
    // only covered once next year's file is in; otherwise the built-in data stays in charge.
    !(m == 12 && d >= 20) || has(y + 1)
}

fn legal_in(st: &mut State, iso: &str) -> Option<Legal> {
    if let Some(hit) = st.legal.get(iso) {
        return hit.clone();
    }
    let (y, m, d) = parts(iso);
    let res = if let Some((name, off)) = st.override_days.get(iso) {
        Some(Legal { name: name.clone(), work: !off })
    } else if online_covers(st, y, m, d) {
        None // an ordinary day per the official schedule
    } else {
        LegalHoliday::from_ymd(y as isize, m as usize, d as usize).map(|lh| Legal { name: lh.get_name(), work: lh.is_work() })
    };
    st.legal.insert(iso.to_owned(), res.clone());
    res
}

pub fn legal_holiday(iso: &str) -> Option<Legal> {
    legal_in(&mut STATE.lock().unwrap(), iso)
}

/// A rest day: statutory holiday, or a configured weekend day not moved to work.
pub fn is_rest_day(iso: &str, rest_days: &[u32]) -> bool {
    match legal_holiday(iso) {
        Some(l) => !l.work,
        None => rest_days.contains(&weekday(iso)),
    }
}

/// Working day per the official schedule (weekends + holidays off, 调休 days on).
pub fn is_legal_workday(iso: &str) -> bool {
    !is_rest_day(iso, &[0, 6])
}

pub fn day_info(iso: &str) -> DayInfo {
    let mut st = STATE.lock().unwrap();
    if let Some(hit) = st.info.get(iso) {
        return hit.clone();
    }
    let (y, m, d) = parts(iso);
    let sd = SolarDay::from_ymd(y as isize, m as usize, d as usize);
    let ld = sd.get_lunar_day();
    let lm = ld.get_lunar_month();
    let month_name = rename(&MONTH_RENAME, lm.get_name());

    let mut festivals: Vec<String> = Vec::new();
    if let Some(f) = ld.get_festival() {
        festivals.push(rename(&FESTIVAL_RENAME, f.get_name()));
    }
    if let Some(f) = sd.get_festival() {
        festivals.push(f.get_name());
    }
    let extra = EXTRA_FIXED.iter().find(|(md, _)| *md == &iso[5..]).map(|(_, n)| *n).or_else(|| extra_movable(y, m, d));
    if let Some(e) = extra {
        festivals.push(e.to_owned());
    }
    let mut seen = Vec::new();
    festivals.retain(|f| if seen.contains(f) { false } else { seen.push(f.clone()); true });

    let td = sd.get_term_day();
    let term = (td.get_day_index() == 0).then(|| td.get_solar_term().get_name());

    let lunar_day = ld.get_day() as u32;
    let lunar_name = ld.get_name();
    // A festival on the same day as its solar term (清明, 冬至) shows once.
    let shown = festivals.iter().find(|f| Some(*f) != term.as_ref()).cloned();
    let (text, kind) = match (shown, &term) {
        (Some(f), _) => (f, Kind::Festival),
        (None, Some(t)) => (t.clone(), Kind::Term),
        (None, None) if lunar_day == 1 => (month_name.clone(), Kind::Month),
        (None, None) => (lunar_name.clone(), Kind::Lunar),
    };
    let legal = legal_in(&mut st, iso);
    let info = DayInfo {
        text,
        kind,
        lunar_year: lm.get_year() as i32,
        lunar_month: lm.get_month_with_leap() as i32,
        lunar_day,
        lunar_label: format!("{month_name}{lunar_name}"),
        // The year's 干支 changes at 立春 (what tyme's deprecated get_year_sixty_cycle returned).
        ganzhi_year: format!("{}年", ld.get_sixty_cycle_day().get_year().get_name()),
        festivals,
        term,
        legal,
    };
    if st.info.len() > 20_000 {
        st.info.clear(); // the calendar was paged through for years
    }
    st.info.insert(iso.to_owned(), info.clone());
    info
}

/// 宜/忌 for the day (only shown on hover).
pub fn almanac(iso: &str) -> (Vec<String>, Vec<String>) {
    let (y, m, d) = parts(iso);
    let ld = SolarDay::from_ymd(y as isize, m as usize, d as usize).get_lunar_day();
    (ld.get_recommends().iter().map(|t| t.get_name()).collect(), ld.get_avoids().iter().map(|t| t.get_name()).collect())
}

/// Solar date of a lunar date in a given lunar year. A leap month falls back to the regular
/// month when that year has no such leap month; day 30 falls back to 29 in short months.
pub fn lunar_to_solar(lunar_year: i32, lunar_month: i32, lunar_day: u32) -> Option<String> {
    let mut tries = vec![(lunar_month, lunar_day)];
    if lunar_month < 0 {
        tries.push((-lunar_month, lunar_day));
    }
    let short: Vec<(i32, u32)> = tries.iter().map(|(mm, _)| (*mm, lunar_day.min(29))).collect();
    for (mm, dd) in tries.into_iter().chain(short) {
        // LunarDay::new unwraps LunarMonth::from_ym: check the month first.
        let Ok(month) = LunarMonth::new(lunar_year as isize, mm as isize) else { continue };
        if dd < 1 || dd as usize > month.get_day_count() {
            continue;
        }
        if let Ok(ld) = LunarDay::new(lunar_year as isize, mm as isize, dd as usize) {
            let s = ld.get_solar_day();
            return Some(ymd(s.get_year() as i32, s.get_month() as u32, s.get_day() as u32));
        }
    }
    None
}

/// Chinese numerals for lunar months, used in rule descriptions.
pub fn lunar_month_name(m: i32) -> String {
    const NAMES: [&str; 12] = ["正", "二", "三", "四", "五", "六", "七", "八", "九", "十", "冬", "腊"];
    let i = (m.unsigned_abs() as usize).clamp(1, 12) - 1;
    format!("{}{}月", if m < 0 { "闰" } else { "" }, NAMES[i])
}

pub fn lunar_day_name(d: u32) -> String {
    const TEN: [&str; 4] = ["初", "十", "廿", "三"];
    const DIGITS: [&str; 11] = ["", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];
    match d {
        10 => "初十".into(),
        20 => "二十".into(),
        30 => "三十".into(),
        _ => format!("{}{}", TEN[(d / 10).min(3) as usize], DIGITS[(d % 10) as usize]),
    }
}
