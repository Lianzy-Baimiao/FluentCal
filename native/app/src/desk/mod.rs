//! The desk calendar window.

pub mod vm;

use crate::DeskWindow;
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::Tokens;
use slint::ComponentHandle;
use vm::DeskUi;

/// The desk window's data: the document, the window's own state and the theme.
pub struct Desk {
    pub window: DeskWindow,
    pub doc: Doc,
    pub ui: DeskUi,
    pub today: String,
    pub tokens: Tokens,
    /// Task lanes that fit in a week row (from the window's layout).
    pub capacity: Option<usize>,
}

impl Desk {
    pub fn new(window: DeskWindow, doc: Doc, today: String, tokens: Tokens) -> Desk {
        let ui = DeskUi::new(&doc, &today);
        Desk { window, doc, ui, today, tokens, capacity: None }
    }

    /// Pushes everything to the window.
    pub fn refresh(&mut self) {
        let w = &self.window;
        let c = &self.doc.settings.calendar;
        w.set_view(c.view.clone().into());
        w.set_locked(self.doc.settings.desk.locked);
        w.set_title_text(vm::view_title(&self.doc, &self.ui, &self.today).into());
        w.set_today_tip(vm::today_tip(&self.today).into());
        w.set_heads(vm::weekday_heads(&self.doc));
        w.set_show_week_no(c.show_week_numbers);
        w.set_selected(self.ui.selected.clone().into());
        w.set_flash_date(self.ui.flash.clone().into());
        w.set_countdowns(vm::countdown_items(&self.doc, &self.today));
        self.refresh_weeks();
    }

    pub fn refresh_weeks(&mut self) {
        let weeks = vm::weeks(&self.doc, &self.ui, &self.today, self.capacity, &self.tokens);
        self.window.set_weeks(weeks);
    }

    /// Re-lays the chips out if the lane capacity changed; true when it did.
    pub fn sync_capacity(&mut self) -> bool {
        let cap = Some(self.window.get_capacity().max(0) as usize);
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
