//! The to-do column's order by hand (GitHub #7): a to-do is dragged to another place in its
//! part (本月 / 上月逾期 / 无日期), or moved one place with Alt+↑ / Alt+↓. The column lists by the
//! tasks' `order`; `tasks::reorder_flex` gives the moved one a number between its neighbours'.

use crate::app::App;
use crate::SideRowData;
use fluentcal_core::date::now_ms;
use fluentcal_core::tasks;
use slint::{Model, ModelRc};

/// A to-do of the column being dragged.
pub struct SideDrag {
    /// The chip's key: the to-do and its part (one left open last month is in two).
    pub key: String,
    pub title: String,
    /// Its row in the column's list.
    pub row: usize,
    /// Where the press was (window logical px).
    pub x: f32,
    pub y: f32,
    pub active: bool,
}

/// The to-dos of the part a row is in: the first one's row and the last one's.
fn part(rows: &ModelRc<SideRowData>, row: usize) -> Option<(usize, usize)> {
    let item = |i: usize| rows.row_data(i).filter(|r| r.kind == "item");
    let zone = item(row)?.zone;
    let same = |i: &usize| item(*i).is_some_and(|r| r.zone == zone);
    let first = (0..=row).rev().take_while(same).last()?;
    let last = (row..rows.row_count()).take_while(same).last()?;
    Some((first, last))
}

/// Where the to-do of `row` goes when it is let go nearest the top edge of row `at`: a place
/// in its own part (the top edge of that row). None beside the column (-1), and where it
/// would stay as it is.
fn target(rows: &ModelRc<SideRowData>, row: usize, at: i32) -> Option<usize> {
    let (first, last) = part(rows, row)?;
    let at = usize::try_from(at).ok()?.clamp(first, last + 1);
    (at != row && at != row + 1).then_some(at)
}

impl App {
    pub fn desk_side_press(&mut self, c: &crate::ChipData, row: i32, x: f32, y: f32) {
        let Ok(row) = usize::try_from(row) else { return };
        if self.desk.window.get_read_only() {
            return;
        }
        self.desk.side_drag = Some(SideDrag { key: c.key.to_string(), title: c.title.to_string(), row, x, y, active: false });
    }

    pub fn desk_side_move(&mut self, x: f32, y: f32, at: i32) {
        let Some(d) = self.desk.side_drag.as_mut() else { return };
        let w = &self.desk.window;
        if !d.active {
            if (x - d.x).hypot(y - d.y) < 5.0 {
                return;
            }
            d.active = true;
            w.set_drag_active(true);
            w.set_drag_label(d.title.clone().into());
            w.set_side_drag_key(d.key.clone().into());
        }
        w.set_drag_x(x);
        w.set_drag_y(y);
        w.set_side_drop_row(target(&w.get_side_rows(), d.row, at).map_or(-1, |r| r as i32));
    }

    pub fn desk_side_release(&mut self, at: i32) {
        let Some(d) = self.desk_side_drag_end().filter(|d| d.active) else { return };
        // (the list may have changed under the drag: a sync, a reminder's 完成)
        let rows = self.desk.window.get_side_rows();
        if rows.row_data(d.row).is_none_or(|r| r.chip.key != d.key.as_str()) {
            return;
        }
        if let Some(to) = target(&rows, d.row, at) {
            self.desk_side_place(d.row, to);
        }
    }

    /// The drag is over: let go, Esc, or the press was taken away.
    pub fn desk_side_drag_end(&mut self) -> Option<SideDrag> {
        let d = self.desk.side_drag.take()?;
        let w = &self.desk.window;
        if d.active {
            w.set_drag_active(false);
        }
        w.set_side_drag_key("".into());
        w.set_side_drop_row(-1);
        Some(d)
    }

    /// Alt+↑ / Alt+↓ on a to-do: one place up or down in its part.
    pub fn desk_side_nudge(&mut self, row: i32, dir: i32) {
        let Ok(row) = usize::try_from(row) else { return };
        if self.desk.window.get_read_only() {
            return;
        }
        let Some((first, last)) = part(&self.desk.window.get_side_rows(), row) else { return };
        if dir < 0 && row > first {
            self.desk_side_place(row, row - 1);
        } else if dir > 0 && row < last {
            self.desk_side_place(row, row + 2);
        }
    }

    /// The to-do of `row` goes to the top edge of row `to` of its part.
    fn desk_side_place(&mut self, row: usize, to: usize) {
        let rows = self.desk.window.get_side_rows();
        let Some((first, last)) = part(&rows, row) else { return };
        let id_of = |i: usize| rows.row_data(i).map(|r| r.chip.task_id.to_string());
        let Some(id) = id_of(row) else { return };
        // its neighbours there, among the others of the part
        let others: Vec<String> = (first..=last).filter(|i| *i != row).filter_map(id_of).collect();
        let at = to.saturating_sub(first + usize::from(to > row)).min(others.len());
        let after = at.checked_sub(1).map(|i| others[i].as_str());
        let before = others.get(at).map(String::as_str);
        self.history.boundary();
        let edit = tasks::reorder_flex(&self.doc, &id, after, before, now_ms());
        if !edit.is_empty() {
            self.apply(edit, true);
        }
    }

    /// The row that has the keys shows another to-do now (the list is rewritten in place when
    /// its order changes: by hand, an undo, a sync): the focus goes to the row the to-do it had
    /// is in, so the keys stay with that one. It stays where it is when that one left the list.
    pub fn desk_side_refocus(&mut self, key: &str) {
        let w = &self.desk.window;
        let rows = w.get_side_rows();
        if let Some(i) = (0..rows.row_count()).find(|i| rows.row_data(*i).is_some_and(|r| r.kind == "item" && r.chip.key == key)) {
            w.set_side_focus_row(i as i32);
            w.set_side_focus_tick(w.get_side_focus_tick() % 1000 + 1);
        }
    }
}
