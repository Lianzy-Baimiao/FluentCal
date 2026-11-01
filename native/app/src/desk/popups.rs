//! The desk window's menus and flyouts: chip and day context menus, "更多", the Gantt group
//! menu and options (menus.rs has their contents), the "+N" day list, the year/month picker,
//! the search panel, confirmations and the info bars. All are drawn by the window
//! (ui/desk.slint); the view models are in overlays.rs.

use super::menus::Target;
use super::overlays;
use crate::app::{run, App};
use crate::ChipData;
use fluentcal_core::date::{now_ms, parts};
use fluentcal_core::model::Task;
use fluentcal_core::search::SearchOptions;
use fluentcal_core::tasks;
use serde_json::json;
use slint::ComponentHandle;
use std::time::Duration;

/// Where a menu opens: at a point (a right click) or below an element (a button).
#[derive(Debug, Clone, Copy)]
pub enum At {
    Point(f32, f32),
    Below(f32, f32, f32, f32),
}

/// The desk's open menus and dialogs (the window draws them).
#[derive(Default)]
pub struct Popups {
    /// What the open (or last) menu was opened on: its activations act on this.
    pub menu_target: Option<Target>,
    /// Runs on the confirmation's primary button.
    pub confirm: Option<Box<dyn FnOnce(&mut App)>>,
    /// The day the "+N" list shows.
    pub day_list: Option<String>,
    pub search_open: bool,
    /// The warning bar (data folder missing, data file repaired).
    pub notice: String,
}

impl App {
    /// Shows the menu for `target` and runs the chosen entry (0.2's showMenu).
    pub fn show_menu(&mut self, target: Target, at: At) {
        if self.desk.window.get_read_only() {
            return;
        }
        let entries = self.menu_entries(&target);
        let (items, has_check, has_icon) = overlays::menu(&entries, &self.tokens);
        let w = &self.desk.window;
        w.set_menu_items(items);
        w.set_menu_has_check(has_check);
        w.set_menu_has_icon(has_icon);
        let bottom_end = matches!(target, Target::More | Target::GanttOptions);
        w.set_menu_bottom_end(bottom_end);
        self.desk.popups.menu_target = Some(target);
        match at {
            At::Point(x, y) => w.invoke_open_menu(x, y, 0.0, 0.0),
            At::Below(x, y, bw, bh) => w.invoke_open_menu(x, y, bw, bh),
        }
    }

    pub fn desk_menu_activated(&mut self, key: &str) {
        if key.is_empty() {
            return;
        }
        if let Some(t) = self.desk.popups.menu_target.clone() {
            self.menu_act(&t, key);
        }
    }

    /// A colour in a swatch row: the row's key is the action's prefix ("color:", "mark:").
    pub fn desk_menu_swatch(&mut self, row: &str, swatch: &str) {
        self.desk_menu_activated(&format!("{row}{swatch}"));
    }

    /// A question with 取消 and `yes_label` (0.2's confirm(); danger style); `yes` runs on it.
    pub fn confirm_later(&mut self, title: &str, text: &str, yes_label: &str, yes: impl FnOnce(&mut App) + 'static) {
        let w = &self.desk.window;
        w.set_confirm_title(title.into());
        w.set_confirm_text(text.into());
        w.set_confirm_primary(yes_label.into());
        w.set_confirm_danger(true);
        self.desk.popups.confirm = Some(Box::new(yes));
        // The dialog takes Enter / Esc: the calendar needs the keyboard.
        self.desk_wants_keyboard();
        self.desk.window.invoke_open_confirm();
    }

    pub fn desk_confirm_result(&mut self, result: &str) {
        let Some(yes) = self.desk.popups.confirm.take() else { return };
        self.desk.window.invoke_close_confirm();
        if result == "primary" {
            self.history.boundary();
            yes(self);
        }
    }

    pub fn desk_chip_menu(&mut self, chip: &ChipData, x: f32, y: f32) {
        self.show_menu(Target::Task(chip.task_id.to_string(), chip.date.to_string()), At::Point(x, y));
    }

    pub fn desk_cell_menu(&mut self, date: &str, x: f32, y: f32) {
        self.show_menu(Target::Day(date.to_owned()), At::Point(x, y));
    }

    /// Menu key / Shift+F10 on the selected day: below the day's cell.
    pub fn desk_cell_menu_key(&mut self) {
        let d = self.desk.ui.selected.clone();
        let at = self.desk_cell_rect(&d).map_or(At::Point(0.0, 0.0), |(x, y, cw, ch)| At::Below(x, y, cw, ch));
        self.show_menu(Target::Day(d), at);
    }

    /// The cell of `date` in window logical px, if it is shown.
    pub fn desk_cell_rect(&self, date: &str) -> Option<(f32, f32, f32, f32)> {
        use slint::Model;
        let w = &self.desk.window;
        let (gx, gy, cw, ch, gap) = (w.get_grid_x(), w.get_grid_y(), w.get_cell_w(), w.get_cell_h(), w.get_cell_gap());
        let weeks = w.get_weeks();
        (0..weeks.row_count()).find_map(|r| {
            let week = weeks.row_data(r)?;
            let c = (0..week.cells.row_count()).find(|&c| week.cells.row_data(c).is_some_and(|cell| cell.date == date))?;
            Some((gx + c as f32 * (cw + gap), gy + r as f32 * (ch + gap), cw, ch))
        })
    }

    pub fn desk_more_menu(&mut self, anchor: (f32, f32, f32, f32)) {
        self.show_menu(Target::More, At::Below(anchor.0, anchor.1, anchor.2, anchor.3));
    }

    pub fn gantt_options(&mut self, anchor: (f32, f32, f32, f32)) {
        self.show_menu(Target::GanttOptions, At::Below(anchor.0, anchor.1, anchor.2, anchor.3));
    }

    pub fn gantt_row_menu(&mut self, row: &crate::GRow, x: f32, y: f32) {
        if row.kind == "group" {
            self.show_menu(Target::Group(row.id.to_string()), At::Point(x, y));
        } else if let Some(t) = self.doc.tasks.get(row.id.as_str()) {
            let start = t.start.clone();
            self.gantt_select(&row.id);
            self.show_menu(Target::Task(row.id.to_string(), start), At::Point(x, y));
        }
    }

    pub fn gantt_bar_menu(&mut self, bar: &crate::GBar, x: f32, y: f32) {
        self.gantt_select(&bar.task_id);
        self.show_menu(Target::Task(bar.task_id.to_string(), bar.date.to_string()), At::Point(x, y));
    }

    /// A press outside the calendar (0.2: the window lost the focus): menus and flyouts close.
    pub fn desk_close_popups(&mut self) {
        let w = &self.desk.window;
        w.invoke_close_menu();
        w.invoke_close_day_list();
        w.invoke_close_ym();
    }

    // ------------------------------------------------------------- "+N" day list

    pub fn desk_day_list(&mut self, date: &str, anchor: (f32, f32, f32, f32)) {
        self.desk.popups.day_list = Some(date.to_owned());
        self.desk.window.set_day_list_date(date.into());
        self.desk.window.set_day_list_text("".into());
        self.desk_refresh_day_list();
        self.desk.window.invoke_open_day_list(anchor.0, anchor.1, anchor.2, anchor.3);
    }

    /// The list follows the data while it is open.
    pub fn desk_refresh_day_list(&mut self) {
        let Some(date) = self.desk.popups.day_list.clone() else { return };
        let (title, lunar, chips) = overlays::day_list(&self.doc, &date, &self.today(), &self.tokens);
        let w = &self.desk.window;
        w.set_day_list_title(title.into());
        w.set_day_list_lunar(lunar.into());
        w.set_day_list_chips(chips);
    }

    pub fn desk_day_list_closed(&mut self) {
        self.desk.popups.day_list = None;
    }

    pub fn desk_day_list_add(&mut self, text: &str) {
        let title = text.trim().to_owned();
        let Some(date) = self.desk.popups.day_list.clone() else { return };
        if title.is_empty() || self.desk.window.get_read_only() {
            return;
        }
        self.history.boundary();
        self.desk.window.set_day_list_text("".into());
        let (edit, _) = tasks::add_task(&self.doc, Task { title, start: date, ..Default::default() }, None, now_ms());
        self.apply(edit, true);
    }

    // ------------------------------------------------------------- year / month

    pub fn desk_pick_month(&mut self, anchor: (f32, f32, f32, f32)) {
        let (y, _, _) = parts(&self.desk.ui.cursor);
        let w = &self.desk.window;
        w.set_ym_year(y);
        w.set_ym_cursor(self.desk.ui.cursor[..7].into());
        w.set_ym_now(self.today()[..7].into());
        w.invoke_open_ym(anchor.0, anchor.1, anchor.2, anchor.3);
    }

    pub fn desk_ym_pick(&mut self, year: i32, month: i32) {
        self.desk.ui.cursor = format!("{year:04}-{month:02}-01");
        self.desk_refresh_view();
    }

    // ------------------------------------------------------------- search

    /// Ctrl+F / the title bar's search button: opens or closes the panel.
    pub fn desk_search(&mut self) {
        if self.desk.popups.search_open {
            return self.desk_search_close();
        }
        self.desk.popups.search_open = true;
        let w = &self.desk.window;
        w.set_search_colors(overlays::search_colors(&self.tokens));
        w.set_search_open(true);
        self.desk_search_refresh();
        // The box is focused when it appears: the calendar needs the keyboard.
        self.desk_wants_keyboard();
    }

    pub fn desk_search_refresh(&mut self) {
        if !self.desk.popups.search_open {
            return;
        }
        let w = &self.desk.window;
        let o = SearchOptions {
            q: w.get_search_query().to_string(),
            range: w.get_search_range().to_string(),
            status: w.get_search_status().to_string(),
            color: w.get_search_color().to_string(),
        };
        let (hits, empty) = overlays::search(&self.doc, &o, &self.today(), &self.tokens);
        w.set_search_hits(hits);
        w.set_search_empty(empty.into());
    }

    pub fn desk_search_close(&mut self) {
        self.desk.popups.search_open = false;
        self.desk.window.set_search_open(false);
        self.desk.window.invoke_focus_root();
    }

    pub fn desk_search_go(&mut self, date: &str) {
        self.desk_search_close();
        self.desk_jump(date);
    }

    // ------------------------------------------------------------- info bars

    /// The welcome bar (0.2: not onboarded and sample tasks left) and the warning bar.
    pub fn desk_refresh_bars(&mut self) {
        let w = &self.desk.window;
        w.set_welcome(overlays::welcome(&self.doc, w.get_read_only()));
        w.set_notice(self.desk.popups.notice.clone().into());
        w.set_save_error(self.save_error.clone().unwrap_or_default().into());
    }

    pub fn desk_set_notice(&mut self, text: &str) {
        self.desk.popups.notice = text.to_owned();
        self.desk_refresh_bars();
    }

    /// The welcome bar's 清除示例数据 (0.2's clearSamples: undoable, with a toast).
    pub fn desk_clear_samples(&mut self) {
        self.history.boundary();
        let (edit, n) = tasks::clear_sample_data(&self.doc, now_ms());
        self.apply(edit, true);
        self.patch_settings(json!({ "general": { "onboarded": true } }));
        self.notify(&format!("已删除 {n} 个示例任务"), true);
    }

    /// Shows a save dialog on the next turn (it is modal: the app must not be borrowed), then
    /// writes what `data` makes and says where (0.2's file_save).
    pub fn save_file_later(&mut self, name: &str, filter: &str, exts: &[&str], data: impl FnOnce() -> Result<Vec<u8>, String> + 'static) {
        let owner = crate::platform::window::hwnd(self.desk.window.window());
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
