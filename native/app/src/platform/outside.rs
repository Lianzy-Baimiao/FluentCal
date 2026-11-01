//! Light dismiss outside our windows: the calendar and the reminder cards never take the
//! focus, so a click on another window doesn't reach them. While one of their menus or
//! flyouts is open, a low-level mouse hook (this thread, removed when none is open) reports
//! presses outside that window.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetWindowRect, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_NCLBUTTONDOWN, WM_NCRBUTTONDOWN,
    WM_RBUTTONDOWN,
};

struct Watch {
    key: &'static str,
    hwnd: Box<dyn Fn() -> Option<HWND>>,
    outside: Rc<dyn Fn()>,
}

thread_local! {
    static HOOK: Cell<Option<HHOOK>> = const { Cell::new(None) };
    static WATCHES: RefCell<Vec<Watch>> = const { RefCell::new(Vec::new()) };
}

unsafe extern "system" fn proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let m = wparam.0 as u32;
        if matches!(m, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_NCLBUTTONDOWN | WM_NCRBUTTONDOWN) {
            let pt = (*(lparam.0 as *const MSLLHOOKSTRUCT)).pt;
            // Collected first: a callback may change the list.
            let hit: Vec<Rc<dyn Fn()>> = WATCHES.with(|w| w.borrow().iter().filter(|w| !inside((w.hwnd)(), pt)).map(|w| w.outside.clone()).collect());
            hit.iter().for_each(|f| f());
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn inside(h: Option<HWND>, pt: POINT) -> bool {
    let Some(h) = h else { return true };
    let mut r = RECT::default();
    unsafe { GetWindowRect(h, &mut r).is_ok() && pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom }
}

/// Watches for presses outside the window `hwnd` gives (`outside` runs on this thread), or
/// stops watching for `key` (None).
pub fn watch(key: &'static str, w: Option<(Box<dyn Fn() -> Option<HWND>>, Rc<dyn Fn()>)>) {
    WATCHES.with(|list| {
        let mut list = list.borrow_mut();
        list.retain(|x| x.key != key);
        if let Some((hwnd, outside)) = w {
            list.push(Watch { key, hwnd, outside });
        }
    });
    let any = WATCHES.with(|l| !l.borrow().is_empty());
    match (any, HOOK.with(Cell::get)) {
        (true, None) => {
            if let Ok(h) = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(proc), None, 0) } {
                HOOK.with(|c| c.set(Some(h)));
            }
        }
        (false, Some(h)) => {
            HOOK.with(|c| c.set(None));
            unsafe {
                let _ = UnhookWindowsHookEx(h);
            }
        }
        _ => {}
    }
}
