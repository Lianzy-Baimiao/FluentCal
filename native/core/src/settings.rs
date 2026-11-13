//! Settings as in 0.2's src/types.ts with the defaults of src/defaults.ts. Stored settings are
//! partial (only what the user changed); reading fills in the rest. Changes are written as
//! patches with Store::update_settings.

use serde::{Deserialize, Serialize};
use serde_json::Value;

macro_rules! section {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty = $d:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", default)]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
        impl Default for $name {
            fn default() -> Self { Self { $($f: $d),* } }
        }
    };
}

section!(General {
    /// Local display language: system | zh-CN | zh-TW | en-US. Never synced.
    language: String = "system".into(),
    autostart: bool = true,
    seeded: bool = false,
    /// The first-run welcome bar was dismissed (or the samples cleared).
    onboarded: bool = false,
});

section!(Theme {
    /// "system" | "light" | "dark"
    mode: String = "system".into(),
    /// "system" or #RRGGBB
    accent: String = "system".into(),
    reduce_motion: bool = false,
});

section!(Desk {
    /// "desktop" | "top" | "normal"
    pin_mode: String = "desktop".into(),
    locked: bool = true,
    click_through: bool = false,
    /// Desktop mode: clicks don't activate the window (keeps Win+D working).
    no_activate: bool = true,
    acrylic: bool = false,
    /// "auto" (follows the theme) or #RRGGBB
    bg_color: String = "auto".into(),
    /// 0–100
    bg_opacity: f64 = 72.0,
    /// Opacity of the day cells, 0–100
    cell_opacity: f64 = 55.0,
    radius: f64 = 8.0,
    shadow: bool = true,
    /// "auto" | "on" | "off"
    text_shadow: String = "auto".into(),
    font_scale: f64 = 100.0,
    cell_gap: f64 = 3.0,
    line_spacing: f64 = 2.0,
    today_color: String = "accent".into(),
    rest_bg_color: String = "auto".into(),
    holiday_text_color: String = "auto".into(),
    term_text_color: String = "auto".into(),
    /// Where the user put the calendar (written by layout.rs; opaque here).
    #[serde(skip_serializing_if = "Option::is_none")]
    bounds: Option<Value> = None,
});

section!(Calendar {
    /// "month" | "weeks" | "gantt" | "list"
    view: String = "month".into(),
    /// The list view: "upcoming" (近期) | "all" (所有)
    list_range: String = "upcoming".into(),
    /// 0 = Sunday, 1 = Monday
    week_start: u32 = 1,
    /// "auto" | "six"
    month_rows: String = "auto".into(),
    weeks_rows: u32 = 5,
    weeks_past: u32 = 1,
    show_lunar: bool = true,
    show_holidays: bool = true,
    show_week_numbers: bool = false,
    show_almanac: bool = false,
    /// Countdown to the next statutory holiday in the countdown bar.
    holiday_countdown: bool = true,
    /// 0 = Sunday … 6 = Saturday
    rest_days: Vec<u32> = vec![0, 6],
});

section!(Tasks {
    show_completed: bool = true,
    completed_last: bool = true,
    auto_postpone: bool = false,
    default_color: String = String::new(),
});

section!(Gantt {
    /// "day" | "week" | "month"
    scale: String = "day".into(),
    table_width: f64 = 220.0,
    show_table: bool = true,
    hide_done: bool = false,
    hide_single_day: bool = true,
    show_deps: bool = true,
    group_filter: Vec<String> = Vec::new(),
});

section!(Reminders {
    enabled: bool = true,
    /// "toast" | "popup" | "both"
    style: String = "both".into(),
    all_day_time: String = "09:00".into(),
    default_offsets: Vec<i64> = Vec::new(),
});

section!(Holidays {
    auto_update: bool = true,
    last_check: i64 = 0,
});

section!(Shortcuts {
    toggle: String = "Ctrl+Alt+D".into(),
    new_task: String = "Ctrl+Alt+N".into(),
});

section!(Web {
    enabled: bool = false,
    port: u16 = 18765,
});

section!(SyncWebDav {
    url: String = String::new(),
    user: String = String::new(),
    folder: String = "FluentCal".into(),
});

section!(SyncGit {
    repo: String = String::new(),
    branch: String = String::new(),
    path: String = String::new(),
});

section!(Sync {
    enabled: bool = false,
    /// "" = off; "webdav" | "github" | "gitee"; "folder" = a shared data folder
    provider: String = String::new(),
    /// Minutes between pulls; 0 = only after start and edits.
    interval: u32 = 5,
    /// "" = Windows system proxy, "direct" = none, otherwise http://host:port.
    proxy: String = String::new(),
    webdav: SyncWebDav = SyncWebDav::default(),
    github: SyncGit = SyncGit::default(),
    gitee: SyncGit = SyncGit::default(),
});

section!(
    /// All settings, defaults filled in.
    Settings {
    general: General = General::default(),
    theme: Theme = Theme::default(),
    desk: Desk = Desk::default(),
    calendar: Calendar = Calendar::default(),
    tasks: Tasks = Tasks::default(),
    gantt: Gantt = Gantt::default(),
    reminders: Reminders = Reminders::default(),
    holidays: Holidays = Holidays::default(),
    shortcuts: Shortcuts = Shortcuts::default(),
    web: Web = Web::default(),
    sync: Sync = Sync::default(),
});

/// 0.2's mergeDefaults: stored values over the defaults, field by field. A stored value whose
/// type differs from the default (corrupt or from an older version) is ignored; keys the
/// defaults don't have are kept (e.g. desk.bounds).
pub fn merge_defaults(defaults: &Value, stored: &Value) -> Value {
    match (defaults, stored) {
        (Value::Object(d), Value::Object(s)) => {
            let mut out = serde_json::Map::new();
            for (k, dv) in d {
                out.insert(k.clone(), merge_defaults(dv, s.get(k).unwrap_or(&Value::Null)));
            }
            for (k, sv) in s {
                out.entry(k.clone()).or_insert_with(|| sv.clone());
            }
            Value::Object(out)
        }
        (d, Value::Null) => d.clone(),
        (Value::Null, s) => s.clone(), // no default (optional): whatever is stored
        (d, s) if std::mem::discriminant(d) == std::mem::discriminant(s) => s.clone(),
        (d, _) => d.clone(),
    }
}

impl Settings {
    /// Stored settings with the defaults filled in (merge_defaults).
    pub fn from_value(v: &Value) -> Settings {
        let defaults = serde_json::to_value(Settings::default()).unwrap_or(Value::Null);
        let mut settings: Settings = serde_json::from_value(merge_defaults(&defaults, v)).unwrap_or_default();
        if !crate::i18n::valid_preference(&settings.general.language) {
            settings.general.language = "system".into();
        }
        settings
    }

    pub fn of_doc(doc: &Value) -> Settings {
        Settings::from_value(doc.get("settings").unwrap_or(&Value::Null))
    }
}
