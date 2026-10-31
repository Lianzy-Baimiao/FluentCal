//! Glue between the app and Windows: what a settings change does outside the views, the desk
//! window's layer and place, the tray, hotkeys and the platform's events.

pub mod events;
pub mod tray;

use crate::app::App;
use crate::platform::desk::{self as pd, PinMode};
use crate::platform::{autostart, screens, window};
use fluentcal_core::layout::{self, Home};
use fluentcal_core::settings::Settings;
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;

pub use tray::tray_refresh;

/// Settings changed (`app.doc.settings` is the new state).
pub fn apply_settings(app: &mut App, old: &Settings) {
    let new = app.doc.settings.clone();
    let (o, n) = (&old.desk, &new.desk);
    if o.pin_mode != n.pin_mode || o.click_through != n.click_through || o.no_activate != n.no_activate || o.acrylic != n.acrylic || o.locked != n.locked {
        apply_desk_platform(app);
    }
    if old.shortcuts != new.shortcuts {
        events::register_hotkeys(app);
    }
    if old.sync != new.sync {
        app.cloud.reconfigure(&app.store.setting("").unwrap_or_default());
    }
    if old.general.autostart != new.general.autostart {
        autostart::sync(new.general.autostart);
    }
    // The saved place (bounds) is written without telling anyone; it isn't appearance.
    let look = |d: &fluentcal_core::settings::Desk| fluentcal_core::settings::Desk { bounds: None, ..d.clone() };
    if old.theme != new.theme || look(o) != look(n) {
        app.recompute_theme();
    }
    if old.calendar.weeks_rows != new.calendar.weeks_rows || old.calendar.weeks_past != new.calendar.weeks_past || old.calendar.week_start != new.calendar.week_start {
        let today = app.today();
        app.desk.ui.weeks_start = crate::desk::vm::default_weeks_start(&app.doc, &today);
    }
    app.desk_refresh();
    tray_refresh(app);
    if old.reminders != new.reminders || old.tasks != new.tasks {
        app.schedule_reminders();
    }
}

/// Layer, taskbar button, click-through, no-activate and acrylic from the settings.
pub fn apply_desk_platform(app: &mut App) {
    let d = &app.doc.settings.desk;
    let mode = PinMode::parse(&d.pin_mode);
    pd::set_mode(mode);
    pd::set_click_through(d.click_through);
    pd::set_no_activate(d.no_activate);
    pd::set_acrylic(d.acrylic);
    app.desk.window.window().with_winit_window(|w| {
        use slint::winit_030::winit::platform::windows::WindowExtWindows;
        w.set_skip_taskbar(mode != PinMode::Normal);
    });
    pd::refresh();
}

/// Where the desk window goes: home if its screen is there, else stay if reachable, else the
/// default place. `startup`: the window isn't placed yet.
pub fn apply_layout(app: &mut App, startup: bool) {
    let mons = screens::monitors();
    if mons.is_empty() {
        return; // not enumerated yet; the next display change runs this again
    }
    let home = app.store.setting("/desk/bounds").and_then(|v| Home::parse(&v));
    let hwnd = window::hwnd(app.desk.window.window());
    let current = if startup { None } else { hwnd.and_then(screens::window_rect) };
    let Some(t) = layout::target(&mons, home.as_ref(), current) else { return };
    if Some(t) == current {
        return;
    }
    match hwnd.filter(|_| !startup) {
        Some(h) => screens::place(h, t),
        None => {
            // Not created yet: created off-screen with its size, so it never shows above other
            // windows before it is on the desktop layer; moved into place once pinned.
            let w = app.desk.window.window();
            w.set_position(slint::PhysicalPosition::new(-32000, -32000));
            w.set_size(slint::PhysicalSize::new(t.w, t.h));
        }
    }
}

/// The user put the calendar somewhere (end of a drag or resize, "显示在"): remember the
/// place and the screen it is on.
pub fn save_home(app: &mut App) {
    let Some(h) = window::hwnd(app.desk.window.window()) else { return };
    if !screens::is_visible(h) {
        return;
    }
    let Some(r) = screens::window_rect(h).filter(|r| r.x > -20000 && r.y > -20000 && r.w >= 100 && r.h >= 100) else { return };
    let mons = screens::monitors();
    let monitor = layout::monitor_at(&mons, r).map(|i| (mons[i].name.clone(), mons[i].rect, mons[i].scale));
    let home = Home { x: r.x, y: r.y, w: r.w, h: r.h, monitor };
    fluentcal_core::store::patch_settings_silent(&app.store, serde_json::json!({ "desk": { "bounds": home } }));
}

/// "显示在": moves the calendar to screen `index`, keeping its corner and logical size.
pub fn move_to_screen(app: &mut App, index: usize) {
    let Some(h) = window::hwnd(app.desk.window.window()) else { return };
    let mons = screens::monitors();
    let (Some(to), Some(r)) = (mons.get(index), screens::window_rect(h)) else { return };
    let target = match layout::monitor_at(&mons, r).and_then(|i| mons.get(i)) {
        Some(from) => layout::moved(r, from, to),
        None => layout::default_rect(std::slice::from_ref(to)).unwrap_or(r),
    };
    screens::place(h, target);
    save_home(app);
}

/// The screen the calendar is on (index into `screens::monitors()`).
pub fn current_screen(app: &App) -> Option<usize> {
    let h = window::hwnd(app.desk.window.window())?;
    layout::monitor_at(&screens::monitors(), screens::window_rect(h)?)
}

pub fn desk_visible(app: &App) -> bool {
    window::hwnd(app.desk.window.window()).is_some_and(screens::is_visible)
}

pub fn show_desk(app: &mut App) {
    if !desk_visible(app) {
        let _ = app.desk.window.show();
        apply_desk_platform(app);
    }
}

/// In front of everything with the keyboard; in desktop mode back to the desktop layer when
/// it loses the focus (0.2's summon).
pub fn summon_desk(app: &mut App) {
    show_desk(app);
    pd::summon();
    tray_refresh(app);
    slint::Timer::single_shot(std::time::Duration::from_millis(400), pd::restore_if_not_foreground);
}

/// The hotkey: summon when hidden or in the background, else send it back (or hide it).
pub fn toggle_desk(app: &mut App) {
    if !(desk_visible(app) && pd::is_foreground()) {
        summon_desk(app);
        return;
    }
    match PinMode::current() {
        PinMode::Desktop if pd::is_summoned() => pd::dismiss(),
        PinMode::Desktop => summon_desk(app),
        _ => {
            let _ = app.desk.window.hide();
        }
    }
    tray_refresh(app);
}
