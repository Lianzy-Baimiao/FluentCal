//! What the desk window's buttons, cells and chips do (0.2's desk/state.ts and the handlers
//! of TitleBar, MonthView, WeekRow and TaskChip). Keyboard, wheel, drag: input.rs.

use fluentcal_core::i18n::{tr, trf};

use super::vm::{self, shown_range};
use crate::app::App;
use fluentcal_core::date::{add_days, now_ms, start_of_month, start_of_week};
use fluentcal_core::model::Task;
use fluentcal_core::tasks;
use serde_json::json;
use slint::{ComponentHandle, Model};
use std::time::Duration;

impl App {
    pub fn desk_page(&mut self, delta: i32) {
        self.desk_clear_reveal();
        vm::page(&self.doc, &mut self.desk.ui, delta);
        self.desk_refresh_view();
    }

    pub fn desk_go_today(&mut self) {
        self.desk_clear_reveal();
        if self.doc.settings.calendar.view == "list" {
            self.desk_refresh_data();
            return self.desk_list_scroll_today();
        }
        let today = self.today();
        let ui = &mut self.desk.ui;
        ui.cursor = start_of_month(&today);
        ui.weeks_start = vm::default_weeks_start(&self.doc, &today);
        ui.selected = today.clone();
        self.desk_flash(&today);
        self.desk_refresh_view();
    }

    /// A date-only link (a holiday, for example).
    pub fn desk_jump(&mut self, date: &str) {
        self.desk_locate("", date);
    }

    pub fn desk_clear_reveal(&mut self) {
        self.desk.ui.reveal_task.clear();
        self.desk.ui.reveal_all = false;
    }

    /// Locate the task/occurrence, keeping list view and all saved visibility preferences.
    pub fn desk_locate(&mut self, task_id: &str, date: &str) {
        if !fluentcal_core::date::is_iso_date(date) { return; }
        self.desk_clear_reveal();
        let task = self.doc.tasks.get(task_id).cloned();
        let hidden = task.as_ref().is_some_and(|t| !vm::shown(&self.doc, &self.desk.ui, t) || (!self.doc.settings.tasks.show_completed && t.done));
        let (start, end) = task.as_ref().and_then(|t| fluentcal_core::recur::occurrence_on(t, date).or_else(|| fluentcal_core::recur::occurrences(t, date, date, 400).into_iter().next())).map_or_else(|| (date.to_owned(), date.to_owned()), |o| (o.start, o.end));
        let list = self.doc.settings.calendar.view == "list" && task.is_some();
        if matches!(self.doc.settings.calendar.view.as_str(), "gantt" | "list") {
            if !list { self.patch_settings(json!({ "calendar": { "view": "month" } })); }
        }
        let today = self.today();
        let outside = list && (task.as_ref().is_some_and(|t| t.flex.is_some()) || end < today || start > add_days(&today, self.desk.ui.list_span - 1));
        let c = &self.doc.settings.calendar;
        let ui = &mut self.desk.ui;
        ui.cursor = start_of_month(date);
        ui.weeks_start = add_days(&start_of_week(date, c.week_start), -7 * c.weeks_past as i32);
        ui.selected = date.to_owned();
        ui.reveal_task = task.as_ref().map_or_else(String::new, |t| t.id.clone());
        ui.reveal_all = outside;
        self.desk_flash_range(&start, &end, task_id);
        self.desk_refresh_view();
        if list {
            let rows = self.desk.window.get_list_rows();
            if let Some(index) = rows.iter().position(|r| r.kind == "item" && r.chip.task_id == task_id) {
                let w = self.desk.window.as_weak();
                slint::Timer::single_shot(Duration::ZERO, move || { if let Some(w) = w.upgrade() { w.invoke_list_scroll_to(index as i32); } });
            }
        }
        if hidden || outside { self.notify(tr("已临时显示定位任务，切换视图或筛选后恢复"), false); }
    }

    pub fn desk_set_view(&mut self, view: &str) {
        let revealed = !self.desk.ui.reveal_task.is_empty();
        self.desk_clear_reveal();
        self.history.boundary();
        // The view that goes takes its focused item with it (a task, the to-do column's add
        // box): the keys go back to the calendar first.
        self.desk.window.invoke_focus_root();
        self.patch_settings(json!({ "calendar": { "view": view } }));
        if revealed { self.desk_refresh_view(); }
    }

    /// The list view's 近期 / 所有 (it opens on today: shell::apply_settings).
    pub fn desk_set_list_range(&mut self, range: &str) {
        let revealed = !self.desk.ui.reveal_task.is_empty();
        self.desk_clear_reveal();
        self.history.boundary();
        self.patch_settings(json!({ "calendar": { "listRange": range } }));
        if revealed { self.desk_refresh_data(); }
    }

    /// 显示更多日期: 30 more days, up to 180.
    pub fn desk_list_more(&mut self) {
        let ui = &mut self.desk.ui;
        ui.list_span = (ui.list_span + vm::LIST_SPAN).min(vm::LIST_SPAN_MAX);
        self.desk_refresh_data();
    }

    /// The list view: today's heading to the top (近期: the first row).
    pub fn desk_list_scroll_today(&mut self) {
        let w = &self.desk.window;
        w.invoke_list_scroll_to(w.get_list_today_row());
    }

    pub fn desk_flash(&mut self, date: &str) {
        self.desk_flash_range(date, date, "");
    }

    fn desk_flash_range(&mut self, date: &str, end: &str, task_id: &str) {
        self.desk.ui.flash = date.to_owned();
        self.desk.ui.flash_end = end.to_owned();
        self.desk.ui.flash_task = task_id.to_owned();
        self.desk.window.set_flash_date(date.into());
        self.desk.window.set_flash_end(end.into());
        self.desk.window.set_flash_task(task_id.into());
        self.desk.window.global::<crate::ChipKeys>().set_located_task(task_id.into());
        self.desk.flash_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(3000), || {
            crate::app::run(|app| {
                app.desk.ui.flash.clear();
                app.desk.ui.flash_end.clear();
                app.desk.ui.flash_task.clear();
                app.desk.window.set_flash_date("".into());
                app.desk.window.set_flash_end("".into());
                app.desk.window.set_flash_task("".into());
                app.desk.window.global::<crate::ChipKeys>().set_located_task("".into());
            })
        });
    }

    /// A click on a day: selects it (arrow keys move from there).
    pub fn desk_select(&mut self, date: &str) {
        self.desk.ui.selected = date.to_owned();
        self.desk.ui.grid_focus = true;
        self.desk.window.set_selected(date.into());
        self.desk.window.invoke_focus_root();
    }

    /// Keyboard: selects `date`, paging when it is outside the shown days.
    pub fn desk_select_key(&mut self, date: &str) {
        let (first, last) = shown_range(&self.doc, &self.desk.ui);
        let ui = &mut self.desk.ui;
        ui.selected = date.to_owned();
        if self.doc.settings.calendar.view == "weeks" {
            if date < first.as_str() {
                ui.weeks_start = add_days(&ui.weeks_start, -7);
            } else if date > last.as_str() {
                ui.weeks_start = add_days(&ui.weeks_start, 7);
            }
        } else if date < first.as_str() || date > last.as_str() {
            ui.cursor = start_of_month(date);
        }
        self.desk_refresh_view();
    }

    /// Transient message at the bottom of the calendar (0.2's notify).
    pub fn notify(&mut self, text: &str, undo: bool) {
        let w = &self.desk.window;
        w.set_toast_text(text.into());
        w.set_toast_undo(undo && self.history.can_undo());
        let ms = if undo { 6000 } else { 2500 };
        self.desk.toast_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(ms), || {
            crate::app::run(|app| app.desk.window.set_toast_text("".into()))
        });
    }

    pub fn desk_close_toast(&mut self) {
        self.desk.toast_timer.stop();
        self.desk.window.set_toast_text("".into());
    }

    // ------------------------------------------------------------- quick add

    pub fn desk_quick_add(&mut self, date: &str) {
        if self.desk.window.get_read_only() {
            return;
        }
        self.desk.ui.quick_add = Some(date.to_owned());
        self.desk.window.set_quick_add_text("".into());
        self.desk.window.set_quick_add_date(date.into());
    }

    pub fn desk_quick_add_close(&mut self) {
        self.desk.ui.quick_add = None;
        self.desk.window.set_quick_add_date("".into());
        self.desk.window.invoke_focus_root();
        // back to the day it was on (0.2: the cell keeps the focus)
        if self.desk.ui.grid_focus {
            self.desk_set_grid_focus(true);
            self.desk_refocus_cell();
        }
    }

    /// The selected day's cell takes the focus again (after the quick-add box closes).
    pub fn desk_refocus_cell(&mut self) {
        let g = slint::ComponentHandle::global::<crate::CellFocus>(&self.desk.window);
        g.set_nudge(g.get_nudge() + 1);
    }

    /// The box got the focus: the calendar (which normally doesn't activate) takes the keyboard.
    pub fn desk_wants_keyboard(&mut self) {
        if crate::platform::desk::allow_keyboard() {
            crate::platform::desk::focus();
        }
    }

    pub fn desk_quick_add_submit(&mut self, text: &str) {
        let title = text.trim().to_owned();
        let Some(date) = self.desk.ui.quick_add.clone() else { return };
        if title.is_empty() {
            return;
        }
        self.history.boundary();
        self.desk.window.set_quick_add_text("".into());
        // (while the calendar shows one group only, the task is one of that group)
        let group_id = self.desk_new_task_group();
        let (edit, _) = tasks::add_task(&self.doc, Task { title: title.clone(), start: date.clone(), group_id: group_id.clone(), ..Default::default() }, None, now_ms());
        self.apply(edit, true);
        if self.desk_filter_told(&title, group_id.as_deref()) {
            return;
        }
        // Stays open for the next entry; tell when the new task went behind "+N".
        if let Some(n) = self.desk_overflow_on(&date).filter(|n| *n > 0) {
            self.notify(&trf("已添加“{0}”（点 +{1} 查看）", &[format!("{}", title), format!("{}", n)]), false);
        }
    }

    /// The to-do column's add boxes (GitHub #7): `zone` "month" (once every month, on whichever
    /// day) or "someday" (no date).
    pub fn desk_side_add(&mut self, zone: &str, text: &str) {
        let title = text.trim();
        if title.is_empty() || self.desk.window.get_read_only() {
            return;
        }
        self.history.boundary();
        let kind = if zone == "month" { fluentcal_core::flex::MONTH } else { fluentcal_core::flex::SOMEDAY };
        let group_id = self.desk_new_task_group();
        let p = Task { group_id: group_id.clone(), ..fluentcal_core::flex::new_task(kind, title, &self.today()) };
        let (edit, _) = tasks::add_task(&self.doc, p, None, now_ms());
        self.apply(edit, true);
        self.desk_filter_told(title, group_id.as_deref());
    }

    pub fn desk_side_set(&mut self, on: bool) {
        // Closing it removes whatever has the focus in it (an add box): the keys go back to
        // the calendar first, or the shortcuts would be gone with it.
        if !on {
            self.desk.window.invoke_focus_root();
        }
        self.patch_settings(json!({ "calendar": { "sidebar": on } }));
    }

    /// Presses are watched while the floating column closes on one outside it, and only
    /// then: on the calendar by its window procedure, anywhere else by the mouse hook.
    pub fn desk_side_watch(&mut self) {
        use crate::platform::{desk as pd, outside};
        // (no window of its own under the testing backend: nothing to watch)
        let on = crate::shell::side_closes_outside(&self.doc.settings) && pd::desk_hwnd().is_some();
        pd::watch_presses(on);
        // (posted: the hook answers at once, whatever closing the column takes; the press is
        // another window's and waits for nothing of ours)
        let away = || {
            std::rc::Rc::new(|| {
                crate::app::post(|a| {
                    a.desk_side_pressed(None);
                })
            }) as std::rc::Rc<dyn Fn()>
        };
        outside::watch_elsewhere("desk-side", on.then(|| (Box::new(pd::desk_hwnd) as Box<dyn Fn() -> _>, away())));
    }

    /// A mouse button went down: on the calendar at `at` (window px), or somewhere else
    /// (None). Outside the floating column that closes it, when the column closes that way
    /// (CAL-17); true when it did. The press itself goes where it was going.
    pub fn desk_side_pressed(&mut self, at: Option<(f32, f32)>) -> bool {
        if !crate::shell::side_closes_outside(&self.doc.settings) || self.desk.side_drag.is_some() {
            return false;
        }
        if at.is_some_and(|(x, y)| self.desk.window.invoke_side_keeps(x, y)) {
            return false;
        }
        self.desk_side_set(false);
        true
    }

    /// … on the calendar, in the window's physical px (its window procedure tells).
    pub fn desk_side_pressed_px(&mut self, x: i32, y: i32) {
        let scale = slint::ComponentHandle::window(&self.desk.window).scale_factor();
        self.desk_side_pressed(Some((x as f32 / scale, y as f32 / scale)));
    }

    /// "+N" count of a shown day.
    fn desk_overflow_on(&self, date: &str) -> Option<i32> {
        let weeks = self.desk.window.get_weeks();
        for w in weeks.iter() {
            if let Some(i) = w.cells.iter().position(|c| c.date == date) {
                return w.more.row_data(i);
            }
        }
        None
    }

    /// Deletes a task (or one occurrence of a repeating one) and offers undo (0.2's removeTask).
    pub fn desk_remove_task(&mut self, t: &Task, occurrence: Option<(&str, tasks::DeleteMode)>) {
        self.history.boundary();
        let edit = match occurrence.filter(|_| t.repeat.is_some()) {
            Some((date, mode)) => tasks::delete_occurrence(&self.doc, t, date, mode, now_ms()),
            None => tasks::delete_task(&self.doc, &t.id, now_ms()),
        };
        self.apply(edit, true);
        let short: String = t.title.chars().take(16).collect();
        let name = if t.title.chars().count() > 16 { format!("{short}…") } else { short };
        self.notify(&trf("已删除“{0}”", &[format!("{}", name)]), true);
    }

    // ------------------------------------------------------------- chips

    pub fn desk_chip_toggle(&mut self, task_id: &str, date: &str, done: bool) {
        if self.desk.window.get_read_only() {
            return;
        }
        self.history.boundary();
        let Some(t) = self.doc.tasks.get(task_id).cloned() else { return };
        let edit = tasks::set_occurrence_done(&self.doc, &t, date, !done, now_ms());
        self.apply(edit, true);
    }

    pub fn desk_undo(&mut self, from_toast: bool) {
        self.history.boundary();
        let doc = &self.doc;
        let r = self.history.undo(|c, id| doc.raw(c, id));
        self.replayed(r, tr("撤销"), from_toast);
    }

    pub fn desk_redo(&mut self) {
        self.history.boundary();
        let doc = &self.doc;
        let r = self.history.redo(|c, id| doc.raw(c, id));
        self.replayed(r, tr("重做"), false);
    }

    fn replayed(&mut self, r: fluentcal_core::history::Replay, verb: &str, quiet: bool) {
        let (done, skipped) = (r.done, r.skipped);
        self.apply(r.edit, false);
        if quiet {
            self.desk_close_toast();
        } else if !done {
            self.notify(&trf("没有可{0}的操作", &[format!("{}", verb)]), false);
        } else if skipped > 0 {
            self.notify(&trf("已{0}（{1} 项已在其他窗口修改，保持不变）", &[format!("{}", verb), format!("{}", skipped)]), false);
        } else {
            self.notify(&trf("已{0}", &[format!("{}", verb)]), false);
        }
    }
}
