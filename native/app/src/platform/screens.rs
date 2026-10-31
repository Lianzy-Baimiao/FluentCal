//! Screens for the desk window (0.2's screens.rs): enumerate monitors, read and place the
//! window. The decisions (where the calendar goes) are in fluentcal_core::layout.

use fluentcal_core::layout::{Mon, Rect};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, IsIconic, IsWindowVisible, SetWindowPos, MONITORINFOF_PRIMARY, SWP_NOACTIVATE, SWP_NOZORDER};

fn rect(r: RECT) -> Rect {
    Rect { x: r.left, y: r.top, w: (r.right - r.left).max(0) as u32, h: (r.bottom - r.top).max(0) as u32 }
}

/// All monitors in physical pixels, in Windows' enumeration order.
pub fn monitors() -> Vec<Mon> {
    unsafe extern "system" fn each(h: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let out = &mut *(data.0 as *mut Vec<Mon>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(h, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
            let (mut dx, mut dy) = (96u32, 96u32);
            let _ = GetDpiForMonitor(h, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
            let name = String::from_utf16_lossy(&info.szDevice).trim_end_matches('\0').to_owned();
            out.push(Mon {
                name,
                rect: rect(info.monitorInfo.rcMonitor),
                work: rect(info.monitorInfo.rcWork),
                scale: f64::from(dx) / 96.0,
                primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        BOOL(1)
    }
    let mut out: Vec<Mon> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(each), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

/// Outer rectangle in physical screen pixels.
pub fn window_rect(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut r).ok()? };
    Some(rect(r))
}

pub fn is_visible(hwnd: HWND) -> bool {
    unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() }
}

/// Sets position and size. The position is set again at the end because moving onto a screen
/// with a different scale makes Windows resize the window on the way.
pub fn place(hwnd: HWND, r: Rect) {
    unsafe {
        let _ = SetWindowPos(hwnd, None, r.x, r.y, r.w as i32, r.h as i32, SWP_NOZORDER | SWP_NOACTIVATE);
        let _ = SetWindowPos(hwnd, None, r.x, r.y, r.w as i32, r.h as i32, SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

/// Number as shown in Windows display settings (\\.\DISPLAY2 -> 2).
pub fn number(name: &str, i: usize) -> usize {
    name.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(i + 1)
}

/// "显示器 2（主）  2560×1600" (tray menu, settings).
pub fn title(m: &Mon, i: usize) -> String {
    format!("显示器 {}{}  {}×{}", number(&m.name, i), if m.primary { "（主）" } else { "" }, m.rect.w, m.rect.h)
}
