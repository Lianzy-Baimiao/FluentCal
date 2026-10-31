//! Tray icon on the hidden message window: left click summons the calendar, right click opens
//! the menu. A plain Win32 menu like 0.2's (muda): check marks, submenus, dark when Windows is
//! dark. (Slint's own tray menu has no check marks.)

use std::sync::Mutex;
use windows::core::{w, HSTRING, PCSTR};
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW};
use windows::Win32::UI::WindowsAndMessaging::*;

pub const WM_TRAY: u32 = WM_APP + 0x51;
const ID: u32 = 1;
const CMD_BASE: u32 = 1000;

#[derive(Debug, Clone)]
pub enum Item {
    Command { id: String, title: String },
    Check { id: String, title: String, checked: bool },
    Separator,
    Sub { title: String, items: Vec<Item> },
}

#[derive(Debug, Clone)]
pub enum TrayEvent {
    Click,
    Command(String),
}

struct State {
    hwnd: isize,
    tip: String,
    items: Vec<Item>,
    on_event: Option<Box<dyn Fn(TrayEvent) + Send>>,
}

static STATE: Mutex<State> = Mutex::new(State { hwnd: 0, tip: String::new(), items: Vec::new(), on_event: None });

fn data(hwnd: HWND, tip: &str) -> NOTIFYICONDATAW {
    let mut d = NOTIFYICONDATAW { cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32, hWnd: hwnd, uID: ID, ..Default::default() };
    d.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    d.uCallbackMessage = WM_TRAY;
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let (cx, cy) = (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON));
        d.hIcon = LoadImageW(Some(hinst.into()), PCWSTR_ICON_1, IMAGE_ICON, cx, cy, LR_DEFAULTCOLOR)
            .map(|h| HICON(h.0))
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))
            .unwrap_or_default();
    }
    for (i, c) in tip.encode_utf16().take(d.szTip.len() - 1).enumerate() {
        d.szTip[i] = c;
    }
    d
}

/// MAKEINTRESOURCE(1): the icon in res/app.rc.
const PCWSTR_ICON_1: windows::core::PCWSTR = windows::core::PCWSTR(1 as *const u16);

/// Adds the icon to the message window `hwnd` (msgwin) and sets the menu handler.
pub fn create(hwnd: HWND, tip: &str, on_event: impl Fn(TrayEvent) + Send + 'static) {
    allow_dark_menus();
    let mut st = STATE.lock().unwrap();
    st.hwnd = hwnd.0 as isize;
    st.tip = tip.to_owned();
    st.on_event = Some(Box::new(on_event));
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &data(hwnd, tip));
    }
}

/// Explorer restarted (TaskbarCreated): add the icon again.
pub fn readd() {
    let st = STATE.lock().unwrap();
    if st.hwnd != 0 {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &data(HWND(st.hwnd as *mut _), &st.tip));
        }
    }
}

pub fn set_tip(tip: &str) {
    let mut st = STATE.lock().unwrap();
    st.tip = tip.to_owned();
    if st.hwnd != 0 {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &data(HWND(st.hwnd as *mut _), tip));
        }
    }
}

/// The menu shown on the next right click.
pub fn set_menu(items: Vec<Item>) {
    STATE.lock().unwrap().items = items;
}

pub fn remove() {
    let st = STATE.lock().unwrap();
    if st.hwnd != 0 {
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data(HWND(st.hwnd as *mut _), ""));
        }
    }
}

fn emit(e: TrayEvent) {
    // Taken out while it runs: the handler may set the menu.
    let f = STATE.lock().unwrap().on_event.take();
    if let Some(f) = f {
        f(e);
        STATE.lock().unwrap().on_event = Some(f);
    }
}

/// From the message window procedure: the tray icon's callback message.
pub fn on_message(hwnd: HWND, lparam: usize) {
    match (lparam & 0xFFFF) as u32 {
        WM_LBUTTONUP => emit(TrayEvent::Click),
        WM_RBUTTONUP | WM_CONTEXTMENU => {
            if let Some(id) = show_menu(hwnd) {
                emit(TrayEvent::Command(id));
            }
        }
        _ => {}
    }
}

fn show_menu(hwnd: HWND) -> Option<String> {
    let items = STATE.lock().unwrap().items.clone();
    track(hwnd, &items)
}

/// A Win32 popup menu at the cursor, owned by `hwnd` (which comes to the foreground so the
/// menu closes on a click elsewhere). The chosen item's id.
pub fn track(hwnd: HWND, items: &[Item]) -> Option<String> {
    allow_dark_menus();
    let mut ids = Vec::new();
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        build(menu, items, &mut ids);
        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        // Required so the menu closes when the user clicks elsewhere (KB Q135788).
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN, pt.x, pt.y, Some(0), hwnd, None);
        let _ = PostMessageW(Some(hwnd), WM_NULL, Default::default(), Default::default());
        let _ = DestroyMenu(menu);
        let cmd = cmd.0 as u32;
        (cmd >= CMD_BASE).then(|| ids.get((cmd - CMD_BASE) as usize).cloned()).flatten()
    }
}

unsafe fn build(menu: HMENU, items: &[Item], ids: &mut Vec<String>) {
    for it in items {
        match it {
            Item::Separator => {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            }
            Item::Command { id, title } | Item::Check { id, title, .. } => {
                let checked = matches!(it, Item::Check { checked: true, .. });
                let flags = MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED };
                let cmd = CMD_BASE as usize + ids.len();
                ids.push(id.clone());
                let _ = AppendMenuW(menu, flags, cmd, &HSTRING::from(title.as_str()));
            }
            Item::Sub { title, items } => {
                if let Ok(sub) = CreatePopupMenu() {
                    build(sub, items, ids);
                    let _ = AppendMenuW(menu, MF_POPUP | MF_STRING, sub.0 as usize, &HSTRING::from(title.as_str()));
                }
            }
        }
    }
}

/// Lets Win32 popup menus follow the dark system theme (uxtheme SetPreferredAppMode, ordinal
/// 135, as tao does for 0.2's tray menu).
fn allow_dark_menus() {
    type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
    type Refresh = unsafe extern "system" fn();
    if windows_version::OsVersion::current().build < 18362 {
        return;
    }
    unsafe {
        let Ok(ux) = LoadLibraryW(w!("uxtheme.dll")) else { return };
        if let Some(f) = GetProcAddress(ux, PCSTR(135 as *const u8)) {
            let f: SetPreferredAppMode = std::mem::transmute(f);
            f(1); // AllowDark
        }
        if let Some(f) = GetProcAddress(ux, PCSTR(104 as *const u8)) {
            let f: Refresh = std::mem::transmute(f);
            f(); // RefreshImmersiveColorPolicyState
        }
    }
}
