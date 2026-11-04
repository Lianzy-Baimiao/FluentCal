use fluentcal_core::{i18n::*, settings::Settings};
use serde_json::json;

#[test]
fn locale_resolution_and_existing_settings() {
    use Language::*;
    for (tag, expected) in [("zh-CN", SimplifiedChinese), ("zh-SG", SimplifiedChinese), ("zh-Hans-HK", SimplifiedChinese), ("zh-TW", TraditionalChinese), ("zh_HK", TraditionalChinese), ("zh-MO", TraditionalChinese), ("zh-Hant-CN", TraditionalChinese), ("en-GB", English), ("ja-JP", English), ("", English)] {
        assert_eq!(resolve("system", tag), expected, "{tag}");
    }
    assert_eq!(resolve("zh-TW", "en-US"), TraditionalChinese);
    assert_eq!(Settings::from_value(&json!({})).general.language, "system");
    assert_eq!(Settings::from_value(&json!({"general":{"language":"broken"}})).general.language, "system");
    assert_eq!(Settings::from_value(&json!({"general":{"language":"en-US"}})).general.language, "en-US");
    assert_eq!(format_message("{1}: {0} / {1}", &["keep {1} literally".into(), "例子".into()]), "例子: keep {1} literally / 例子");
    assert_eq!(translate(English, "任务名称"), "Task name");
    assert_eq!(translate(TraditionalChinese, "设置"), "設定");
    assert_eq!(translate(English, "Unlisted text"), "Unlisted text");
}

#[test]
fn language_changes_only_presentation_not_calendar_rules() {
    use fluentcal_core::{date, lunar, countdown};
    let canonical = lunar::day_info("2026-11-03");
    let holiday = countdown::next_holiday("2026-11-03");
    set_language(Language::English);
    assert_eq!(date::format_friendly("2026-11-03", "2026-11-03"), "11/3 Tue");
    assert_eq!(fluentcal_core::reminders::offset_label(5), "5 minutes before");
    assert_eq!(lunar::display_info("2026-11-03").lunar_label, "M9 D25");
    assert_eq!(lunar::day_info("2026-11-03"), canonical);
    assert_eq!(countdown::next_holiday("2026-11-03"), holiday);
    set_language(Language::SimplifiedChinese);
}
