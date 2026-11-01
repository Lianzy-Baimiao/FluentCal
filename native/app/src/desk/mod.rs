//! The desk calendar window: view models (vm), the live window (live, wire, actions) and an
//! offscreen helper for the parity snapshots (`Desk`).

pub mod actions;
pub mod gantt;
pub mod input;
pub mod live;
pub mod menus;
pub mod menus_act;
pub mod menus_build;
pub mod overlays;
pub mod popups;
pub mod sync;
pub mod vm;
pub mod wire;

use crate::DeskWindow;
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::Tokens;
use slint::ComponentHandle;
use vm::DeskUi;

/// Sets everything the window shows that comes from the document and the view state.
pub fn push_all(w: &DeskWindow, doc: &Doc, ui: &DeskUi, today: &str, capacity: Option<usize>, t: &Tokens) {
    let c = &doc.settings.calendar;
    w.set_view(c.view.clone().into());
    w.set_locked(doc.settings.desk.locked);
    w.set_title_text(vm::view_title(doc, ui, today).into());
    w.set_today_tip(vm::today_tip(today).into());
    w.set_heads(vm::weekday_heads(doc));
    w.set_show_week_no(c.show_week_numbers);
    w.set_selected(ui.selected.clone().into());
    w.set_flash_date(ui.flash.clone().into());
    push_weeks(w, doc, ui, today, capacity, t);
}

/// The week rows and the countdowns (the parts that follow tasks).
pub fn push_weeks(w: &DeskWindow, doc: &Doc, ui: &DeskUi, today: &str, capacity: Option<usize>, t: &Tokens) {
    use slint::Model;
    let weeks: Vec<crate::WeekData> = vm::weeks(doc, ui, today, capacity, t).iter().collect();
    sync::weeks(&w.get_weeks(), weeks, |m| w.set_weeks(m));
    let cds: Vec<crate::CountdownData> = vm::countdown_items(doc, today).iter().collect();
    sync::rows(&w.get_countdowns(), cds, |m| w.set_countdowns(m));
}

/// Lane capacity the window's layout gives now.
pub fn capacity_of(w: &DeskWindow) -> Option<usize> {
    Some(w.get_capacity().max(0) as usize)
}

/// The desk window drawn offscreen with fixed data (examples/snapshot.rs).
pub struct Desk {
    pub window: DeskWindow,
    pub doc: Doc,
    pub ui: DeskUi,
    pub today: String,
    pub tokens: Tokens,
    /// Task lanes that fit in a week row (from the window's layout).
    pub capacity: Option<usize>,
    pub gantt: gantt::GanttUi,
}

impl Desk {
    pub fn new(window: DeskWindow, doc: Doc, today: String, tokens: Tokens) -> Desk {
        let ui = DeskUi::new(&doc, &today);
        let gantt = gantt::GanttUi { center: Some((today.clone(), 0.3)), ..Default::default() };
        Desk { window, doc, ui, today, tokens, capacity: None, gantt }
    }

    fn gantt_view(&mut self) -> gantt::View<'_> {
        gantt::View { window: &self.window, doc: &self.doc, tokens: &self.tokens, today: &self.today, g: &mut self.gantt }
    }

    pub fn refresh(&mut self) {
        push_all(&self.window, &self.doc, &self.ui, &self.today, self.capacity, &self.tokens);
        self.window.set_welcome(overlays::welcome(&self.doc, false));
        if self.doc.settings.calendar.view == "gantt" {
            self.gantt_view().push();
        }
    }

    /// Opens a flyout or panel as the parity scene says (`{"open": "dayList" | "ym" | "search"
    /// | "confirm", "anchor": [x, y, w, h], …}`), with the same view models as the app.
    pub fn open(&mut self, spec: &serde_json::Value) {
        let s = |k: &str| spec.get(k).and_then(serde_json::Value::as_str).unwrap_or("").to_owned();
        let a: Vec<f32> = spec.get("anchor").and_then(serde_json::Value::as_array).map_or(vec![0.0; 4], |v| v.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect());
        let w = &self.window;
        match s("open").as_str() {
            "dayList" => {
                let (title, lunar, chips) = overlays::day_list(&self.doc, &s("date"), &self.today, &self.tokens);
                w.set_day_list_title(title.into());
                w.set_day_list_lunar(lunar.into());
                w.set_day_list_chips(chips);
                w.invoke_open_day_list(a[0], a[1], a[2], a[3]);
            }
            "ym" => {
                w.set_ym_year(fluentcal_core::date::parts(&self.ui.cursor).0);
                w.set_ym_cursor(self.ui.cursor[..7].into());
                w.set_ym_now(self.today[..7].into());
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
            "confirm" => {
                w.set_confirm_title(s("title").into());
                w.set_confirm_text(s("text").into());
                w.set_confirm_primary(s("primary").into());
                w.set_confirm_danger(true);
                w.invoke_open_confirm();
            }
            other => eprintln!("unknown overlay {other}"),
        }
    }

    pub fn refresh_weeks(&mut self) {
        push_weeks(&self.window, &self.doc, &self.ui, &self.today, self.capacity, &self.tokens);
    }

    /// Re-lays the chips out if the lane capacity changed; true when it did. (Also applies the
    /// Gantt view's first scroll, which needs the laid-out size.)
    pub fn sync_capacity(&mut self) -> bool {
        if self.doc.settings.calendar.view == "gantt" && self.gantt.center.is_some() {
            self.gantt_view().scrolled();
            return true;
        }
        let cap = capacity_of(&self.window);
        if cap == self.capacity {
            return false;
        }
        self.capacity = cap;
        self.refresh_weeks();
        true
    }

    pub fn show(&self) -> Result<(), slint::PlatformError> {
        self.window.show()
    }
}
