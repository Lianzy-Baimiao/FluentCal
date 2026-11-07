//! Cross-week dragging must retain native Ctrl+Z / Ctrl+Y / Ctrl+F routing.
//! cargo test -p fluentcal-native --test native_drag -- --ignored --nocapture
//! Uses an isolated window and thread-local keyboard state, never global input.

use fluentcal_app::{app, platform::window, DeskWindow};
use i_slint_backend_testing::{ElementHandle, ElementRoot};
use serde_json::json;
use slint::{platform::{PointerEventButton, WindowEvent}, ComponentHandle, LogicalPosition};
use std::{cell::{Cell, RefCell}, rc::Rc, time::Duration};
use windows::Win32::{Foundation::{HWND, LPARAM, WPARAM}, System::Threading::{AttachThreadInput, GetCurrentThreadId}, UI::{Input::KeyboardAndMouse::{GetKeyboardState, SetKeyboardState, MapVirtualKeyW, MAPVK_VK_TO_VSC, VK_CONTROL, VK_LCONTROL}, WindowsAndMessaging::*}};

fn ctrl(hwnd: HWND, letter: char) {
    // SetKeyboardState affects only this test's UI thread. Restore it immediately;
    // the actual user's keyboard state is left alone.
    unsafe {
        let mut saved = [0u8; 256];
        GetKeyboardState(&mut saved).unwrap();
        let mut pressed = [0u8; 256];
        pressed[VK_CONTROL.0 as usize] = 0x80;
        pressed[VK_LCONTROL.0 as usize] = 0x80;
        SetKeyboardState(&pressed).unwrap();
        let ctrl_scan = MapVirtualKeyW(VK_CONTROL.0 as u32, MAPVK_VK_TO_VSC) as isize;
        SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(VK_CONTROL.0 as usize)), Some(LPARAM(1 | (ctrl_scan << 16))));
        let vk = letter.to_ascii_uppercase() as u32;
        let scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) as isize;
        SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(vk as usize)), Some(LPARAM(1 | (scan << 16))));
        SendMessageW(hwnd, WM_KEYUP, Some(WPARAM(vk as usize)), Some(LPARAM(1 | (scan << 16) | (3 << 30))));
        SetKeyboardState(&saved).unwrap();
        SendMessageW(hwnd, WM_KEYUP, Some(WPARAM(VK_CONTROL.0 as usize)), Some(LPARAM(1 | (ctrl_scan << 16) | (3 << 30))));
    }
}

fn drag(w: &DeskWindow) {
    let chip = ElementHandle::find_by_accessible_label(w, "跨周拖动测试").next().unwrap();
    let p = chip.absolute_position();
    let from = LogicalPosition::new(p.x + 40.0, p.y + chip.size().height / 2.0);
    let cell = w.root_element().query_descendants().match_predicate(|e| e.accessible_label().is_some_and(|s| s.starts_with("11月9日 "))).find_first().unwrap();
    let p = cell.absolute_position();
    let to = LogicalPosition::new(p.x + 40.0, p.y + 50.0);
    assert_eq!(app::with(|a| a.desk_date_at(to.x, to.y)).flatten().as_deref(), Some("2026-11-09"));
    let button = PointerEventButton::Left;
    w.window().dispatch_event(WindowEvent::PointerMoved { position: from });
    w.window().dispatch_event(WindowEvent::PointerPressed { position: from, button });
    for n in 1..=10 {
        let k = n as f32 / 10.0;
        w.window().dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x + (to.x - from.x) * k, from.y + (to.y - from.y) * k) });
    }
    w.window().dispatch_event(WindowEvent::PointerReleased { position: to, button });
}

#[test]
#[ignore = "shows a real Windows calendar; run separately"]
fn native_shortcuts_work_after_cross_week_drag() {
    let dir = std::env::temp_dir().join(format!("fluentcal-native-drag-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    std::env::set_var("FLUENTCAL_DATA_DIR", &dir);
    std::fs::write(dir.join("data.json"), json!({
        "version": 1, "tasks": {"drag": {"id": "drag", "title": "跨周拖动测试",
            "start": "2026-11-08", "end": "2026-11-08", "done": false, "progress": 0,
            "order": 1, "createdAt": 1, "updatedAt": 1}},
        "groups": {}, "dayMarks": {}, "holidays": {},
        "settings": {"general": {"seeded": true, "onboarded": true, "language": "zh-CN"},
            "calendar": {"weekStart": 1}, "holidays": {"autoUpdate": false}}
    }).to_string()).unwrap();
    slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into())
        .with_winit_window_attributes_hook(window::attributes).select().unwrap();
    fluentcal_app::fonts::setup();
    let a = fluentcal_app::startup::build(dir.clone()).unwrap();
    let w = a.desk.window.clone_strong();
    app::install(a);
    app::run(|a| {
        a.recompute_theme();
        a.desk_jump("2026-11-08");
    });
    w.window().set_size(slint::LogicalSize::new(880.0, 640.0));
    w.window().set_position(slint::PhysicalPosition::new(100, 100));
    let previous_foreground = unsafe { GetForegroundWindow() };
    w.show().unwrap();

    let step = Rc::new(Cell::new(0));
    let ticks = Rc::new(Cell::new(0));
    let failure = Rc::new(RefCell::new(None));
    let result = failure.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(150), move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ticks.set(ticks.get() + 1);
            assert!(ticks.get() < 80, "native window did not become ready");
            let Some(hwnd) = window::hwnd(w.window()) else { return };
            if !unsafe { IsWindowVisible(hwnd).as_bool() } { return; }
            match step.get() {
                0 => {
                    unsafe {
                        let foreground = GetWindowThreadProcessId(GetForegroundWindow(), None);
                        let current = GetCurrentThreadId();
                        let attached = AttachThreadInput(current, foreground, true).as_bool();
                        let _ = SetForegroundWindow(hwnd);
                        if attached { let _ = AttachThreadInput(current, foreground, false); }
                    }
                    w.invoke_focus_root();
                }
                1 => ctrl(hwnd, 'f'),
                2 => {
                    assert!(w.get_search_open(), "native Ctrl+F must work before the drag (input sanity check)");
                    w.invoke_search_close();
                }
                3 => drag(&w),
                4 => {
                    assert_eq!(app::with(|a| a.doc.tasks["drag"].start.clone()).unwrap(), "2026-11-09");
                    ctrl(hwnd, 'z');
                }
                5 => {
                    assert_eq!(app::with(|a| a.doc.tasks["drag"].start.clone()).unwrap(), "2026-11-08", "native Ctrl+Z must undo after the focused chip moves to another week");
                    ctrl(hwnd, 'y');
                }
                6 => {
                    assert_eq!(app::with(|a| a.doc.tasks["drag"].start.clone()).unwrap(), "2026-11-09", "native Ctrl+Y must redo after undo");
                    ctrl(hwnd, 'f');
                }
                7 => {
                    assert!(w.get_search_open(), "native Ctrl+F must still work after drag, undo and redo");
                    w.hide().unwrap();
                    println!("PASS native cross-week drag: Ctrl+Z, Ctrl+Y and Ctrl+F");
                    slint::quit_event_loop().unwrap();
                }
                _ => unreachable!(),
            }
            step.set(step.get() + 1);
        }));
        if let Err(e) = r {
            let _ = w.hide();
            *result.borrow_mut() = Some(e);
            let _ = slint::quit_event_loop();
        }
    });
    slint::run_event_loop_until_quit().unwrap();
    timer.stop();
    if let Some(a) = app::uninstall() { a.store.flush(); }
    unsafe { let _ = SetForegroundWindow(previous_foreground); }
    assert_eq!(dir.canonicalize().unwrap().parent(), Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
    std::fs::remove_dir_all(&dir).unwrap();
    std::env::remove_var("FLUENTCAL_DATA_DIR");
    if let Some(e) = failure.borrow_mut().take() { std::panic::resume_unwind(e); };
}
