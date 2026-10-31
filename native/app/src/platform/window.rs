//! Window helpers: the HWND of a Slint window, creation attributes per kind of window,
//! Mica and the dark title bar for the settings/editor windows.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::{winit, WinitWindowAccessor};
use std::cell::Cell;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE};

/// What a window being created is for (its creation attributes differ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    /// The calendar: never activated by being shown, no taskbar button, per-pixel alpha.
    Desk,
    /// Settings, editor: ordinary windows with Mica.
    #[default]
    Aux,
    /// Reminder cards: no taskbar button, never activated.
    Popup,
}

thread_local! {
    static NEXT: Cell<Kind> = const { Cell::new(Kind::Aux) };
}

/// Call before creating a window component: its winit window gets this kind's attributes.
/// (Slint creates the window adapter inside `Component::new()`.)
pub fn next(kind: Kind) {
    NEXT.with(|n| n.set(kind));
}

/// For BackendSelector::with_winit_window_attributes_hook.
pub fn attributes(a: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    use winit::platform::windows::WindowAttributesExtWindows;
    let kind = NEXT.with(|n| n.replace(Kind::Aux));
    match kind {
        Kind::Desk => a.with_active(false).with_skip_taskbar(true).with_transparent(true).with_decorations(false),
        Kind::Popup => a.with_active(false).with_skip_taskbar(true).with_transparent(true).with_decorations(false).with_window_level(winit::window::WindowLevel::AlwaysOnTop),
        Kind::Aux => a.with_transparent(true),
    }
}

/// The HWND of a shown Slint window (None before the window exists).
pub fn hwnd(w: &slint::Window) -> Option<HWND> {
    w.with_winit_window(|win| match win.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    })
    .flatten()
}

/// Runs `f` with the window's HWND as soon as the window exists (Slint creates it lazily,
/// once the event loop runs after show()).
pub fn when_created(w: slint::Weak<impl slint::ComponentHandle + 'static>, f: impl FnOnce(HWND) + 'static) {
    let f = Cell::new(Some(f));
    let timer = slint::Timer::default();
    let tries = Cell::new(0);
    let t = std::rc::Rc::new(timer);
    let t2 = t.clone();
    t.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(5), move || {
        tries.set(tries.get() + 1);
        let got = w.upgrade().and_then(|c| hwnd(c.window()));
        if let Some(h) = got {
            if let Some(f) = f.take() {
                f(h);
            }
            t2.stop();
        } else if tries.get() > 400 {
            t2.stop();
        }
    });
    std::mem::forget(t);
}

/// Mica behind an ordinary window (Windows 11), and the title bar in the app's theme.
pub fn set_mica(hwnd: HWND, dark: bool, mica: bool) {
    unsafe {
        let d: i32 = dark.into();
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &d as *const _ as *const _, 4);
        // DWMSBT_MAINWINDOW (Mica) = 2, DWMSBT_NONE = 1
        let t: i32 = if mica { 2 } else { 1 };
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &t as *const _ as *const _, 4);
    }
}
