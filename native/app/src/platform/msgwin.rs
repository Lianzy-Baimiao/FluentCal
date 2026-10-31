//! A hidden window for messages that aren't about a visible window: the single-instance
//! signal (a second start brings up the settings, like 0.2) and global hotkeys.
//!
//! Single instance follows tauri-plugin-single-instance exactly (mutex "<id>-sim", window class
//! "<id>-sic", title "<id>-siw", arguments by WM_COPYDATA), so the native release and 0.2 never
//! run at the same time on the same data, whichever starts first.

use std::sync::OnceLock;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::COPYDATASTRUCT;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone)]
pub enum MsgEvent {
    /// Another start of the program (its command line arguments).
    SecondInstance(Vec<String>),
    /// Explorer (the taskbar) started again.
    TaskbarCreated,
    /// A registered hotkey (its id).
    Hotkey(i32),
}

static ON_EVENT: OnceLock<Box<dyn Fn(MsgEvent) + Send + Sync>> = OnceLock::new();
static HWND_MSG: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static MSG_TASKBAR_CREATED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

const COPYDATA_NUL: usize = 1543;
const COPYDATA_PIPE: usize = 1542;

/// The instance name: the installed program's identifier (shared with 0.2). Debug builds use
/// their own name, so they run next to the installed program; FLUENTCAL_INSTANCE overrides.
pub fn instance_id() -> String {
    std::env::var("FLUENTCAL_INSTANCE").unwrap_or_else(|_| {
        if cfg!(debug_assertions) { "com.fluentcal.native.dev".into() } else { "com.fluentcal.desktop".into() }
    })
}

/// Claims the single instance. Returns false after handing our arguments to the running
/// instance: the caller exits.
pub fn claim(id: &str) -> bool {
    unsafe {
        let _ = CreateMutexW(None, true, &HSTRING::from(format!("{id}-sim")));
        if GetLastError() != ERROR_ALREADY_EXISTS {
            return true;
        }
        let (class, title) = (HSTRING::from(format!("{id}-sic")), HSTRING::from(format!("{id}-siw")));
        let mut hwnd = None;
        for _ in 0..40 {
            if let Ok(h) = FindWindowW(&class, &title) {
                hwnd = Some(h);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        if let Some(h) = hwnd {
            let mut pid = 0;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid != 0 {
                let _ = AllowSetForegroundWindow(pid);
            }
            let mut data = std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned();
            for a in std::env::args_os() {
                data.push('\0');
                data.push_str(&a.to_string_lossy());
            }
            let cds = COPYDATASTRUCT { dwData: COPYDATA_NUL, cbData: data.len() as u32, lpData: data.as_ptr() as *mut _ };
            let _ = SendMessageTimeoutW(h, WM_COPYDATA, WPARAM(0), LPARAM(&cds as *const _ as isize), SMTO_NORMAL, 10_000, None);
        }
        false
    }
}

/// Creates the window (after `claim` succeeded). `f` runs on the UI thread inside the window
/// procedure: post the work to the event loop.
pub fn create(id: &str, f: impl Fn(MsgEvent) + Send + Sync + 'static) {
    let _ = ON_EVENT.set(Box::new(f));
    unsafe {
        let class = HSTRING::from(format!("{id}-sic"));
        let title = HSTRING::from(format!("{id}-siw"));
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: hinst.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);
        // As tauri-plugin-single-instance: a tool window (never in the taskbar), visible but
        // layered and transparent so it never shows.
        let Ok(hwnd) = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            &class,
            &title,
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst.into()),
            None,
        ) else {
            return;
        };
        SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_VISIBLE.0 | WS_POPUP.0) as isize);
        HWND_MSG.store(hwnd.0 as isize, std::sync::atomic::Ordering::Relaxed);
        MSG_TASKBAR_CREATED.store(RegisterWindowMessageW(windows::core::w!("TaskbarCreated")), std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn hwnd() -> Option<HWND> {
    let v = HWND_MSG.load(std::sync::atomic::Ordering::Relaxed);
    (v != 0).then_some(HWND(v as *mut _))
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COPYDATA => {
            let cds = &*(lp.0 as *const COPYDATASTRUCT);
            let sep = match cds.dwData {
                COPYDATA_NUL => '\0',
                COPYDATA_PIPE => '|',
                _ => return LRESULT(1),
            };
            let bytes = if cds.lpData.is_null() { &[][..] } else { std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize) };
            let bytes = if sep == '|' { bytes.split(|b| *b == 0).next().unwrap_or_default() } else { bytes };
            let text = String::from_utf8_lossy(bytes);
            let args = text.split(sep).skip(1).map(str::to_owned).collect();
            if let Some(f) = ON_EVENT.get() {
                f(MsgEvent::SecondInstance(args));
            }
            LRESULT(1)
        }
        WM_HOTKEY => {
            if let Some(f) = ON_EVENT.get() {
                f(MsgEvent::Hotkey(wp.0 as i32));
            }
            LRESULT(0)
        }
        super::tray::WM_TRAY => {
            super::tray::on_message(hwnd, lp.0 as usize);
            LRESULT(0)
        }
        m if m != 0 && m == MSG_TASKBAR_CREATED.load(std::sync::atomic::Ordering::Relaxed) => {
            super::tray::readd();
            if let Some(f) = ON_EVENT.get() {
                f(MsgEvent::TaskbarCreated);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Registers `accel` ("Ctrl+Alt+D", 0.2's format) as hotkey `id`; Err with the reason.
pub fn register_hotkey(id: i32, accel: &str) -> Result<(), String> {
    let (mods, vk) = parse_accel(accel).ok_or_else(|| "无法识别的快捷键".to_owned())?;
    let h = hwnd().ok_or_else(|| "程序窗口还没有创建".to_owned())?;
    unsafe { RegisterHotKey(Some(h), id, mods | MOD_NOREPEAT, vk) }.map_err(|_| "已被其他程序占用".to_owned())
}

pub fn unregister_hotkey(id: i32) {
    if let Some(h) = hwnd() {
        unsafe {
            let _ = UnregisterHotKey(Some(h), id);
        }
    }
}

/// "Ctrl+Alt+D" → modifiers and virtual key (0.2's shortcut.ts tokens).
pub fn parse_accel(s: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    let mut mods = HOT_KEY_MODIFIERS(0);
    let mut key = None;
    for t in s.split('+').map(str::trim).filter(|t| !t.is_empty()) {
        match t.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => mods |= MOD_CONTROL,
            "alt" | "option" => mods |= MOD_ALT,
            "shift" => mods |= MOD_SHIFT,
            "super" | "win" | "meta" | "cmd" | "command" => mods |= MOD_WIN,
            _ => key = Some(vk_of(t)?),
        }
    }
    Some((mods, key?))
}

fn vk_of(t: &str) -> Option<u32> {
    let up = t.to_ascii_uppercase();
    let b = up.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_uppercase() || b[0].is_ascii_digit()) {
        return Some(u32::from(b[0]));
    }
    if let Some(n) = up.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()).filter(|n| (1..=24).contains(n)) {
        return Some(0x70 + n - 1);
    }
    if let Some(n) = up.strip_prefix("NUM").and_then(|n| n.parse::<u32>().ok()).filter(|n| *n <= 9) {
        return Some(0x60 + n);
    }
    Some(match up.as_str() {
        "SPACE" => 0x20,
        "ENTER" => 0x0D,
        "TAB" => 0x09,
        "ESCAPE" => 0x1B,
        "BACKSPACE" => 0x08,
        "DELETE" => 0x2E,
        "INSERT" => 0x2D,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "UP" => 0x26,
        "DOWN" => 0x28,
        "LEFT" => 0x25,
        "RIGHT" => 0x27,
        "PRINTSCREEN" => 0x2C,
        "PAUSE" => 0x13,
        "SCROLLLOCK" => 0x91,
        "MINUS" => 0xBD,
        "EQUAL" => 0xBB,
        "BRACKETLEFT" => 0xDB,
        "BRACKETRIGHT" => 0xDD,
        "BACKSLASH" => 0xDC,
        "SEMICOLON" => 0xBA,
        "QUOTE" => 0xDE,
        "BACKQUOTE" => 0xC0,
        "COMMA" => 0xBC,
        "PERIOD" => 0xBE,
        "SLASH" => 0xBF,
        _ => return None,
    })
}
