//! Program start and exit (0.2's lib.rs run()).

use crate::app::{self, install, post, run, App};
use crate::desk::{live::DeskLive, vm::DeskUi};
use crate::platform::{autostart, desk as pd, msgwin, sysinfo, toast, tray, window};
use crate::reminders::{Reminders, Style};
use crate::shell::{self, events};
use crate::DeskWindow;
use fluentcal_core::cloud::{self, service::Cloud};
use fluentcal_core::store::Store;
use fluentcal_core::tasks::{Doc, Edit};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

/// The per-PC folder: %APPDATA%\<instance id> (0.2's app data folder for the installed
/// program); FLUENTCAL_DATA_DIR overrides (tests, development).
fn local_dir(id: &str) -> PathBuf {
    if let Some(d) = std::env::var_os("FLUENTCAL_DATA_DIR") {
        return PathBuf::from(d);
    }
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(id)
}

fn fatal(text: &str) {
    use windows::core::HSTRING;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR};
    unsafe {
        MessageBoxW(None, &HSTRING::from(text), &HSTRING::from("FluentCal"), MB_ICONERROR);
    }
}

pub fn main() {
    let id = msgwin::instance_id();
    if !msgwin::claim(&id) {
        return; // the running instance opens its settings
    }
    if let Err(e) = slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into()).with_winit_window_attributes_hook(window::attributes).select() {
        return fatal(&format!("无法初始化界面：{e}"));
    }
    crate::fonts::setup();
    let app = match build(local_dir(&id)) {
        Ok(a) => a,
        Err(e) => return fatal(&e),
    };
    install(app);
    run(move |app| {
        init(app, false);
        start_platform(app, &id);
    });
    if let Err(e) = slint::run_event_loop_until_quit() {
        eprintln!("[app] event loop: {e}");
    }
    if let Some(app) = app::uninstall() {
        app.cloud.quit_flush();
        app.store.flush();
    }
    tray::remove();
}


/// The app with its data, background threads and desk window (not shown yet).
pub fn build(local: PathBuf) -> Result<App, String> {
    if crate::platform::isolated() {
        cloud::secrets::set_namespace("FluentCal-dev");
    }
    let _ = std::fs::create_dir_all(&local);
    let (dir, local_settings, data_dir) = fluentcal_core::datadir::resolve(&local);
    let store = match Store::open_with(dir, local_settings) {
        Ok(s) => s,
        Err(e) => return Err(format!("无法打开数据文件夹：{e}")),
    };
    if let Some(patch) = cloud::drop_google_drive(&store.snapshot()["settings"]) {
        store.patch_settings(&patch);
        cloud::secrets::OLD_GOOGLE.iter().for_each(|k| cloud::secrets::delete(k));
    }
    store.listen(|e| {
        let e = e.clone();
        post(move |app| app.on_store_event(e));
    });
    // Sync state is per PC: kept out of a shared data folder.
    let on_status: fluentcal_core::cloud::service::StatusListener = Box::new(|s| {
        let s = s.clone();
        post(move |app| {
            app.sync = s;
            app.desk_refresh_status();
        });
    });
    let cloud = Cloud::start(store.clone(), Some(on_status), local.join("cloud-state.json"));
    let reminders = Reminders::start(&local, |due, style| {
        if style != Style::Popup {
            for r in due {
                if let Err(e) = toast::show(&r.title, &r.body) {
                    eprintln!("[reminder] notification failed: {e}");
                }
            }
        }
        if style == Style::Popup {
            // No toast means no sound from Windows; play the standard notification sound.
            unsafe {
                let _ = windows::Win32::System::Diagnostics::Debug::MessageBeep(windows::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION);
            }
        }
        post(|app| {
            app.reminder_cards_changed();
            app.schedule_reminders();
        });
    });

    let doc = Doc::from_value(&store.snapshot());
    let today = fluentcal_core::date::local_of_ms(fluentcal_core::date::now_ms(), fluentcal_core::date::utc_offset_minutes()).0;
    window::next(window::Kind::Desk);
    let w = match DeskWindow::new() {
        Ok(w) => w,
        Err(e) => return Err(format!("无法创建日历窗口：{e}")),
    };
    crate::desk::wire::wire(&w);
    let sys = sysinfo::system_info();
    let tokens = fluentcal_core::theme::tokens(sys.dark, "#0078D4", None);
    let style = fluentcal_core::theme::desk_style(&doc.settings.desk, &tokens);
    let ui = DeskUi::new(&doc, &today);
    Ok(App {
        store,
        cloud,
        sync: Default::default(),
        doc,
        sys,
        tokens,
        style,
        desk: DeskLive::new(w, ui),
        history: Default::default(),
        reminders,
        local_dir: local,
        data_dir,
        save_error: None,
        hotkey_errors: Vec::new(),
        timers: Vec::new(),
        reminder_timer: Default::default(),
        theme_timer: Default::default(),
        last_monitor: None,
        day: today,
        web: Default::default(),
        reminder_win: None,
        editor: None,
    })
}

/// What every start does: theme, the calendar shown, first-run data, overdue tasks moved on.
/// `headless`: the testing backend (no real window to pin).
pub fn init(app: &mut App, headless: bool) {
    app.load_holidays();
    app.sync = app.cloud.status();
    app.recompute_theme();
    if app.doc.settings.calendar.view == "gantt" {
        let today = app.today();
        app.desk.gantt.center = Some((today, 0.3));
    }
    if headless {
        app.desk_refresh();
        let _ = app.desk.window.show();
    } else {
        pd::on_event(|e| post(move |app| events::on_desk_event(app, e)));
        show_desk_window(app);
    }
    seed_if_first_run(app);
    startup_notice(app);
    events::auto_postpone(app);
    app.schedule_reminders();
}

/// The warning bar at start (0.2's DeskApp): a custom data folder not reachable, or a data
/// file that was damaged and replaced.
fn startup_notice(app: &mut App) {
    let mut text = app.data_dir.missing.as_ref().map(|m| format!("找不到数据文件夹 {m}（同步盘未启动？），本次使用本机默认文件夹。")).unwrap_or_default();
    if let Some(r) = app.store.take_recovered() {
        text = if r == "last-saved.json" {
            "数据文件已损坏，已恢复到最后一次保存的内容。".to_owned()
        } else if !r.is_empty() {
            format!("数据文件已损坏，已从备份 {r} 恢复。")
        } else {
            "数据文件已损坏且没有可用备份，已重新开始。".to_owned()
        };
    }
    app.desk_set_notice(&text);
}

/// The parts that touch Windows: tray, hotkeys, notifications' name, autostart, the web
/// server, holiday updates, the clock.
fn start_platform(app: &mut App, id: &str) {
    let icon = app.local_dir.join("fluentcal.png");
    if !icon.exists() {
        let _ = std::fs::write(&icon, include_bytes!("../res/icon-128.png"));
    }
    toast::register(Some(&icon));
    msgwin::create(id, |e| post(move |app| events::on_msg(app, e)));
    if let Some(h) = msgwin::hwnd() {
        let tip = if crate::platform::isolated() { "FluentCal 桌面日历（开发版）" } else { "FluentCal 桌面日历" };
        tray::create(h, tip, |e| post(move |app| shell::tray::on_tray(app, e)));
    }
    events::register_hotkeys(app);
    autostart::sync(app.doc.settings.general.autostart);
    app.web_apply();
    events::update_holidays(app, false);
    let every = |ms: u64, f: fn(&mut App)| {
        let t = slint::Timer::default();
        t.start(slint::TimerMode::Repeated, Duration::from_millis(ms), move || run(f));
        t
    };
    app.timers = vec![
        every(30_000, events::tick),
        every(30 * 60_000, |app| app.schedule_reminders()),
        every(6 * 3_600_000, |app| events::update_holidays(app, false)),
    ];
    // Tests and measurements: FLUENTCAL_TEST_QUIT=seconds quits normally (tray icon removed,
    // data saved), as "退出" does.
    if let Some(s) = std::env::var("FLUENTCAL_TEST_QUIT").ok().and_then(|s| s.parse::<f64>().ok()) {
        slint::Timer::single_shot(Duration::from_secs_f64(s), || run(quit));
    }
}

/// Shows the (new) desk window: sized first, then pinned and moved into place once it exists.
fn show_desk_window(app: &mut App) {
    app.desk_refresh();
    shell::apply_layout(app, true);
    app.desk.window.window().on_close_requested(|| {
        // Alt+F4 on the calendar hides it; the program stays in the tray.
        run(|app| {
            let _ = app.desk.window.hide();
            shell::tray_refresh(app);
        });
        slint::CloseRequestResponse::KeepWindowShown
    });
    let _ = app.desk.window.show();
    window::when_created(app.desk.window.as_weak(), |hwnd| {
        pd::install(hwnd);
        run(|app| {
            shell::apply_desk_platform(app);
            shell::apply_layout(app, false);
            app.desk_apply_theme(); // the window's real scale factor
            app.last_monitor = shell::current_screen(app);
            shell::tray_refresh(app);
        });
    });
}

/// Explorer restarts can take the calendar down with Progman (its owner): build a new one
/// (0.2's recreate_desk_later). The view state (month, selection) carries over.
pub fn ensure_desk_window(app: &mut App) {
    let gone = pd::desk_hwnd().is_some_and(|h| unsafe { !windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(h)).as_bool() });
    if !gone {
        return;
    }
    pd::uninstall();
    window::next(window::Kind::Desk);
    let Ok(w) = DeskWindow::new() else { return };
    crate::desk::wire::wire(&w);
    let ui = std::mem::take(&mut app.desk.ui);
    app.desk = DeskLive::new(w, ui);
    app.recompute_theme();
    show_desk_window(app);
}

/// First start with an empty document: sample tasks so the calendar isn't blank (REL-03; the
/// welcome bar offers to remove them). Not undoable.
fn seed_if_first_run(app: &mut App) {
    if app.doc.settings.general.seeded {
        return;
    }
    if app.doc.tasks.is_empty() && app.doc.groups.is_empty() {
        let (groups, tasks) = fluentcal_core::seed::sample_data(&app.today(), fluentcal_core::date::now_ms());
        let mut edit = Edit::default();
        groups.iter().for_each(|g| edit.put("groups", &g.id, g.to_value()));
        tasks.iter().for_each(|t| edit.put_task(t));
        app.apply(edit, false);
    }
    app.patch_settings(serde_json::json!({ "general": { "seeded": true } }));
}

/// "退出" (tray, menu).
pub fn quit(_app: &mut App) {
    let _ = slint::quit_event_loop();
}

/// Integration tests (Slint's testing backend, set up by the caller): the app on a test data
/// folder, its desk window `w`×`h` logical px, nothing registered with Windows.
pub fn test_app(local: PathBuf, w: f32, h: f32) -> Result<DeskWindow, String> {
    crate::fonts::setup();
    cloud::secrets::set_namespace("FluentCal-test");
    let app = build(local)?;
    let win = app.desk.window.clone_strong();
    win.window().set_size(slint::LogicalSize::new(w, h));
    install(app);
    run(|app| init(app, true));
    Ok(win)
}

