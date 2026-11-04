//! Gantt toolbar, table and keyboard (0.2's gantt/actions.ts and the handlers of GanttView and
//! GanttTable). Dragging: drag.rs.

use fluentcal_core::i18n::{tr};

use super::model::RH;
use crate::app::App;
use crate::{GRow, GanttData};
use fluentcal_core::date::{add_days, diff_days, now_ms};
use fluentcal_core::gantt::UNGROUPED;
use fluentcal_core::model::Task;
use fluentcal_core::tasks;
use serde_json::json;
use slint::ComponentHandle;

const SCALES: [&str; 3] = ["month", "week", "day"];

impl App {
    fn gd(&self) -> GanttData<'_> {
        self.desk.window.global::<GanttData>()
    }

    pub fn gantt_set_scale(&mut self, scale: &str) {
        self.patch_settings(json!({ "gantt": { "scale": scale } }));
    }

    /// Ctrl+wheel: one step finer (1) or coarser (-1).
    pub fn gantt_zoom(&mut self, dir: i32) {
        let i = SCALES.iter().position(|s| *s == self.doc.settings.gantt.scale).unwrap_or(2) as i32 + dir;
        if (0..3).contains(&i) {
            self.gantt_set_scale(SCALES[i as usize]);
        }
    }

    pub fn gantt_go_today(&mut self) {
        let today = self.today();
        self.gantt_glide_to(&today, 0.3);
    }

    pub fn gantt_select(&mut self, id: &str) {
        self.desk.gantt.selected = id.to_owned();
        self.gd().set_selected(id.into());
    }

    /// Scrolls horizontally only when the date is outside the visible chart.
    fn gantt_reveal(&mut self, date: &str) {
        let Some(m) = &self.desk.gantt.model else { return };
        let x = diff_days(&m.range.0, date) as f64 * m.dw;
        let sx = -f64::from(self.gd().get_scroll_x());
        let w = f64::from(self.desk.window.get_gantt_view_w());
        if x < sx || x > sx + w - 40.0 {
            self.gantt_glide_to(date, 0.2);
        }
    }

    pub fn gantt_row_click(&mut self, r: &GRow) {
        self.gantt_select(&r.id);
        if let Some(start) = self.doc.tasks.get(r.id.as_str()).map(|t| t.start.clone()) {
            self.gantt_reveal(&start);
        }
    }

    pub fn gantt_row_check(&mut self, r: &GRow) {
        self.history.boundary();
        let Some(t) = self.doc.tasks.get(r.id.as_str()).cloned() else { return };
        let edit = tasks::set_occurrence_done(&self.doc, &t, &t.start, !t.done, now_ms());
        self.apply(edit, true);
    }

    pub fn gantt_toggle_group(&mut self, id: &str) {
        if id == UNGROUPED {
            self.desk.gantt.ungrouped_collapsed = !self.desk.gantt.ungrouped_collapsed;
            self.gantt_refresh();
        } else if let Some(g) = self.doc.groups.get(id).cloned() {
            self.history.boundary();
            let collapsed = g.collapsed != Some(true);
            let edit = tasks::save_group(&self.doc, fluentcal_core::model::Group { collapsed: Some(collapsed), ..g });
            self.apply(edit, true);
        }
    }

    /// Inline rename in the table: `t:<id>` or `g:<id>`.
    pub fn gantt_start_rename(&mut self, key: &str) {
        let (kind, id) = key.split_at(2);
        if self.desk.window.get_read_only() || !self.doc.settings.gantt.show_table {
            if kind == "t:" {
                self.open_editor_task(id, None);
            }
            return;
        }
        let text = if kind == "t:" { self.doc.tasks.get(id).map(|t| t.title.clone()) } else { self.doc.groups.get(id).map(|g| g.name.clone()) };
        let gd = self.gd();
        gd.set_rename_text(text.unwrap_or_default().into());
        gd.set_renaming(key.into());
    }

    pub fn gantt_commit_rename(&mut self) {
        let gd = self.gd();
        let key = gd.get_renaming().to_string();
        let v = gd.get_rename_text().trim().to_owned();
        gd.set_renaming("".into());
        if key.len() < 2 || v.is_empty() {
            return;
        }
        self.history.boundary();
        let (kind, id) = key.split_at(2);
        let edit = match kind {
            "t:" => self.doc.tasks.get(id).filter(|t| t.title != v).map(|t| tasks::save_task(&self.doc, Task { title: v, ..t.clone() }, now_ms())),
            _ => self.doc.groups.get(id).filter(|g| g.name != v).map(|g| tasks::save_group(&self.doc, fluentcal_core::model::Group { name: v, ..g.clone() })),
        };
        if let Some(e) = edit {
            self.apply(e, true);
        }
    }

    pub fn gantt_cancel_rename(&mut self) {
        self.gd().set_renaming("".into());
        self.desk.window.invoke_focus_root();
    }

    /// A new task in a group, then its title is edited. Ungrouped tasks get ≥ 2 days so they
    /// stay visible (single-day ungrouped tasks are hidden by default).
    pub fn gantt_create(&mut self, group: &str, start: &str, mut end: String, milestone: bool) {
        let gid = (group != UNGROUPED).then(|| group.to_owned());
        if gid.is_none() && !milestone && end == start {
            end = add_days(start, 1);
        }
        let p = Task {
            title: if milestone { tr("新里程碑") } else { tr("新任务") }.into(),
            start: start.into(),
            end: if milestone { start.into() } else { end },
            group_id: gid,
            milestone: milestone.then_some(true),
            ..Default::default()
        };
        self.history.boundary();
        let (edit, t) = tasks::add_task(&self.doc, p, None, now_ms());
        self.apply(edit, true);
        self.gantt_select(&t.id);
        self.gantt_ensure_row(&format!("t:{}", t.id));
        self.gantt_start_rename(&format!("t:{}", t.id));
    }

    /// "添加任务": a few days right after the group's last task (or today).
    pub fn gantt_add_in_group(&mut self, group: &str, milestone: bool) {
        let last = self.doc.tasks.values().filter(|t| t.group_id.as_deref().unwrap_or(UNGROUPED) == group).map(|t| t.end.clone()).max();
        let start = last.map_or_else(|| self.today(), |d| add_days(&d, 1));
        let len = match self.doc.settings.gantt.scale.as_str() {
            "day" => 2,
            "week" => 6,
            _ => 13,
        };
        self.gantt_create(group, &start, add_days(&start, len), milestone);
    }

    pub fn gantt_new_group(&mut self) {
        let palette = ["blue", "purple", "teal", "orange", "pink", "green", "indigo", "red"];
        let g = tasks::make_group(&self.doc, tr("新分组"), palette[self.doc.groups.len() % palette.len()]);
        self.history.boundary();
        let edit = tasks::save_group(&self.doc, g.clone());
        self.apply(edit, true);
        if !self.doc.settings.gantt.group_filter.is_empty() {
            self.patch_settings(json!({ "gantt": { "groupFilter": [] } }));
        }
        self.gantt_ensure_row(&format!("g:{}", g.id));
        self.gantt_start_rename(&format!("g:{}", g.id));
    }

    /// Scrolls the chart vertically so the row is visible.
    pub fn gantt_ensure_row(&mut self, key: &str) {
        let Some(i) = self.desk.gantt.model.as_ref().and_then(|m| m.rows.iter().position(|r| r.key == key)) else { return };
        let gd = self.gd();
        let (top, h) = (i as f64 * RH, f64::from(self.desk.window.get_gantt_view_h()));
        let sy = -f64::from(gd.get_scroll_y());
        if top < sy {
            gd.set_scroll_y(-(top - RH).max(0.0) as f32);
        } else if top + RH > sy + h {
            gd.set_scroll_y(-(top + RH * 2.0 - h).max(0.0) as f32);
        }
    }
}
