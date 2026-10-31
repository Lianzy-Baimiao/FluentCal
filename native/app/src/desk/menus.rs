//! The desk window's menus as data (0.2's desk/menus.ts and the Gantt group menu): what they
//! contain and what each entry does. How they are drawn is popups.rs (the Fluent menu once the
//! control library is merged; a system menu until then).

use crate::app::App;
use fluentcal_core::theme::TASK_COLORS;

/// One menu entry. `id` is what activating it runs (`act`); empty for labels and separators.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub shortcut: String,
    pub checked: Option<bool>,
    pub radio: bool,
    pub disabled: bool,
    pub danger: bool,
    pub separator: bool,
    /// Colour swatches: (key, colour hex or "", title, selected); activating runs `id` + key.
    pub swatches: Vec<(String, String, String, bool)>,
    pub children: Vec<Entry>,
}

pub fn item(id: &str, label: &str, icon: &str) -> Entry {
    Entry { id: id.into(), label: label.into(), icon: icon.into(), ..Default::default() }
}

pub fn check(id: &str, label: &str, on: bool) -> Entry {
    Entry { checked: Some(on), ..item(id, label, "") }
}

pub fn radio(id: &str, label: &str, on: bool) -> Entry {
    Entry { radio: true, ..check(id, label, on) }
}

pub fn sep() -> Entry {
    Entry { separator: true, ..Default::default() }
}

pub fn sub(label: &str, icon: &str, children: Vec<Entry>) -> Entry {
    Entry { children, ..item("", label, icon) }
}

pub fn shortcut(mut e: Entry, s: &str) -> Entry {
    e.shortcut = s.into();
    e
}

/// The task palette as swatches (0.2's colorSwatches); `with_default`: a "默认" (no colour).
pub fn swatches(prefix: &str, selected: Option<&str>, with_default: bool) -> Entry {
    let mut s = Vec::new();
    if with_default {
        s.push((String::new(), String::new(), "默认".to_owned(), selected.is_none_or(str::is_empty)));
    }
    for c in TASK_COLORS {
        s.push((c.key.into(), c.bar.into(), c.label.into(), selected == Some(c.key)));
    }
    Entry { id: prefix.into(), swatches: s, ..Default::default() }
}

/// What a menu was opened on.
#[derive(Debug, Clone)]
pub enum Target {
    /// A chip / Gantt bar: task id and the occurrence's date.
    Task(String, String),
    Day(String),
    More,
    Group(String),
    GanttOptions,
}

impl App {
    /// Menu entries for `target` (0.2's chipMenu, cellMenu, moreMenu, groupMenu).
    pub fn menu_entries(&self, target: &Target) -> Vec<Entry> {
        match target {
            Target::Task(id, date) => super::menus_build::task(self, id, date),
            Target::Day(date) => super::menus_build::day(self, date),
            Target::More => super::menus_build::more(self),
            Target::Group(id) => super::menus_build::group(self, id),
            Target::GanttOptions => super::menus_build::gantt_options(self),
        }
    }
}
