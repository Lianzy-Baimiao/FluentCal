//! Hotkeys, the events of the hidden window and of the desk window, the clock, reminder
//! scheduling and holiday updates.

use crate::app::{run, App};
use crate::platform::desk::DeskEvent;
use crate::platform::msgwin::{self, MsgEvent};
use crate::reminders::{Reminder, Style};
use fluentcal_core::date::{now_ms, parts, start_of_month, utc_offset_minutes};
use fluentcal_core::reminders::{compute_reminders, GRACE_MS, HORIZON_MS};
use std::time::Duration;

const HK_TOGGLE: i32 = 1;
const HK_NEW: i32 = 2;

pub fn register_hotkeys(app: &mut App) {
    msgwin::unregister_hotkey(HK_TOGGLE);
    msgwin::unregister_hotkey(HK_NEW);
    let s = &app.doc.settings.shortcuts;
    let mut errors = Vec::new();
    for (id, accel) in [(HK_TOGGLE, s.toggle.clone()), (HK_NEW, s.new_task.clone())] {
        if accel.trim().is_empty() {
            continue;
        }
        if let Err(e) = msgwin::register_hotkey(id, &accel) {
            errors.push(format!("{accel}：{e}"));
        }
    }
    app.hotkey_errors = errors;
}

pub fn on_msg(app: &mut App, e: MsgEvent) {
    match e {
        // Starting the program again opens the settings (as 0.2).
        MsgEvent::SecondInstance(_) => app.open_settings(None),
        MsgEvent::Hotkey(HK_TOGGLE) => super::toggle_desk(app),
        MsgEvent::Hotkey(HK_NEW) => {
            crate::platform::desk::allow_foreground();
            app.open_editor_new(None);
        }
        MsgEvent::Hotkey(_) => {}
        // Progman comes back a little later; the calendar may have gone with the old one.
        MsgEvent::TaskbarCreated => {
            for ms in [1500u64, 5000] {
                slint::Timer::single_shot(Duration::from_millis(ms), || crate::app::run(crate::startup::ensure_desk_window));
            }
        }
    }
}

pub fn on_desk_event(app: &mut App, e: DeskEvent) {
    match e {
        DeskEvent::Resumed => app.cloud.nudge_after_wake(),
        DeskEvent::Theme => {
            // One change sends several messages: read the system once they settle.
            app.theme_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(250), || {
                run(|app| {
                    let sys = crate::platform::sysinfo::system_info();
                    if sys != app.sys {
                        app.sys = sys;
                        app.recompute_theme();
                    }
                })
            });
        }
        DeskEvent::DisplayChanged => {
            super::apply_layout(app, false);
            super::tray_refresh(app);
        }
        DeskEvent::MovedByUser => super::save_home(app),
        DeskEvent::Moved => {
            app.desk_check_scale();
            let cur = super::current_screen(app);
            if cur != app.last_monitor {
                app.last_monitor = cur;
                super::tray_refresh(app);
            }
        }
        DeskEvent::Reattach => super::apply_desk_platform(app),
        DeskEvent::Deactivated => {}
    }
}

impl App {
    /// The reminders of the next 48 hours to the reminder thread (after edits settle).
    pub fn schedule_reminders(&mut self) {
        self.reminder_timer.start(slint::TimerMode::SingleShot, Duration::from_millis(400), || {
            run(|app| {
                let r = &app.doc.settings.reminders;
                let items: Vec<Reminder> = if r.enabled {
                    let tasks: Vec<_> = app.doc.tasks.values().cloned().collect();
                    compute_reminders(&tasks, now_ms(), &r.all_day_time, HORIZON_MS, GRACE_MS, utc_offset_minutes()).into_iter().map(Reminder::from).collect()
                } else {
                    Vec::new()
                };
                app.reminders.set(items, Style::parse(&r.style));
            })
        });
    }
}

/// Every 30 s. A new day: a calendar left on the desktop overnight follows the date unless the
/// user paged away; overdue tasks move on; reminders are rescheduled.
pub fn tick(app: &mut App) {
    crate::startup::ensure_desk_window(app);
    let today = app.today();
    if today == app.day {
        return;
    }
    let prev = std::mem::replace(&mut app.day, today.clone());
    let ui = &mut app.desk.ui;
    if ui.cursor == start_of_month(&prev) {
        ui.cursor = start_of_month(&today);
    }
    if ui.weeks_start == crate::desk::vm::default_weeks_start(&app.doc, &prev) {
        ui.weeks_start = crate::desk::vm::default_weeks_start(&app.doc, &today);
    }
    if ui.selected == prev {
        ui.selected = today;
    }
    auto_postpone(app);
    app.desk_refresh();
    app.schedule_reminders();
}

/// Overdue single-day tasks move to today (setting tasks.autoPostpone); not undoable.
pub fn auto_postpone(app: &mut App) {
    let today = app.today();
    let edit = fluentcal_core::tasks::auto_postpone(&app.doc, &today, now_ms());
    app.apply(edit, false);
}

/// Statutory holidays for this and next year, at most every three days unless forced. The
/// result comes back as a store change.
pub fn update_holidays(app: &App, force: bool) {
    let store = app.store.clone();
    let proxy = app.doc.settings.sync.proxy.clone();
    let year = parts(&app.today()).0;
    std::thread::spawn(move || {
        fluentcal_core::holidays::update(&store, year, force, &proxy);
    });
}
