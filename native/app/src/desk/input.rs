//! Keyboard, wheel, chip dragging and the adjust mode's move/resize (0.2's DeskApp onKey,
//! MonthView onGridKey/onWheel/chipDown…, DeskApp onAdjustDown/onGripDown).

use crate::app::App;
use fluentcal_core::date::{add_days, add_months, diff_days, now_ms};
use fluentcal_core::recur::occurrence_on;
use fluentcal_core::tasks;
use slint::platform::Key;
use slint::winit_030::{winit::window::ResizeDirection, WinitWindowAccessor};
use slint::{ComponentHandle, Model};

fn is(text: &str, k: Key) -> bool {
    text == slint::SharedString::from(k).as_str()
}

impl App {
    /// A key the text boxes didn't take. True when handled.
    pub fn desk_key(&mut self, text: &str, ctrl: bool, shift: bool) -> bool {
        self.history.boundary();
        let ch = text.to_lowercase();
        // Ctrl+letter arrives as the letter or as its control code.
        let letter = |c: char| ch == c.to_string() || ch == ((c as u8 - b'a' + 1) as char).to_string();
        let gantt = self.doc.settings.calendar.view == "gantt";
        if ctrl && letter('f') {
            self.desk_search();
        } else if ctrl && letter('n') {
            let start = self.desk.ui.selected.clone();
            self.open_editor_new(Some(start));
        } else if ctrl && letter('z') && !shift {
            self.desk_undo(false);
        } else if ctrl && (letter('y') || (shift && letter('z'))) {
            self.desk_redo();
        } else if is(text, Key::Escape) && self.gantt_dragging() {
            self.gantt_cancel_drag();
        } else if gantt && self.gantt_key(text) {
        } else if is(text, Key::Escape) {
            if self.desk.drag.is_some() {
                self.desk_drag_end();
            } else {
                self.desk_set_grid_focus(false);
            }
        } else if gantt {
            return false;
        } else if self.doc.settings.calendar.view == "list" {
            // No day grid: Home goes to today, PageUp / PageDown scroll the list.
            if is(text, Key::Home) {
                self.desk_list_scroll_today();
            } else if is(text, Key::PageUp) || is(text, Key::PageDown) {
                self.desk.window.invoke_list_page(if is(text, Key::PageUp) { -1 } else { 1 });
            } else {
                return false;
            }
        } else if self.desk.ui.grid_focus && self.desk_grid_key(text, shift) {
        } else if is(text, Key::Home) {
            self.desk_go_today();
        } else if is(text, Key::PageUp) || is(text, Key::PageDown) {
            self.desk_page(if is(text, Key::PageUp) { -1 } else { 1 });
        } else {
            return false;
        }
        true
    }

    /// The day grid has the keyboard (arrow keys move the selected day); off: back to the window.
    pub fn desk_set_grid_focus(&mut self, on: bool) {
        self.desk.ui.grid_focus = on;
        if on {
            slint::ComponentHandle::global::<crate::CellFocus>(&self.desk.window).set_on(true);
        } else {
            self.desk.window.invoke_focus_root();
        }
    }

    /// Arrow keys, Enter, Home, PageUp/Down and the menu key on the selected day.
    fn desk_grid_key(&mut self, text: &str, shift: bool) -> bool {
        let sel = self.desk.ui.selected.clone();
        let step = [(Key::LeftArrow, -1), (Key::RightArrow, 1), (Key::UpArrow, -7), (Key::DownArrow, 7)];
        if let Some((_, d)) = step.iter().find(|(k, _)| is(text, k.clone())) {
            self.desk_select_key(&add_days(&sel, *d));
        } else if is(text, Key::Return) {
            self.desk_quick_add(&sel);
        } else if is(text, Key::Home) {
            self.desk_go_today();
        } else if is(text, Key::PageUp) || is(text, Key::PageDown) {
            let dir = if is(text, Key::PageUp) { -1 } else { 1 };
            let to = if self.doc.settings.calendar.view == "weeks" { add_days(&sel, dir * 7) } else { add_months(&sel, dir) };
            self.desk_select_key(&to);
        } else if is(text, Key::Menu) || (shift && is(text, Key::F10)) {
            self.desk_cell_menu_key();
        } else {
            return false;
        }
        true
    }

    /// Wheel over the calendar pages (0.2: 50 px of delta, at most every 200 ms).
    pub fn desk_wheel(&mut self, delta_y: f32, ctrl: bool) {
        if ctrl || self.desk.drag.as_ref().is_some_and(|d| d.active) || self.doc.settings.calendar.view == "gantt" {
            return;
        }
        let ui = &mut self.desk.ui;
        // DOM deltaY is positive when scrolling down; Slint's delta-y is the opposite.
        ui.wheel_acc -= delta_y;
        let t = now_ms();
        if ui.wheel_acc.abs() >= 50.0 && t - ui.wheel_at > 200 {
            let dir = if ui.wheel_acc > 0.0 { 1 } else { -1 };
            ui.wheel_acc = 0.0;
            ui.wheel_at = t;
            self.desk_page(dir);
        }
    }

    // ------------------------------------------------------------- dragging a chip

    pub fn desk_chip_press(&mut self, key: &str, task_id: &str, date: &str, x: f32, y: f32) {
        if self.desk.window.get_read_only() {
            return;
        }
        let Some(t) = self.doc.tasks.get(task_id) else { return };
        let start = occurrence_on(t, date).map_or_else(|| t.start.clone(), |o| o.start);
        let offset = self.desk_date_at(x, y).map_or(0, |at| diff_days(&start, &at));
        let title = t.title.clone();
        self.desk.drag = Some(super::live::Drag { key: key.into(), task_id: task_id.into(), date: date.into(), title, x, y, offset, active: false });
    }

    pub fn desk_chip_move(&mut self, x: f32, y: f32) {
        let over = self.desk_date_at(x, y).unwrap_or_default();
        let Some(d) = self.desk.drag.as_mut() else { return };
        if !d.active {
            if (x - d.x).hypot(y - d.y) < 5.0 {
                return;
            }
            d.active = true;
            let w = &self.desk.window;
            w.set_drag_active(true);
            w.set_drag_label(d.title.clone().into());
            w.set_dragging_key(d.key.clone().into());
        }
        let w = &self.desk.window;
        w.set_drag_x(x);
        w.set_drag_y(y);
        w.set_drop_date(over.into());
    }

    pub fn desk_chip_release(&mut self, x: f32, y: f32) {
        let target = self.desk.drag.as_ref().filter(|d| d.active).and_then(|_| self.desk_date_at(x, y));
        let Some(d) = self.desk_drag_end() else { return };
        let (Some(target), Some(t)) = (target, self.doc.tasks.get(&d.task_id)) else { return };
        let Some(occ) = occurrence_on(t, &d.date) else { return };
        self.history.boundary();
        let edit = tasks::move_occurrence(&self.doc, &occ, &add_days(&target, -d.offset), now_ms());
        // Moving to another week removes the focused chip. Keep the keyboard on a
        // stable item before refreshing the models so undo/redo and search still work.
        self.desk.window.invoke_focus_root();
        self.apply(edit, true);
    }

    pub fn desk_drag_end(&mut self) -> Option<super::live::Drag> {
        let w = &self.desk.window;
        w.set_drag_active(false);
        w.set_drop_date("".into());
        w.set_dragging_key("".into());
        self.desk.drag.take()
    }

    /// The day cell under a point (window logical px); None over a gap or outside the grid.
    pub fn desk_date_at(&self, x: f32, y: f32) -> Option<String> {
        let w = &self.desk.window;
        let (gx, gy, cw, ch, gap) = (w.get_grid_x(), w.get_grid_y(), w.get_cell_w(), w.get_cell_h(), w.get_cell_gap());
        let (rx, ry) = (x - gx, y - gy);
        if rx < 0.0 || ry < 0.0 || cw <= 0.0 || ch <= 0.0 {
            return None;
        }
        let (col, row) = ((rx / (cw + gap)) as usize, (ry / (ch + gap)) as usize);
        if col > 6 || rx - col as f32 * (cw + gap) > cw || ry - row as f32 * (ch + gap) > ch {
            return None;
        }
        let week = w.get_weeks().row_data(row)?;
        week.cells.row_data(col).map(|c| c.date.to_string())
    }

    // ------------------------------------------------------------- adjust mode

    pub fn desk_start_move(&mut self) {
        if self.doc.settings.desk.locked {
            return;
        }
        self.desk.window.window().with_winit_window(|w| {
            let _ = w.drag_window();
        });
    }

    pub fn desk_start_resize(&mut self, edge: &str) {
        let dir = match edge {
            "North" => ResizeDirection::North,
            "South" => ResizeDirection::South,
            "East" => ResizeDirection::East,
            "West" => ResizeDirection::West,
            "NorthEast" => ResizeDirection::NorthEast,
            "NorthWest" => ResizeDirection::NorthWest,
            "SouthEast" => ResizeDirection::SouthEast,
            _ => ResizeDirection::SouthWest,
        };
        self.desk.window.window().with_winit_window(|w| {
            let _ = w.drag_resize_window(dir);
        });
    }
}
