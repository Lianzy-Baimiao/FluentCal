//! Reminder popup (REM-03): a small card stack in the bottom-right corner of the primary
//! monitor with 完成 / 稍后提醒 / 关闭. It never takes focus, so it doesn't interrupt typing
//! or end "Show desktop".

use crate::reminder::{Reminder, Reminders};
use crate::store::Store;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "reminder";
/// Card height incl. the gap between cards (logical px).
const CARD_H: f64 = 136.0;
const WIDTH: f64 = 380.0;
const MAX_CARDS: usize = 3;

/// settings.reminders.style: "toast" (Windows notification), "popup" or "both".
pub fn style(app: &AppHandle) -> String {
    app.state::<Arc<Store>>().setting_str("/reminders/style", "both")
}

pub fn load_snoozed(file: &Path) -> Vec<Reminder> {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_snoozed(file: &Path, items: &[Reminder]) {
    let _ = std::fs::write(file, serde_json::to_string(items).unwrap_or_default());
}

/// Queues a fired reminder and shows the popup (from the reminder thread).
pub fn show(app: &AppHandle, r: &Reminder, beep: bool) {
    let state = app.state::<Arc<Reminders>>();
    {
        let mut g = state.inner.lock().unwrap();
        g.popup.retain(|x| x.key != r.key);
        g.popup.push(r.clone());
    }
    if beep {
        // No toast means no sound from Windows; play the standard notification sound.
        unsafe {
            let _ = windows::Win32::System::Diagnostics::Debug::MessageBeep(windows::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION);
        }
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || reflow(&handle));
}

/// Creates, resizes or hides the popup window to fit the queue.
fn reflow(app: &AppHandle) {
    let n = app.state::<Arc<Reminders>>().inner.lock().unwrap().popup.len();
    let win = app.get_webview_window(LABEL);
    if n == 0 {
        if let Some(w) = win {
            let _ = w.hide();
        }
        return;
    }
    let win = match win {
        Some(w) => w,
        None => match WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
            .title("FluentCal 提醒")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focusable(false)
            .focused(false)
            .visible(false)
            .inner_size(WIDTH, CARD_H)
            .build()
        {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[reminder] popup window failed: {e}");
                return;
            }
        },
    };
    let _ = win.emit("reminder://popup", ());
    if let Ok(Some(m)) = win.primary_monitor() {
        let s = m.scale_factor();
        let a = m.work_area();
        let w = (WIDTH * s) as u32;
        let h = ((CARD_H * n.min(MAX_CARDS) as f64 + 8.0) * s) as u32;
        let margin = (12.0 * s) as i32;
        let _ = win.set_size(PhysicalSize::new(w, h));
        let _ = win.set_position(PhysicalPosition::new(
            a.position.x + a.size.width as i32 - w as i32 - margin,
            a.position.y + a.size.height as i32 - h as i32 - margin,
        ));
    }
    let _ = win.show();
    // Keep it out of Alt+Tab. Set after show(): tao rewrites the extended style on show.
    if let Ok(h) = win.hwnd() {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TOOLWINDOW};
        unsafe {
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex | WS_EX_TOOLWINDOW.0 as isize);
        }
    }
}

#[tauri::command]
pub fn reminder_popup_list(state: State<'_, Arc<Reminders>>) -> Vec<Reminder> {
    state.inner.lock().unwrap().popup.clone()
}

#[tauri::command]
pub fn reminder_popup_dismiss(app: AppHandle, state: State<'_, Arc<Reminders>>, key: String) {
    state.inner.lock().unwrap().popup.retain(|x| x.key != key);
    crate::on_event_loop(&app, reflow); // may build the window: not inside a sync command
}

/// "Remind me later": fires the same reminder again after `minutes`.
#[tauri::command]
pub fn reminder_snooze(app: AppHandle, state: State<'_, Arc<Reminders>>, key: String, minutes: i64) {
    {
        let mut g = state.inner.lock().unwrap();
        let Some(pos) = g.popup.iter().position(|x| x.key == key) else { return };
        let mut r = g.popup.remove(pos);
        let at = now_ms() + minutes.clamp(1, 24 * 60) * 60_000;
        let base = r.key.split("#snooze").next().unwrap_or(&r.key).to_owned();
        r.key = format!("{base}#snooze{at}");
        r.at_ms = at;
        g.snoozed.push(r);
        save_snoozed(&state.snooze_file, &g.snoozed);
    }
    state.wake.notify_all();
    crate::on_event_loop(&app, reflow);
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}
