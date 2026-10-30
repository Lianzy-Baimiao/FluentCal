//! 0.2's src/lib/date.test.ts.
use fluentcal_core::date::*;

#[test]
fn converts_day_numbers_both_ways() {
    for d in ["1970-01-01", "2000-02-29", "2026-10-29", "2099-12-31", "1899-12-31"] {
        assert_eq!(from_day_num(day_num(d)), d);
    }
    assert_eq!(day_num("1970-01-02"), 1);
}

#[test]
fn adds_days_across_month_and_year_ends() {
    assert_eq!(add_days("2026-12-31", 1), "2027-01-01");
    assert_eq!(add_days("2024-03-01", -1), "2024-02-29");
    assert_eq!(diff_days("2026-10-01", "2026-11-01"), 31);
}

#[test]
fn knows_weekdays() {
    assert_eq!(weekday("2026-10-29"), 4); // Thursday
    assert_eq!(weekday("1970-01-01"), 4);
    assert_eq!(start_of_week("2026-10-29", 1), "2026-10-26");
    assert_eq!(start_of_week("2026-10-29", 0), "2026-10-25");
    assert_eq!(start_of_week("2026-10-25", 1), "2026-10-19"); // Sunday belongs to the previous Monday week
}

#[test]
fn adds_months_with_day_clamping() {
    assert_eq!(add_months("2026-01-31", 1), "2026-02-28");
    assert_eq!(add_months("2024-01-31", 1), "2024-02-29");
    assert_eq!(add_months("2026-03-15", -3), "2025-12-15");
    assert_eq!(add_months("2026-11-30", 14), "2028-01-30");
}

#[test]
fn computes_iso_week_numbers() {
    assert_eq!(iso_week("2026-01-01"), 1);
    assert_eq!(iso_week("2026-12-31"), 53);
    assert_eq!(iso_week("2027-01-01"), 53);
    assert_eq!(iso_week("2027-01-04"), 1);
    assert_eq!(iso_week("2021-01-03"), 53);
}

#[test]
fn builds_month_week_rows() {
    let auto = month_weeks(2026, 10, 1, false);
    assert_eq!(auto[0], "2026-09-28");
    assert_eq!(auto.len(), 5);
    assert_eq!(month_weeks(2026, 10, 1, true).len(), 6);
    assert_eq!(month_weeks(2026, 2, 0, false).len(), 4); // Feb 2026 starts on Sunday, 28 days
}

#[test]
fn validates_iso_dates() {
    assert!(!is_iso_date("2026-02-29"));
    assert!(is_iso_date("2024-02-29"));
    assert!(!is_iso_date("2026-13-01"));
    assert!(!is_iso_date("20261001"));
    assert!(!is_iso_date("2026-1a-01"));
}

#[test]
fn describes_relative_days() {
    assert_eq!(relative_day("2026-10-30", "2026-10-29"), "明天");
    assert_eq!(relative_day("2026-10-29", "2026-10-29"), "今天");
    assert_eq!(relative_day("2026-11-05", "2026-10-29"), "7 天后");
    assert_eq!(format_friendly("2026-10-30", "2026-10-29"), "10月30日 周五");
    assert_eq!(format_friendly("2027-01-01", "2026-10-29"), "2027年1月1日 周五");
}
