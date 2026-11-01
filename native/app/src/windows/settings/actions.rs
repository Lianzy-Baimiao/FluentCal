//! What the settings pages' controls do (0.2's page handlers and the Tauri commands they
//! called: sync_*, backup_*, data_dir_set, file_save/file_open, notify_test, web_*…).

use super::view::{qr_image, Info, WebView};
use crate::app::{run, App};
use crate::Severity;
use fluentcal_core::cloud::secrets;
use fluentcal_core::settings_page as sp;
use fluentcal_core::tasks;
use serde_json::{json, Value};
use std::time::Duration;

/// Blocking work (network, files) off the UI thread; `done` gets its result on the UI thread.
/// Headless runs (tests: no message window, no event-loop thread) run it on the next turn.
pub fn job<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(&mut App, T) + 'static + Send) {
    if crate::platform::msgwin::hwnd().is_none() {
        slint::Timer::single_shot(Duration::ZERO, move || {
            let r = work();
            run(move |app| done(app, r));
        });
        return;
    }
    std::thread::spawn(move || {
        let r = work();
        crate::app::post(move |app| done(app, r));
    });
}

impl App {
    /// What the pages show from the app (view::Info).
    pub fn settings_info(&self) -> Info {
        // Screens and the holiday years are on 常规 only (the document snapshot isn't free).
        let general = self.settings_win.as_ref().is_some_and(|s| s.local.page == "general");
        let (screens, holiday_years) = if general {
            let mons = crate::platform::screens::monitors();
            let cur = crate::shell::current_screen(self);
            let screens = mons.iter().enumerate().map(|(i, m)| (i.to_string(), crate::platform::screens::title(m, i), cur == Some(i))).collect();
            let snap = self.store.snapshot();
            (screens, snap.get("holidays").and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default())
        } else {
            (Vec::new(), Vec::new())
        };
        Info {
            version: env!("CARGO_PKG_VERSION").into(),
            build: self.sys.build,
            debug: crate::platform::isolated(),
            transparency: Some(self.sys.transparency),
            system_accent: self.sys.accent.clone(),
            shortcut_errors: self.hotkey_errors.clone(),
            data_dir: self.data_dir.clone(),
            sync: self.sync.clone(),
            screens,
            holiday_years,
            now_ms: fluentcal_core::date::now_ms(),
            offset: fluentcal_core::date::utc_offset_minutes(),
        }
    }

    fn local(&mut self) -> Option<&mut super::view::Local> {
        self.settings_win.as_mut().map(|s| &mut s.local)
    }

    pub fn settings_load_backups(&mut self) {
        let list = self.store.backups_list();
        if let Some(l) = self.local() {
            l.backups = list;
        }
    }

    pub fn settings_load_secrets(&mut self) {
        let list = secrets::KEYS.iter().filter(|k| secrets::get(k).is_some()).map(|k| (*k).to_owned()).collect();
        if let Some(l) = self.local() {
            l.secrets = list;
        }
    }

    /// The web server's state, and the QR code of the first link (made again only when the
    /// link changes).
    pub fn settings_load_web(&mut self) {
        let info = self.web_info();
        let scale = self.settings_win.as_ref().map_or(1.0, |s| super::view::scale_of(&s.window, self));
        let Some(l) = self.local() else { return };
        let link = info.addresses.first().map(|a| a.3.clone());
        let qr = match (link, l.web.qr.take()) {
            (Some(url), Some((old, img))) if old == url => Some((old, img)),
            (Some(url), _) => {
                let img = qr_image(&url, (168.0 * scale).round() as u32);
                Some((url, img))
            }
            (None, _) => None,
        };
        l.web = WebView { enabled: info.enabled, running: info.running, port: info.port, error: info.error, addresses: info.addresses, qr };
    }

    /// A switch, select, slider or box changed `path` (0.2's useSetting).
    pub fn settings_set(&mut self, path: &str, value: Value) {
        let patch = match path {
            // 跟随 Windows 强调色: off = the system's colour as a fixed one
            "follow-accent" => {
                let on = value.as_bool().unwrap_or(true);
                let accent = if on { "system".to_owned() } else { self.sys.accent.clone().unwrap_or_else(|| "#0078D4".into()) };
                json!({ "theme": { "accent": accent } })
            }
            "calendar.weekStart" | "sync.interval" | "web.port" | "calendar.weeksRows" | "calendar.weeksPast" => {
                let n = value.as_f64().unwrap_or(0.0).round();
                sp::patch(path, json!(n as i64))
            }
            _ => sp::patch(path, value),
        };
        self.patch_settings(patch);
        if path.starts_with("web.") {
            // The server follows the settings a moment later (0.2: 400 ms).
            if let Some(l) = self.local() {
                l.web_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(400), || {
                    run(|app| {
                        app.settings_load_web();
                        app.settings_refresh();
                    })
                });
            }
        }
    }

    pub fn settings_rest_day(&mut self, day: u32, on: bool) {
        let days = sp::rest_days(&self.doc.settings.calendar.rest_days, day, on);
        self.patch_settings(json!({ "calendar": { "restDays": days } }));
    }

    /// The default reminders: set row `i` to `v` (minutes), "" removes it.
    pub fn settings_offset(&mut self, i: usize, v: &str) {
        let list = self.doc.settings.reminders.default_offsets.clone();
        let next = sp::set_offset(&list, i, v.parse::<i64>().ok());
        self.patch_settings(json!({ "reminders": { "defaultOffsets": next } }));
    }

    pub fn settings_group_name(&mut self, id: &str, name: &str) {
        let name = name.trim();
        let Some(g) = self.doc.groups.get(id).cloned() else { return };
        if name.is_empty() || name == g.name {
            return;
        }
        let edit = tasks::save_group(&self.doc, fluentcal_core::model::Group { name: name.into(), ..g });
        self.apply(edit, false);
    }

    pub fn settings_group_color(&mut self, id: &str, color: &str) {
        let Some(g) = self.doc.groups.get(id).cloned() else { return };
        let edit = tasks::save_group(&self.doc, fluentcal_core::model::Group { color: color.into(), ..g });
        self.apply(edit, false);
    }

    pub fn settings_add_group(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let colors = fluentcal_core::theme::TASK_COLORS;
        let g = tasks::make_group(&self.doc, name, colors[self.doc.groups.len() % colors.len()].key);
        let edit = tasks::save_group(&self.doc, g);
        self.apply(edit, false);
    }

    /// Password / token typed: into Windows Credential Manager (empty = delete), never shown.
    pub fn settings_save_secret(&mut self, key: &str, value: &str) {
        let value = value.trim();
        if value.is_empty() || !secrets::KEYS.contains(&key) {
            return;
        }
        match secrets::set(key, value) {
            Ok(()) => {
                self.cloud.secrets_changed();
                self.settings_load_secrets();
                self.settings_say(Severity::Success, "已保存到 Windows 凭据管理器");
            }
            Err(e) => self.settings_say(Severity::Error, &e),
        }
    }
}
