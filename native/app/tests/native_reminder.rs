//! Real Windows reminder lifetime regression (no global mouse/keyboard input).
//! cargo test -p fluentcal-native --test native_reminder -- --ignored --nocapture

use fluentcal_app::{app, platform::window, reminders::{Reminder, Reminders, Style}};
use i_slint_backend_testing::ElementHandle;
use serde_json::json;
use slint::{platform::{PointerEventButton, WindowEvent}, ComponentHandle, LogicalPosition};
use std::{cell::{Cell, RefCell}, rc::Rc, time::Duration};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindowVisible};

#[test]
#[ignore = "shows real Windows reminder cards; run separately"]
fn reminder_actions_close_the_native_window() {
    let dir = std::env::temp_dir().join(format!("fluentcal-native-reminder-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    std::env::set_var("FLUENTCAL_DATA_DIR", &dir);
    let today = fluentcal_core::date::today();
    let mut tasks = serde_json::Map::new();
    for id in ["close", "snooze", "done"] {
        tasks.insert(id.into(), json!({"id": id, "title": "提醒关闭回归测试", "start": today,
            "end": today, "done": false, "progress": 0, "order": 1, "createdAt": 1, "updatedAt": 1}));
    }
    std::fs::write(dir.join("data.json"), json!({
        "version": 1, "tasks": tasks, "groups": {}, "dayMarks": {}, "holidays": {},
        "settings": {"general": {"seeded": true, "onboarded": true, "language": "zh-CN"},
            "holidays": {"autoUpdate": false}}
    }).to_string()).unwrap();
    slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into())
        .with_winit_window_attributes_hook(window::attributes).select().unwrap();
    fluentcal_app::fonts::setup();
    let mut a = fluentcal_app::startup::build(dir.clone()).unwrap();
    a.reminders = Reminders::start(&dir, |_, _| {});
    app::install(a);

    let step = Rc::new(Cell::new(0));
    let ticks = Rc::new(Cell::new(0));
    let previous = Rc::new(Cell::new(HWND::default()));
    let failure = Rc::new(RefCell::new(None));
    let result = failure.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ticks.set(ticks.get() + 1);
            assert!(ticks.get() < 100, "timed out waiting for native reminder");
            let n = step.get();
            let (id, label) = [("close", "关闭提醒"), ("snooze", "10 分钟后提醒"), ("done", "完成")][n / 4];
            match n % 4 {
                0 => app::with(|a| {
                    a.reminders.set(vec![Reminder {
                        key: id.into(), at_ms: fluentcal_core::date::now_ms(),
                        title: "提醒关闭回归测试".into(), body: "仅测试数据".into(),
                        task_id: id.into(), date: a.today(),
                    }], Style::Popup);
                }).unwrap(),
                1 => {
                    if app::with(|a| a.reminders.popup_list().is_empty()).unwrap() { return; }
                    app::with(|a| a.reminder_cards_changed()).unwrap();
                }
                2 => {
                    let w = app::with(|a| a.reminder_win.as_ref().unwrap().clone_strong()).unwrap();
                    let Some(hwnd) = window::hwnd(w.window()) else { return };
                    if !unsafe { IsWindowVisible(hwnd).as_bool() } { return; }
                    previous.set(hwnd);
                    let e = ElementHandle::find_by_accessible_label(&w, label).next().expect(label);
                    let p = e.absolute_position();
                    let position = LogicalPosition::new(p.x + e.size().width / 2.0, p.y + e.size().height / 2.0);
                    w.window().dispatch_event(WindowEvent::PointerMoved { position });
                    w.window().dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
                    w.window().dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
                    // Drop every test-owned component reference before checking the HWND.
                }
                3 => {
                    assert!(app::with(|a| a.reminders.popup_list().is_empty() && a.reminder_win.is_none()).unwrap());
                    assert!(!unsafe { IsWindowVisible(previous.get()).as_bool() }, "{label}: native reminder is still visible after releasing all app/test references");
                    assert_eq!(app::with(|a| a.doc.tasks[id].done).unwrap(), id == "done");
                    println!("PASS {label}: native window hidden");
                    if n == 11 { slint::quit_event_loop().unwrap(); }
                }
                _ => unreachable!(),
            }
            step.set(n + 1);
        }));
        if let Err(e) = r {
            *result.borrow_mut() = Some(e);
            let _ = slint::quit_event_loop();
        }
    });
    slint::run_event_loop_until_quit().unwrap();
    timer.stop();
    if let Some(a) = app::uninstall() { a.store.flush(); }
    assert_eq!(dir.canonicalize().unwrap().parent(), Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
    std::fs::remove_dir_all(&dir).unwrap();
    std::env::remove_var("FLUENTCAL_DATA_DIR");
    if let Some(e) = failure.borrow_mut().take() { std::panic::resume_unwind(e); };
}
