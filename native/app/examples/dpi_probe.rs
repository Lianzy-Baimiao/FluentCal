//! 跨屏缩放检查（TODO.md「跨屏拖动时日历大小失控」）：在临时数据文件夹上启动完整程序，只用窗口消息
//! 操作桌面日历，不动鼠标、不把窗口提到前面。
//!
//! 1. 拖动跨屏（启动后先做，和用户把日历拖到另一块屏幕一样）：WM_ENTERSIZEMOVE 后，在窗口大半
//!    还在原屏幕上时发 WM_DPICHANGED，建议矩形以抓取点为中心按新缩放算好、大半在新屏幕上，这正是
//!    Windows 11 拖动时的顺序。winit 0.30.13 在这里把窗口推回原屏幕，于是 DPI 与界面缩放对不上、
//!    尺寸越变越乱；修好后窗口落在建议矩形上，逻辑大小不变。
//! 2. 「显示在」两块缩放不同的屏幕之间来回移动：窗口的 DPI 与界面缩放一致，逻辑大小不变。
//!
//! 运行：cargo run --example dpi_probe（需要两块缩放不同、左右相邻的屏幕）。退出码 0 = 全部通过。

use fluentcal_app::{app, platform::desk as pd, platform::screens, shell, startup};
use fluentcal_core::layout::{Mon, Rect};
use slint::ComponentHandle;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::UI::HiDpi::{AreDpiAwarenessContextsEqual, GetDpiForWindow, GetWindowDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, SendMessageW, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER, WM_DPICHANGED, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE};

static FAILED: AtomicBool = AtomicBool::new(false);
static T0: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
const WM_GETDPISCALEDSIZE: u32 = 0x02E4;

fn ms() -> u128 {
    T0.get_or_init(Instant::now).elapsed().as_millis()
}

fn main() {
    ms();
    let dir = std::env::temp_dir().join(format!("fluentcal-dpi-probe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("data folder");
    std::env::set_var("FLUENTCAL_DATA_DIR", &dir);
    std::env::set_var("FLUENTCAL_INSTANCE", "com.fluentcal.dpiprobe");
    // Never left running: quits on its own if the probe gets stuck.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(120));
        eprintln!("[probe] timeout");
        std::process::exit(3);
    });
    std::thread::spawn(probe);
    startup::main();
    let _ = std::fs::remove_dir_all(&dir);
    std::process::exit(if FAILED.load(Ordering::Relaxed) { 1 } else { 0 });
}

fn hwnd() -> HWND {
    pd::desk_hwnd().expect("desk window")
}

fn win_rect() -> Rect {
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd(), &mut r);
    }
    Rect { x: r.left, y: r.top, w: (r.right - r.left) as u32, h: (r.bottom - r.top) as u32 }
}

/// The screen with the largest share of `r` (MonitorFromRect's rule).
fn majority(mons: &[Mon], r: Rect) -> Option<usize> {
    let area = |m: &Mon| {
        let w = (r.x + r.w as i32).min(m.rect.x + m.rect.w as i32) - r.x.max(m.rect.x);
        let h = (r.y + r.h as i32).min(m.rect.y + m.rect.h as i32) - r.y.max(m.rect.y);
        i64::from(w.max(0)) * i64::from(h.max(0))
    };
    (0..mons.len()).filter(|i| area(&mons[*i]) > 0).max_by_key(|i| area(&mons[*i]))
}

/// The scale factor Slint/winit render with (read on the UI thread).
fn ui_scale() -> f32 {
    let (tx, rx) = std::sync::mpsc::channel();
    app::post(move |app| {
        let _ = tx.send(app.desk.window.window().scale_factor());
    });
    rx.recv_timeout(Duration::from_secs(5)).unwrap_or(f32::NAN)
}

struct Snap {
    rect: Rect,
    dpi: u32,
    scale: f32,
}

fn snap() -> Snap {
    Snap { rect: win_rect(), dpi: unsafe { GetDpiForWindow(hwnd()) }, scale: ui_scale() }
}

fn fail(what: &str) {
    println!("    FAIL: {what}");
    FAILED.store(true, Ordering::Relaxed);
}

/// Logical width at the window's own DPI.
fn logical_w(s: &Snap) -> f64 {
    f64::from(s.rect.w) * 96.0 / f64::from(s.dpi)
}

/// The window's DPI is the scale it renders with, and it is mostly on screen `want` at that
/// screen's scale.
fn check(label: &str, mons: &[Mon], want: usize) -> Snap {
    let s = snap();
    let on = majority(mons, s.rect);
    println!(
        "[{:>6}] {label}: rect ({},{}) {}x{}  dpi {}  ui scale {:.3}  logical w {:.1}  mostly on {:?}",
        ms(),
        s.rect.x,
        s.rect.y,
        s.rect.w,
        s.rect.h,
        s.dpi,
        s.scale,
        logical_w(&s),
        on.map(|i| mons[i].name.as_str())
    );
    let dpi_scale = f64::from(s.dpi) / 96.0;
    if (f64::from(s.scale) - dpi_scale).abs() > 0.01 {
        fail(&format!("renders at {:.3} but the window's DPI is {}", s.scale, s.dpi));
    }
    if on != Some(want) {
        fail(&format!("expected mostly on {}", mons[want].name));
    }
    if (mons[want].scale - dpi_scale).abs() > 0.01 {
        fail(&format!("DPI {} doesn't match {} ({}%)", s.dpi, mons[want].name, mons[want].scale * 100.0));
    }
    s
}

/// Logs the DPI messages the desk window gets (installed above the app's own subclass).
unsafe extern "system" fn trace(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _: usize, _: usize) -> LRESULT {
    match msg {
        WM_DPICHANGED => {
            let r = &*(lp.0 as *const RECT);
            println!("[{:>6}]   WM_DPICHANGED dpi {} suggested ({},{}) {}x{}", ms(), wp.0 & 0xFFFF, r.left, r.top, r.right - r.left, r.bottom - r.top);
        }
        WM_GETDPISCALEDSIZE => {
            let s = &*(lp.0 as *const SIZE);
            println!("[{:>6}]   WM_GETDPISCALEDSIZE dpi {} size {}x{}", ms(), wp.0, s.cx, s.cy);
        }
        WM_ENTERSIZEMOVE => println!("[{:>6}]   WM_ENTERSIZEMOVE", ms()),
        WM_EXITSIZEMOVE => println!("[{:>6}]   WM_EXITSIZEMOVE", ms()),
        _ => {}
    }
    DefSubclassProc(h, msg, wp, lp)
}

/// Two side-by-side screens with different scales: (left, right).
fn pair(mons: &[Mon]) -> Option<(usize, usize)> {
    for (i, a) in mons.iter().enumerate() {
        for (j, b) in mons.iter().enumerate() {
            let shared = a.rect.x + a.rect.w as i32 == b.rect.x;
            let overlap = a.rect.y.max(b.rect.y) < (a.rect.y + a.rect.h as i32).min(b.rect.y + b.rect.h as i32);
            if i != j && shared && overlap && (a.scale - b.scale).abs() > 0.01 {
                return Some((i, j));
            }
        }
    }
    None
}

fn probe() {
    // The desk window, placed and settled.
    let until = Instant::now() + Duration::from_secs(30);
    while pd::desk_hwnd().is_none_or(|h| !screens::is_visible(h)) {
        if Instant::now() > until {
            fail("the desk window never showed");
            app::post(startup::quit);
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(1500));
    let mons = screens::monitors();
    for (i, m) in mons.iter().enumerate() {
        println!("screen {i}: {} ({},{}) {}x{} {}%{}", m.name, m.rect.x, m.rect.y, m.rect.w, m.rect.h, m.scale * 100.0, if m.primary { " primary" } else { "" });
    }
    let pmv2 = unsafe { AreDpiAwarenessContextsEqual(GetWindowDpiAwarenessContext(hwnd()), DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).as_bool() };
    println!("per-monitor v2: {pmv2}");
    if !pmv2 {
        fail("the desk window is not per-monitor v2 aware");
    }
    let Some((left, right)) = pair(&mons) else {
        println!("SKIP: needs two side-by-side screens with different scales");
        app::post(startup::quit);
        return;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let h = hwnd().0 as isize;
    app::post(move |_| {
        let ok = unsafe { SetWindowSubclass(HWND(h as *mut _), Some(trace), 0xD917, 0).as_bool() };
        let _ = tx.send(ok);
    });
    if rx.recv_timeout(Duration::from_secs(5)) != Ok(true) {
        fail("could not trace the desk window");
    }

    let start = majority(&mons, win_rect()).unwrap_or(left);
    let other = |i: usize| if i == left { right } else { left };
    let base = check("start", &mons, start);
    let base_w = logical_w(&base);
    let mut on = start;

    // A drag across the edge, as Windows 11 runs it; first, right after startup.
    println!("== drag across the edge");
    let (top, bottom) = (mons[left].rect.y.max(mons[right].rect.y), (mons[left].rect.y + mons[left].rect.h as i32).min(mons[right].rect.y + mons[right].rect.h as i32));
    let edge = mons[right].rect.x;
    for step in 1..=6 {
        let (from, to) = (on, other(on));
        let (sa, sb) = (mons[from].scale, mons[to].scale);
        let k = sb / sa;
        let w = (base_w * sa).round() as i32;
        let h = (f64::from(base.rect.h) * 96.0 / f64::from(base.dpi) * sa).round() as i32;
        let y = top + ((bottom - top) - (h as f64 * k.max(1.0)) as i32).max(0) / 2;
        // 48 % over the edge, the rest still on the screen it comes from.
        let over = (f64::from(w) * 0.48).round() as i32;
        let x = if to == right { edge + over - w } else { edge - over };
        unsafe {
            let _ = SetWindowPos(hwnd(), None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
        }
        std::thread::sleep(Duration::from_millis(500));
        check(&format!("drag{step} on {}", mons[from].name), &mons, from);
        // Held near the side it was grabbed on: the trailing edge when the window grows, the
        // leading one when it shrinks (where Windows' rule puts the window on the new screen).
        let grab_from_to_side = if k > 1.0 { 0.9 } else { 0.1 };
        let gx = if to == right { x + w - (f64::from(w) * grab_from_to_side) as i32 } else { x + (f64::from(w) * grab_from_to_side) as i32 };
        let gy = y + 20;
        let (nw, nh) = ((f64::from(w) * k).round() as i32, (f64::from(h) * k).round() as i32);
        let nl = gx - (f64::from(gx - x) * k).round() as i32;
        let nt = gy - (f64::from(gy - y) * k).round() as i32;
        let suggested = RECT { left: nl, top: nt, right: nl + nw, bottom: nt + nh };
        let sr = Rect { x: nl, y: nt, w: nw as u32, h: nh as u32 };
        if majority(&mons, sr) != Some(to) {
            fail("test setup: the suggested rect isn't mostly on the new screen");
            break;
        }
        unsafe {
            SendMessageW(hwnd(), WM_ENTERSIZEMOVE, None, None);
            let dpi = (sb * 96.0).round() as usize;
            SendMessageW(hwnd(), WM_DPICHANGED, Some(WPARAM(dpi | (dpi << 16))), Some(LPARAM(&suggested as *const RECT as isize)));
        }
        std::thread::sleep(Duration::from_millis(300));
        let got = win_rect();
        if got != sr {
            println!("    moved to ({},{}) {}x{}, Windows suggested ({},{}) {}x{}", got.x, got.y, got.w, got.h, sr.x, sr.y, sr.w, sr.h);
            fail("the window didn't take the rect Windows suggested");
        }
        unsafe {
            SendMessageW(hwnd(), WM_EXITSIZEMOVE, None, None);
        }
        std::thread::sleep(Duration::from_millis(500));
        let s = check(&format!("drag{step} -> {}", mons[to].name), &mons, to);
        if (logical_w(&s) - base_w).abs() > 2.0 {
            fail(&format!("logical width {:.1}, was {base_w:.1}", logical_w(&s)));
        }
        on = to;
    }

    // 「显示在」 back and forth.
    println!("== 显示在 {} <-> {}", mons[left].name, mons[right].name);
    for step in 1..=4 {
        let to = other(on);
        app::post(move |app| shell::move_to_screen(app, to));
        std::thread::sleep(Duration::from_millis(800));
        let s = check(&format!("move{step} -> {}", mons[to].name), &mons, to);
        if (logical_w(&s) - base_w).abs() > 2.0 {
            fail(&format!("logical width {:.1}, was {base_w:.1}", logical_w(&s)));
        }
        on = to;
    }

    println!("{}", if FAILED.load(Ordering::Relaxed) { "RESULT: FAIL" } else { "RESULT: PASS" });
    app::post(startup::quit);
}
