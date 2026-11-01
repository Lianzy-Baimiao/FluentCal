//! Renders a window offscreen to PNG for the visual comparison with 0.2 (scripts/parity.mjs
//! writes the scenarios and 0.2's screenshots): `snapshot <scenario.json> <out.png>`.
//! The scenario has the window ("desk", "gallery" or "editor" with its `payload`), the document, the fixed "now", the window
//! size and scale, optionally a wallpaper PNG to draw a transparent window on, and input
//! `events` played before rendering (pointer and keys, logical px). Time only moves when the
//! events say so (`wait`), and 1 s more before the picture so transitions have finished.
//! Nothing appears on screen.

use fluentcal_app::desk::Desk;
use fluentcal_app::theme::{apply, Look};
use fluentcal_app::windows::settings::view::{self as settings_view, Info, Local, WebView};
use fluentcal_app::{DeskWindow, GalleryWindow, SetState, SettingsWindow};
use fluentcal_core::date::local_of_ms;
use fluentcal_core::settings::Desk as DeskSettings;
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::{desk_style, tokens};
use serde_json::Value;
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize, PlatformError, SharedString};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// A platform whose windows render into memory, with a clock the scenario drives.
struct Offscreen {
    window: Rc<RefCell<Option<Rc<MinimalSoftwareWindow>>>>,
    clock: Rc<Cell<Duration>>,
}

impl Platform for Offscreen {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        *self.window.borrow_mut() = Some(w.clone());
        Ok(w)
    }
    fn duration_since_start(&self) -> Duration {
        self.clock.get()
    }
}

fn render(w: &MinimalSoftwareWindow, size: PhysicalSize) -> Vec<PremultipliedRgbaColor> {
    let mut buf = vec![PremultipliedRgbaColor::default(); (size.width * size.height) as usize];
    w.request_redraw();
    w.draw_if_needed(|r| {
        r.render(&mut buf, size.width as usize);
    });
    buf
}

/// Key names as Playwright's keyboard.press uses them → the text Slint's KeyEvent carries.
fn key_text(k: &str) -> SharedString {
    use slint::platform::Key;
    let named = match k {
        "Enter" => Some(Key::Return),
        "Escape" => Some(Key::Escape),
        "Tab" => Some(Key::Tab),
        "Backspace" => Some(Key::Backspace),
        "Delete" => Some(Key::Delete),
        "ArrowUp" => Some(Key::UpArrow),
        "ArrowDown" => Some(Key::DownArrow),
        "ArrowLeft" => Some(Key::LeftArrow),
        "ArrowRight" => Some(Key::RightArrow),
        "Home" => Some(Key::Home),
        "End" => Some(Key::End),
        "PageUp" => Some(Key::PageUp),
        "PageDown" => Some(Key::PageDown),
        "Control" => Some(Key::Control),
        "Alt" => Some(Key::Alt),
        "Shift" => Some(Key::Shift),
        "Space" => None,
        _ => None,
    };
    match named {
        Some(key) => key.into(),
        None if k == "Space" => " ".into(),
        None => k.into(),
    }
}

struct Player<'a> {
    w: &'a MinimalSoftwareWindow,
    clock: &'a Cell<Duration>,
    size: PhysicalSize,
}

impl Player<'_> {
    fn tick(&self, ms: u64) {
        // Small steps so timers and animations see time pass like in an event loop.
        let mut left = ms;
        while left > 0 {
            let step = left.min(16);
            self.clock.set(self.clock.get() + Duration::from_millis(step));
            slint::platform::update_timers_and_animations();
            left -= step;
        }
        slint::platform::update_timers_and_animations();
    }
    fn send(&self, e: WindowEvent) {
        self.w.dispatch_event(e);
        self.tick(16);
    }
    fn play(&self, events: &[Value]) {
        for e in events {
            let f = |k: &str| e.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let pos = LogicalPosition::new(f("x"), f("y"));
            let button = match e.get("button").and_then(Value::as_str) {
                Some("right") => PointerEventButton::Right,
                _ => PointerEventButton::Left,
            };
            match e.get("type").and_then(Value::as_str).unwrap_or("") {
                "move" => self.send(WindowEvent::PointerMoved { position: pos }),
                "down" => self.send(WindowEvent::PointerPressed { position: pos, button }),
                "up" => self.send(WindowEvent::PointerReleased { position: pos, button }),
                "click" => {
                    self.send(WindowEvent::PointerMoved { position: pos });
                    self.send(WindowEvent::PointerPressed { position: pos, button });
                    self.send(WindowEvent::PointerReleased { position: pos, button });
                }
                "key" => {
                    let k = e.get("key").and_then(Value::as_str).unwrap_or("");
                    // "Control+Alt+D": modifiers down, key, then up again.
                    let parts: Vec<&str> = k.split('+').collect();
                    for p in &parts {
                        self.send(WindowEvent::KeyPressed { text: key_text(p) });
                    }
                    for p in parts.iter().rev() {
                        self.send(WindowEvent::KeyReleased { text: key_text(p) });
                    }
                }
                "text" => {
                    for ch in e.get("text").and_then(Value::as_str).unwrap_or("").chars() {
                        let t: SharedString = ch.to_string().into();
                        self.send(WindowEvent::KeyPressed { text: t.clone() });
                        self.send(WindowEvent::KeyReleased { text: t });
                    }
                }
                "wait" => {
                    self.tick(f("ms") as u64);
                    // Elements under `if`/`for` are created when the tree is next visited; an
                    // event loop draws right after a change, so they start their transitions now
                    // rather than in the final picture.
                    let _ = render(self.w, self.size);
                }
                other => eprintln!("unknown event {other}"),
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scenario: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("scenario")).expect("json");
    let out = &args[2];
    let num = |k: &str, d: f64| scenario.get(k).and_then(Value::as_f64).unwrap_or(d);
    let (lw, lh, scale) = (num("width", 880.0), num("height", 640.0), num("scale", 2.0) as f32);
    let size = PhysicalSize::new((lw * f64::from(scale)).round() as u32, (lh * f64::from(scale)).round() as u32);

    let slot = Rc::new(RefCell::new(None));
    let clock = Rc::new(Cell::new(Duration::from_secs(10)));
    slint::platform::set_platform(Box::new(Offscreen { window: slot.clone(), clock: clock.clone() })).unwrap();
    fluentcal_app::fonts::setup();
    let win11 = fluentcal_app::fonts::is_win11();
    let kind = scenario.get("window").and_then(Value::as_str).unwrap_or("desk");

    let doc = Doc::from_value(scenario.get("doc").unwrap_or(&Value::Null));
    let (today, _) = local_of_ms(num("now", 0.0) as i64, num("offset", 480.0) as i32);
    let s = &doc.settings;
    let dark = scenario.get("dark").and_then(Value::as_bool).unwrap_or(s.theme.mode == "dark");
    let system_accent = scenario.get("systemAccent").and_then(Value::as_str).unwrap_or("#0078D4");
    let accent = if s.theme.accent == "system" { system_accent.to_owned() } else { s.theme.accent.clone() };
    let t = tokens(dark, &accent, None);
    let plain = DeskSettings::default();
    let ds = desk_style(if kind == "desk" { &s.desk } else { &plain }, &t);
    let look = Look { tokens: &t, desk: &ds, reduce_motion: false, win11, scale };

    // Keeps the window (and the desk model) alive while rendering.
    let mut desk: Option<Desk> = None;
    let _gallery: Option<GalleryWindow>;
    let mut _editor: Option<fluentcal_app::windows::editor::Offline> = None;
    let mut _settings: Option<SettingsWindow> = None;
    let show: Box<dyn Fn()>;
    if kind == "editor" {
        let p: fluentcal_core::editor::Payload = serde_json::from_value(scenario.get("payload").cloned().unwrap_or(Value::Null)).expect("payload");
        let e = fluentcal_app::windows::editor::Offline::new(doc, &p, &today, t.clone()).unwrap();
        apply(&e.window, &look);
        let h = e.window.clone_strong();
        show = Box::new(move || h.show().unwrap());
        _editor = Some(e);
        _gallery = None;
    } else if kind == "settings" {
        let w = SettingsWindow::new().unwrap();
        apply(&w, &look);
        fluentcal_app::fluent::install(&w, &today, None);
        settings_view::setup_with(&w, scale);
        let (info, local) = settings_scene(&scenario, &doc, scale);
        w.global::<SetState>().set_page(local.page.clone().into());
        settings_view::push(&w, &doc, &info, &local);
        let (weak, d2) = (w.as_weak(), doc.clone());
        w.global::<SetState>().on_action_arg(move |k, v| {
            let (Some(w), true) = (weak.upgrade(), k == "delete-group") else { return };
            if let Some((t, c)) = fluentcal_app::windows::settings::commands::delete_group_question(&d2, &v) {
                fluentcal_app::windows::settings::show_confirm(&w, &t, &c, "删除", "", true);
            }
        });
        let h = w.clone_strong();
        show = Box::new(move || h.show().unwrap());
        _settings = Some(w);
        _gallery = None;
    } else if kind == "gallery" {
        let g = GalleryWindow::new().unwrap();
        apply(&g, &look);
        fluentcal_app::gallery::install(&g, &today);
        let h = g.clone_strong();
        show = Box::new(move || h.show().unwrap());
        _gallery = Some(g);
    } else {
        let window = DeskWindow::new().unwrap();
        apply(&window, &look);
        let mut d = Desk::new(window.clone_strong(), doc, today.clone(), t.clone());
        d.refresh();
        let h = window.clone_strong();
        show = Box::new(move || h.show().unwrap());
        desk = Some(d);
        _gallery = None;
    }
    let sw = slot.borrow().clone().expect("window adapter");
    sw.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
    sw.set_size(size);
    show();
    let player = Player { w: &sw, clock: &clock, size };
    player.tick(16);
    // The desk's lane capacity comes from the layout; lay the chips out again until it settles.
    if let Some(d) = desk.as_mut() {
        for _ in 0..3 {
            let _ = render(&sw, size);
            if !d.sync_capacity() {
                break;
            }
        }
    }
    let _ = render(&sw, size);
    // A desk overlay opened directly (0.2 opened it with its steps; their events aren't
    // replayed here, the offscreen desk has no app behind its callbacks).
    let native = scenario.get("native").filter(|_| kind == "desk");
    if let (Some(spec), Some(d)) = (native, desk.as_mut()) {
        d.open(spec);
        player.tick(16);
        // creates what the overlay shows under `if`, so its transitions start now
        let _ = render(&sw, size);
    } else if let Some(events) = scenario.get("events").and_then(Value::as_array) {
        player.play(events);
    }
    let _ = render(&sw, size);
    player.tick(1000);
    let buf = render(&sw, size);

    let wallpaper = scenario.get("wallpaper").and_then(Value::as_str).map(read_png);
    let mut rgba = Vec::with_capacity(buf.len() * 4);
    for (i, p) in buf.iter().enumerate() {
        let (br, bg, bb) = match &wallpaper {
            Some((ww, data)) if i < data.len() / 3 => {
                let (x, y) = (i as u32 % size.width, i as u32 / size.width);
                let j = ((y * ww + x) * 3) as usize;
                if j + 2 < data.len() { (data[j], data[j + 1], data[j + 2]) } else { (0, 0, 0) }
            }
            _ => (0, 0, 0),
        };
        let a = u32::from(p.alpha);
        let mix = |c: u8, b: u8| (u32::from(c) + u32::from(b) * (255 - a) / 255).min(255) as u8;
        if wallpaper.is_some() {
            rgba.extend_from_slice(&[mix(p.red, br), mix(p.green, bg), mix(p.blue, bb), 255]);
        } else {
            let un = |c: u8| if a == 0 { 0 } else { (u32::from(c) * 255 / a).min(255) as u8 };
            rgba.extend_from_slice(&[un(p.red), un(p.green), un(p.blue), p.alpha]);
        }
    }
    write_png(out, size.width, size.height, &rgba);
    println!("{out} {}x{}{}", size.width, size.height, desk.map_or(String::new(), |d| format!(" capacity {:?}", d.capacity)));
}

/// The settings window's state in a parity scene: 0.2's browser preview mock (src/api.ts),
/// written into the scenario by scripts/parity.mjs as `settingsInfo`.
fn settings_scene(sc: &Value, doc: &Doc, scale: f32) -> (Info, Local) {
    let si = &sc["settingsInfo"];
    let st = |v: &Value| v.as_str().unwrap_or("").to_owned();
    let screens = si["screens"].as_array().map(|a| a.iter().map(|x| (st(&x["value"]), st(&x["label"]), x["current"].as_bool().unwrap_or(false))).collect()).unwrap_or_default();
    let holiday_years = sc["doc"]["holidays"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
    let sync = fluentcal_core::cloud::service::Status {
        enabled: si["sync"]["enabled"].as_bool().unwrap_or(false),
        provider: st(&si["sync"]["provider"]),
        last_sync: si["sync"]["lastSync"].as_i64(),
        remote_device: si["sync"]["remoteDevice"].as_str().map(str::to_owned),
        remote_saved_at: si["sync"]["remoteSavedAt"].as_i64(),
        ..Default::default()
    };
    let info = Info {
        version: st(&si["version"]),
        build: si["build"].as_u64().unwrap_or(26300) as u32,
        debug: si["debug"].as_bool().unwrap_or(true),
        transparency: si["transparency"].as_bool(),
        system_accent: sc["systemAccent"].as_str().map(str::to_owned),
        shortcut_errors: Vec::new(),
        data_dir: fluentcal_core::datadir::DataDir { path: st(&si["dataDir"]), custom: false, missing: si["missingDir"].as_str().map(str::to_owned) },
        sync,
        screens,
        holiday_years,
        now_ms: sc["now"].as_i64().unwrap_or(0),
        offset: sc["offset"].as_i64().unwrap_or(480) as i32,
    };
    let w = &si["web"];
    let addresses: Vec<(String, String, bool, String)> =
        w["addresses"].as_array().map(|a| a.iter().map(|x| (st(&x["ip"]), st(&x["label"]), x["virt"].as_bool().unwrap_or(false), st(&x["url"]))).collect()).unwrap_or_default();
    let qr = addresses.first().map(|a| (a.3.clone(), settings_view::qr_image(&a.3, (168.0 * scale).round() as u32)));
    let web = WebView {
        enabled: doc.settings.web.enabled,
        running: w["running"].as_bool().unwrap_or(false),
        port: w["port"].as_u64().unwrap_or(18765) as u16,
        error: None,
        addresses,
        qr,
    };
    let secrets = si["secrets"].as_array().map(|a| a.iter().map(st).collect()).unwrap_or_default();
    let local = Local { page: st(&si["page"]), web, secrets, ..Default::default() };
    (info, local)
}

/// RGB bytes of a PNG (any colour type png can expand to RGB/RGBA).
fn read_png(path: &str) -> (u32, Vec<u8>) {
    let mut dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).expect("wallpaper")));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().expect("png");
    let mut data = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut data).expect("frame");
    let ch = info.color_type.samples();
    let rgb = data[..info.buffer_size()].chunks(ch).flat_map(|p| if ch >= 3 { [p[0], p[1], p[2]] } else { [p[0]; 3] }).collect();
    (info.width, rgb)
}

fn write_png(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::io::BufWriter::new(std::fs::File::create(path).expect("out"));
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}
