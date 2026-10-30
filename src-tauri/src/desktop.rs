//! Pinning the desk window to the Windows desktop.
//!
//! Windows 11 24H2 renders the desktop (Progman) through DirectComposition, so the classic
//! trick of re-parenting a window into Progman/WorkerW no longer draws anything. Instead the
//! desk window stays a top-level WS_EX_TOOLWINDOW whose owner is Progman: owned windows always
//! sit above their owner, so the calendar survives "Show desktop", and a subclass forces
//! HWND_BOTTOM on every z-order change so clicking it never lifts it above other apps.

use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU8, Ordering},
    OnceLock,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use windows::core::{s, w, BOOL, PCWSTR};
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    DWMWCP_ROUND,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PinMode {
    /// On the desktop layer: below all windows, survives Show Desktop, not in taskbar/Alt+Tab.
    Desktop = 0,
    /// Always on top, not in taskbar.
    Top = 1,
    /// Ordinary window with a taskbar button.
    Normal = 2,
}

impl PinMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "top" => Self::Top,
            "normal" => Self::Normal,
            _ => Self::Desktop,
        }
    }

    fn current() -> Self {
        match MODE.load(Ordering::Relaxed) {
            1 => Self::Top,
            2 => Self::Normal,
            _ => Self::Desktop,
        }
    }
}

static MODE: AtomicU8 = AtomicU8::new(0);
static CLICK_THROUGH: AtomicBool = AtomicBool::new(false);
static ACRYLIC: AtomicBool = AtomicBool::new(false);
/// Temporarily raised by the hotkey/tray; dropped back to the desktop layer on deactivation.
static SUMMONED: AtomicBool = AtomicBool::new(false);
/// The user is typing in the calendar (see `activate_for_keyboard`); cleared on deactivation.
static KEYBOARD: AtomicBool = AtomicBool::new(false);
/// Setting desk.noActivate.
static NO_ACTIVATE: AtomicBool = AtomicBool::new(true);
static DESK: AtomicIsize = AtomicIsize::new(0);
static MSG_TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static APP: OnceLock<AppHandle> = OnceLock::new();

const SUBCLASS_ID: usize = 0xFC01;
const WM_APP_RESTORE_LAYER: u32 = WM_APP + 1;
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

fn desk_hwnd() -> Option<HWND> {
    let v = DESK.load(Ordering::Relaxed);
    (v != 0).then_some(HWND(v as *mut _))
}

fn progman() -> Option<HWND> {
    unsafe { FindWindowW(w!("Progman"), PCWSTR::null()).ok().filter(|h| !h.is_invalid()) }
}

/// Subclasses the desk window. Call once per window, on the main thread.
pub fn install(app: &AppHandle, win: &WebviewWindow) {
    let _ = APP.set(app.clone());
    let Ok(hwnd) = win.hwnd() else { return };
    DESK.store(hwnd.0 as isize, Ordering::Relaxed);
    unsafe {
        MSG_TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::Relaxed);
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0);
    }
}

pub fn set_mode(win: &WebviewWindow, mode: PinMode) {
    MODE.store(mode as u8, Ordering::Relaxed);
    SUMMONED.store(false, Ordering::Relaxed);
    let _ = win.set_skip_taskbar(mode != PinMode::Normal);
    let Some(hwnd) = desk_hwnd() else { return };
    unsafe {
        let owner = if mode == PinMode::Desktop { progman() } else { None };
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, owner.map_or(0, |h| h.0 as isize));
        refresh_ex_style(hwnd);
        let after = match mode {
            PinMode::Desktop if test_raised() => HWND_TOPMOST,
            PinMode::Desktop => HWND_BOTTOM,
            PinMode::Top => HWND_TOPMOST,
            PinMode::Normal => HWND_NOTOPMOST,
        };
        let _ = SetWindowPos(hwnd, Some(after), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

/// Whether the desk window belongs in the topmost band: only in "always on top" mode.
fn wants_topmost() -> bool {
    match PinMode::current() {
        PinMode::Top => true,
        PinMode::Desktop => test_raised(),
        PinMode::Normal => false,
    }
}

/// Extended style the desk window must have for the current mode. tao rewrites the whole
/// extended style whenever one of its own flags changes, so this is enforced in
/// WM_STYLECHANGING as well. That message also comes with SetWindowPos(HWND_TOPMOST), so
/// WS_EX_TOPMOST must follow the mode here, or the window could never become topmost.
fn fix_ex(_old: u32, new: u32) -> u32 {
    let mut ex = new;
    if PinMode::current() == PinMode::Normal {
        ex = (ex | WS_EX_APPWINDOW.0) & !WS_EX_TOOLWINDOW.0;
    } else {
        ex = (ex | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
    }
    if CLICK_THROUGH.load(Ordering::Relaxed) {
        ex |= WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0;
    } else {
        ex &= !(WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0);
    }
    if no_activate() {
        ex |= WS_EX_NOACTIVATE.0;
    } else {
        ex &= !WS_EX_NOACTIVATE.0;
    }
    if wants_topmost() {
        ex | WS_EX_TOPMOST.0
    } else {
        ex & !WS_EX_TOPMOST.0
    }
}

/// On the desktop layer clicks must not activate the calendar: activating any window ends
/// Windows' "Show desktop" state, so the next Win+D would not bring the windows back.
/// Keyboard input (typing in a box, hotkey summon) activates it explicitly.
fn no_activate() -> bool {
    PinMode::current() == PinMode::Desktop
        && NO_ACTIVATE.load(Ordering::Relaxed)
        && !SUMMONED.load(Ordering::Relaxed)
        && !KEYBOARD.load(Ordering::Relaxed)
}

pub fn set_no_activate(enable: bool) {
    NO_ACTIVATE.store(enable, Ordering::Relaxed);
    if let Some(hwnd) = desk_hwnd() {
        unsafe { refresh_ex_style(hwnd) };
    }
}

/// Test hook: FLUENTCAL_TEST_RAISED=1 keeps every desktop-mode rule (owner, tool window,
/// no-activate) but puts the window in the topmost band instead of the bottom, so real
/// mouse tests can reach it without uncovering the user's desktop.
fn test_raised() -> bool {
    static V: OnceLock<bool> = OnceLock::new();
    *V.get_or_init(|| std::env::var("FLUENTCAL_TEST_RAISED").is_ok_and(|v| v == "1"))
}

/// Lets the calendar take keyboard focus (the user started typing in it).
pub fn activate_for_keyboard(win: &WebviewWindow) {
    if is_foreground() {
        return;
    }
    KEYBOARD.store(true, Ordering::Relaxed);
    if let Some(hwnd) = desk_hwnd() {
        unsafe { refresh_ex_style(hwnd) };
    }
    let _ = win.set_focus();
}

unsafe fn refresh_ex_style(hwnd: HWND) {
    let cur = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
    let want = fix_ex(cur, cur);
    if want == cur {
        return;
    }
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want as isize);
    if want & WS_EX_LAYERED.0 != 0 {
        // A layered window stays invisible until its attributes are set.
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA);
    }
    let _ = SetWindowPos(
        hwnd,
        None,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_WINDOWPOSCHANGING => {
            let pos = &mut *(lp.0 as *mut WINDOWPOS);
            if pos.flags.0 & SWP_NOZORDER.0 == 0
                && PinMode::current() == PinMode::Desktop
                && !SUMMONED.load(Ordering::Relaxed)
                && !test_raised()
            {
                pos.hwndInsertAfter = HWND_BOTTOM;
            }
        }
        WM_STYLECHANGING if wp.0 as i32 == GWL_EXSTYLE.0 => {
            let ss = &mut *(lp.0 as *mut STYLESTRUCT);
            ss.styleNew = fix_ex(ss.styleOld, ss.styleNew);
        }
        WM_ACTIVATE => {
            let inactive = (wp.0 & 0xFFFF) as u32 == WA_INACTIVE;
            if inactive && (SUMMONED.load(Ordering::Relaxed) || KEYBOARD.load(Ordering::Relaxed)) {
                let _ = PostMessageW(Some(hwnd), WM_APP_RESTORE_LAYER, WPARAM(0), LPARAM(0));
            }
        }
        WM_APP_RESTORE_LAYER => {
            KEYBOARD.store(false, Ordering::Relaxed);
            restore_layer();
            refresh_ex_style(hwnd);
            return LRESULT(0);
        }
        // PBT_APMRESUMESUSPEND (7) / PBT_APMRESUMEAUTOMATIC (0x12): back from sleep.
        WM_POWERBROADCAST if matches!(wp.0, 7 | 0x12) => {
            if let Some(app) = APP.get() {
                crate::on_resume(app);
            }
        }
        WM_SETTINGCHANGE | WM_DWMCOLORIZATIONCOLORCHANGED | WM_SYSCOLORCHANGE => {
            if let Some(app) = APP.get() {
                let _ = app.emit("system://theme", ());
            }
        }
        WM_DISPLAYCHANGE => {
            // A screen was added, removed or rearranged. Windows itself moves windows around
            // afterwards (screen waking up, "remember window locations"), so check twice.
            std::thread::spawn(|| {
                for wait in [1000, 3000] {
                    std::thread::sleep(Duration::from_millis(wait));
                    if let Some(app) = APP.get() {
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || crate::screens::on_display_change(&handle));
                    }
                }
            });
        }
        WM_EXITSIZEMOVE => {
            // End of a drag or resize by the user (also through startDragging): the new home.
            // Saved outside this window procedure, which tao is still running.
            std::thread::spawn(|| {
                std::thread::sleep(Duration::from_millis(50));
                if let Some(app) = APP.get() {
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || crate::screens::save_home(&handle));
                }
            });
        }
        m if m != 0 && m == MSG_TASKBAR_CREATED.load(Ordering::Relaxed) => reattach_later(),
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

/// Explorer restarted: wait for the new Progman, then re-apply ownership and layer.
fn reattach_later() {
    std::thread::spawn(|| {
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(500));
            if progman().is_some() {
                if let Some(app) = APP.get() {
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || crate::apply_desk_settings(&handle));
                }
                return;
            }
        }
    });
}

pub fn is_foreground() -> bool {
    desk_hwnd().is_some_and(|h| unsafe { GetForegroundWindow() } == h)
}

/// Window that was in front before the hotkey summoned the calendar.
static PREV_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

/// Hotkey pressed again: send the calendar back and give keyboard focus back to the window
/// the user was in, so typing doesn't go to the (now hidden) calendar.
pub fn dismiss() {
    let prev = PREV_FOREGROUND.swap(0, Ordering::Relaxed);
    restore_layer();
    if prev != 0 {
        let h = HWND(prev as *mut _);
        unsafe {
            if IsWindow(Some(h)).as_bool() && IsWindowVisible(h).as_bool() && Some(h) != desk_hwnd() {
                let _ = SetForegroundWindow(h);
            }
        }
    }
}

pub fn restore_layer() {
    if !SUMMONED.swap(false, Ordering::Relaxed) {
        return;
    }
    if let (Some(hwnd), PinMode::Desktop) = (desk_hwnd(), PinMode::current()) {
        unsafe {
            let after = if test_raised() { HWND_TOPMOST } else { HWND_BOTTOM };
            let _ = SetWindowPos(hwnd, Some(after), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            refresh_ex_style(hwnd);
        }
    }
}

/// Show the calendar in front of everything and focus it. In desktop mode it drops back to
/// the desktop layer as soon as it loses focus.
pub fn summon(win: &WebviewWindow) {
    let fg = unsafe { GetForegroundWindow() };
    if Some(fg) != desk_hwnd() {
        PREV_FOREGROUND.store(fg.0 as isize, Ordering::Relaxed);
    }
    let _ = win.show();
    if PinMode::current() == PinMode::Desktop {
        if let Some(hwnd) = desk_hwnd() {
            SUMMONED.store(true, Ordering::Relaxed);
            unsafe {
                refresh_ex_style(hwnd); // drop WS_EX_NOACTIVATE so it can take focus
                // In front of the other windows, but not "always on top": it is a desktop calendar.
                let _ = SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            }
        }
    }
    let _ = win.set_focus();
    if SUMMONED.load(Ordering::Relaxed) {
        // Without focus there is no WM_ACTIVATE(WA_INACTIVE) to send it back to the desktop.
        let app = win.app_handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            let _ = app.run_on_main_thread(|| {
                if !is_foreground() {
                    restore_layer();
                }
            });
        });
    }
}

/// Hotkey behaviour: summon when hidden or in the background; otherwise send it back.
pub fn toggle(win: &WebviewWindow) {
    let visible = win.is_visible().unwrap_or(false);
    if !(visible && is_foreground()) {
        summon(win);
        return;
    }
    match PinMode::current() {
        PinMode::Desktop if SUMMONED.load(Ordering::Relaxed) => dismiss(),
        PinMode::Desktop => summon(win),
        _ => {
            let _ = win.hide();
        }
    }
}

pub fn set_click_through(enable: bool) {
    CLICK_THROUGH.store(enable, Ordering::Relaxed);
    if let Some(hwnd) = desk_hwnd() {
        unsafe { refresh_ex_style(hwnd) };
    }
}

/// Acrylic blur behind the desk window. Uses the accent policy API rather than the DWM
/// system backdrop because the latter turns opaque whenever the window is inactive, and a
/// desktop calendar is inactive almost all the time.
pub fn set_acrylic(enable: bool) {
    let Some(hwnd) = desk_hwnd() else { return };
    let was = ACRYLIC.swap(enable, Ordering::Relaxed);
    unsafe {
        if enable || was {
            set_accent(hwnd, if enable { 4 } else { 0 });
        }
        let corner = if enable { DWMWCP_ROUND } else { DWMWCP_DONOTROUND };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of_val(&corner) as u32,
        );
        let none = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &none as *const _ as *const _, 4);
    }
}

#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    gradient: u32,
    animation: u32,
}

#[repr(C)]
struct CompositionData {
    attrib: u32,
    data: *mut std::ffi::c_void,
    size: usize,
}

unsafe fn set_accent(hwnd: HWND, state: u32) {
    type Swca = unsafe extern "system" fn(HWND, *mut CompositionData) -> BOOL;
    let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return };
    let Some(proc_addr) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return };
    let swca: Swca = std::mem::transmute(proc_addr);
    // The visible tint comes from the CSS layer so it can follow the theme and opacity
    // settings; acrylic misbehaves with a fully transparent gradient, hence alpha 1.
    let mut policy = AccentPolicy { state, flags: 0, gradient: 0x0100_0000, animation: 0 };
    let mut data = CompositionData {
        attrib: 19, // WCA_ACCENT_POLICY
        data: &mut policy as *mut _ as *mut _,
        size: std::mem::size_of::<AccentPolicy>(),
    };
    let _ = swca(hwnd, &mut data);
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub build: u32,
    pub dark: bool,
    pub transparency: bool,
    pub accent: Option<String>,
    /// light3, light2, light1, accent, dark1, dark2, dark3
    pub accent_palette: Vec<String>,
}

const PERSONALIZE: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");

pub fn system_dark() -> bool {
    reg_dword(PERSONALIZE, w!("AppsUseLightTheme")).is_some_and(|v| v == 0)
}

pub fn system_info() -> SystemInfo {
    let mut transparency = reg_dword(PERSONALIZE, w!("EnableTransparency")).map_or(true, |v| v != 0);
    let mut palette = Vec::with_capacity(7);
    if let Ok(ui) = UISettings::new() {
        for t in [
            UIColorType::AccentLight3,
            UIColorType::AccentLight2,
            UIColorType::AccentLight1,
            UIColorType::Accent,
            UIColorType::AccentDark1,
            UIColorType::AccentDark2,
            UIColorType::AccentDark3,
        ] {
            if let Ok(c) = ui.GetColorValue(t) {
                palette.push(format!("#{:02X}{:02X}{:02X}", c.R, c.G, c.B));
            }
        }
        if let Ok(fx) = ui.AdvancedEffectsEnabled() {
            transparency = fx;
        }
    }
    if palette.len() != 7 {
        palette.clear();
    }
    let accent = palette.get(3).cloned().or_else(|| {
        // DWM stores the colour as 0xAABBGGRR
        reg_dword(w!(r"Software\Microsoft\Windows\DWM"), w!("AccentColor"))
            .map(|v| format!("#{:02X}{:02X}{:02X}", v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF))
    });
    SystemInfo {
        build: windows_version::OsVersion::current().build,
        dark: system_dark(),
        transparency,
        accent,
        accent_palette: palette,
    }
}

fn reg_dword(key: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut _ as *mut _),
            Some(&mut size),
        )
    };
    r.is_ok().then_some(data)
}
