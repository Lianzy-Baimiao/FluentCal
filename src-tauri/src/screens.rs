//! Multi-monitor support: lists the screens, moves the calendar to another one, and brings it
//! back when the screen it was on goes away (unplugged, resolution or layout change).

use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};
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

fn number(m: &Monitor, i: usize) -> usize {
    m.name()
        .and_then(|n| n.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().ok())
        .unwrap_or(i + 1)
}

fn contains(m: &Monitor, x: i32, y: i32) -> bool {
    let (p, s) = (m.position(), m.size());
    x >= p.x && x < p.x + s.width as i32 && y >= p.y && y < p.y + s.height as i32
}

/// Index of the screen holding the centre of the calendar.
fn current(win: &WebviewWindow, all: &[Monitor]) -> Option<usize> {
    let (p, s) = (win.outer_position().ok()?, win.outer_size().ok()?);
    let (cx, cy) = (p.x + s.width as i32 / 2, p.y + s.height as i32 / 2);
    all.iter().position(|m| contains(m, cx, cy))
}

pub fn list(app: &AppHandle) -> Vec<Screen> {
    let Some(win) = app.get_webview_window("desk") else { return Vec::new() };
    let all = win.available_monitors().unwrap_or_default();
    let primary = win.primary_monitor().ok().flatten();
    let cur = current(&win, &all);
    all.iter()
        .enumerate()
        .map(|(i, m)| Screen {
            index: i,
            number: number(m, i),
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
            primary: primary.as_ref().is_some_and(|p| p.position() == m.position() && p.size() == m.size()),
            current: cur == Some(i),
        })
        .collect()
}

/// Whether the top strip (title bar) of a window at x,y with width w is on some screen, so it
/// can still be dragged.
pub fn title_visible(all: &[Monitor], x: i32, y: i32, w: u32) -> bool {
    all.iter().any(|m| {
        let a = m.work_area();
        let ix = (x + w as i32).min(a.position.x + a.size.width as i32) - x.max(a.position.x);
        let iy = (y + 40).min(a.position.y + a.size.height as i32) - y.max(a.position.y);
        ix >= 120 && iy >= 30
    })
}

/// Sets position and size. The position is set again at the end because moving onto a screen
/// with a different scale makes Windows resize the window on the way.
pub fn place(win: &WebviewWindow, x: i32, y: i32, w: u32, h: u32) {
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(w, h));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Top-right corner of the screen's work area, 760×560 logical pixels.
pub fn place_default(win: &WebviewWindow, m: &Monitor) {
    let a = m.work_area();
    let s = m.scale_factor();
    let (w, h) = ((760.0 * s) as u32, (560.0 * s) as u32);
    let margin = (24.0 * s) as i32;
    place(win, a.position.x + a.size.width as i32 - w as i32 - margin, a.position.y + margin, w, h);
}

/// Moves the calendar to screen `index`, keeping its relative place in the work area (a
/// calendar in the top-right corner stays in the top-right corner) and its logical size.
pub fn move_to(app: &AppHandle, index: usize) {
    let Some(win) = app.get_webview_window("desk") else { return };
    let all = win.available_monitors().unwrap_or_default();
    let Some(target) = all.get(index) else { return };
    let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) else { return };
    let Some(from) = current(&win, &all).and_then(|i| all.get(i)) else {
        return place_default(&win, target);
    };
    let (fa, ta) = (from.work_area(), target.work_area());
    let ratio = target.scale_factor() / from.scale_factor();
    let w = ((s.width as f64 * ratio) as u32).min(ta.size.width);
    let h = ((s.height as f64 * ratio) as u32).min(ta.size.height);
    let frac = |off: i32, free: i64| if free > 0 { (off as f64 / free as f64).clamp(0.0, 1.0) } else { 0.0 };
    let fx = frac(p.x - fa.position.x, fa.size.width as i64 - s.width as i64);
    let fy = frac(p.y - fa.position.y, fa.size.height as i64 - s.height as i64);
    let x = ta.position.x + ((ta.size.width - w) as f64 * fx).round() as i32;
    let y = ta.position.y + ((ta.size.height - h) as f64 * fy).round() as i32;
    place(&win, x, y, w, h);
}

/// After a display change: if the calendar's title bar is on no screen any more, put it back
/// on the primary screen. Always tells the tray and the settings window to refresh.
pub fn on_display_change(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("desk") {
        let all = win.available_monitors().unwrap_or_default();
        if let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) {
            if !all.is_empty() && !title_visible(&all, p.x, p.y, s.width) {
                if let Ok(Some(m)) = win.primary_monitor() {
                    place_default(&win, &m);
                }
            }
        }
    }
    crate::tray::refresh(app);
    let _ = app.emit("desk://screen", ());
}

static LAST: AtomicUsize = AtomicUsize::new(usize::MAX);

/// The calendar moved: refresh the tray and the settings window when it is now on another
/// screen.
pub fn note_moved(app: &AppHandle) {
    let Some(win) = app.get_webview_window("desk") else { return };
    let all = win.available_monitors().unwrap_or_default();
    let cur = current(&win, &all).unwrap_or(usize::MAX);
    if LAST.swap(cur, Ordering::Relaxed) != cur {
        crate::tray::refresh(app);
        let _ = app.emit("desk://screen", ());
    }
}
