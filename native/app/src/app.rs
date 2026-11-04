//! The running program: data, theme and windows, owned by the UI thread. Background threads
//! (cloud sync, reminders, holiday updates, the web server) talk to it with `post`.

use crate::desk::live::DeskLive;
use crate::platform::sysinfo::SystemInfo;
use crate::reminders::Reminders;
use fluentcal_core::cloud::service::{Cloud, Status};
use fluentcal_core::datadir::DataDir;
use fluentcal_core::history::History;
use fluentcal_core::settings::Settings;
use fluentcal_core::store::{Store, StoreEvent};
use fluentcal_core::tasks::{Doc, Edit};
use fluentcal_core::theme::{desk_style, tokens, DeskStyle, Tokens};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

/// Origin of the writes made here; their store events are skipped (already applied).
pub const ORIGIN: &str = "native";

pub struct App {
    pub store: Arc<Store>,
    pub cloud: Arc<Cloud>,
    pub sync: Status,
    pub doc: Doc,
    pub sys: SystemInfo,
    pub tokens: Tokens,
    pub style: DeskStyle,
    pub desk: DeskLive,
    /// The desk window's undo history (0.2: each window had its own).
    pub history: History,
    pub reminders: Arc<Reminders>,
    /// Per-PC folder (app state, cloud state); the data may live elsewhere (`data_dir`).
    pub local_dir: PathBuf,
    pub data_dir: DataDir,
    pub save_error: Option<String>,
    pub hotkey_errors: Vec<String>,
    pub timers: Vec<slint::Timer>,
    pub reminder_timer: slint::Timer,
    pub theme_timer: slint::Timer,
    pub last_monitor: Option<usize>,
    /// The date the clock last saw (a new day moves the calendar along).
    pub day: String,
    pub web: crate::webcmd::WebState,
    pub reminder_win: Option<crate::ReminderWindow>,
    pub editor: Option<crate::windows::editor::EditorWin>,
    pub settings_win: Option<crate::windows::settings::SettingsWin>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

pub fn install(app: App) {
    APP.with(|a| *a.borrow_mut() = Some(app));
}

pub fn uninstall() -> Option<App> {
    APP.with(|a| a.borrow_mut().take())
}

/// Runs `f` with the app on the UI thread. Called while the app is busy (a callback fired
/// from inside another call), it runs on the next event-loop turn instead.
pub fn run(f: impl FnOnce(&mut App) + 'static) {
    let f = APP.with(|a| match a.try_borrow_mut() {
        Ok(mut g) => {
            if let Some(app) = g.as_mut() {
                f(app);
            }
            None
        }
        Err(_) => Some(f),
    });
    if let Some(f) = f {
        slint::Timer::single_shot(std::time::Duration::ZERO, move || run(f));
    }
}

/// Like `run`, returning a value; None when the app is busy or gone.
pub fn with<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok()?.as_mut().map(f))
}

/// From another thread: runs `f` on the UI thread.
pub fn post(f: impl FnOnce(&mut App) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || run(f));
}

impl App {
    pub fn settings(&self) -> &Settings {
        &self.doc.settings
    }

    /// A change from elsewhere (cloud sync, holiday update, import): ours are applied already.
    pub fn on_store_event(&mut self, e: StoreEvent) {
        match &e {
            StoreEvent::Changed { origin, collection, .. } if origin != ORIGIN => {
                if collection == "holidays" {
                    self.load_holidays();
                }
                self.doc.apply(&e);
                self.doc_changed();
            }
            StoreEvent::Settings { origin, old, new } if origin != ORIGIN => {
                let old = Settings::from_value(old);
                self.doc.settings = Settings::from_value(new);
                self.settings_changed(&old);
            }
            StoreEvent::Reload { .. } => self.reload(),
            StoreEvent::SaveStatus(err) => {
                self.save_error = err.clone();
                self.desk_refresh();
            }
            _ => {}
        }
    }

    pub fn reload(&mut self) {
        let old = self.doc.settings.clone();
        self.doc = Doc::from_value(&self.store.snapshot());
        self.history.clear();
        self.load_holidays();
        self.settings_changed(&old);
        self.doc_changed();
    }

    pub fn load_holidays(&self) {
        fluentcal_core::lunar::set_holiday_overrides(fluentcal_core::model::holidays(&self.store.snapshot()));
    }

    /// Writes an edit (recorded for the desk's undo when `undoable`) and updates the windows.
    pub fn apply(&mut self, edit: Edit, undoable: bool) {
        if edit.is_empty() {
            return;
        }
        if undoable {
            let doc = &self.doc;
            self.history.record(&edit, |c, id| doc.raw(c, id), fluentcal_core::date::now_ms());
        }
        if let Err(e) = edit.write(&self.store, ORIGIN) {
            eprintln!("[app] write failed: {e}");
        }
        for (collection, id, v) in edit.items {
            let items = serde_json::Map::from_iter([(id, v.unwrap_or(serde_json::Value::Null))]);
            self.doc.apply(&StoreEvent::Changed { origin: ORIGIN.into(), collection, items });
        }
        self.doc_changed();
    }

    /// Tasks, groups or day marks changed: every view that shows them.
    pub fn doc_changed(&mut self) {
        self.desk_refresh_data();
        self.editor_refresh();
        crate::shell::tray_refresh(self);
        self.schedule_reminders();
        self.settings_refresh();
    }

    pub fn patch_settings(&mut self, patch: serde_json::Value) {
        let old = self.doc.settings.clone();
        if let Err(e) = self.store.update_settings(ORIGIN, &patch) {
            eprintln!("[app] settings: {e}");
            return;
        }
        self.doc.settings = Settings::from_value(&self.store.setting("").unwrap_or_default());
        self.settings_changed(&old);
    }

    pub fn settings_changed(&mut self, old: &Settings) {
        if old.general.language != self.doc.settings.general.language {
            self.refresh_language();
        }
        crate::shell::apply_settings(self, old);
        self.editor_refresh();
        self.settings_refresh();
    }

    /// Theme mode, accent, appearance or the system theme changed.
    pub fn recompute_theme(&mut self) {
        let t = &self.doc.settings.theme;
        let dark = match t.mode.as_str() {
            "dark" => true,
            "light" => false,
            _ => self.sys.dark,
        };
        let system = t.accent == "system";
        let accent = if system { self.sys.accent.clone().unwrap_or_else(|| "#0078D4".into()) } else { t.accent.clone() };
        let palette = system.then_some(self.sys.accent_palette.as_slice());
        self.tokens = tokens(dark, &accent, palette);
        self.style = desk_style(&self.doc.settings.desk, &self.tokens);
        self.desk_apply_theme();
        crate::windows::retheme(self);
    }
}
