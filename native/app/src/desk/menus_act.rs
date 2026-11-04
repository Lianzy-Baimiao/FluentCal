//! What the menu entries do (the actions of 0.2's menus.ts and gantt/actions.ts).

use super::menus::Target;
use crate::app::App;
use fluentcal_core::date::{add_days, now_ms};
use fluentcal_core::model::{Group, Task};
use fluentcal_core::recur::occurrence_on;
use fluentcal_core::tasks::{self, DeleteMode, Edit};
use serde_json::json;

impl App {
    /// Runs entry `id` of the menu opened on `target`.
    pub fn menu_act(&mut self, target: &Target, id: &str) {
        self.history.boundary();
        match target {
            Target::Task(tid, date) => self.task_act(tid, date, id),
            Target::Day(date) => self.day_act(date, id),
            Target::More => self.more_act(id),
            Target::Group(gid) => self.group_act(gid, id),
            Target::GanttOptions => self.gantt_options_act(id),
        }
    }

    fn task_act(&mut self, tid: &str, date: &str, id: &str) {
        let Some(t) = self.doc.tasks.get(tid).cloned() else { return };
        let now = now_ms();
        let edit = match id {
            "done" => {
                let done = occurrence_on(&t, date).map_or(t.done, |o| o.done);
                Some(tasks::set_occurrence_done(&self.doc, &t, date, !done, now))
            }
            "duplicate" => Some(tasks::duplicate_task(&self.doc, &t, now).0),
            "postpone" => Some(tasks::move_task(&self.doc, &t, &add_days(&t.start, 1), now)),
            "autoPostpone" => {
                let on = !t.auto_postpone.unwrap_or(self.doc.settings.tasks.auto_postpone);
                Some(tasks::save_task(&self.doc, Task { auto_postpone: Some(on), ..t.clone() }, now))
            }
            "countdown" => {
                let on = t.countdown != Some(true);
                Some(tasks::save_task(&self.doc, Task { countdown: on.then_some(true), ..t.clone() }, now))
            }
            c if c.starts_with("color:") => Some(tasks::set_color(&self.doc, &t, &c[6..], now)),
            _ => None,
        };
        if let Some(e) = edit {
            return self.apply(e, true);
        }
        match id {
            "edit" => self.open_editor_task(tid, Some(date.to_owned())),
            "gantt" => self.gantt_focus(tid),
            "delete:this" => self.desk_remove_task(&t, Some((date, DeleteMode::This))),
            "delete:future" => self.desk_remove_task(&t, Some((date, DeleteMode::Future))),
            "delete:all" => self.desk_remove_task(&t, Some((date, DeleteMode::All))),
            _ => {}
        }
    }

    fn day_act(&mut self, date: &str, id: &str) {
        match id {
            "quickAdd" => self.desk_quick_add(date),
            "new" => self.open_editor_new(Some(date.to_owned())),
            "unmark" => self.apply(tasks::set_day_mark(date, None), true),
            "today" => self.desk_go_today(),
            "browser" => self.open_in_browser(),
            m if m.starts_with("mark:") => self.apply(tasks::set_day_mark(date, Some(&m[5..])), true),
            _ => {}
        }
    }

    fn more_act(&mut self, id: &str) {
        match id {
            "new" => self.open_editor_new(None),
            "search" => self.desk_search(),
            "syncNow" => self.cloud.request(),
            "browser" => self.open_in_browser(),
            "showCompleted" => {
                let v = !self.doc.settings.tasks.show_completed;
                self.patch_settings(json!({ "tasks": { "showCompleted": v } }));
            }
            "lock" => self.desk_toggle_lock(),
            "appearance" => self.open_settings(Some("appearance")),
            "settings" => self.open_settings(None),
            "hide" => {
                let _ = slint::ComponentHandle::hide(&self.desk.window);
                crate::shell::tray_refresh(self);
            }
            "quit" => crate::startup::quit(self),
            x => {
                if let Some(m) = x.strip_prefix("theme:") {
                    self.patch_settings(json!({ "theme": { "mode": m } }));
                } else if let Some(p) = x.strip_prefix("pin:") {
                    self.patch_settings(json!({ "desk": { "pinMode": p } }));
                } else if let Some(i) = x.strip_prefix("screen:").and_then(|i| i.parse().ok()) {
                    crate::shell::move_to_screen(self, i);
                }
            }
        }
    }

    fn group_act(&mut self, gid: &str, id: &str) {
        match id {
            "addTask" => self.gantt_add_in_group(gid, false),
            "addMilestone" => self.gantt_add_in_group(gid, true),
            "rename" => self.gantt_start_rename(&format!("g:{gid}")),
            "up" => self.gantt_move_group(gid, -1),
            "down" => self.gantt_move_group(gid, 1),
            "deleteGroup" => self.open_group_delete(crate::group_delete::Host::Desk, gid),
            c if c.starts_with("groupColor:") => {
                if let Some(g) = self.doc.groups.get(gid).cloned() {
                    let e = tasks::save_group(&self.doc, Group { color: c[11..].to_owned(), ..g });
                    self.apply(e, true);
                }
            }
            _ => {}
        }
    }

    fn gantt_options_act(&mut self, id: &str) {
        let g = self.doc.settings.gantt.clone();
        match id {
            "showTable" => self.patch_settings(json!({ "gantt": { "showTable": !g.show_table } })),
            "showDeps" => self.patch_settings(json!({ "gantt": { "showDeps": !g.show_deps } })),
            "hideDone" => self.patch_settings(json!({ "gantt": { "hideDone": !g.hide_done } })),
            "hideSingleDay" => self.patch_settings(json!({ "gantt": { "hideSingleDay": !g.hide_single_day } })),
            "export:png" => self.gantt_export("png"),
            "export:svg" => self.gantt_export("svg"),
            f if f.starts_with("filter:") => {
                let list: Vec<&str> = if f.len() > 7 { vec![&f[7..]] } else { Vec::new() };
                self.patch_settings(json!({ "gantt": { "groupFilter": list } }));
            }
            _ => {}
        }
    }

    /// 上移 / 下移: swaps the order with the neighbouring group.
    pub fn gantt_move_group(&mut self, gid: &str, dir: i32) {
        let mut list: Vec<Group> = self.doc.groups.values().cloned().collect();
        list.sort_by(|a, b| a.order.total_cmp(&b.order));
        let Some(i) = list.iter().position(|g| g.id == gid) else { return };
        let Some(j) = i.checked_add_signed(dir as isize).filter(|j| *j < list.len()) else { return };
        let (a, b) = (list[i].clone(), list[j].clone());
        let mut e = Edit::default();
        e.put("groups", &a.id, Group { order: b.order, ..a.clone() }.to_value());
        let bid = b.id.clone();
        e.put("groups", &bid, Group { order: a.order, ..b }.to_value());
        self.apply(e, true);
    }

    /// "在甘特图中查看" (0.2's focusTask): the task's group open, its row selected, its start
    /// in view.
    pub fn gantt_focus(&mut self, id: &str) {
        let Some(t) = self.doc.tasks.get(id).cloned() else { return };
        if let Some(g) = t.group_id.as_deref().and_then(|g| self.doc.groups.get(g)).filter(|g| g.collapsed == Some(true)).cloned() {
            let e = tasks::save_group(&self.doc, Group { collapsed: Some(false), ..g });
            self.apply(e, true);
        }
        if t.group_id.is_none() {
            self.desk.gantt.ungrouped_collapsed = false;
        }
        let f = &self.doc.settings.gantt.group_filter;
        if !f.is_empty() && !f.contains(&t.group_id.clone().unwrap_or_default()) {
            self.patch_settings(json!({ "gantt": { "groupFilter": [] } }));
        }
        if self.doc.settings.calendar.view != "gantt" {
            self.patch_settings(json!({ "calendar": { "view": "gantt" } }));
        }
        self.gantt_select(id);
        self.gantt_scroll_to(&t.start, 0.3);
        self.gantt_ensure_row(&format!("t:{id}"));
    }
}
