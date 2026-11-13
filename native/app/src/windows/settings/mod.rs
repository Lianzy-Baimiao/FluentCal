//! The settings window (0.2's src/windows/settings): one at a time, dropped when closed.
//! view.rs turns the settings and the app's state into what the pages show; actions.rs is
//! what their buttons and switches do (0.2's Tauri commands).

pub mod actions;
pub mod commands;
pub mod io;
pub mod view;
pub mod mobile;

use crate::app::{run, App};
use crate::windows::framed::{self, Spec};
use crate::{SetState, SettingsWindow, Severity};
use slint::ComponentHandle;
use view::Local;

/// Opens the window's confirmation dialog (0.2's confirm(); the answer comes back through
/// SetState.confirm-answer).
pub fn show_confirm(w: &SettingsWindow, title: &str, content: &str, primary: &str, secondary: &str, danger: bool) {
    w.set_confirm_title(title.into());
    w.set_confirm_content(content.into());
    w.set_confirm_primary(primary.into());
    w.set_confirm_secondary(secondary.into());
    w.set_confirm_danger(danger);
    w.invoke_open_confirm();
}

/// What a confirmation's answer runs with: "primary", "secondary" or "close".
pub type Answer = Box<dyn FnOnce(&mut App, &str)>;

pub struct SettingsWin {
    pub window: SettingsWindow,
    pub local: Local,
    /// The open confirmation's continuation.
    pending: Option<Answer>,
}

/// Wires the pages' callbacks to the app.
fn wire(w: &SettingsWindow) {
    let g = w.global::<SetState>();
    g.on_set_bool(|p, v| run(move |a| a.settings_set(&p, serde_json::json!(v))));
    g.on_set_num(|p, v| run(move |a| a.settings_set(&p, fluentcal_core::settings_page::num(f64::from(v)))));
    g.on_set_str(|p, v| run(move |a| a.settings_set(&p, serde_json::json!(v.as_str()))));
    g.on_action(|k| run(move |a| a.settings_action(&k, "")));
    g.on_action_arg(|k, v| run(move |a| a.settings_action(&k, &v)));
    g.on_set_rest_day(|d, on| run(move |a| a.settings_rest_day(d.max(0) as u32, on)));
    g.on_set_offset(|i, v| run(move |a| a.settings_offset(i.max(0) as usize, &v)));
    g.on_set_group_name(|id, name| run(move |a| a.settings_group_name(&id, &name)));
    g.on_set_group_color(|id, c| run(move |a| a.settings_group_color(&id, &c)));
    g.on_add_group(|name| run(move |a| a.settings_add_group(&name)));
    g.on_save_secret(|k, v| run(move |a| a.settings_save_secret(&k, &v)));
    g.on_navigate(|k| run(move |a| a.settings_navigate(&k)));
    crate::group_delete::wire(w, crate::group_delete::Host::Settings);
    // Taken as it comes, not on a timer: the dialog answers, then says "close" as it closes,
    // and zero-delay timers started in the same millisecond run newest first, so "close" used
    // to win and the buttons did nothing (GitHub #2).
    g.on_confirm_answer(|r| run(move |a| a.settings_answer(&r)));
}

impl App {
    /// Opens the settings window (on `page`, e.g. "appearance", "web"), or brings it up.
    pub fn open_settings(&mut self, page: Option<&str>) {
        if self.settings_win.is_none() {
            let make = || {
                let w = SettingsWindow::new()?;
                wire(&w);
                crate::fluent::install(&w, "", None);
                Ok(w)
            };
            let Some(window) = framed::open(self, &Spec { width: 980.0, height: 700.0 }, make, |app| app.settings_win = None) else { return };
            window.set_mica(self.has_mica());
            view::setup(&window, self);
            self.settings_win = Some(SettingsWin { window, local: Local::default(), pending: None });
            self.settings_navigate(page.unwrap_or("general"));
        } else if let Some(s) = &self.settings_win {
            framed::raise(&s.window);
            if let Some(p) = page {
                self.settings_navigate(p);
            }
        }
    }

    /// Settings or data changed (anywhere): the open window shows the new state.
    pub fn settings_refresh(&mut self) {
        if self.settings_win.is_none() {
            return;
        }
        if let Some(s) = &mut self.settings_win {
            if s.local.pairing.as_ref().is_some_and(|p| !p.active()) { s.local.pair_timer.stop(); }
        }
        let info = self.settings_info();
        if let Some(s) = &self.settings_win {
            view::push(&s.window, &self.doc, &info, &s.local);
        }
        self.group_delete_refresh(crate::group_delete::Host::Settings);
    }

    /// The theme changed: colours, the title bar, the thumbnails' shadow.
    pub fn settings_retheme(&mut self) {
        if let Some(s) = &self.settings_win {
            framed::retheme(self, &s.window);
            view::setup(&s.window, self);
        }
    }

    /// A page's message bar (0.2's `say`).
    pub fn settings_say(&mut self, severity: Severity, text: &str) {
        if let Some(s) = &mut self.settings_win {
            s.local.msg = Some((severity, String::new(), text.to_owned()));
        }
        self.settings_refresh();
    }

    /// 0.2's confirm(): asks in the window's dialog; `then` gets the answer.
    pub fn settings_confirm(&mut self, title: &str, content: &str, primary: &str, secondary: &str, danger: bool, then: impl FnOnce(&mut App, &str) + 'static) {
        let Some(s) = &mut self.settings_win else { return };
        s.pending = Some(Box::new(then));
        show_confirm(&s.window, title, content, primary, secondary, danger);
    }

    fn settings_answer(&mut self, r: &str) {
        let Some(s) = &mut self.settings_win else { return };
        // The dialog sends its answer, then "close" as it closes: only the first counts.
        let Some(then) = s.pending.take() else { return };
        // On the next turn: the dialog finishes closing before the answer may open another.
        let r = r.to_owned();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || run(move |a| then(a, &r)));
    }

    pub fn settings_navigate(&mut self, page: &str) {
        const PAGES: [&str; 9] = ["general", "appearance", "calendar", "gantt", "reminders", "data", "sync", "web", "about"];
        let Some(s) = &mut self.settings_win else { return };
        if !PAGES.contains(&page) {
            return;
        }
        if s.local.page != page {
            s.local.pairing = None;
            s.local.pair_qr = None;
            s.local.pair_timer.stop();
            s.local.msg = None;
            s.window.invoke_scroll_top();
        }
        s.local.page = page.to_owned();
        s.window.global::<SetState>().set_page(page.into());
        match page {
            "data" => self.settings_load_backups(),
            "sync" => self.settings_load_secrets(),
            "web" => self.settings_load_web(),
            _ => {}
        }
        self.settings_refresh();
    }
}
