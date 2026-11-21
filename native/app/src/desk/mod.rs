//! The desk calendar window: view models (vm), the live window (live, wire, actions) and an
//! offscreen helper for the parity snapshots (`Desk`).

pub mod actions;
pub mod filter;
pub mod gantt;
pub mod input;
pub mod live;
pub mod menus;
pub mod menus_act;
pub mod menus_build;
pub mod overlays;
pub mod popups;
pub mod reorder;
pub mod sync;
pub mod vm;
pub mod wire;

use crate::DeskWindow;
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::Tokens;
use slint::ComponentHandle;
use vm::DeskUi;

/// Sets everything the window shows that comes from the document and the view state.
pub fn push_all(w: &DeskWindow, doc: &Doc, ui: &DeskUi, today: &str, room: Option<vm::Room>, t: &Tokens) {
    let c = &doc.settings.calendar;
    w.set_view(c.view.clone().into());
    w.set_locked(doc.settings.desk.locked);
    w.set_title_text(vm::view_title(doc, ui, today).into());
    w.set_today_tip(vm::today_tip(today).into());
    w.set_heads(vm::weekday_heads(doc));
    w.set_show_week_no(c.show_week_numbers);
    w.set_selected(ui.selected.clone().into());
    w.set_flash_date(ui.flash.clone().into());
    w.set_flash_end(ui.flash_end.clone().into());
    w.set_flash_task(ui.flash_task.clone().into());
    w.global::<crate::ChipKeys>().set_located_task(ui.flash_task.clone().into());
    push_weeks(w, doc, ui, today, room, t);
}

/// The week rows, the list view and the countdowns (the parts that follow tasks).
pub fn push_weeks(w: &DeskWindow, doc: &Doc, ui: &DeskUi, today: &str, room: Option<vm::Room>, t: &Tokens) {
    use slint::Model;
    let weeks: Vec<crate::WeekData> = vm::weeks(doc, ui, today, room, t).iter().collect();
    sync::weeks(&w.get_weeks(), weeks, |m| w.set_weeks(m));
    // the group filter's button: red, and named after what it shows, while it filters
    w.set_filter_on(!ui.group_filter.is_empty());
    w.set_filter_tip(vm::filter_tip(doc, ui).into());
    push_list(w, doc, ui, today, t);
    let side = doc.settings.calendar.sidebar;
    w.set_side_open(side);
    w.set_side_mode(doc.settings.calendar.sidebar_mode.as_str().into());
    let rows = if side { vm::side_rows(doc, ui, today, t) } else { Vec::new() };
    let add_row = |zone: &str| rows.iter().position(|r| r.kind == "add" && r.zone == zone).unwrap_or(0) as i32;
    w.set_side_add_month_row(add_row("month"));
    w.set_side_add_someday_row(add_row("someday"));
    sync::rows(&w.get_side_rows(), rows, |m| w.set_side_rows(m));
    let cds: Vec<crate::CountdownData> = vm::countdown_items(doc, today).iter().collect();
    sync::rows(&w.get_countdowns(), cds.iter().take(6).cloned().collect(), |m| w.set_countdowns(m));
    sync::rows(&w.get_all_countdowns(), cds, |m| w.set_all_countdowns(m));
}

/// The list view's rows; none while another view shows (nothing kept for it).
pub fn push_list(w: &DeskWindow, doc: &Doc, ui: &DeskUi, today: &str, t: &Tokens) {
    let c = &doc.settings.calendar;
    w.set_list_range(if ui.reveal_all { "all".into() } else { c.list_range.clone().into() });
    let (rows, today_row, footer) = if c.view == "list" { vm::list_rows(doc, ui, today, t) } else { (Vec::new(), 0, String::new()) };
    sync::rows(&w.get_list_rows(), rows, |m| w.set_list_rows(m));
    w.set_list_today_row(today_row);
    w.set_list_footer(footer.into());
}

/// What the window's layout gives the week rows now; the rest of the room only for rows that
/// follow their tasks (equal rows aren't laid out again on every resize).
pub fn room_of(w: &DeskWindow, fit: bool) -> Option<vm::Room> {
    let capacity = w.get_capacity().max(0) as usize;
    Some(if fit { vm::Room { capacity, grid_h: w.get_rows_room(), top: w.get_lane_top(), lane: w.get_lane_step(), gap: w.get_cell_gap() } } else { vm::Room { capacity, ..Default::default() } })
}

/// A week row's top and height in window px: its own when the rows follow their tasks, else
/// row `r` of equal rows `ch` high.
pub fn row_box(week: &crate::WeekData, r: usize, gy: f32, ch: f32, gap: f32) -> (f32, f32) {
    if week.row_h > 0.0 {
        (gy + week.row_y, week.row_h)
    } else {
        (gy + r as f32 * (ch + gap), ch)
    }
}

/// The desk window drawn offscreen with fixed data (examples/snapshot.rs).
pub struct Desk {
    pub window: DeskWindow,
    pub doc: Doc,
    pub ui: DeskUi,
    pub today: String,
    pub tokens: Tokens,
    /// Task lanes that fit in a week row (from the window's layout).
    pub room: Option<vm::Room>,
    pub gantt: gantt::GanttUi,
}

impl Desk {
    pub fn new(window: DeskWindow, doc: Doc, today: String, tokens: Tokens) -> Desk {
        let ui = DeskUi::new(&doc, &today);
        let gantt = gantt::GanttUi { center: Some((today.clone(), 0.3)), ..Default::default() };
        Desk { window, doc, ui, today, tokens, room: None, gantt }
    }

    fn gantt_view(&mut self) -> gantt::View<'_> {
        gantt::View { window: &self.window, doc: &self.doc, tokens: &self.tokens, today: &self.today, g: &mut self.gantt }
    }

    pub fn refresh(&mut self) {
        push_all(&self.window, &self.doc, &self.ui, &self.today, self.room, &self.tokens);
        self.window.set_welcome(overlays::welcome(&self.doc, false));
        if self.doc.settings.calendar.view == "gantt" {
            self.gantt_view().push();
        }
    }

    /// Opens a flyout or panel as the parity scene says (`{"open": "dayList" | "ym" | "search"
    /// | "confirm", "anchor": [x, y, w, h], …}`), with the same view models as the app.
    /// "sideDrag": a to-do of the column held over another place (`row` is dragged, the line
    /// is above `drop`, the pointer is at the anchor's x, y).
    pub fn open(&mut self, spec: &serde_json::Value) {
        let s = |k: &str| spec.get(k).and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
        let a: Vec<f32> = spec.get("anchor").and_then(serde_json::Value::as_array).map_or(vec![0.0; 4], |v| v.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect());
        // 按分组筛选 with the groups `groups` (ids, "" = 未分组) ticked; `closed`: only the
        // filtered calendar and its red button.
        if s("open") == "filter" {
            let ids = spec.get("groups").and_then(serde_json::Value::as_array);
            self.ui.group_filter = ids.map_or(Vec::new(), |v| v.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect());
            self.refresh_weeks();
        }
        let w = &self.window;
        match s("open").as_str() {
            "locate" => {
                w.set_flash_date(s("date").into());
                w.set_flash_end(s("end").into());
                w.set_flash_task(s("task").into());
            }
            "dayList" => {
                let (title, lunar, chips) = overlays::day_list(&self.doc, &self.ui, &s("date"), &self.today, &self.tokens);
                // `drag`: the task in row `row` is on its way to the day `over`, the pointer at
                // `at` (CAL-10: the list is out of sight meanwhile).
                if let Some(d) = spec.get("drag") {
                    use slint::Model;
                    let at: Vec<f32> = d.get("at").and_then(serde_json::Value::as_array).map_or(vec![0.0; 2], |v| v.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect());
                    let row = d.get("row").and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
                    if let Some(c) = chips.row_data(row) {
                        w.set_dragging_key(c.key.clone());
                        w.set_drag_label(c.title.clone());
                        w.set_drag_x(at[0]);
                        w.set_drag_y(at[1]);
                        w.set_drop_date(d.get("over").and_then(serde_json::Value::as_str).unwrap_or("").into());
                        w.set_drag_active(true);
                    }
                }
                w.set_day_list_title(title.into());
                w.set_day_list_lunar(lunar.into());
                w.set_day_list_chips(chips);
                w.invoke_open_day_list(a[0], a[1], a[2], a[3]);
            }
            "ym" => {
                w.set_ym_year(fluentcal_core::date::parts(&self.ui.cursor).0);
                w.set_ym_cursor(self.ui.cursor[..7].into());
                w.set_ym_now(self.today[..7].into());
                w.set_ym_now_year(fluentcal_core::date::parts(&self.today).0);
                w.invoke_open_ym(a[0], a[1], a[2], a[3]);
            }
            "search" => {
                let o = fluentcal_core::search::SearchOptions { q: s("query"), range: "all".into(), status: "all".into(), color: String::new() };
                let (hits, empty) = overlays::search(&self.doc, &o, &self.today, &self.tokens);
                w.set_search_colors(overlays::search_colors(&self.tokens));
                w.set_search_query(o.q.into());
                w.set_search_hits(hits);
                w.set_search_empty(empty.into());
                w.set_search_open(true);
            }
            // The sync button: "static" (the glyph) or "spin" (frame `frame` of the turn).
            "sync" => {
                w.set_sync_enabled(true);
                if s("state") == "spin" {
                    let i = spec.get("frame").and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
                    let f = crate::spin::frames('\u{E895}');
                    let one: Vec<slint::SharedString> = f.get(i).map(|x| x.as_str().into()).into_iter().collect();
                    w.set_spin_frames(slint::ModelRc::new(slint::VecModel::from(one)));
                    w.set_sync_busy(true);
                }
            }
            "confirm" => {
                w.set_confirm_title(s("title").into());
                w.set_confirm_text(s("text").into());
                w.set_confirm_primary(s("primary").into());
                w.set_confirm_danger(true);
                w.invoke_open_confirm();
            }
            // 删除分组 with the groups `checked` (ids) ticked; `keepTasks` unticks 同时删除.
            "groupDelete" => {
                let ids = spec.get("checked").and_then(serde_json::Value::as_array);
                let checked: Vec<String> = ids.map_or(Vec::new(), |v| v.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect());
                let with_tasks = !spec.get("keepTasks").and_then(serde_json::Value::as_bool).unwrap_or(false);
                let mut picks = crate::group_delete::Picks { checked, with_tasks };
                crate::group_delete::push(w, &self.doc, &mut picks, &self.tokens);
                w.invoke_open_group_delete();
            }
            "filter" => {
                let (rows, all, hint) = overlays::filter_rows(&self.doc, &self.ui, &self.tokens);
                w.set_filter_rows(slint::ModelRc::new(slint::VecModel::from(rows)));
                w.set_filter_all_count(all.into());
                w.set_filter_hint(hint.into());
                if !spec.get("closed").and_then(serde_json::Value::as_bool).unwrap_or(false) {
                    w.invoke_open_filter(a[0], a[1], a[2], a[3]);
                }
            }
            "sideDrag" => {
                use slint::Model;
                let n = |k: &str| spec.get(k).and_then(serde_json::Value::as_i64).unwrap_or(-1);
                if let Some(r) = usize::try_from(n("row")).ok().and_then(|i| w.get_side_rows().row_data(i)) {
                    w.set_side_drag_key(r.chip.key.clone());
                    w.set_drag_label(r.chip.title.clone());
                    w.set_drag_active(true);
                    w.set_drag_x(a[0]);
                    w.set_drag_y(a[1]);
                    w.set_side_drop_row(n("drop") as i32);
                }
            }
            other => eprintln!("unknown overlay {other}"),
        }
    }

    pub fn refresh_weeks(&mut self) {
        push_weeks(&self.window, &self.doc, &self.ui, &self.today, self.room, &self.tokens);
    }

    /// Re-lays the chips out if the lane capacity changed; true when it did. (Also applies the
    /// Gantt view's first scroll, which needs the laid-out size.)
    pub fn sync_capacity(&mut self) -> bool {
        if self.doc.settings.calendar.view == "gantt" && self.gantt.center.is_some() {
            self.gantt_view().scrolled();
            return true;
        }
        let room = room_of(&self.window, self.doc.settings.calendar.fit_rows);
        if room == self.room {
            return false;
        }
        self.room = room;
        self.refresh_weeks();
        true
    }

    pub fn show(&self) -> Result<(), slint::PlatformError> {
        self.window.show()
    }
}
