//! Framed windows (settings, task editor; 0.2's open_aux_window): one per kind, Mica on
//! Windows 11 (build 22000+, else a solid background), centred on the calendar's screen,
//! the title bar in the app's theme. Closing drops the window (it costs nothing while closed).

use crate::app::{run, App};
use crate::platform::{desk, screens, window};
use crate::theme::{apply, Look};
use crate::Theme;
use fluentcal_core::layout::Mon;
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Global};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};

/// Size in logical px; the window's minimum size comes from its layout.
pub struct Spec {
    pub width: f64,
    pub height: f64,
}

impl App {
    /// Mica behind the settings/editor windows (0.2's hasMica): Windows 11 only.
    pub fn has_mica(&self) -> bool {
        self.sys.build >= 22000
    }

    /// The screen the calendar is on, else the primary one.
    pub fn aux_screen(&self) -> Option<Mon> {
        let mons = screens::monitors();
        let i = crate::shell::current_screen(self);
        i.and_then(|i| mons.get(i).cloned()).or_else(|| mons.iter().find(|m| m.primary).cloned()).or_else(|| mons.first().cloned())
    }

    /// The theme for a window on a screen with `scale`.
    pub fn look(&self, scale: f32) -> Look<'_> {
        Look { tokens: &self.tokens, desk: &self.style, reduce_motion: self.doc.settings.theme.reduce_motion, win11: crate::fonts::is_win11(), scale }
    }
}

/// Creates the window `make` builds, themed, centred and shown; `closed` runs (on the next
/// turn) when the user closes it, and should drop the window.
pub fn open<C>(app: &App, spec: &Spec, make: impl FnOnce() -> Result<C, slint::PlatformError>, closed: impl Fn(&mut App) + 'static) -> Option<C>
where
    C: ComponentHandle + 'static,
    for<'a> Theme<'a>: Global<'a, C>,
{
    window::next(window::Kind::Aux);
    let w = match make() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[window] could not open a window: {e}");
            return None;
        }
    };
    let scale = app.aux_screen().map_or(1.0, |m| m.scale);
    apply(&w, &app.look(scale as f32));
    let (pw, ph) = ((spec.width * scale) as u32, (spec.height * scale) as u32);
    w.window().set_size(slint::PhysicalSize::new(pw, ph));
    if let Some(m) = app.aux_screen() {
        let x = m.work.x + (m.work.w as i32 - pw as i32) / 2;
        let y = m.work.y + (m.work.h as i32 - ph as i32) / 2;
        w.window().set_position(slint::PhysicalPosition::new(x.max(m.work.x), y.max(m.work.y)));
    }
    let closed = std::rc::Rc::new(closed);
    w.window().on_close_requested(move || {
        let c = closed.clone();
        // Not inside the window's own callback: `closed` drops it.
        slint::Timer::single_shot(std::time::Duration::ZERO, move || run(move |app| c(app)));
        slint::CloseRequestResponse::HideWindow
    });
    let (dark, mica) = (app.tokens.dark, app.has_mica());
    window::when_created(w.as_weak(), move |h| window::set_mica(h, dark, mica));
    desk::allow_foreground();
    if let Err(e) = w.show() {
        eprintln!("[window] show: {e}");
    }
    window::when_shown(w.as_weak(), |w, h| to_front(w, h));
    Some(w)
}

/// Brings an open window back up (restored if minimised) and to the front.
pub fn raise(w: &impl ComponentHandle) {
    let _ = w.show();
    if let Some(h) = window::hwnd(w.window()) {
        to_front(w, h);
    }
}

/// Restores and activates the window. Windows refuses SetForegroundWindow when the click came
/// from the calendar (a window that never activates) or another program is in front: the
/// window then opens behind it. winit's focus_window gets past that the way 0.2 did (Tauri's
/// set_focus): it taps Alt, which lifts the foreground lock, and tries again.
fn to_front(w: &impl ComponentHandle, h: HWND) {
    unsafe {
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        let _ = SetForegroundWindow(h);
        if GetForegroundWindow() != h {
            w.window().with_winit_window(|win| win.focus_window());
        }
    }
}

/// The theme changed: the window's colours and its title bar follow.
pub fn retheme<C>(app: &App, w: &C)
where
    C: ComponentHandle,
    for<'a> Theme<'a>: Global<'a, C>,
{
    apply(w, &app.look(w.window().scale_factor()));
    if let Some(h) = window::hwnd(w.window()) {
        window::set_mica(h, app.tokens.dark, app.has_mica());
    }
}
