// The settings window's helpers (0.2's src/windows/settings scripts).

use fluentcal_core::settings::Settings;
use fluentcal_core::settings_page::*;
use serde_json::json;

// 2026-10-14 10:30 in Beijing (UTC+8), the parity scenes' "now".
const NOW: i64 = 1_791_945_000_000;

#[test]
fn dates_as_edge_formats_them() {
    // Values from Edge's toLocaleString('zh-CN', …) (Asia/Shanghai).
    assert_eq!(zh_datetime(NOW, 480), "2026/10/14 10:30:00");
    assert_eq!(zh_datetime(1_767_456_309_000, 480), "2026/1/4 00:05:09");
    assert_eq!(zh_reminder_time(NOW, 480), "10/14周三 10:30");
    assert_eq!(zh_reminder_time(NOW + 22 * 3_600_000 + 30 * 60_000, 480), "10/15周四 09:00");
    assert_eq!(kb(100), "1 KB");
    assert_eq!(kb(10_752), "11 KB");
}

#[test]
fn patch_from_path() {
    assert_eq!(patch("desk.locked", json!(true)), json!({ "desk": { "locked": true } }));
    assert_eq!(patch("sync.webdav.url", json!("x")), json!({ "sync": { "webdav": { "url": "x" } } }));
    assert_eq!(num(8.0), json!(8));
    assert_eq!(num(1.5), json!(1.5));
}

#[test]
fn presets_match_and_apply() {
    let mut s = Settings::default();
    assert_eq!(match_preset(&s), "", "the defaults (72/55) are no preset");
    s.desk.bg_opacity = 18.0;
    s.desk.cell_opacity = 38.0;
    assert_eq!(match_preset(&s), "glass");
    let p = preset_patch("light").unwrap();
    assert_eq!(p["theme"]["mode"], "light");
    assert_eq!(p["desk"]["bgOpacity"], json!(92));
    assert_eq!(preset_patch("minimal").unwrap()["desk"]["textShadow"], "on");
    assert!(preset_patch("acrylic").unwrap().get("theme").is_none());
}

#[test]
fn rest_days_and_offsets() {
    assert_eq!(rest_days(&[0, 6], 5, true), vec![0, 5, 6]);
    assert_eq!(rest_days(&[0, 6], 0, false), vec![6]);
    assert_eq!(add_offset(&[]), vec![0]);
    assert_eq!(add_offset(&[0, 5]), vec![0, 5, 10]);
    assert_eq!(set_offset(&[0, 10], 1, Some(0)), vec![0], "duplicates collapse");
    assert_eq!(set_offset(&[30, 60], 0, Some(1440)), vec![60, 1440]);
    assert_eq!(set_offset(&[30, 60], 0, None), vec![60]);
    let opts = offset_options(&[90]);
    assert_eq!(opts.len(), 13);
    assert_eq!(opts.last().unwrap().1, "提前 90 分钟");
}

#[test]
fn texts() {
    assert_eq!(proxy_mode(""), "system");
    assert_eq!(proxy_mode("http://127.0.0.1:7890"), "custom");
    assert_eq!(proxy_for_mode("direct"), "direct");
    assert!(pin_hint("top").starts_with("始终显示"));
    assert_eq!(holiday_desc(0, &[], 480), "联网获取国务院公布的放假和调休安排。上次检查：从未；在线数据：无（使用内置数据）");
    assert!(holiday_desc(NOW, &["2027".into(), "2026".into()], 480).ends_with("2026/10/14 10:30:00；在线数据：2026、2027"));
    assert_eq!(holiday_result(&[2026, 2027], false), "已更新 2026、2027 年放假安排。");
    assert_eq!(holiday_result(&[], false), "已是最新，暂无新的放假安排。");
}
