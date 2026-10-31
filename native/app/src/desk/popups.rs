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

    /// The Gantt view's 显示选项 (0.2's optionsMenu). A native menu until the Fluent one lands.
    pub fn gantt_options(&mut self, _anchor: (f32, f32, f32, f32)) {
        use crate::platform::tray::Item;
        let g = &self.doc.settings.gantt;
        let check = |id: &str, title: &str, on: bool| Item::Check { id: id.into(), title: title.into(), checked: on };
        let mut groups = vec![check("filter:", "全部分组", g.group_filter.is_empty())];
        let mut list: Vec<_> = self.doc.groups.values().collect();
        list.sort_by(|a, b| a.order.total_cmp(&b.order));
        for x in list {
            groups.push(check(&format!("filter:{}", x.id), &x.name, g.group_filter.len() == 1 && g.group_filter[0] == x.id));
        }
        let items = vec![
            check("showTable", "显示任务表", g.show_table),
            check("showDeps", "显示依赖连线", g.show_deps),
            check("hideDone", "隐藏已完成任务", g.hide_done),
            check("hideSingleDay", "隐藏未分组的单日任务", g.hide_single_day),
            Item::Separator,
            Item::Sub { title: "导出图片".into(), items: vec![Item::Command { id: "png".into(), title: "PNG 图片…".into() }, Item::Command { id: "svg".into(), title: "SVG 矢量图…".into() }] },
            Item::Sub { title: "只显示分组".into(), items: groups },
        ];
        let flags = (g.show_table, g.show_deps, g.hide_done, g.hide_single_day);
        let Some(h) = crate::platform::window::hwnd(slint::ComponentHandle::window(&self.desk.window)) else { return };
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let Some(id) = crate::platform::tray::track(h, &items) else { return };
            crate::app::run(move |app| {
                app.history.boundary();
                match id.as_str() {
                    "showTable" => app.patch_settings(json!({ "gantt": { "showTable": !flags.0 } })),
                    "showDeps" => app.patch_settings(json!({ "gantt": { "showDeps": !flags.1 } })),
                    "hideDone" => app.patch_settings(json!({ "gantt": { "hideDone": !flags.2 } })),
                    "hideSingleDay" => app.patch_settings(json!({ "gantt": { "hideSingleDay": !flags.3 } })),
                    "png" => app.gantt_export("png"),
                    "svg" => app.gantt_export("svg"),
                    f => {
                        let id = f.strip_prefix("filter:").unwrap_or("");
                        let list: Vec<&str> = if id.is_empty() { Vec::new() } else { vec![id] };
                        app.patch_settings(json!({ "gantt": { "groupFilter": list } }));
                    }
                }
            });
        });
    }

    pub fn gantt_row_menu(&mut self, _row: &crate::GRow, _x: f32, _y: f32) {
        self.notify(PENDING, false);
    }

    pub fn gantt_bar_menu(&mut self, _bar: &crate::GBar, _x: f32, _y: f32) {
        self.notify(PENDING, false);
    }

    /// Shows a save dialog on the next turn (it is modal: the app must not be borrowed), then
    /// writes what `data` makes and says where (0.2's file_save).
    pub fn save_file_later(&mut self, name: &str, filter: &str, exts: &[&str], data: impl FnOnce() -> Result<Vec<u8>, String> + 'static) {
        let owner = crate::platform::window::hwnd(slint::ComponentHandle::window(&self.desk.window));
        let (name, filter, exts) = (name.to_owned(), filter.to_owned(), exts.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>());
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
            let Some(path) = crate::platform::dialog::save_file(owner, &name, &filter, &exts) else { return };
            let res = data().and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string()));
            crate::app::run(move |app| match res {
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
