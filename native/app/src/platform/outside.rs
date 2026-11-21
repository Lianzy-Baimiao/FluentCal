//! Light dismiss outside our windows: the calendar and the reminder cards never take the
//! focus, so a click on another window doesn't reach them. While one of their menus or
//! flyouts is open, a low-level mouse hook (this thread, removed when none is open) reports
//! presses outside that window. The floating to-do column that closes on a press outside it
//! (设置 › 日历) watches the same way while it is open.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetAncestor, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, SetWindowsHookExW, UnhookWindowsHookEx, WindowFromPoint, GA_ROOT, GWL_EXSTYLE, HHOOK, MSLLHOOKSTRUCT,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_NCLBUTTONDOWN, WM_NCRBUTTONDOWN, WM_RBUTTONDOWN, WS_EX_NOACTIVATE,
};

struct Watch {
    key: &'static str,
    hwnd: Box<dyn Fn() -> Option<HWND>>,
    outside: Rc<dyn Fn()>,
    /// What counts: a press off the window's rectangle, or (true) one that goes to anything
    /// but the window — another program's window lying over it too — and not to another
    /// window of ours.
    elsewhere: bool,
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
            let hit: Vec<Rc<dyn Fn()>> =
                WATCHES.with(|w| w.borrow().iter().filter(|w| if w.elsewhere { elsewhere((w.hwnd)(), pt) } else { !inside((w.hwnd)(), pt) }).map(|w| w.outside.clone()).collect());
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

/// The press at `pt` goes to a window that is neither `h` nor another one of this program
/// (the editor, the settings, a reminder card, the tray menu), nor one that doesn't take the
/// focus: the candidates of an input method and the on-screen keyboard are such windows of
/// other programs, and a press there is part of typing into ours.
fn elsewhere(h: Option<HWND>, pt: POINT) -> bool {
    let Some(h) = h else { return false };
    unsafe {
        let top = GetAncestor(WindowFromPoint(pt), GA_ROOT);
        if top == h {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(top, Some(&mut pid));
        pid != GetCurrentProcessId() && GetWindowLongPtrW(top, GWL_EXSTYLE) as u32 & WS_EX_NOACTIVATE.0 == 0
    }
}

type Target = (Box<dyn Fn() -> Option<HWND>>, Rc<dyn Fn()>);

/// Watches for presses outside the window `hwnd` gives (`outside` runs on this thread), or
/// stops watching for `key` (None).
pub fn watch(key: &'static str, w: Option<Target>) {
    set(key, w, false);
}

/// The same for presses that go anywhere but to that window or another window of ours: a
/// window of another program counts where it lies over ours, too. (What a press on the
/// window itself means is the window's business.)
pub fn watch_elsewhere(key: &'static str, w: Option<Target>) {
    set(key, w, true);
}

/// Windows takes a low-level hook away without a word when its thread didn't answer in time
/// (a moment of heavy work while the mouse moved), and nothing tells. A watch that stays for
/// long (the floating to-do column) gets a new hook now and then: every 30 s, from the app's
/// tick. Nothing happens while nothing is watched.
pub fn renew() {
    let Some(old) = HOOK.with(Cell::get) else { return };
    unsafe {
        let _ = UnhookWindowsHookEx(old);
        HOOK.with(|c| c.set(SetWindowsHookExW(WH_MOUSE_LL, Some(proc), None, 0).ok()));
    }
}

fn set(key: &'static str, w: Option<Target>, elsewhere: bool) {
    WATCHES.with(|list| {
        let mut list = list.borrow_mut();
        list.retain(|x| x.key != key);
        if let Some((hwnd, outside)) = w {
            list.push(Watch { key, hwnd, outside, elsewhere });
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
