//! Real Windows focus regression (0.3.12): the add box of the to-do column must activate the
//! inactive desktop calendar and receive native keys, one entry after another. Run separately:
//! cargo test -p fluentcal-native --test native_side_focus -- --ignored --nocapture

use fluentcal_app::{app, platform::{desk, window}, DeskWindow};
use i_slint_backend_testing::ElementHandle;
use serde_json::json;
use slint::{ComponentHandle, LogicalPosition};
use slint::platform::{PointerEventButton, WindowEvent};
use std::{cell::{Cell, RefCell}, rc::Rc, time::Duration};
use windows::core::w;
use windows::Win32::{Foundation::{HWND, LPARAM, WPARAM}, System::Threading::{AttachThreadInput, GetCurrentThreadId}, UI::{Input::KeyboardAndMouse::{GetFocus, MapVirtualKeyW, MAPVK_VK_TO_VSC}, WindowsAndMessaging::*}};

fn key(hwnd: HWND, vk: u32) {
    unsafe {
        let scan = MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) as isize;
        SendMessageW(hwnd, WM_KEYDOWN, Some(WPARAM(vk as usize)), Some(LPARAM(1 | (scan << 16))));
        SendMessageW(hwnd, WM_KEYUP, Some(WPARAM(vk as usize)), Some(LPARAM(1 | (scan << 16) | (3 << 30))));
    }
}

fn click(w: &DeskWindow, p: LogicalPosition) {
    w.window().dispatch_event(WindowEvent::PointerMoved { position: p });
    w.window().dispatch_event(WindowEvent::PointerPressed { position: p, button: PointerEventButton::Left });
    w.window().dispatch_event(WindowEvent::PointerReleased { position: p, button: PointerEventButton::Left });
}

fn point(w: &DeskWindow, label: &str) -> LogicalPosition {
    let e = ElementHandle::find_by_accessible_label(w, label).next().expect(label);
    let p = e.absolute_position();
    LogicalPosition::new(p.x + e.size().width / 2.0, p.y + e.size().height / 2.0)
}

#[test]
#[ignore = "uses a real Windows desktop window; run separately"]
fn todo_column_box_receives_keyboard_on_an_inactive_desktop_window() {
    let dir = std::env::temp_dir().join(format!("fluentcal-native-side-focus-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    std::env::set_var("FLUENTCAL_DATA_DIR", &dir);
    std::env::set_var("FLUENTCAL_TEST_RAISED", "1");
    std::fs::write(dir.join("data.json"), json!({
        "version": 1, "tasks": {}, "groups": {}, "dayMarks": {}, "holidays": {},
        "settings": { "general": {"seeded": true, "onboarded": true},
            "holidays": {"autoUpdate": false},
            "calendar": {"sidebar": true},
            "desk": {"locked": true, "pinMode": "desktop", "noActivate": true,
                "bounds": {"x": 100, "y": 100, "w": 880, "h": 640}},
            "shortcuts": {"toggle": "", "newTask": ""} }
    }).to_string()).unwrap();
    slint::BackendSelector::new().backend_name("winit".into()).renderer_name("software".into())
        .with_winit_window_attributes_hook(window::attributes).select().unwrap();
    fluentcal_app::fonts::setup();
    let a = fluentcal_app::startup::build(dir.clone()).unwrap();
    let w = a.desk.window.clone_strong();
    app::install(a);
    app::run(|a| fluentcal_app::startup::init(a, false));

    let step = Rc::new(Cell::new(0));
    let failure = Rc::new(RefCell::new(None));
    let result = failure.clone();
    let other = Rc::new(Cell::new(HWND::default()));
    let other_window = other.clone();
    let previous_foreground = unsafe { GetForegroundWindow() };
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(150), move || {
        let n = step.get();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let hwnd = window::hwnd(w.window()).expect("native HWND");
            let has = |title: &str| app::with(|a| a.doc.tasks.values().any(|t| t.title == title && t.flex.as_deref() == Some("month"))).unwrap();
            match n {
                0 => {
                    // Establish a foreground window in this test process, independent
                    // of which application launched cargo. No global keys are sent.
                    unsafe {
                        let foreground = GetWindowThreadProcessId(GetForegroundWindow(), None);
                        let current = GetCurrentThreadId();
                        let attached = AttachThreadInput(current, foreground, true).as_bool();
                        let _ = SetForegroundWindow(hwnd);
                        if attached { let _ = AttachThreadInput(current, foreground, false); }
                    }
                }
                1 => {
                    // The calendar on the desktop is inactive: another window (of this
                    // process, so Windows' foreground rules don't hide a missing request)
                    // has the keyboard.
                    unsafe {
                        let h = CreateWindowExW(WS_EX_TOOLWINDOW, w!("STATIC"), w!("FluentCal focus test"), WS_POPUP | WS_VISIBLE, -32000, -32000, 10, 10, None, None, None, None).unwrap();
                        other_window.set(h);
                        assert!(SetForegroundWindow(h).as_bool());
                    }
                }
                2 => {
                    assert_eq!(unsafe { GetForegroundWindow() }, other_window.get(), "the calendar starts inactive");
                    assert!(w.get_side_open(), "the to-do column is open");
                    click(&w, point(&w, "添加每月待办"));
                }
                3 => {
                    assert_eq!(unsafe { GetForegroundWindow() }, hwnd, "the add box must activate the native window");
                    // Send native keys to the OS keyboard focus, not directly to the
                    // Slint input (which would hide a missing activation request).
                    let focused = unsafe { GetFocus() };
                    assert_eq!(focused, hwnd, "the add box receives OS keyboard focus");
                    for ch in "first".chars() {
                        key(focused, ch.to_ascii_uppercase() as u32);
                    }
                }
                4 => key(hwnd, 13),
                5 => {
                    assert!(has("first"), "typing must reach the to-do column's add box");
                    // the next entry, without another click
                    assert_eq!(unsafe { GetFocus() }, hwnd);
                    for ch in "second".chars() {
                        key(hwnd, ch.to_ascii_uppercase() as u32);
                    }
                }
                6 => key(hwnd, 13),
                7 => {
                    assert!(has("second"), "the box stays open for the next entry");
                    slint::quit_event_loop().unwrap();
                }
                _ => unreachable!(),
            }
        }));
        if let Err(e) = r {
            *result.borrow_mut() = Some(e);
            let _ = slint::quit_event_loop();
        }
        step.set(n + 1);
    });
    slint::run_event_loop_until_quit().unwrap();
    timer.stop();
    desk::uninstall();
    if let Some(a) = app::uninstall() { a.store.flush(); }
    unsafe {
        let _ = DestroyWindow(other.get());
        let _ = SetForegroundWindow(previous_foreground);
    }
    assert_eq!(dir.canonicalize().unwrap().parent(), Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
    std::fs::remove_dir_all(&dir).unwrap();
    std::env::remove_var("FLUENTCAL_DATA_DIR");
    std::env::remove_var("FLUENTCAL_TEST_RAISED");
    if let Some(e) = failure.borrow_mut().take() { std::panic::resume_unwind(e); };
}
