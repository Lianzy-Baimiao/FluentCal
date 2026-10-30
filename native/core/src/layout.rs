//! Where the desk window goes when screens change (PIN-05, PIN-09). Pure geometry in physical
//! pixels, so it can be tested without screens. "Home" is where the user last put the
//! calendar (drag, resize, "显示在"), together with the screen it was on. Moves made by the
//! system (a screen turned off, startup) never change it, so the calendar goes back home when
//! that screen returns.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    fn right(&self) -> i32 {
        self.x + self.w as i32
    }
    fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

#[derive(Debug, Clone)]
pub struct Mon {
    /// Device name (\\.\DISPLAY1); stays the same when the screen is turned off and on.
    pub name: String,
    pub rect: Rect,
    pub work: Rect,
    pub scale: f64,
    pub primary: bool,
}

/// Saved as settings.desk.bounds: { x, y, w, h, monitor: [name, rect, scale] | null }.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Home {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    /// The screen it was on, and that screen's rect and scale then. None in data from before
    /// this was recorded.
    pub monitor: Option<(String, Rect, f64)>,
}

impl Home {
    pub fn rect(&self) -> Rect {
        Rect { x: self.x, y: self.y, w: self.w, h: self.h }
    }

    /// Accepts integers or floats; None when x/y/w/h are missing or implausible.
    pub fn parse(v: &Value) -> Option<Home> {
        let n = |k: &str| v.get(k).and_then(Value::as_f64);
        let (x, y, w, h) = (n("x")?, n("y")?, n("w")?, n("h")?);
        if w < 100.0 || h < 100.0 {
            return None;
        }
        let monitor = v.get("monitor").and_then(Value::as_array).and_then(|a| {
            let r = a.get(1)?;
            let rn = |k: &str| r.get(k).and_then(Value::as_f64);
            let rect = Rect { x: rn("x")? as i32, y: rn("y")? as i32, w: rn("w")? as u32, h: rn("h")? as u32 };
            Some((a.first()?.as_str()?.to_owned(), rect, a.get(2)?.as_f64()?))
        });
        Some(Home { x: x as i32, y: y as i32, w: w as u32, h: h as u32, monitor })
    }
}

/// The title bar (top 40 px) is on some screen, so the window can be dragged.
pub fn title_visible(mons: &[Mon], r: Rect) -> bool {
    mons.iter().any(|m| {
        let a = m.work;
        let ix = r.right().min(a.right()) - r.x.max(a.x);
        let iy = (r.y + 40).min(a.bottom()) - r.y.max(a.y);
        ix >= 120 && iy >= 30
    })
}

/// The screen holding the centre of `r`.
pub fn monitor_at(mons: &[Mon], r: Rect) -> Option<usize> {
    let (cx, cy) = (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2);
    mons.iter().position(|m| m.rect.contains(cx, cy))
}

/// Shrinks `r` to fit `area` and moves it inside.
fn clamp(r: Rect, area: Rect) -> Rect {
    let (w, h) = (r.w.min(area.w), r.h.min(area.h));
    let x = r.x.clamp(area.x, area.right() - w as i32);
    let y = r.y.clamp(area.y, area.bottom() - h as i32);
    Rect { x, y, w, h }
}

/// Top-right corner of the primary work area, 760×560 logical pixels.
pub fn default_rect(mons: &[Mon]) -> Option<Rect> {
    let m = mons.iter().find(|m| m.primary).or_else(|| mons.first())?;
    let (w, h) = ((760.0 * m.scale) as u32, (560.0 * m.scale) as u32);
    let margin = (24.0 * m.scale) as i32;
    Some(clamp(Rect { x: m.work.right() - w as i32 - margin, y: m.work.y + margin, w, h }, m.work))
}

/// Home on today's screens: None when its screen isn't connected.
fn home_rect(mons: &[Mon], home: &Home) -> Option<Rect> {
    let Some((name, then, scale)) = &home.monitor else {
        // Older data: only the absolute position is known.
        return title_visible(mons, home.rect()).then_some(home.rect());
    };
    let m = mons.iter().find(|m| m.name == *name)?;
    if m.rect.w == then.w && m.rect.h == then.h && (m.scale - scale).abs() < 0.01 {
        // Same screen, maybe at another place in the virtual desktop.
        let r = Rect { x: home.x + m.rect.x - then.x, y: home.y + m.rect.y - then.y, w: home.w, h: home.h };
        return Some(if title_visible(mons, r) { r } else { clamp(r, m.work) });
    }
    // Resolution or scale changed: keep the relative place and the logical size.
    let fx = (home.x - then.x) as f64 / then.w.max(1) as f64;
    let fy = (home.y - then.y) as f64 / then.h.max(1) as f64;
    let k = m.scale / scale;
    let r = Rect {
        x: m.rect.x + (fx * m.rect.w as f64).round() as i32,
        y: m.rect.y + (fy * m.rect.h as f64).round() as i32,
        w: (home.w as f64 * k) as u32,
        h: (home.h as f64 * k) as u32,
    };
    Some(clamp(r, m.work))
}

/// Where the window should be. `current`: its rect now (None at startup). Order: home if its
/// screen is there; else stay if still reachable; else the default place.
pub fn target(mons: &[Mon], home: Option<&Home>, current: Option<Rect>) -> Option<Rect> {
    if let Some(r) = home.and_then(|h| home_rect(mons, h)) {
        return Some(r);
    }
    if let Some(c) = current.filter(|c| title_visible(mons, *c)) {
        return Some(c);
    }
    default_rect(mons)
}

/// "显示在" another screen: same relative place in the work area, same logical size.
pub fn moved(r: Rect, from: &Mon, to: &Mon) -> Rect {
    let k = to.scale / from.scale;
    let (w, h) = (((r.w as f64 * k) as u32).min(to.work.w), ((r.h as f64 * k) as u32).min(to.work.h));
    let frac = |off: i32, free: i64| if free > 0 { (off as f64 / free as f64).clamp(0.0, 1.0) } else { 0.0 };
    let fx = frac(r.x - from.work.x, from.work.w as i64 - r.w as i64);
    let fy = frac(r.y - from.work.y, from.work.h as i64 - r.h as i64);
    Rect {
        x: to.work.x + ((to.work.w - w) as f64 * fx).round() as i32,
        y: to.work.y + ((to.work.h - h) as f64 * fy).round() as i32,
        w,
        h,
    }
}
