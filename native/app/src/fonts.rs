//! Fonts: 0.2's CSS stack is "Segoe UI Variable Text", then "Microsoft YaHei UI" for Chinese.
//! Slint takes one family per text; the CJK fallback is set on the shared font collection so
//! Chinese text gets YaHei UI like in WebView2 (DirectWrite would pick it too, but asking
//! keeps it independent of the system's fallback table).

use slint::fontique_011::fontique::{FallbackKey, Language, Script};

/// Call once, after the Slint backend is selected and before the first window is shown.
pub fn setup() {
    let mut fonts = slint::fontique_011::shared_collection();
    let Some(yahei) = fonts.family_id("Microsoft YaHei UI").or_else(|| fonts.family_id("Microsoft YaHei")) else {
        return;
    };
    let zh = Language::parse("zh-CN").ok();
    for script in [*b"Hani", *b"Bopo"] {
        let script = Script::from_bytes(script);
        fonts.set_fallbacks(FallbackKey::new(script, None), std::iter::once(yahei));
        if let Some(zh) = &zh {
            fonts.set_fallbacks(FallbackKey::new(script, Some(zh)), std::iter::once(yahei));
        }
    }
}

/// Windows 11 (build 22000+) has Segoe UI Variable and Segoe Fluent Icons.
pub fn is_win11() -> bool {
    let mut fonts = slint::fontique_011::shared_collection();
    fonts.family_id("Segoe Fluent Icons").is_some()
}
