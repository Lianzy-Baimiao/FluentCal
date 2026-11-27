use fluentcal_app::{app, platform::desk, shell::events, startup};
use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};
use slint::ComponentHandle;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::{LibraryLoader::GetModuleHandleW, StationsAndDesktops::*, Threading::GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::*;

fn fixture_window(class: PCWSTR) -> HWND {
    unsafe {
        CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, class, w!("FluentCal layer test"), WS_POPUP | WS_VISIBLE,
            0, 0, 320, 240, None, None, Some(GetModuleHandleW(None).unwrap().into()), None).unwrap()
    }
}

unsafe extern "system" fn host_proc(hwnd: HWND, message: u32, word: WPARAM, param: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, message, word, param)
}

fn above(front: HWND, back: HWND) -> bool {
    let mut current = front;
    while let Ok(next) = unsafe { GetWindow(current, GW_HWNDNEXT) } {
        if next == back { return true; }
        current = next;
    }
    false
}

#[test]
#[ignore = "needs a Windows window station; creates a private, never displayed desktop"]
fn summon_lock_and_late_desktop_owner() {
    std::env::remove_var("FLUENTCAL_TEST_RAISED");
    let name = format!("fluentcal-layer-{}", std::process::id());
    let dir = std::env::temp_dir().join(&name);
    std::fs::create_dir(&dir).unwrap();
    let previous = unsafe { GetThreadDesktop(GetCurrentThreadId()).unwrap() };
    let access = DESKTOP_CREATEWINDOW.0 | DESKTOP_READOBJECTS.0 | DESKTOP_WRITEOBJECTS.0 | DESKTOP_ENUMERATE.0;
    let private = unsafe { CreateDesktopW(&HSTRING::from(name), None, None, Default::default(), access, None).unwrap() };
    unsafe { SetThreadDesktop(private).unwrap() };
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions::default()))).unwrap();
    let component = startup::test_app(dir.clone(), 880.0, 640.0).unwrap();
    let calendar = fixture_window(w!("STATIC"));
    desk::install(calendar);
    desk::set_mode(desk::PinMode::Desktop);
    assert!(unsafe { GetWindow(calendar, GW_OWNER) }.is_err());

    let host_class = WNDCLASSW { hInstance: unsafe { GetModuleHandleW(None).unwrap().into() },
        lpszClassName: w!("Progman"), lpfnWndProc: Some(host_proc), ..Default::default() };
    assert_ne!(unsafe { RegisterClassW(&host_class) }, 0);
    let host = fixture_window(host_class.lpszClassName);
    let cover = fixture_window(w!("STATIC"));
    assert_eq!(unsafe { FindWindowW(w!("Progman"), None) }.unwrap(), host);
    assert!(component.window().is_visible(), "calendar starts logically shown");
    app::with(events::tick).unwrap();
    let late_owner = unsafe { GetWindow(calendar, GW_OWNER) }.ok();
    let above_host = above(calendar, host);
    let below_cover = above(cover, calendar);

    desk::summon();
    assert!(desk::is_summoned() && above(calendar, cover));
    component.invoke_toggle_lock();
    let unlocked_in_front = !component.get_locked() && desk::is_summoned() && above(calendar, cover);
    component.invoke_toggle_lock();
    unsafe { SetWindowLongPtrW(calendar, GWLP_HWNDPARENT, 0); }
    app::with(|app| events::on_desk_event(app, desk::DeskEvent::Reattach)).unwrap();
    let still_in_front = component.get_locked() && desk::is_summoned() && above(calendar, cover)
        && unsafe { GetWindow(calendar, GW_OWNER) }.ok() == Some(host);
    unsafe {
        SendMessageW(calendar, WM_ACTIVATE, Some(WPARAM(WA_INACTIVE as usize)), None);
        let mut message = MSG::default();
        while PeekMessageW(&mut message, Some(calendar), WM_APP, WM_APP + 255, PM_REMOVE).as_bool() {
            DispatchMessageW(&message);
        }
    }
    let dismissed = !desk::is_summoned() && above(cover, calendar) && above(calendar, host);

    desk::set_mode(desk::PinMode::Top);
    app::with(events::tick).unwrap();
    let topmost = || unsafe { GetWindowLongPtrW(calendar, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 != 0 };
    let top_unowned = topmost() && unsafe { GetWindow(calendar, GW_OWNER) }.is_err();
    desk::set_mode(desk::PinMode::Desktop);
    let pinned_again = !topmost() && above(cover, calendar) && above(calendar, host);

    unsafe {
        let _ = ShowWindow(calendar, SW_HIDE);
        SetWindowLongPtrW(calendar, GWLP_HWNDPARENT, 0);
    }
    app::with(events::tick).unwrap();
    let still_hidden = !unsafe { IsWindowVisible(calendar) }.as_bool();
    desk::set_mode(desk::PinMode::Normal);
    app::with(events::tick).unwrap();
    let normal_unowned = unsafe { GetWindow(calendar, GW_OWNER) }.is_err();

    component.hide().unwrap();
    unsafe { DestroyWindow(calendar).unwrap() };
    app::with(events::tick).unwrap();
    let hidden_after_destroy = app::with(|app| !app.desk.window.window().is_visible()).unwrap();
    desk::uninstall();
    if let Some(app) = app::uninstall() { app.store.flush(); }
    drop(component);
    unsafe {
        for window in [cover, host] { DestroyWindow(window).unwrap(); }
        UnregisterClassW(host_class.lpszClassName, Some(host_class.hInstance)).unwrap();
        SetThreadDesktop(previous).unwrap();
        CloseDesktop(private).unwrap();
    }
    assert_eq!(dir.canonicalize().unwrap().parent(), Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
    std::fs::remove_dir_all(dir).unwrap();
    assert!(unlocked_in_front && still_in_front, "unlock/relock and shell notifications must not dismiss a summoned calendar");
    assert_eq!(late_owner, Some(host), "a live unowned calendar must attach to the late Progman");
    assert!(above_host && below_cover, "a recovered calendar must stack between its owner and ordinary windows");
    assert!(dismissed, "deactivation must return the summoned calendar to the desktop");
    assert!(top_unowned && pinned_again && normal_unowned, "desktop recovery must respect explicit pin mode changes");
    assert!(still_hidden && hidden_after_destroy, "recovery must not reveal an explicitly hidden calendar");
}
