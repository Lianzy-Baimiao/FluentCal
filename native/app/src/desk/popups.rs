//! The desk window's menus and flyouts: chip and day context menus, the "更多" menu, the
//! "+N" day list, the year/month picker and the search panel (0.2's menus.ts, DayListFlyout,
//! YearMonthPicker, SearchPanel). They need the control library's menu and flyout.

use crate::app::App;
use crate::ChipData;
use serde_json::json;

const PENDING: &str = "这个菜单还在移植中";

impl App {
    pub fn desk_chip_menu(&mut self, _chip: &ChipData, _x: f32, _y: f32) {
        self.notify(PENDING, false);
    }

    pub fn desk_cell_menu(&mut self, _date: &str, _x: f32, _y: f32) {
        self.notify(PENDING, false);
    }

    /// Menu key / Shift+F10 on the selected day.
    pub fn desk_cell_menu_key(&mut self) {
        let d = self.desk.ui.selected.clone();
        self.desk_cell_menu(&d, 0.0, 0.0);
    }

    pub fn desk_more_menu(&mut self, _anchor: (f32, f32, f32, f32)) {
        self.notify(PENDING, false);
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

    pub fn gantt_options(&mut self, _anchor: (f32, f32, f32, f32)) {
        self.notify(PENDING, false);
    }

    pub fn gantt_row_menu(&mut self, _row: &crate::GRow, _x: f32, _y: f32) {
        self.notify(PENDING, false);
    }

    pub fn gantt_bar_menu(&mut self, _bar: &crate::GBar, _x: f32, _y: f32) {
        self.notify(PENDING, false);
    }

    pub fn desk_toggle_lock(&mut self) {
        self.history.boundary();
        let locked = !self.doc.settings.desk.locked;
        self.patch_settings(json!({ "desk": { "locked": locked } }));
    }
}
