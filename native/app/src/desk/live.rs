//! The live desk window's state and how the app refreshes it.

use super::vm::DeskUi;
use crate::app::App;
use crate::theme::{apply, Look};
use crate::DeskWindow;
use slint::ComponentHandle;

/// A chip being dragged onto another day.
pub struct Drag {
    pub key: String,
    pub task_id: String,
    /// The occurrence's date (repeating tasks: identifies the instance).
    pub date: String,
    pub title: String,
    /// Where the press was (window logical px).
    pub x: f32,
    pub y: f32,
    /// Days from the occurrence's start to the day under the press: keeps a multi-day bar
    /// under the pointer.
    pub offset: i32,
    pub active: bool,
}

pub struct DeskLive {
    pub window: DeskWindow,
    pub ui: DeskUi,
    pub capacity: Option<usize>,
    pub drag: Option<Drag>,
    pub toast_timer: slint::Timer,
    pub flash_timer: slint::Timer,
    /// Scale factor the theme images (shadows) were rendered for.
    pub scale: f32,
}

impl DeskLive {
    pub fn new(window: DeskWindow, ui: DeskUi) -> DeskLive {
        DeskLive { window, ui, capacity: None, drag: None, toast_timer: Default::default(), flash_timer: Default::default(), scale: 0.0 }
    }
}

impl App {
    /// Everything the desk window shows.
    pub fn desk_refresh(&mut self) {
        let today = self.today();
        let d = &self.desk;
        super::push_all(&d.window, &self.doc, &d.ui, &today, d.capacity, &self.tokens);
        self.desk_refresh_status();
    }

    /// The parts that follow tasks (week rows, countdowns).
    pub fn desk_refresh_data(&mut self) {
        let today = self.today();
        let d = &self.desk;
        super::push_weeks(&d.window, &self.doc, &d.ui, &today, d.capacity, &self.tokens);
    }

    /// Title, paging and selection (after paging or a view switch).
    pub fn desk_refresh_view(&mut self) {
        self.desk_refresh();
    }

    /// Sync button and save error.
    pub fn desk_refresh_status(&mut self) {
        let w = &self.desk.window;
        let s = &self.sync;
        w.set_sync_enabled(s.enabled);
        w.set_sync_busy(s.syncing);
        w.set_sync_failed(s.error.is_some());
        let line = fluentcal_core::sync_text::status_line(s, fluentcal_core::date::now_ms(), fluentcal_core::date::utc_offset_minutes());
        w.set_sync_tip(format!("{line}（点击立即同步）").into());
    }

    /// Lane capacity from the layout changed: lay the chips out again.
    pub fn desk_sync_capacity(&mut self) {
        let cap = super::capacity_of(&self.desk.window);
        if cap != self.desk.capacity {
            self.desk.capacity = cap;
            self.desk_refresh_data();
        }
    }

    /// Theme global of the desk window (and its chip colours, which come from the theme).
    pub fn desk_apply_theme(&mut self) {
        let scale = self.desk.window.window().scale_factor();
        self.desk.scale = scale;
        let look = Look {
            tokens: &self.tokens,
            desk: &self.style,
            reduce_motion: self.doc.settings.theme.reduce_motion,
            win11: crate::fonts::is_win11(),
            scale,
        };
        apply(&self.desk.window, &look);
        self.desk_refresh_data();
    }

    /// The window moved to a screen with another scale: the shadow images follow.
    pub fn desk_check_scale(&mut self) {
        if (self.desk.window.window().scale_factor() - self.desk.scale).abs() > 0.01 {
            self.desk_apply_theme();
        }
    }

    pub fn today(&self) -> String {
        fluentcal_core::date::local_of_ms(fluentcal_core::date::now_ms(), fluentcal_core::date::utc_offset_minutes()).0
    }
}
