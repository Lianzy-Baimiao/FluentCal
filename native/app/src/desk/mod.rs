//! The desk calendar window: view models (vm), the live window (live, wire, actions) and an
//! offscreen helper for the parity snapshots (`Desk`).

pub mod actions;
pub mod gantt;
pub mod input;
pub mod live;
pub mod menus;
pub mod menus_act;
pub mod menus_build;
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
        if self.doc.settings.calendar.view == "gantt" {
            self.gantt_view().push();
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
