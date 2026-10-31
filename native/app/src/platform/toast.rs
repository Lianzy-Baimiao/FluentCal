//! Windows notifications (toasts) for reminders. Toasts need an AppUserModelID: the installed
//! program's is "com.fluentcal.desktop" (its Start menu shortcut carries it, as with 0.2);
//! debug builds register their own under HKCU so their toasts show a proper name.

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::System::Registry::*;

pub fn app_id() -> &'static str {
    if cfg!(debug_assertions) { "com.fluentcal.native.dev" } else { "com.fluentcal.desktop" }
}

/// Display name and icon for the AppUserModelID (HKCU\Software\Classes\AppUserModelId\<id>).
pub fn register(icon_png: Option<&std::path::Path>) {
    let key = HSTRING::from(format!(r"Software\Classes\AppUserModelId\{}", app_id()));
    let name = if cfg!(debug_assertions) { "FluentCal 桌面日历（开发版）" } else { "FluentCal 桌面日历" };
    let set = |value: PCWSTR, text: &str| unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = RegSetKeyValueW(HKEY_CURRENT_USER, &key, value, REG_SZ.0, Some(wide.as_ptr() as *const _), (wide.len() * 2) as u32);
    };
    set(w!("DisplayName"), name);
    if let Some(p) = icon_png {
        set(w!("IconUri"), &p.display().to_string());
    }
}

pub fn show(title: &str, body: &str) -> Result<(), String> {
    tauri_winrt_notification::Toast::new(app_id()).title(title).text1(body).show().map_err(|e| e.to_string())
}
