//! 按分组筛选 (GitHub #14): the title bar's filter button and its list. The month, multi-week
//! and list views, the "+N" list and the to-do column show only the tasks of the ticked
//! groups (several can be ticked; 未分组 is one of the choices) until 全部 is chosen again. It is
//! for a look, so it is kept for the session only, and the button is red while it filters.
//! The Gantt chart has its own 只显示分组; search and the countdowns always cover everything.

use fluentcal_core::i18n::trf;

use super::overlays;
use super::popups::At;
use crate::app::App;

impl App {
    /// The button (or 更多 › 按分组筛选…): the list of groups under it.
    pub fn desk_filter_open(&mut self, at: At) {
        self.desk.popups.filter_open = true;
        self.desk_filter_refresh();
        let w = &self.desk.window;
        match at {
            At::Point(x, y) => w.invoke_open_filter(x, y, 0.0, 0.0),
            At::Below(x, y, bw, bh) => w.invoke_open_filter(x, y, bw, bh),
        }
    }

    /// The list follows the groups and tasks while it is open (nothing is made for it closed).
    pub fn desk_filter_refresh(&mut self) {
        if !self.desk.popups.filter_open {
            return;
        }
        let (rows, all, hint) = overlays::filter_rows(&self.doc, &self.desk.ui, &self.tokens);
        let w = &self.desk.window;
        super::sync::rows(&w.get_filter_rows(), rows, |m| w.set_filter_rows(m));
        w.set_filter_all_count(all.into());
        w.set_filter_hint(hint.into());
    }

    pub fn desk_filter_closed(&mut self) {
        self.desk.popups.filter_open = false;
    }

    /// A group (or 未分组, "") ticked or unticked.
    pub fn desk_filter_toggle(&mut self, id: &str, on: bool) {
        let f = &mut self.desk.ui.group_filter;
        f.retain(|g| g != id);
        if on {
            f.push(id.to_owned());
        }
        // (every choice ticked is 全部 again: vm::tidy_filter, on the refresh)
        self.desk_refresh_data();
    }

    /// 全部.
    pub fn desk_filter_clear(&mut self) {
        self.desk.ui.group_filter.clear();
        self.desk_refresh_data();
    }

    /// 管理分组…: where groups are added, renamed, coloured and deleted.
    pub fn desk_filter_manage(&mut self) {
        self.desk.window.invoke_close_filter();
        self.open_settings(Some("gantt"));
    }

    /// The group a task made on the calendar now belongs to: the one group the calendar shows
    /// alone (the task would vanish as it is made otherwise). None in the Gantt chart, which the
    /// filter isn't for.
    pub fn desk_new_task_group(&self) -> Option<String> {
        if self.doc.settings.calendar.view == "gantt" {
            return None;
        }
        match self.desk.ui.group_filter.as_slice() {
            [g] if !g.is_empty() => Some(g.clone()),
            _ => None,
        }
    }

    /// A task was added that the filter hides (several groups are shown and it is in none):
    /// says so, or it would look as if nothing happened. True when it did.
    pub fn desk_filter_told(&mut self, title: &str, group: Option<&str>) -> bool {
        let f = &self.desk.ui.group_filter;
        if f.is_empty() || f.iter().any(|g| g == group.unwrap_or("")) {
            return false;
        }
        self.notify(&trf("已添加“{0}”（未分组，当前筛选下不显示）", &[title.to_owned()]), false);
        true
    }
}
