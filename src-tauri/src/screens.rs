//! Multi-monitor support for the desk window: lists the screens, moves the calendar to
//! another one, remembers where the user put it, and puts it back there when screens change
//! (the geometry is in layout.rs).

use crate::layout::{self, Home, Mon, Rect};
use crate::store::{self, Store};
use serde::Serialize;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Screen {
    pub index: usize,
    /// Number as shown in Windows display settings (\\.\DISPLAY2 -> 2).
    pub number: usize,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub primary: bool,
    /// The calendar is on this screen.
    pub current: bool,
}

impl Screen {
    pub fn title(&self) -> String {
        format!("显示器 {}{}  {}×{}", self.number, if self.primary { "（主）" } else { "" }, self.width, self.height)
    }
}

fn number(name: &str, i: usize) -> usize {
    name.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(i + 1)
}

fn mon(m: &Monitor, primary: bool) -> Mon {
    let (p, s, a) = (m.position(), m.size(), m.work_area());
    Mon {
        name: m.name().cloned().unwrap_or_default(),
        rect: Rect { x: p.x, y: p.y, w: s.width, h: s.height },
        work: Rect { x: a.position.x, y: a.position.y, w: a.size.width, h: a.size.height },
        scale: m.scale_factor(),
        primary,
    }
}

fn mons(win: &WebviewWindow) -> Vec<Mon> {
    let primary = win.primary_monitor().ok().flatten().map(|m| (*m.position(), *m.size()));
    win.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| mon(m, primary == Some((*m.position(), *m.size()))))
        .collect()
}

fn rect_of(win: &WebviewWindow) -> Option<Rect> {
    let (p, s) = (win.outer_position().ok()?, win.outer_size().ok()?);
    Some(Rect { x: p.x, y: p.y, w: s.width, h: s.height })
}

pub fn list(app: &AppHandle) -> Vec<Screen> {
    let Some(win) = app.get_webview_window("desk") else { return Vec::new() };
    let all = mons(&win);
    let cur = rect_of(&win).and_then(|r| layout::monitor_at(&all, r));
    all.iter()
        .enumerate()
        .map(|(i, m)| Screen {
            index: i,
            number: number(&m.name, i),
            width: m.rect.w,
            height: m.rect.h,
            scale: m.scale,
            primary: m.primary,
            current: cur == Some(i),
        })
        .collect()
}

/// Sets position and size. The position is set again at the end because moving onto a screen
/// with a different scale makes Windows resize the window on the way.
fn place(win: &WebviewWindow, r: Rect) {
    let _ = win.set_position(PhysicalPosition::new(r.x, r.y));
    let _ = win.set_size(PhysicalSize::new(r.w, r.h));
    let _ = win.set_position(PhysicalPosition::new(r.x, r.y));
}

/// "显示在": moves the calendar to screen `index`, keeping its corner and logical size.
pub fn move_to(app: &AppHandle, index: usize) {
    let Some(win) = app.get_webview_window("desk") else { return };
    let all = mons(&win);
    let (Some(to), Some(r)) = (all.get(index), rect_of(&win)) else { return };
    let target = match layout::monitor_at(&all, r).and_then(|i| all.get(i)) {
        Some(from) => layout::moved(r, from, to),
        None => layout::default_rect(std::slice::from_ref(to)).unwrap_or(r),
    };
    place(&win, target);
    save_home(app);
}

/// The user put the calendar somewhere (end of a drag or resize, "显示在"): remember the place
/// and the screen it is on.
pub fn save_home(app: &AppHandle) {
    let Some(win) = app.get_webview_window("desk") else { return };
    if !win.is_visible().unwrap_or(false) || win.is_minimized().unwrap_or(false) {
        return;
    }
    let Some(r) = rect_of(&win).filter(|r| r.x > -20000 && r.y > -20000 && r.w >= 100 && r.h >= 100) else { return };
    let all = mons(&win);
    let monitor = layout::monitor_at(&all, r).map(|i| (all[i].name.clone(), all[i].rect, all[i].scale));
    let home = Home { x: r.x, y: r.y, w: r.w, h: r.h, monitor };
    let s = app.state::<Arc<Store>>();
    store::patch_settings_silent(&s, json!({ "desk": { "bounds": home } }));
}

/// At startup and after a display change: home if its screen is connected, else stay put if
/// reachable, else the default place. Never saved: the user's place stays the home.
pub fn apply_layout(app: &AppHandle, startup: bool) {
    let Some(win) = app.get_webview_window("desk") else { return };
    let all = mons(&win);
    if all.is_empty() {
        return; // screens not enumerated yet; the next WM_DISPLAYCHANGE runs this again
    }
    let home = app.state::<Arc<Store>>().setting("/desk/bounds").and_then(|v| Home::parse(&v));
    let current = if startup { None } else { rect_of(&win) };
    if let Some(t) = layout::target(&all, home.as_ref(), current) {
        if Some(t) != current {
            place(&win, t);
        }
    }
}

/// A screen was added, removed or rearranged.
pub fn on_display_change(app: &AppHandle) {
    apply_layout(app, false);
    crate::tray::refresh(app);
    let _ = app.emit("desk://screen", ());
}

static LAST: AtomicUsize = AtomicUsize::new(usize::MAX);

/// The calendar moved: refresh the tray and the settings window when it is now on another
/// screen.
pub fn note_moved(app: &AppHandle) {
    let Some(win) = app.get_webview_window("desk") else { return };
    let all = mons(&win);
    let cur = rect_of(&win).and_then(|r| layout::monitor_at(&all, r)).unwrap_or(usize::MAX);
    if LAST.swap(cur, Ordering::Relaxed) != cur {
        crate::tray::refresh(app);
        let _ = app.emit("desk://screen", ());
    }
}
