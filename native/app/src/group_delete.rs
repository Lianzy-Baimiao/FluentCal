//! 删除分组 (ui/group_delete.slint): one dialog deletes one or several groups. It lists every
//! group with a check box; the group it was opened from (the Gantt group menu, the delete
//! button in 设置 › 甘特图) is ticked. The groups' tasks are deleted with them unless
//! 同时删除分组里的任务 is unticked; then they move to 未分组. The desk and the settings window
//! each have one; the ticks are kept here, in the window's state. Every deletion is one step
//! of the desk's undo history, wherever it was made.

use crate::app::{run, App};
use crate::{GroupDelete, GroupPick, Severity};
use fluentcal_core::i18n::{tr, trf};
use fluentcal_core::tasks::{self, Doc};
use fluentcal_core::theme::{task_color, Tokens};
use serde_json::json;
use slint::{ComponentHandle, Global};

/// The window a dialog is open in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Desk,
    Settings,
}

/// An open dialog's choices: the ticked groups and whether their tasks go too.
#[derive(Debug, Clone, PartialEq)]
pub struct Picks {
    pub checked: Vec<String>,
    pub with_tasks: bool,
}

impl Picks {
    /// The dialog opened from group `id`: it is ticked, and its tasks are deleted with it.
    pub fn new(id: &str) -> Self {
        Picks { checked: vec![id.to_owned()], with_tasks: true }
    }
}

/// What the dialog shows.
pub struct View {
    pub rows: Vec<GroupPick>,
    pub title: String,
    pub can_delete: bool,
    pub with_tasks_label: String,
    pub note: String,
}

/// The rows (in the Gantt chart's order), title, check box and note. Ticks of groups that are
/// gone are dropped from `picks` first.
pub fn view(doc: &Doc, picks: &mut Picks, t: &Tokens) -> View {
    picks.checked.retain(|id| doc.groups.contains_key(id));
    let mut groups: Vec<_> = doc.groups.values().collect();
    groups.sort_by(|a, b| a.order.total_cmp(&b.order));
    let rows = groups
        .iter()
        .map(|g| {
            let n = doc.tasks.values().filter(|x| x.group_id.as_deref() == Some(g.id.as_str())).count();
            GroupPick {
                id: g.id.as_str().into(),
                name: g.name.as_str().into(),
                color: crate::theme::color(task_color(Some(&g.color), t).bar),
                desc: trf("{0} 个任务", &[n.to_string()]).into(),
                checked: picks.checked.contains(&g.id),
            }
        })
        .collect();
    let title = match picks.checked.as_slice() {
        [] => tr("删除分组").to_owned(),
        [one] => trf("删除分组“{0}”？", &[doc.groups[one].name.clone()]),
        many => trf("删除 {0} 个分组？", &[many.len().to_string()]),
    };
    let n = tasks::tasks_in_groups(doc, &picks.checked);
    let with_tasks_label = if n > 0 { trf("同时删除分组里的任务（{0} 个）", &[n.to_string()]) } else { String::new() };
    let note = if n > 0 && !picks.with_tasks { tr("所选分组里的任务会移到“未分组”，不会被删除。").to_owned() } else { String::new() };
    View { rows, title, can_delete: !picks.checked.is_empty(), with_tasks_label, note }
}

/// Shows `view` in the window's GroupDelete global; rows are rewritten in place, so a ticked
/// box keeps the keyboard focus.
pub fn push<'a, C>(window: &'a C, doc: &Doc, picks: &mut Picks, t: &Tokens)
where
    C: ComponentHandle,
    GroupDelete<'a>: Global<'a, C>,
{
    let v = view(doc, picks, t);
    let g = window.global::<GroupDelete>();
    g.set_title(v.title.into());
    g.set_can_delete(v.can_delete);
    g.set_with_tasks(picks.with_tasks);
    g.set_with_tasks_label(v.with_tasks_label.into());
    g.set_note(v.note.into());
    crate::desk::sync::rows(&g.get_items(), v.rows, |m| g.set_items(m));
}

/// Connects the window's dialog to the app.
pub fn wire<'a, C>(window: &'a C, host: Host)
where
    C: ComponentHandle,
    GroupDelete<'a>: Global<'a, C>,
{
    let g = window.global::<GroupDelete>();
    g.on_toggled(move |id, on| run(move |a| a.group_delete_toggle(host, &id, on)));
    g.on_with_tasks_toggled(move |on| run(move |a| a.group_delete_with_tasks(host, on)));
    g.on_answer(move |r| run(move |a| a.group_delete_answer(host, &r)));
}

impl App {
    /// Opens 删除分组 in `host` with group `id` ticked.
    pub fn open_group_delete(&mut self, host: Host, id: &str) {
        if !self.doc.groups.contains_key(id) {
            return;
        }
        let mut picks = Picks::new(id);
        match host {
            Host::Desk => {
                push(&self.desk.window, &self.doc, &mut picks, &self.tokens);
                self.desk.popups.group_delete = Some(picks);
                // The dialog takes Enter / Esc: the calendar needs the keyboard.
                self.desk_wants_keyboard();
                self.desk.window.invoke_open_group_delete();
            }
            Host::Settings => {
                let Some(s) = &mut self.settings_win else { return };
                push(&s.window, &self.doc, &mut picks, &self.tokens);
                s.local.group_delete = Some(picks);
                s.window.invoke_open_group_delete();
            }
        }
    }

    /// The data, language or theme changed: an open dialog follows (groups deleted or added
    /// elsewhere, e.g. by sync).
    pub fn group_delete_refresh(&mut self, host: Host) {
        let (doc, t) = (&self.doc, &self.tokens);
        match host {
            Host::Desk => {
                if let Some(p) = &mut self.desk.popups.group_delete {
                    push(&self.desk.window, doc, p, t);
                }
            }
            Host::Settings => {
                if let Some(s) = &mut self.settings_win {
                    if let Some(p) = &mut s.local.group_delete {
                        push(&s.window, doc, p, t);
                    }
                }
            }
        }
    }

    fn group_delete_picks(&mut self, host: Host) -> Option<&mut Picks> {
        match host {
            Host::Desk => self.desk.popups.group_delete.as_mut(),
            Host::Settings => self.settings_win.as_mut()?.local.group_delete.as_mut(),
        }
    }

    fn group_delete_toggle(&mut self, host: Host, id: &str, on: bool) {
        let Some(p) = self.group_delete_picks(host) else { return };
        p.checked.retain(|x| x != id);
        if on {
            p.checked.push(id.to_owned());
        }
        self.group_delete_refresh(host);
    }

    fn group_delete_with_tasks(&mut self, host: Host, on: bool) {
        let Some(p) = self.group_delete_picks(host) else { return };
        p.with_tasks = on;
        self.group_delete_refresh(host);
    }

    /// 删除 deletes the ticked groups (and their tasks) as one undo step of the desk, which
    /// also offers 撤销 in its toast; closing the dialog only forgets the ticks.
    fn group_delete_answer(&mut self, host: Host, r: &str) {
        let Some(p) = (match host {
            Host::Desk => self.desk.popups.group_delete.take(),
            Host::Settings => self.settings_win.as_mut().and_then(|s| s.local.group_delete.take()),
        }) else {
            return;
        };
        if r != "primary" || p.checked.is_empty() {
            return;
        }
        let ids = p.checked;
        let names: Vec<String> = ids.iter().filter_map(|id| self.doc.groups.get(id)).map(|g| g.name.clone()).collect();
        let n_tasks = if p.with_tasks { tasks::tasks_in_groups(&self.doc, &ids) } else { 0 };
        self.history.boundary();
        let edit = tasks::delete_groups(&self.doc, &ids, p.with_tasks, fluentcal_core::date::now_ms());
        self.apply(edit, true);
        // "只显示分组" on a deleted group would leave the chart empty.
        let filter = &self.doc.settings.gantt.group_filter;
        if filter.iter().any(|g| ids.contains(g)) {
            let kept: Vec<String> = filter.iter().filter(|g| !ids.contains(g)).cloned().collect();
            self.patch_settings(json!({ "gantt": { "groupFilter": kept } }));
        }
        if n_tasks > 0 {
            let text = match names.as_slice() {
                [one] => {
                    let short: String = one.chars().take(16).collect();
                    let name = if one.chars().count() > 16 { format!("{short}…") } else { short };
                    trf("已删除分组“{0}”和其中的任务（{1} 个）", &[name, n_tasks.to_string()])
                }
                many => trf("已删除 {0} 个分组和其中的任务（{1} 个）", &[many.len().to_string(), n_tasks.to_string()]),
            };
            self.notify(&text, true);
            if host == Host::Settings {
                self.settings_say(Severity::Success, &text);
            }
        }
    }
}
