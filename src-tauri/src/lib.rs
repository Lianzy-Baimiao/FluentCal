pub mod cloud;
mod datadir;
mod desktop;
mod reminder;
mod reminder_popup;
mod screens;
pub mod merge;
pub mod store;
pub mod sync;
mod tray;

use desktop::PinMode;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use store::Store;
use tauri::{
    AppHandle, Emitter, Manager, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri::utils::config::{Color, WindowEffectsConfig};
use tauri::window::Effect;
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

static QUITTING: AtomicBool = AtomicBool::new(false);
static DESK_SHOWN: AtomicBool = AtomicBool::new(false);

/// Payload handed to a window that is opened (or re-targeted) from Rust, e.g. which task
/// the editor should load. Taken once by the window on mount.
#[derive(Default)]
struct Pending {
    settings: Mutex<Option<Value>>,
    editor: Mutex<Option<Value>>,
}

#[derive(Default)]
struct ShortcutErrors(Mutex<Vec<String>>);

// ---------------------------------------------------------------------------------------
// Desk window

fn create_desk(app: &AppHandle) -> tauri::Result<()> {
    let store = app.state::<Arc<Store>>();
    let locked = store.setting_bool("/desk/locked", true);
    let win = WebviewWindowBuilder::new(app, "desk", WebviewUrl::App("index.html".into()))
        .title("FluentCal")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(!locked)
        .maximizable(false)
        .minimizable(false)
        .visible(false)
        .focused(false)
        .min_inner_size(360.0, 280.0)
        .inner_size(760.0, 560.0)
        .build()?;
    restore_bounds(app, &win);
    desktop::install(app, &win);
    apply_desk_settings(app);

    let handle = app.clone();
    win.on_window_event(move |event| match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            save_bounds(&handle);
            screens::note_moved(&handle);
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if let Some(w) = handle.get_webview_window("desk") {
                let _ = w.hide();
            }
            tray::refresh(&handle);
        }
        WindowEvent::Destroyed if !QUITTING.load(Ordering::Relaxed) => recreate_desk_later(&handle),
        _ => {}
    });

    // Reveal even if the frontend never reports ready (e.g. a script error).
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(4));
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || reveal_desk(&h));
    });
    Ok(())
}

/// Explorer restarts can take owned windows down with Progman; bring the calendar back.
fn recreate_desk_later(app: &AppHandle) {
    DESK_SHOWN.store(false, Ordering::Relaxed);
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || {
            if h.get_webview_window("desk").is_none() {
                let _ = create_desk(&h);
            }
        });
    });
}

fn reveal_desk(app: &AppHandle) {
    if DESK_SHOWN.swap(true, Ordering::Relaxed) {
        return;
    }
    if let Some(w) = app.get_webview_window("desk") {
        let _ = w.show();
        apply_desk_settings(app);
        tray::refresh(app);
    }
}

fn restore_bounds(app: &AppHandle, win: &WebviewWindow) {
    let store = app.state::<Arc<Store>>();
    let num = |k: &str| store.setting(&format!("/desk/bounds/{k}")).and_then(|v| v.as_f64());
    let monitors = win.available_monitors().unwrap_or_default();
    if let (Some(x), Some(y), Some(w), Some(h)) = (num("x"), num("y"), num("w"), num("h")) {
        let (x, y, w, h) = (x as i32, y as i32, w.max(300.0) as u32, h.max(240.0) as u32);
        // Require the top strip (title bar) to be on some monitor, so it can be dragged back.
        if screens::title_visible(&monitors, x, y, w) {
            screens::place(win, x, y, w, h);
            return;
        }
    }
    // First run (or its screen is gone): top-right corner of the primary work area.
    if let Ok(Some(m)) = win.primary_monitor() {
        screens::place_default(win, &m);
    }
}

fn save_bounds(app: &AppHandle) {
    let Some(win) = app.get_webview_window("desk") else { return };
    if !win.is_visible().unwrap_or(false) || win.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(p), Ok(s)) = (win.outer_position(), win.outer_size()) else { return };
    if p.x < -20000 || p.y < -20000 || s.width < 100 || s.height < 100 {
        return;
    }
    let store = app.state::<Arc<Store>>();
    store::patch_settings_silent(
        &store,
        json!({ "desk": { "bounds": { "x": p.x, "y": p.y, "w": s.width, "h": s.height } } }),
    );
}

/// Applies layer mode, lock, click-through and acrylic from settings to the desk window.
pub(crate) fn apply_desk_settings(app: &AppHandle) {
    let Some(win) = app.get_webview_window("desk") else { return };
    let store = app.state::<Arc<Store>>();
    desktop::set_mode(&win, PinMode::parse(&store.setting_str("/desk/pinMode", "desktop")));
    desktop::set_click_through(store.setting_bool("/desk/clickThrough", false));
    desktop::set_no_activate(store.setting_bool("/desk/noActivate", true));
    desktop::set_acrylic(store.setting_bool("/desk/acrylic", false));
    let _ = win.set_resizable(!store.setting_bool("/desk/locked", true));
}

pub(crate) fn on_settings_changed(app: &AppHandle, old: &Value, new: &Value) {
    let changed = |p: &str| old.pointer(p) != new.pointer(p);
    // Apply only what changed: re-applying the layer mode would drop a calendar the user
    // just brought forward with the hotkey.
    if let Some(win) = app.get_webview_window("desk") {
        let store = app.state::<Arc<Store>>();
        if changed("/desk/pinMode") {
            desktop::set_mode(&win, PinMode::parse(&store.setting_str("/desk/pinMode", "desktop")));
        }
        if changed("/desk/clickThrough") {
            desktop::set_click_through(store.setting_bool("/desk/clickThrough", false));
        }
        if changed("/desk/noActivate") {
            desktop::set_no_activate(store.setting_bool("/desk/noActivate", true));
        }
        if changed("/desk/acrylic") {
            desktop::set_acrylic(store.setting_bool("/desk/acrylic", false));
        }
        if changed("/desk/locked") {
            let _ = win.set_resizable(!store.setting_bool("/desk/locked", true));
        }
    }
    if changed("/shortcuts") {
        register_shortcuts(app);
    }
    if changed("/sync") {
        if let Some(c) = app.try_state::<Arc<cloud::service::Cloud>>() {
            c.reconfigure(new);
        }
    }
    if changed("/general/autostart") {
        sync_autostart(app);
    }
    if ["/desk/pinMode", "/desk/locked", "/desk/clickThrough", "/calendar/view", "/sync/enabled", "/sync/provider"]
        .iter()
        .any(|p| changed(p))
    {
        tray::refresh(app);
    }
}

pub(crate) fn summon_desk(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("desk") {
        desktop::summon(&w);
        tray::refresh(app);
    }
}

pub(crate) fn show_desk(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("desk") {
        if !w.is_visible().unwrap_or(true) {
            let _ = w.show();
            apply_desk_settings(app);
        }
    }
}

pub(crate) fn toggle_desk_visible(app: &AppHandle) {
    let Some(w) = app.get_webview_window("desk") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
    } else {
        let _ = w.show();
        apply_desk_settings(app);
    }
}

/// Restarts the app after the data folder changed.
pub(crate) fn restart(app: &AppHandle) {
    QUITTING.store(true, Ordering::Relaxed);
    app.state::<Arc<Store>>().flush();
    app.restart();
}

pub(crate) fn quit(app: &AppHandle) {
    QUITTING.store(true, Ordering::Relaxed);
    if let Some(c) = app.try_state::<Arc<cloud::service::Cloud>>() {
        c.quit_flush();
    }
    app.state::<Arc<Store>>().flush();
    app.exit(0);
}

/// Back from sleep (WM_POWERBROADCAST).
pub(crate) fn on_resume(app: &AppHandle) {
    if let Some(c) = app.try_state::<Arc<cloud::service::Cloud>>() {
        c.nudge_after_wake();
    }
}

// ---------------------------------------------------------------------------------------
// Settings and editor windows

fn resolved_dark(app: &AppHandle) -> bool {
    match app.state::<Arc<Store>>().setting_str("/theme/mode", "system").as_str() {
        "dark" => true,
        "light" => false,
        _ => desktop::system_dark(),
    }
}

fn mica() -> WindowEffectsConfig {
    WindowEffectsConfig {
        effects: vec![Effect::Mica],
        state: None,
        radius: None,
        color: None::<Color>,
        ..Default::default()
    }
}

/// Runs `f` on the main thread from a fresh event-loop turn. Creating a webview inside a
/// synchronous command or an event handler deadlocks on Windows (WebView2 waits for a
/// message the blocked thread never pumps), so every window is built through this.
pub(crate) fn on_event_loop(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let h = handle.clone();
        let _ = handle.run_on_main_thread(move || f(&h));
    });
}

fn open_aux_window(app: &AppHandle, label: &'static str, title: &'static str, size: (f64, f64), min: (f64, f64)) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    on_event_loop(app, move |app| {
        if app.get_webview_window(label).is_some() {
            return; // opened twice in quick succession
        }
        let theme = if resolved_dark(app) { Theme::Dark } else { Theme::Light };
        let built = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
            .title(title)
            .inner_size(size.0, size.1)
            .min_inner_size(min.0, min.1)
            .center()
            .transparent(true)
            .effects(mica())
            .theme(Some(theme))
            .visible(false)
            .build();
        if let Err(e) = built {
            eprintln!("could not open the {label} window: {e}");
        }
    });
}

pub(crate) fn open_settings(app: &AppHandle, page: Option<String>) {
    if let Some(p) = &page {
        *app.state::<Pending>().settings.lock().unwrap() = Some(json!({ "page": p }));
        let _ = app.emit_to("settings", "pending://settings", ());
    }
    open_aux_window(app, "settings", "FluentCal 设置", (980.0, 700.0), (760.0, 520.0));
}

pub(crate) fn open_editor(app: &AppHandle, payload: Value) {
    *app.state::<Pending>().editor.lock().unwrap() = Some(payload);
    let _ = app.emit_to("editor", "pending://editor", ());
    open_aux_window(app, "editor", "编辑任务", (560.0, 720.0), (460.0, 560.0));
}

pub(crate) fn open_editor_new(app: &AppHandle) {
    open_editor(app, json!({ "mode": "new" }));
}

// ---------------------------------------------------------------------------------------
// Shortcuts and autostart

fn register_shortcuts(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let store = app.state::<Arc<Store>>();
    let mut errors = Vec::new();
    let bindings: [(String, fn(&AppHandle)); 2] = [
        (store.setting_str("/shortcuts/toggle", "Ctrl+Alt+D"), |app| {
            if let Some(w) = app.get_webview_window("desk") {
                desktop::toggle(&w);
            }
        }),
        (store.setting_str("/shortcuts/newTask", "Ctrl+Alt+N"), open_editor_new),
    ];
    for (accel, action) in bindings {
        if accel.trim().is_empty() {
            continue;
        }
        let res = gs.on_shortcut(accel.as_str(), move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                action(app);
            }
        });
        if let Err(e) = res {
            errors.push(format!("{accel}：{e}"));
        }
    }
    *app.state::<ShortcutErrors>().0.lock().unwrap() = errors;
}

fn sync_autostart(app: &AppHandle) {
    let want = app.state::<Arc<Store>>().setting_bool("/general/autostart", true);
    if cfg!(debug_assertions) {
        // Never register a dev build in the Run key.
        return;
    }
    let al = app.autolaunch();
    match (want, al.is_enabled().unwrap_or(false)) {
        (true, false) => {
            let _ = al.enable();
        }
        (false, true) => {
            let _ = al.disable();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------
// Commands

#[tauri::command]
fn desk_ready(app: AppHandle) {
    reveal_desk(&app);
}

#[tauri::command]
fn desk_summon(app: AppHandle) {
    summon_desk(&app);
}

/// The desk window needs the keyboard (a text box got focus). Clicks alone don't activate
/// it on the desktop layer, see desktop::no_activate.
#[tauri::command]
fn desk_activate(app: AppHandle) {
    if let Some(w) = app.get_webview_window("desk") {
        desktop::activate_for_keyboard(&w);
    }
}

#[tauri::command]
fn desk_hide(app: AppHandle) {
    if let Some(w) = app.get_webview_window("desk") {
        let _ = w.hide();
    }
    tray::refresh(&app);
}

#[tauri::command]
fn settings_open(app: AppHandle, page: Option<String>) {
    open_settings(&app, page);
}

#[tauri::command]
fn editor_open(app: AppHandle, payload: Value) {
    open_editor(&app, payload);
}

#[tauri::command]
fn pending_take(app: AppHandle, webview: tauri::Webview) -> Option<Value> {
    let pending = app.state::<Pending>();
    match webview.label() {
        "settings" => pending.settings.lock().unwrap().take(),
        "editor" => pending.editor.lock().unwrap().take(),
        _ => None,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    #[serde(flatten)]
    system: desktop::SystemInfo,
    version: String,
    data_dir: String,
    autostart: bool,
    shortcut_errors: Vec<String>,
    debug: bool,
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        system: desktop::system_info(),
        version: app.package_info().version.to_string(),
        data_dir: app.state::<Arc<Store>>().dir.display().to_string(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        shortcut_errors: app.state::<ShortcutErrors>().0.lock().unwrap().clone(),
        debug: cfg!(debug_assertions),
    }
}

#[tauri::command]
fn app_quit(app: AppHandle) {
    quit(&app);
}

#[tauri::command]
fn screens_list(app: AppHandle) -> Vec<screens::Screen> {
    screens::list(&app)
}

#[tauri::command]
fn desk_move_to_screen(app: AppHandle, index: usize) {
    // Moving can resize the webview (other scale factor): do it outside the IPC callback.
    on_event_loop(&app, move |app| screens::move_to(app, index));
}

#[tauri::command]
fn notify_test(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title("FluentCal 提醒测试")
        .body("如果你看到这条通知，任务提醒可以正常工作。")
        .show()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_data_dir(app: AppHandle) -> Result<(), String> {
    let dir = app.state::<Arc<Store>>().dir.clone();
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only http(s) urls are allowed".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Shows a save dialog and writes `content`. Returns the chosen path, or None if cancelled.
#[tauri::command]
async fn file_save(
    app: AppHandle,
    window: tauri::Window,
    default_name: String,
    filter_name: String,
    extensions: Vec<String>,
    content: String,
    bytes: Option<Vec<u8>>,
    bom: bool,
) -> Result<Option<String>, String> {
    let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let Some(path) = app
        .dialog()
        .file()
        .set_parent(&window)
        .set_file_name(&default_name)
        .add_filter(&filter_name, &exts)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let data = match bytes {
        Some(b) => b,
        None => {
            let mut out = Vec::with_capacity(content.len() + 3);
            if bom {
                out.extend_from_slice(b"\xEF\xBB\xBF");
            }
            out.extend_from_slice(content.as_bytes());
            out
        }
    };
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

/// Shows an open dialog and returns the file's text (BOM stripped), or None if cancelled.
#[tauri::command]
async fn file_open(
    app: AppHandle,
    window: tauri::Window,
    filter_name: String,
    extensions: Vec<String>,
) -> Result<Option<String>, String> {
    let exts: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let Some(path) = app
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter(&filter_name, &exts)
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 50 * 1024 * 1024 {
        return Err("文件过大（超过 50 MB）".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(Some(text.trim_start_matches('\u{FEFF}').to_owned()))
}

// ---------------------------------------------------------------------------------------

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            open_settings(app, None);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Pending::default())
        .manage(ShortcutErrors::default())
        .setup(|app| {
            let handle = app.handle().clone();
            let default = app.path().app_data_dir()?;
            std::fs::create_dir_all(&default)?;
            let (dir, local_settings, info) = datadir::resolve(&default);
            let store = Store::open_with(dir, local_settings)?;
            store.set_app(handle.clone());
            // Sync state is per PC: keep it out of a shared data folder.
            app.manage(cloud::service::Cloud::start(store.clone(), Some(handle.clone()), default.join("cloud-state.json")));
            app.manage(store);
            app.manage(info);
            // Fired/snoozed reminders are per PC: keep them out of a shared folder.
            app.manage(reminder::start(handle.clone(), default));
            create_desk(&handle)?;
            tray::create(&handle)?;
            register_shortcuts(&handle);
            sync_autostart(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            screens_list,
            desk_move_to_screen,
            cloud::commands::sync_status,
            cloud::commands::sync_now,
            cloud::commands::sync_probe,
            cloud::commands::sync_enable,
            cloud::commands::sync_disable,
            cloud::commands::sync_set_secret,
            cloud::commands::sync_secrets,
            cloud::commands::sync_google_login,
            cloud::commands::sync_google_logout,
            store::store_get,
            store::store_put,
            store::store_patch_settings,
            store::store_replace,
            store::backups_list,
            store::backup_create,
            store::backup_restore,
            reminder::reminders_set,
            reminder_popup::reminder_popup_list,
            reminder_popup::reminder_popup_dismiss,
            reminder_popup::reminder_snooze,
            desk_ready,
            desk_summon,
            desk_activate,
            desk_hide,
            settings_open,
            editor_open,
            pending_take,
            app_info,
            app_quit,
            notify_test,
            datadir::data_dir_info,
            datadir::data_dir_set,
            datadir::pick_folder,
            open_data_dir,
            open_url,
            file_save,
            file_open,
        ])
        .build(tauri::generate_context!())
        .expect("error while building FluentCal")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, code, .. } => {
                // Closing the last window keeps the app in the tray; only "quit" exits.
                if code.is_none() && !QUITTING.load(Ordering::Relaxed) {
                    api.prevent_exit();
                }
            }
            tauri::RunEvent::Exit => {
                app.state::<Arc<Store>>().flush();
            }
            _ => {}
        });
}
