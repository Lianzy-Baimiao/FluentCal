//! Reminder cards (0.2's src/windows/reminder, REM-03): a stack in the bottom-right corner of
//! the primary screen with 完成 / 稍后提醒 / 关闭. Never takes the focus.

use crate::app::App;

impl App {
    /// The list of fired reminders changed (shown, dismissed, snoozed): show, resize or hide
    /// the cards.
    pub fn reminder_cards_changed(&mut self) {}
}
