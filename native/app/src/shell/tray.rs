//! The tray menu (0.2's tray.rs): built from the settings whenever they change, so the check
//! marks stay in sync.

use fluentcal_core::i18n::{tr};

use crate::app::App;
use crate::platform::screens;
use crate::platform::tray::{self, Item, TrayEvent};
use serde_json::json;

fn check(id: &str, title: &str, checked: bool) -> Item {
    Item::Check { id: id.into(), title: title.into(), checked }
}

fn cmd(id: &str, title: &str) -> Item {
    Item::Command { id: id.into(), title: title.into() }
}

pub fn tray_refresh(app: &mut App) {
    tray::set_tip(tr(if crate::platform::isolated() { "FluentCal 桌面日历（开发版）" } else { "FluentCal 桌面日历" }));
    let s = &app.doc.settings;
    let (view, pin) = (s.calendar.view.as_str(), s.desk.pin_mode.as_str());
    let mut items = vec![
        check("toggle", tr("显示日历"), super::desk_visible(app)),
        cmd("new", tr("新建任务…")),
        Item::Separator,
        Item::Sub {
            title: tr("视图").into(),
            items: vec![check("view:list", tr("列表视图"), view == "list"), check("view:month", tr("月视图"), view == "month"), check("view:weeks", tr("多周视图"), view == "weeks"), check("view:gantt", tr("甘特图"), view == "gantt")],
        },
        Item::Sub {
            title: tr("窗口层级").into(),
            items: vec![check("pin:desktop", tr("嵌入桌面"), pin == "desktop"), check("pin:top", tr("始终置顶"), pin == "top"), check("pin:normal", tr("普通窗口"), pin == "normal")],
        },
    ];
    let cloud = matches!(s.sync.provider.as_str(), "webdav" | "github" | "gitee");
    if cloud && s.sync.enabled {
        items.insert(2, cmd("syncNow", tr("立即同步")));
    }
    let mons = screens::monitors();
    if mons.len() > 1 {
        let cur = super::current_screen(app);
        let on = mons.iter().enumerate().map(|(i, m)| check(&format!("screen:{i}"), &screens::title(m, i), cur == Some(i))).collect();
        items.push(Item::Sub { title: tr("显示在").into(), items: on });
    }
    items.extend([
        cmd("openInBrowser", tr("在浏览器中查看")),
        check("lock", tr("锁定位置和大小"), s.desk.locked),
        check("clickThrough", tr("鼠标穿透"), s.desk.click_through),
        Item::Separator,
        cmd("settings", tr("设置…")),
        cmd("quit", tr("退出")),
    ]);
    tray::set_menu(items);
}

pub fn on_tray(app: &mut App, e: TrayEvent) {
    let id = match e {
        TrayEvent::Click => return super::summon_desk(app),
        TrayEvent::Command(id) => id,
    };
    crate::platform::desk::allow_foreground();
    match id.as_str() {
        "toggle" => {
            if super::desk_visible(app) {
                let _ = slint::ComponentHandle::hide(&app.desk.window);
            } else {
                super::show_desk(app);
            }
        }
        "new" => app.open_editor_new(None),
        "settings" => app.open_settings(None),
        "syncNow" => app.cloud.request(),
        "quit" => crate::startup::quit(app),
        "openInBrowser" => app.open_in_browser(),
        "lock" => {
            let v = !app.doc.settings.desk.locked;
            app.patch_settings(json!({ "desk": { "locked": v } }));
        }
        "clickThrough" => {
            let v = !app.doc.settings.desk.click_through;
            app.patch_settings(json!({ "desk": { "clickThrough": v } }));
        }
        _ => {
            if let Some(view) = id.strip_prefix("view:") {
                app.patch_settings(json!({ "calendar": { "view": view } }));
                super::show_desk(app);
            } else if let Some(pin) = id.strip_prefix("pin:") {
                app.patch_settings(json!({ "desk": { "pinMode": pin } }));
            } else if let Some(i) = id.strip_prefix("screen:").and_then(|i| i.parse().ok()) {
                super::move_to_screen(app, i);
                super::show_desk(app);
            }
        }
    }
    tray_refresh(app);
}
