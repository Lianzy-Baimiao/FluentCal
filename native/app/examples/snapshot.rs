//! Renders a window offscreen to PNG for the visual comparison with 0.2 (scripts/parity.mjs
//! writes the scenarios and 0.2's screenshots): `snapshot <scenario.json> <out.png>`.
//! The scenario has the document, the fixed "now", the window size and scale, and optionally a
//! wallpaper PNG to draw the transparent window on. Nothing appears on screen.

use fluentcal_app::desk::Desk;
use fluentcal_app::theme::{apply, Look};
use fluentcal_app::DeskWindow;
use fluentcal_core::date::local_of_ms;
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::{desk_style, tokens};
use serde_json::Value;
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, PhysicalSize, PlatformError};
use std::cell::RefCell;
use std::rc::Rc;

/// A platform whose windows render into memory; keeps the last window for drawing.
struct Holder(Rc<RefCell<Option<Rc<MinimalSoftwareWindow>>>>);

impl Platform for Holder {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let w = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        *self.0.borrow_mut() = Some(w.clone());
        Ok(w)
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scenario: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).expect("scenario")).expect("json");
    let out = &args[2];
    let num = |k: &str, d: f64| scenario.get(k).and_then(Value::as_f64).unwrap_or(d);
    let (lw, lh, scale) = (num("width", 880.0), num("height", 640.0), num("scale", 2.0) as f32);
    let size = PhysicalSize::new((lw * f64::from(scale)).round() as u32, (lh * f64::from(scale)).round() as u32);

    let slot = Rc::new(RefCell::new(None));
    slint::platform::set_platform(Box::new(Holder(slot.clone()))).unwrap();
    fluentcal_app::fonts::setup();

    let doc = Doc::from_value(&scenario["doc"]);
    let (today, _) = local_of_ms(num("now", 0.0) as i64, num("offset", 480.0) as i32);
    let s = &doc.settings;
    let dark = s.theme.mode == "dark";
    let system_accent = scenario.get("systemAccent").and_then(Value::as_str).unwrap_or("#0078D4");
    let accent = if s.theme.accent == "system" { system_accent.to_owned() } else { s.theme.accent.clone() };
    let t = tokens(dark, &accent, None);
    let ds = desk_style(&s.desk, &t);

    let window = DeskWindow::new().unwrap();
    let sw = slot.borrow().clone().expect("window adapter");
    sw.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: scale });
    sw.set_size(size);
    apply(&window, &Look { tokens: &t, desk: &ds, reduce_motion: false, win11: fluentcal_app::fonts::is_win11(), scale });
    let mut desk = Desk::new(window.clone_strong(), doc, today, t.clone());
    desk.refresh();
    window.show().unwrap();
    // The lane capacity comes from the layout; lay the chips out again until it settles.
    for _ in 0..3 {
        let _ = render(&sw, size);
        if !desk.sync_capacity() {
            break;
        }
    }
    let buf = render(&sw, size);

    let wallpaper = scenario.get("wallpaper").and_then(Value::as_str).map(read_png);
    let mut rgba = Vec::with_capacity(buf.len() * 4);
    for (i, p) in buf.iter().enumerate() {
        let (br, bg, bb) = match &wallpaper {
            Some((ww, data)) if i < data.len() / 3 => {
                let (x, y) = (i as u32 % size.width, i as u32 / size.width);
                let j = ((y * ww + x) * 3) as usize;
                (data[j], data[j + 1], data[j + 2])
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
    println!("{out} {}x{} capacity {:?}", size.width, size.height, desk.capacity);
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
