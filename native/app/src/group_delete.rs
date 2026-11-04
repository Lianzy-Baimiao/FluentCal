//! 删除分组 (ui/group_delete.slint): one dialog deletes one or several groups. It lists every
//! group with a check box; the group it was opened from (the Gantt group menu, the delete
//! button in 设置 › 甘特图) is ticked. The desk and the settings window each have one; the
//! ticks are kept here, in the window's state.

use crate::app::{run, App};
use crate::{GroupDelete, GroupPick};
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

/// The rows (in the Gantt chart's order), the title and whether 删除 is enabled. Ticks of
/// groups that are gone are dropped from `checked` first.
pub fn view(doc: &Doc, checked: &mut Vec<String>, t: &Tokens) -> (Vec<GroupPick>, String, bool) {
    checked.retain(|id| doc.groups.contains_key(id));
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
                checked: checked.contains(&g.id),
            }
        })
        .collect();
    let title = match checked.as_slice() {
        [] => tr("删除分组").to_owned(),
        [one] => trf("删除分组“{0}”？", &[doc.groups[one].name.clone()]),
        many => trf("删除 {0} 个分组？", &[many.len().to_string()]),
    };
    (rows, title, !checked.is_empty())
}

/// Shows `view` in the window's GroupDelete global; rows are rewritten in place, so a ticked
/// box keeps the keyboard focus.
pub fn push<'a, C>(window: &'a C, doc: &Doc, checked: &mut Vec<String>, t: &Tokens)
where
    C: ComponentHandle,
    GroupDelete<'a>: Global<'a, C>,
{
    let (rows, title, can) = view(doc, checked, t);
    let g = window.global::<GroupDelete>();
    g.set_title(title.into());
    g.set_can_delete(can);
    crate::desk::sync::rows(&g.get_items(), rows, |m| g.set_items(m));
}

/// Connects the window's dialog to the app.
pub fn wire<'a, C>(window: &'a C, host: Host)
where
    C: ComponentHandle,
    GroupDelete<'a>: Global<'a, C>,
{
    let g = window.global::<GroupDelete>();
    g.on_toggled(move |id, on| run(move |a| a.group_delete_toggle(host, &id, on)));
    g.on_answer(move |r| run(move |a| a.group_delete_answer(host, &r)));
}

impl App {
    /// Opens 删除分组 in `host` with group `id` ticked.
    pub fn open_group_delete(&mut self, host: Host, id: &str) {
        if !self.doc.groups.contains_key(id) {
            return;
        }
        let mut checked = vec![id.to_owned()];
        match host {
            Host::Desk => {
                push(&self.desk.window, &self.doc, &mut checked, &self.tokens);
                self.desk.popups.group_delete = Some(checked);
                // The dialog takes Enter / Esc: the calendar needs the keyboard.
                self.desk_wants_keyboard();
                self.desk.window.invoke_open_group_delete();
            }
            Host::Settings => {
                let Some(s) = &mut self.settings_win else { return };
                push(&s.window, &self.doc, &mut checked, &self.tokens);
                s.local.group_delete = Some(checked);
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
                if let Some(c) = &mut self.desk.popups.group_delete {
                    push(&self.desk.window, doc, c, t);
                }
            }
            Host::Settings => {
                if let Some(s) = &mut self.settings_win {
                    if let Some(c) = &mut s.local.group_delete {
                        push(&s.window, doc, c, t);
                    }
                }
            }
        }
    }

    fn group_delete_ticks(&mut self, host: Host) -> Option<&mut Vec<String>> {
        match host {
            Host::Desk => self.desk.popups.group_delete.as_mut(),
            Host::Settings => self.settings_win.as_mut()?.local.group_delete.as_mut(),
        }
    }

    fn group_delete_toggle(&mut self, host: Host, id: &str, on: bool) {
        let Some(c) = self.group_delete_ticks(host) else { return };
        c.retain(|x| x != id);
        if on {
            c.push(id.to_owned());
        }
        self.group_delete_refresh(host);
    }

    /// 删除 deletes the ticked groups (on the desk one undo step; the settings window has no
    /// undo, as before); closing the dialog only forgets the ticks.
    fn group_delete_answer(&mut self, host: Host, r: &str) {
        let Some(ids) = (match host {
            Host::Desk => self.desk.popups.group_delete.take(),
            Host::Settings => self.settings_win.as_mut().and_then(|s| s.local.group_delete.take()),
        }) else {
            return;
        };
        if r != "primary" || ids.is_empty() {
            return;
        }
        let undoable = host == Host::Desk;
        if undoable {
            self.history.boundary();
        }
        let edit = tasks::delete_groups(&self.doc, &ids, fluentcal_core::date::now_ms());
        self.apply(edit, undoable);
        // "只显示分组" on a deleted group would leave the chart empty.
        let filter = &self.doc.settings.gantt.group_filter;
        if filter.iter().any(|g| ids.contains(g)) {
            let kept: Vec<String> = filter.iter().filter(|g| !ids.contains(g)).cloned().collect();
            self.patch_settings(json!({ "gantt": { "groupFilter": kept } }));
        }
    }
}
