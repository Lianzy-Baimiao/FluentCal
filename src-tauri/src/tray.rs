//! Tray icon: left click summons the calendar, right click opens the menu. The menu is
//! rebuilt whenever a relevant setting changes so check marks stay in sync.

use crate::store::{patch_settings_from_rust, Store};
use serde_json::json;
use std::sync::Arc;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

const TRAY_ID: &str = "main";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("FluentCal 桌面日历")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::summon_desk(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let store = app.state::<Arc<Store>>();
    let visible = app
        .get_webview_window("desk")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let view = store.setting_str("/calendar/view", "month");
    let pin = store.setting_str("/desk/pinMode", "desktop");
    let locked = store.setting_bool("/desk/locked", true);
    let click_through = store.setting_bool("/desk/clickThrough", false);

    let views = Submenu::with_items(
        app,
        "视图",
        true,
        &[
            &CheckMenuItem::with_id(app, "view:month", "月视图", true, view == "month", None::<&str>)?,
            &CheckMenuItem::with_id(app, "view:weeks", "多周视图", true, view == "weeks", None::<&str>)?,
            &CheckMenuItem::with_id(app, "view:gantt", "甘特图", true, view == "gantt", None::<&str>)?,
        ],
    )?;
    let pins = Submenu::with_items(
        app,
        "窗口层级",
        true,
        &[
            &CheckMenuItem::with_id(app, "pin:desktop", "嵌入桌面", true, pin == "desktop", None::<&str>)?,
            &CheckMenuItem::with_id(app, "pin:top", "始终置顶", true, pin == "top", None::<&str>)?,
            &CheckMenuItem::with_id(app, "pin:normal", "普通窗口", true, pin == "normal", None::<&str>)?,
        ],
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &CheckMenuItem::with_id(app, "toggle", "显示日历", true, visible, None::<&str>)?,
            &MenuItem::with_id(app, "new", "新建任务…", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &views,
            &pins,
        ],
    )?;
    let cloud = matches!(store.setting_str("/sync/provider", "").as_str(), "webdav" | "github" | "gitee" | "gdrive");
    if cloud && store.setting_bool("/sync/enabled", false) {
        menu.insert(&MenuItem::with_id(app, "syncNow", "立即同步", true, None::<&str>)?, 2)?;
    }
    let screens = crate::screens::list(app);
    if screens.len() > 1 {
        let on = Submenu::new(app, "显示在", true)?;
        for s in &screens {
            on.append(&CheckMenuItem::with_id(app, format!("screen:{}", s.index), s.title(), true, s.current, None::<&str>)?)?;
        }
        menu.append(&on)?;
    }
    menu.append_items(&[
        &MenuItem::with_id(app, "openInBrowser", "在浏览器中查看", true, None::<&str>)?,
        &CheckMenuItem::with_id(app, "lock", "锁定位置和大小", true, locked, None::<&str>)?,
        &CheckMenuItem::with_id(app, "clickThrough", "鼠标穿透", true, click_through, None::<&str>)?,
        &PredefinedMenuItem::separator(app)?,
        &MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?,
        &MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?,
    ])?;
    Ok(menu)
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    let store = app.state::<Arc<Store>>().inner().clone();
    let id = event.id().as_ref();
    match id {
        "toggle" => crate::toggle_desk_visible(app),
        "new" => crate::open_editor_new(app),
        "settings" => crate::open_settings(app, None),
        "syncNow" => {
            if let Some(c) = app.try_state::<Arc<crate::cloud::service::Cloud>>() {
                c.request();
            }
        }
        "quit" => crate::quit(app),
        "openInBrowser" => {
            // Off the UI thread: starting the server may wait for an old one to stop.
            let app = app.clone();
            std::thread::spawn(move || match crate::webcmd::local_url(&app) {
                Ok(url) => {
                    let _ = crate::open_in_browser(&app, url);
                }
                // e.g. the port is taken: the settings page shows why and has the port field
                Err(_) => crate::open_settings(&app, Some("web".into())),
            });
        }
        "lock" => {
            let v = !store.setting_bool("/desk/locked", true);
            patch_settings_from_rust(app, &store, json!({ "desk": { "locked": v } }));
        }
        "clickThrough" => {
            let v = !store.setting_bool("/desk/clickThrough", false);
            patch_settings_from_rust(app, &store, json!({ "desk": { "clickThrough": v } }));
        }
        _ => {
            if let Some(view) = id.strip_prefix("view:") {
                patch_settings_from_rust(app, &store, json!({ "calendar": { "view": view } }));
                crate::show_desk(app);
            } else if let Some(pin) = id.strip_prefix("pin:") {
                patch_settings_from_rust(app, &store, json!({ "desk": { "pinMode": pin } }));
            } else if let Some(i) = id.strip_prefix("screen:").and_then(|i| i.parse().ok()) {
                crate::screens::move_to(app, i);
                crate::show_desk(app);
            }
        }
    }
    refresh(app);
}
