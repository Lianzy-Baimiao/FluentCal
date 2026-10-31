//! The desk window's menus and flyouts: chip and day context menus, "更多", the Gantt group
//! menu and options (menus.rs has their contents), the "+N" day list, the year/month picker,
//! the search panel, confirmations. Menus are system menus until the control library's Fluent
//! menu is merged; the flyouts need its controls.

use super::menus::{Entry, Target};
use crate::app::{run, App};
use crate::platform::tray::{self, Item};
use crate::ChipData;
use serde_json::json;
use std::time::Duration;

const PENDING: &str = "这个面板还在移植中";

/// Menu entries as a system menu: separators, check marks, submenus; swatches become a
/// submenu named after the label before them (日期颜色) or 颜色.
fn native(entries: &[Entry]) -> Vec<Item> {
    let mut out = Vec::new();
    let mut label: Option<String> = None;
    for e in entries {
        if e.separator {
            out.push(Item::Separator);
        } else if !e.swatches.is_empty() {
            let items = e.swatches.iter().map(|(k, _, title, on)| Item::Check { id: format!("{}{k}", e.id), title: title.clone(), checked: *on }).collect();
            out.push(Item::Sub { title: label.take().unwrap_or_else(|| "颜色".into()), items });
        } else if e.disabled {
            label = Some(e.label.clone());
        } else if !e.children.is_empty() {
            out.push(Item::Sub { title: e.label.clone(), items: native(&e.children) });
        } else {
            let title = if e.shortcut.is_empty() { e.label.clone() } else { format!("{}\t{}", e.label, e.shortcut) };
            out.push(match e.checked {
                Some(c) => Item::Check { id: e.id.clone(), title, checked: c },
                None => Item::Command { id: e.id.clone(), title },
            });
        }
    }
    out
}

impl App {
    /// Shows the menu for `target` at the pointer and runs the chosen entry.
    pub fn show_menu(&mut self, target: Target) {
        if self.desk.window.get_read_only() {
            return;
        }
        let items = native(&self.menu_entries(&target));
        // Owned by the hidden window: the calendar itself must not be activated.
        let Some(owner) = crate::platform::msgwin::hwnd() else { return };
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(id) = tray::track(owner, &items) {
                run(move |app| app.menu_act(&target, &id));
            }
        });
    }

    /// A yes/no question (system dialog until the Fluent one is merged); `yes` runs on 是.
    pub fn confirm_later(&mut self, title: &str, text: &str, _yes_label: &str, yes: impl FnOnce(&mut App) + 'static) {
        let (title, text) = (title.to_owned(), text.to_owned());
        slint::Timer::single_shot(Duration::ZERO, move || {
            use windows::core::HSTRING;
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONWARNING, MB_YESNO};
            let body = if text.is_empty() { title.clone() } else { format!("{title}\n\n{text}") };
            crate::platform::desk::allow_foreground();
            if unsafe { MessageBoxW(None, &HSTRING::from(body), &HSTRING::from("FluentCal"), MB_YESNO | MB_ICONWARNING) } == IDYES {
                run(yes);
            }
        });
    }

    pub fn desk_chip_menu(&mut self, chip: &ChipData, _x: f32, _y: f32) {
        self.show_menu(Target::Task(chip.task_id.to_string(), chip.date.to_string()));
    }

    pub fn desk_cell_menu(&mut self, date: &str, _x: f32, _y: f32) {
        self.show_menu(Target::Day(date.to_owned()));
    }

    /// Menu key / Shift+F10 on the selected day.
    pub fn desk_cell_menu_key(&mut self) {
        let d = self.desk.ui.selected.clone();
        self.show_menu(Target::Day(d));
    }

    pub fn desk_more_menu(&mut self, _anchor: (f32, f32, f32, f32)) {
        self.show_menu(Target::More);
    }

    pub fn gantt_options(&mut self, _anchor: (f32, f32, f32, f32)) {
        self.show_menu(Target::GanttOptions);
    }

    pub fn gantt_row_menu(&mut self, row: &crate::GRow, _x: f32, _y: f32) {
        if row.kind == "group" {
            self.show_menu(Target::Group(row.id.to_string()));
        } else if let Some(t) = self.doc.tasks.get(row.id.as_str()) {
            let start = t.start.clone();
            self.gantt_select(&row.id);
            self.show_menu(Target::Task(row.id.to_string(), start));
        }
    }

    pub fn gantt_bar_menu(&mut self, bar: &crate::GBar, _x: f32, _y: f32) {
        self.gantt_select(&bar.task_id);
        self.show_menu(Target::Task(bar.task_id.to_string(), bar.date.to_string()));
    }

    pub fn desk_day_list(&mut self, _date: &str, _anchor: (f32, f32, f32, f32)) {
        self.notify(PENDING, false);
    }

    pub fn desk_pick_month(&mut self, _anchor: (f32, f32, f32, f32)) {
        self.notify(PENDING, false);
    }

    pub fn desk_search(&mut self) {
        self.notify(PENDING, false);
    }

    /// Shows a save dialog on the next turn (it is modal: the app must not be borrowed), then
    /// writes what `data` makes and says where (0.2's file_save).
    pub fn save_file_later(&mut self, name: &str, filter: &str, exts: &[&str], data: impl FnOnce() -> Result<Vec<u8>, String> + 'static) {
        let owner = crate::platform::window::hwnd(slint::ComponentHandle::window(&self.desk.window));
        let (name, filter, exts) = (name.to_owned(), filter.to_owned(), exts.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>());
        slint::Timer::single_shot(Duration::ZERO, move || {
            let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
            let Some(path) = crate::platform::dialog::save_file(owner, &name, &filter, &exts) else { return };
            let res = data().and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
            run(move |app| match res {
                Ok(()) => app.notify(&format!("已导出到 {path}"), false),
                Err(e) => app.notify(&format!("导出失败：{e}"), false),
            });
        });
    }

    pub fn desk_toggle_lock(&mut self) {
        self.history.boundary();
        let locked = !self.doc.settings.desk.locked;
        self.patch_settings(json!({ "desk": { "locked": locked } }));
    }
}
