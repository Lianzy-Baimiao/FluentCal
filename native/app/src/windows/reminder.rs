//! Reminder cards (0.2's src/windows/reminder, REM-03): a stack in the bottom-right corner of
//! the primary screen with 完成 / 稍后提醒 / 关闭. Never takes the focus. The window exists only
//! while there are cards.

use crate::app::{run, App};
use crate::platform::{screens, window};
use crate::theme::{apply, Look};
use crate::{ReminderCard, ReminderWindow};
use fluentcal_core::date::now_ms;
use fluentcal_core::tasks;
use slint::{ComponentHandle, ModelRc, VecModel};

/// Card height incl. the gap between cards (logical px), width, cards before scrolling.
const CARD_H: f64 = 136.0;
const WIDTH: f64 = 380.0;
const MAX_CARDS: usize = 3;

fn wire(w: &ReminderWindow) {
    w.on_done(|k| run(move |a| a.reminder_done(&k)));
    w.on_snooze(|k, m| run(move |a| a.reminder_snooze(&k, i64::from(m))));
    w.on_dismiss(|k| run(move |a| a.reminder_dismiss(&k)));
    let weak = w.as_weak();
    w.on_popups_changed(move |on| {
        let (hw, close) = (weak.clone(), weak.clone());
        let hwnd: Box<dyn Fn() -> Option<windows::Win32::Foundation::HWND>> = Box::new(move || hw.upgrade().and_then(|w| window::hwnd(w.window())));
        let close: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(move || {
            if let Some(w) = close.upgrade() {
                w.invoke_close_menu();
            }
        });
        crate::platform::outside::watch("reminder", on.then_some((hwnd, close)));
    });
}

impl App {
    /// The list of fired reminders changed (shown, dismissed, snoozed): show, resize or hide
    /// the cards.
    pub fn reminder_cards_changed(&mut self) {
        // A snoozed reminder may belong to a task that was completed or deleted meanwhile.
        for r in self.reminders.popup_list() {
            let gone = self.doc.tasks.get(&r.task_id).is_none_or(|t| t.done && t.repeat.is_none());
            if gone {
                self.reminders.dismiss(&r.key);
            }
        }
        let list = self.reminders.popup_list();
        if list.is_empty() {
            crate::platform::outside::watch("reminder", None);
            self.reminder_win = None; // dropping the window closes it
            return;
        }
        let cards: Vec<ReminderCard> = list.iter().rev().map(|r| ReminderCard { key: r.key.clone().into(), title: r.title.clone().into(), body: r.body.clone().into() }).collect();
        let n = cards.len();
        if self.reminder_win.is_none() {
            window::next(window::Kind::Popup);
            let Ok(w) = ReminderWindow::new() else { return };
            wire(&w);
            self.reminder_win = Some(w);
        }
        let w = self.reminder_win.as_ref().unwrap();
        w.set_items(ModelRc::new(VecModel::from(cards)));
        let mons = screens::monitors();
        let Some(m) = mons.iter().find(|m| m.primary).or(mons.first()) else { return };
        let s = m.scale;
        let (pw, ph) = ((WIDTH * s) as u32, ((CARD_H * n.min(MAX_CARDS) as f64 + 8.0) * s) as u32);
        let margin = (12.0 * s) as i32;
        let look = Look { tokens: &self.tokens, desk: &self.style, reduce_motion: self.doc.settings.theme.reduce_motion, win11: crate::fonts::is_win11(), scale: s as f32 };
        apply(w, &look);
        w.window().set_size(slint::PhysicalSize::new(pw, ph));
        w.window().set_position(slint::PhysicalPosition::new(m.work.x + m.work.w as i32 - pw as i32 - margin, m.work.y + m.work.h as i32 - ph as i32 - margin));
        let _ = w.show();
        // Clicking the buttons must not take the focus from what the user is doing.
        window::when_created(w.as_weak(), |h| unsafe {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex | (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) as isize);
        });
    }

    fn reminder_done(&mut self, key: &str) {
        let Some(r) = self.reminders.popup_list().into_iter().find(|r| r.key == key) else { return };
        if let Some(t) = self.doc.tasks.get(&r.task_id).cloned() {
            let edit = tasks::set_occurrence_done(&self.doc, &t, &r.date, true, now_ms());
            self.apply(edit, false);
        }
        self.reminder_dismiss(key);
    }

    fn reminder_dismiss(&mut self, key: &str) {
        self.reminders.dismiss(key);
        self.reminder_cards_changed();
    }

    fn reminder_snooze(&mut self, key: &str, minutes: i64) {
        self.reminders.snooze(key, minutes);
        self.reminder_cards_changed();
    }
}
