//! System theme, accent colour and Windows version (0.2's desktop.rs system_info).

use windows::core::{w, PCWSTR};
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SystemInfo {
    pub build: u32,
    pub dark: bool,
    pub transparency: bool,
    pub accent: Option<String>,
    /// light3, light2, light1, accent, dark1, dark2, dark3 (empty when Windows didn't say)
    pub accent_palette: Vec<String>,
}

const PERSONALIZE: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

pub fn system_dark() -> bool {
    reg_dword(PERSONALIZE, w!("AppsUseLightTheme")).is_some_and(|v| v == 0)
}

pub fn system_info() -> SystemInfo {
    let mut transparency = reg_dword(PERSONALIZE, w!("EnableTransparency")).is_none_or(|v| v != 0);
    let mut palette = Vec::with_capacity(7);
    if let Ok(ui) = UISettings::new() {
        for t in [
            UIColorType::AccentLight3,
            UIColorType::AccentLight2,
            UIColorType::AccentLight1,
            UIColorType::Accent,
            UIColorType::AccentDark1,
            UIColorType::AccentDark2,
            UIColorType::AccentDark3,
        ] {
            if let Ok(c) = ui.GetColorValue(t) {
                palette.push(format!("#{:02X}{:02X}{:02X}", c.R, c.G, c.B));
            }
        }
        if let Ok(fx) = ui.AdvancedEffectsEnabled() {
            transparency = fx;
        }
    }
    if palette.len() != 7 {
        palette.clear();
    }
    let accent = palette.get(3).cloned().or_else(|| {
        // DWM stores the colour as 0xAABBGGRR
        reg_dword(w!(r"Software\Microsoft\Windows\DWM"), w!("AccentColor"))
            .map(|v| format!("#{:02X}{:02X}{:02X}", v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF))
    });
    SystemInfo { build: windows_version::OsVersion::current().build, dark: system_dark(), transparency, accent, accent_palette: palette }
}

pub fn reg_dword(key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, key, value, RRF_RT_REG_DWORD, None, Some(&mut data as *mut _ as *mut _), Some(&mut size))
    };
    r.is_ok().then_some(data)
}
