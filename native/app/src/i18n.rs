//! Windows display-language discovery and live refresh of the native UI.
use fluentcal_core::i18n;

pub fn system_language() -> String {
    use windows::Win32::Globalization::{GetUserDefaultUILanguage, LCIDToLocaleName};
    let mut name = [0u16; 85];
    let n = unsafe { LCIDToLocaleName(u32::from(GetUserDefaultUILanguage()), Some(&mut name), 0) };
    if n > 1 { String::from_utf16_lossy(&name[..n as usize - 1]) } else { "en-US".into() }
}

pub fn select(preference: &str) -> bool {
    let language = i18n::resolve(preference, &system_language());
    let changed = i18n::set_language(language);
    // Call only after the first component exists.
    if let Err(e) = slint::select_bundled_translation(language.tag()) { eprintln!("[language] {e}"); }
    changed
}

impl crate::app::App {
    pub fn refresh_language(&mut self) {
        if i18n::resolve(&self.doc.settings.general.language, &system_language()) == i18n::current() { return; }
        if !select(&self.doc.settings.general.language) { return; }
        // Rebuild generated labels while preserving draft values and the selected dates.
        self.desk_refresh();
        if let Some(editor) = &self.editor { crate::windows::editor::host::setup(&editor.window); }
        if let Some(settings) = &self.settings_win { crate::windows::settings::view::refresh_labels(&settings.window); }
        self.editor_refresh();
        self.settings_refresh();
        crate::shell::tray_refresh(self);
        self.schedule_reminders();
    }
}
