//! M10 原型（需求文档 7.7）：Slint 能否用这台电脑的 GNU 工具链编译，空闲内存多少，中文文字和
//! 输入法效果如何。渲染器用环境变量 SLINT_BACKEND 选（winit-software、winit-femtovg）。

slint::slint! {
    import { LineEdit } from "std-widgets.slint";

    // Does per-pixel transparency reach the screen? Fully transparent apart from a red bar, a
    // 30 % white band and an orange square (FLUENTCAL_SPIKE_ALPHATEST=1).
    export component AlphaTest inherits Window {
        title: "FluentCal 原型";
        no-frame: true;
        width: 600px;
        height: 300px;
        background: transparent;
        Rectangle { y: 20px; height: 30px; background: #ff0000; }
        Rectangle { y: 160px; height: 50px; background: #ffffff4d; }
        Rectangle { x: 280px; y: 240px; width: 40px; height: 40px; background: #ff8000; }
    }

    // An opaque pure-blue window to put the transparency test on (FLUENTCAL_SPIKE_BACKDROP=1).
    export component Backdrop inherits Window {
        title: "FluentCal 背景";
        no-frame: true;
        width: 600px;
        height: 300px;
        background: #0000ff;
    }

    export component Spike inherits Window {
        title: "FluentCal 原型";
        preferred-width: 760px;
        preferred-height: 400px;
        background: transparent;
        in property <string> renderer;

        Rectangle {
            background: #f3f3f3f0;
            border-radius: 8px;
            VerticalLayout {
                padding: 16px;
                spacing: 10px;
                Text { text: "2026年10月"; font-size: 22px; font-weight: 600; color: #1a1a1a; }
                HorizontalLayout {
                    spacing: 4px;
                    for d in [
                        { n: "26", l: "十七", rest: false }, { n: "27", l: "十八", rest: false }, { n: "28", l: "十九", rest: false },
                        { n: "29", l: "二十", rest: false }, { n: "30", l: "廿一", rest: false }, { n: "31", l: "廿二", rest: true }, { n: "1", l: "廿三", rest: true },
                    ]: Rectangle {
                        background: d.rest ? #f0f0f0 : #ffffff;
                        border-radius: 4px;
                        height: 76px;
                        VerticalLayout {
                            padding: 6px;
                            spacing: 4px;
                            HorizontalLayout {
                                spacing: 6px;
                                Text { text: d.n; font-size: 14px; font-weight: 600; color: d.rest ? #c42b1c : #1a1a1a; }
                                Text { text: d.l; font-size: 12px; color: #616161; }
                            }
                            Rectangle {
                                height: 20px;
                                border-radius: 3px;
                                background: #8764b8;
                                Text { x: 6px; text: "视觉设计"; font-size: 12px; color: #ffffff; }
                            }
                        }
                    }
                }
                LineEdit { placeholder-text: "添加任务（测试中文输入法）"; }
                Text { text: "12px：今天 周三 九月初五 国庆节 休 班 寒露 霜降 交周报 17:00 评审会 牙医复诊"; font-size: 12px; color: #1a1a1a; }
                Text { text: "14px：双击日期添加任务，右键任务可完成、改颜色、删除"; font-size: 14px; color: #1a1a1a; }
                Text { text: "渲染器：" + root.renderer; font-size: 12px; color: #616161; }
            }
        }
    }
}

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::{winit, WinitWindowAccessor};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

/// The same pinning as 0.2's desktop.rs: owned by Progman (stays on the desktop through Win+D),
/// a tool window (no taskbar button), never activated, at the bottom of the z-order.
fn pin_to_desktop(hwnd: HWND, pos: Option<(i32, i32)>) {
    unsafe {
        let Ok(progman) = FindWindowW(w!("Progman"), PCWSTR::null()) else { return };
        SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, progman.0 as isize);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) & !WS_EX_APPWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);
        let (x, y, keep) = pos.map_or((0, 0, SWP_NOMOVE), |(x, y)| (x, y, SET_WINDOW_POS_FLAGS(0)));
        let _ = SetWindowPos(hwnd, Some(HWND_BOTTOM), x, y, 0, 0, keep | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }
}

/// Acrylic behind the window, as 0.2 does it (SetWindowCompositionAttribute, ACCENT_ENABLE_ACRYLICBLURBEHIND).
fn set_acrylic(hwnd: HWND) {
    #[repr(C)]
    struct AccentPolicy {
        state: u32,
        flags: u32,
        gradient: u32,
        animation: u32,
    }
    #[repr(C)]
    struct CompositionData {
        attrib: u32,
        data: *mut std::ffi::c_void,
        size: usize,
    }
    type Swca = unsafe extern "system" fn(HWND, *mut CompositionData) -> windows::core::BOOL;
    unsafe {
        use windows::core::s;
        use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
        let Ok(user32) = GetModuleHandleW(w!("user32.dll")) else { return };
        let Some(f) = GetProcAddress(user32, s!("SetWindowCompositionAttribute")) else { return };
        let swca: Swca = std::mem::transmute(f);
        let mut policy = AccentPolicy { state: 4, flags: 0, gradient: 0x0100_0000, animation: 0 };
        let mut data = CompositionData { attrib: 19, data: &mut policy as *mut _ as *mut _, size: std::mem::size_of::<AccentPolicy>() };
        let _ = swca(hwnd, &mut data);
    }
}

fn main() -> Result<(), slint::PlatformError> {
    // FLUENTCAL_SPIKE_POS="x,y" (physical pixels, screen coordinates): where the window goes.
    let pos = std::env::var("FLUENTCAL_SPIKE_POS").ok().and_then(|p| {
        let (x, y) = p.split_once(',')?;
        Some((x.trim().parse::<i32>().ok()?, y.trim().parse::<i32>().ok()?))
    });
    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(|a| {
            use winit::platform::windows::WindowAttributesExtWindows;
            // Never activated, never a taskbar button (not even before pinning).
            let a = a.with_active(false).with_skip_taskbar(true);
            // FLUENTCAL_SPIKE_TRANSPARENT=1: per-pixel transparency (does the renderer keep alpha?)
            if std::env::var("FLUENTCAL_SPIKE_TRANSPARENT").is_ok_and(|v| v == "1") { a.with_transparent(true) } else { a }
        })
        .select()?;
    // FLUENTCAL_SPIKE_QUIT=seconds: measurement runs end by themselves.
    if let Some(s) = std::env::var("FLUENTCAL_SPIKE_QUIT").ok().and_then(|s| s.parse().ok()) {
        slint::Timer::single_shot(std::time::Duration::from_secs(s), || {
            let _ = slint::quit_event_loop();
        });
    }
    if std::env::var("FLUENTCAL_SPIKE_ALPHATEST").is_ok_and(|v| v == "1") {
        let _backdrop = if std::env::var("FLUENTCAL_SPIKE_BACKDROP").is_ok_and(|v| v == "1") {
            let bg = Backdrop::new()?;
            let wb = bg.as_weak();
            let setup = when_created(move |f| wb.upgrade().map(|ui| ui.window().with_winit_window(|win| f(win))).flatten(), pos);
            bg.show()?;
            Some((bg, setup))
        } else {
            None
        };
        let ui = AlphaTest::new()?;
        let w = ui.as_weak();
        let _setup = when_created(move |f| w.upgrade().map(|ui| ui.window().with_winit_window(|win| f(win))).flatten(), pos);
        return ui.run();
    }
    let ui = Spike::new()?;
    ui.set_renderer(std::env::var("SLINT_BACKEND").unwrap_or_else(|_| "默认".into()).into());
    let w = ui.as_weak();
    let _setup = when_created(move |f| w.upgrade().map(|ui| ui.window().with_winit_window(|win| f(win))).flatten(), pos);
    ui.run()
}

type WithWindow = dyn Fn(&mut dyn FnMut(&winit::window::Window) -> bool) -> Option<bool>;

/// The window is created once the event loop runs: set it up as soon as it exists.
fn when_created(with: impl Fn(&mut dyn FnMut(&winit::window::Window) -> bool) -> Option<bool> + 'static, pos: Option<(i32, i32)>) -> slint::Timer {
    let with: Box<WithWindow> = Box::new(with);
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(5), move || {
        if done.get() {
            return;
        }
        let ok = with(&mut |w| match w.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => {
                setup(HWND(h.hwnd.get() as *mut _), pos);
                true
            }
            _ => false,
        });
        if ok == Some(true) {
            done.set(true);
        }
    });
    timer
}

fn setup(hwnd: HWND, pos: Option<(i32, i32)>) {
    let env = |k: &str| std::env::var(k).ok();
    // FLUENTCAL_SPIKE_INSERT_AFTER=hwnd: directly under that window in the z-order (tests).
    if env("FLUENTCAL_SPIKE_TOPMOST").as_deref() == Some("1") {
        // Tests: above everything, at `pos`, never activated.
        let (x, y) = pos.unwrap_or((0, 0));
        unsafe {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
        }
    } else if let Some(after) = env("FLUENTCAL_SPIKE_INSERT_AFTER").and_then(|v| v.parse::<isize>().ok()) {
        let (x, y) = pos.unwrap_or((0, 0));
        unsafe {
            let _ = SetWindowPos(hwnd, Some(HWND(after as *mut _)), x, y, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE);
        }
    } else if env("FLUENTCAL_SPIKE_PIN").as_deref() != Some("0") {
        // FLUENTCAL_SPIKE_PIN=0: an ordinary window, for trying it by hand.
        pin_to_desktop(hwnd, pos);
    }
    if env("FLUENTCAL_SPIKE_ACRYLIC").as_deref() == Some("1") {
        set_acrylic(hwnd);
    }
}
