//! 0.2's src/lib/lunar.test.ts. Holiday overrides are global: the tests take a lock.
use fluentcal_core::lunar::*;
use fluentcal_core::model::{HolidayDay, HolidayYear};
use std::collections::BTreeMap;
use std::sync::Mutex;

static SERIAL: Mutex<()> = Mutex::new(());

fn year(y: i32, days: &[(&str, &str, bool)]) -> HolidayYear {
    HolidayYear { year: y, fetched_at: 0, days: days.iter().map(|(d, n, off)| HolidayDay { date: (*d).into(), name: (*n).into(), is_off_day: *off }).collect() }
}

#[test]
fn labels_festivals_solar_terms_and_lunar_days() {
    let _g = SERIAL.lock().unwrap();
    set_holiday_overrides(BTreeMap::new());
    assert_eq!(day_info("2026-02-17").text, "春节");
    let qm = day_info("2026-04-05");
    assert_eq!((qm.text.as_str(), qm.kind), ("清明", Kind::Term));
    let d = day_info("2026-10-29");
    assert_eq!(d.lunar_label, "九月二十");
    assert_eq!(d.text, "二十");
    assert_eq!(d.ganzhi_year, "丙午年");
    assert!(day_info("2026-10-01").festivals.contains(&"国庆节".to_owned()));
    assert!(day_info("2026-05-10").festivals.contains(&"母亲节".to_owned()));
    assert!(day_info("2026-06-21").festivals.contains(&"父亲节".to_owned()));
}

#[test]
fn marks_statutory_holidays_and_adjusted_working_days() {
    let _g = SERIAL.lock().unwrap();
    set_holiday_overrides(BTreeMap::new());
    assert_eq!(day_info("2026-10-01").legal, Some(Legal { name: "国庆节".into(), work: false }));
    assert_eq!(day_info("2026-10-10").legal, Some(Legal { name: "国庆节".into(), work: true }));
    assert!(is_legal_workday("2026-10-10")); // Saturday, 调休
    assert!(!is_legal_workday("2026-10-01"));
    assert!(is_legal_workday("2026-10-12"));
    assert!(is_rest_day("2026-10-31", &[0, 6])); // ordinary Saturday
}

#[test]
fn online_data_replaces_years_and_year_end_needs_next_year() {
    let _g = SERIAL.lock().unwrap();
    set_holiday_overrides(BTreeMap::from([("2026".into(), year(2026, &[("2026-03-02", "测试假", true)]))]));
    assert!(is_rest_day("2026-03-02", &[0, 6]));
    assert_eq!(legal_holiday("2026-10-01"), None);
    assert_eq!(legal_holiday("2025-10-01").map(|l| l.name), Some("国庆中秋".into())); // other years keep the built-in data

    let y2018 = year(2018, &[("2018-02-16", "春节", true)]);
    // holiday-cn's 2019.json lists the 2018 year-end days
    let y2019 = year(2019, &[("2018-12-29", "元旦", false), ("2018-12-31", "元旦", true), ("2019-01-01", "元旦", true)]);
    set_holiday_overrides(BTreeMap::from([("2018".into(), y2018.clone()), ("2019".into(), y2019)]));
    assert_eq!(legal_holiday("2018-12-29"), Some(Legal { name: "元旦".into(), work: true }));
    assert_eq!(legal_holiday("2018-12-31"), Some(Legal { name: "元旦".into(), work: false }));
    assert_eq!(legal_holiday("2018-06-01"), None); // ordinary day per the 2018 file
    set_holiday_overrides(BTreeMap::from([("2018".into(), y2018)])); // 2019 not fetched: year end falls back to built-in data
    assert_eq!(legal_holiday("2018-12-31").map(|l| l.work), Some(false));
    set_holiday_overrides(BTreeMap::new());
}

#[test]
fn converts_lunar_dates_to_solar_dates() {
    assert_eq!(lunar_to_solar(2027, 1, 1).as_deref(), Some("2027-02-06"));
    assert_eq!(lunar_to_solar(2025, -6, 1).as_deref(), Some("2025-07-25"));
    assert_eq!(lunar_to_solar(2026, -6, 1), lunar_to_solar(2026, 6, 1)); // no leap month that year
    assert!(lunar_to_solar(2026, 13, 1).is_none());
}

#[test]
fn names_lunar_months_and_days() {
    assert_eq!(lunar_month_name(1), "正月");
    assert_eq!(lunar_month_name(-6), "闰六月");
    assert_eq!(lunar_month_name(12), "腊月");
    assert_eq!(lunar_day_name(1), "初一");
    assert_eq!(lunar_day_name(15), "十五");
    assert_eq!(lunar_day_name(21), "廿一");
    assert_eq!(lunar_day_name(30), "三十");
}
