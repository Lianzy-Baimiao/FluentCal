//! The Gantt view's keys, the table splitter, cancelling a drag, and connecting the view's
//! callbacks to the app.

use fluentcal_core::i18n::{tr};

use crate::app::{run, App};
use crate::{DeskWindow, GanttData};
use serde_json::json;
use slint::platform::Key;
use slint::ComponentHandle;

fn is(text: &str, k: Key) -> bool {
    text == slint::SharedString::from(k).as_str()
}

impl App {
    /// Keys in the Gantt view (0.2's onKey): ↑/↓ select, Delete, Enter, F2.
    pub fn gantt_key(&mut self, text: &str) -> bool {
        let list: Vec<String> = self.desk.gantt.model.as_ref().map_or(Vec::new(), |m| m.rows.iter().filter(|r| r.kind == "task").map(|r| r.id.to_string()).collect());
        let sel = self.desk.gantt.selected.clone();
        let i = list.iter().position(|id| *id == sel);
        if is(text, Key::DownArrow) || is(text, Key::UpArrow) {
            let d: i64 = if is(text, Key::DownArrow) { 1 } else { -1 };
            let at = i.map_or(0, |i| (i as i64 + d).clamp(0, list.len() as i64 - 1) as usize);
            if let Some(next) = list.get(at).cloned() {
                self.gantt_select(&next);
                self.gantt_ensure_row(&format!("t:{next}"));
            }
            return true;
        }
        let Some(t) = self.doc.tasks.get(&sel).cloned() else { return false };
        if is(text, Key::Delete) {
            if t.repeat.is_some() {
                // 0.2 asks first: the row is the whole series.
                self.notify(tr("删除重复任务的确认对话框还在移植中"), false);
            } else {
                self.desk_remove_task(&t, None);
            }
        } else if is(text, Key::Return) {
            self.open_editor_task(&t.id, None);
        } else if is(text, Key::F2) {
            self.gantt_start_rename(&format!("t:{}", t.id));
        } else {
            return false;
        }
        true
    }

    pub fn gantt_dragging(&self) -> bool {
        let g = &self.desk.gantt;
        g.bar_drag.is_some() || g.zone_drag.is_some() || g.link_drag.is_some()
    }

    /// Escape while dragging: nothing changes.
    pub fn gantt_cancel_drag(&mut self) {
        let g = &mut self.desk.gantt;
        (g.bar_drag, g.zone_drag, g.link_drag) = (None, None, None);
        let gd = self.desk.window.global::<GanttData>();
        gd.set_ghost_show(false);
        gd.set_tip_show(false);
        gd.set_rubber_show(false);
        self.gantt_virtualize(true);
    }

    pub fn gantt_split_down(&mut self, x: f64) {
        let w = super::model::table_width(&self.doc, None);
        self.desk.gantt.split = Some((x, w));
    }

    pub fn gantt_split_move(&mut self, x: f64) {
        let Some((x0, w0)) = self.desk.gantt.split else { return };
        self.desk.gantt.live_width = Some((w0 + x - x0).clamp(140.0, 480.0));
        self.desk.gantt.dirty = true;
        self.gantt_push();
    }

    pub fn gantt_split_up(&mut self) {
        self.desk.gantt.split = None;
        if let Some(w) = self.desk.gantt.live_width.take() {
            self.history.boundary();
            self.patch_settings(json!({ "gantt": { "tableWidth": w.round() } }));
        }
    }
}

pub fn wire(w: &DeskWindow) {
    let g = w.global::<GanttData>();
    g.on_set_scale(|s| run(move |a| a.gantt_set_scale(&s)));
    g.on_go_today(|| run(|a| a.gantt_go_today()));
    g.on_new_group(|| run(|a| a.gantt_new_group()));
    g.on_options(|x, y, bw, bh| run(move |a| a.gantt_options((x, y, bw, bh))));
    g.on_scrolled(|| run(|a| a.gantt_scrolled()));
    g.on_zoom(|d| run(move |a| a.gantt_zoom(d)));
    g.on_row_click(|r| run(move |a| a.gantt_row_click(&r)));
    g.on_row_dbl(|r| run(move |a| a.open_editor_task(&r.id, None)));
    g.on_row_menu(|r, x, y| run(move |a| a.gantt_row_menu(&r, x, y)));
    g.on_row_check(|r| run(move |a| a.gantt_row_check(&r)));
    g.on_row_chevron(|r| run(move |a| a.gantt_toggle_group(&r.id)));
    g.on_row_rename(|r| run(move |a| a.gantt_start_rename(&r.key)));
    g.on_add_task(|id| run(move |a| a.gantt_add_in_group(&id, false)));
    g.on_rename_commit(|| run(|a| a.gantt_commit_rename()));
    g.on_rename_cancel(|| run(|a| a.gantt_cancel_rename()));
    g.on_rename_focus(|| run(|a| a.desk_wants_keyboard()));
    g.on_split_down(|x| run(move |a| a.gantt_split_down(f64::from(x))));
    g.on_split_move(|x| run(move |a| a.gantt_split_move(f64::from(x))));
    g.on_split_up(|| run(|a| a.gantt_split_up()));
    g.on_chart_down(|| run(|a| a.gantt_select("")));
    g.on_bar_down(|b, mode, x, _y| run(move |a| a.gantt_bar_down(&b, &mode, f64::from(x))));
    g.on_bar_move(|x, y| run(move |a| a.gantt_bar_move(f64::from(x), f64::from(y))));
    g.on_bar_up(|| run(|a| a.gantt_bar_up()));
    g.on_bar_dbl(|b| run(move |a| a.open_editor_task(&b.task_id, Some(b.date.to_string()))));
    g.on_bar_menu(|b, x, y| run(move |a| a.gantt_bar_menu(&b, x, y)));
    g.on_zone_down(|id, row, x| run(move |a| a.gantt_zone_down(&id, row, f64::from(x))));
    g.on_zone_move(|x| run(move |a| a.gantt_zone_move(f64::from(x))));
    g.on_zone_up(|| run(|a| a.gantt_zone_up()));
    g.on_link_down(|b, x, y| run(move |a| a.gantt_link_down(&b, f64::from(x), f64::from(y))));
    g.on_link_move(|x, y| run(move |a| a.gantt_link_move(f64::from(x), f64::from(y))));
    g.on_link_up(|x, y| run(move |a| a.gantt_link_up(f64::from(x), f64::from(y))));
}
