//! Real Windows focus regression: after adjusting an inactive desktop calendar, double
//! clicking a date must activate its window and receive native keys. Run separately:
//! cargo test -p fluentcal-native --test native_focus -- --ignored --nocapture

use fluentcal_app::{app, platform::{desk, window}, DeskWindow};
use i_slint_backend_testing::{ElementHandle, ElementRoot};
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
fn quick_add_receives_keyboard_after_native_move() {
    let dir = std::env::temp_dir().join(format!("fluentcal-native-focus-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    std::env::set_var("FLUENTCAL_DATA_DIR", &dir);
    std::env::set_var("FLUENTCAL_TEST_RAISED", "1");
    std::fs::write(dir.join("data.json"), json!({
        "version": 1, "tasks": {}, "groups": {}, "dayMarks": {}, "holidays": {},
        "settings": { "general": {"seeded": true, "onboarded": true},
            "holidays": {"autoUpdate": false},
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
                    w.invoke_toggle_lock();
                }
                1 => {
                    // Execute winit's real OS move path. Releasing the left button ends
                    // the modal move loop; no global mouse/keyboard input is injected.
                    let h = hwnd.0 as isize;
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(80));
                        unsafe { let _ = PostMessageW(Some(windows::Win32::Foundation::HWND(h as *mut _)), WM_LBUTTONUP, WPARAM(0), LPARAM(0)); }
                    });
                    w.invoke_start_move();
                }
                2 => {
                    click(&w, point(&w, "完成"));
                    // Back on the desktop, the calendar can be inactive. Use another
                    // window of this process so Windows foreground restrictions do not
                    // turn a missing activation request into a false positive.
                    unsafe {
                        let h = CreateWindowExW(WS_EX_TOOLWINDOW, w!("STATIC"), w!("FluentCal focus test"), WS_POPUP | WS_VISIBLE, -32000, -32000, 10, 10, None, None, None, None).unwrap();
                        other_window.set(h);
                        assert!(SetForegroundWindow(h).as_bool());
                    }
                }
                3 | 4 => {
                    assert!(app::with(|a| a.doc.settings.desk.locked).unwrap());
                    let today = app::with(|a| a.today()).unwrap();
                    let (_, m, d) = fluentcal_core::date::parts(&today);
                    let prefix = format!("{m}月{d}日 ");
                    let cell = w.root_element().query_descendants().match_predicate(move |e| e.accessible_label().is_some_and(|l| l.starts_with(&prefix))).find_first().unwrap();
                    let p = cell.absolute_position();
                    click(&w, LogicalPosition::new(p.x + 40.0, p.y + 50.0));
                }
                5 => {
                    assert!(!w.get_quick_add_date().is_empty(), "double-click opens quick add");
                    assert_eq!(unsafe { GetForegroundWindow() }, hwnd, "quick add must activate the native window");
                    // Send native keys to the OS keyboard focus, not directly to the
                    // Slint input (which would hide a missing activation request).
                    let focused = unsafe { GetFocus() };
                    assert_eq!(focused, hwnd, "quick add receives OS keyboard focus");
                    for ch in "after move".chars() {
                        key(focused, ch.to_ascii_uppercase() as u32);
                    }
                }
                6 => {
                    assert_eq!(w.get_quick_add_text(), "after move", "typing must reach the date's quick-add input after completing a move");
                    key(hwnd, 13);
                }
                7 => {
                    assert!(app::with(|a| a.doc.tasks.values().any(|t| t.title == "after move")).unwrap());
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
