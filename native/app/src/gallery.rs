//! The control gallery window (ui/gallery.slint), 0.2's `?w=gallery`.

use crate::GalleryWindow;
use std::rc::Rc;

/// The gallery's made-up lunar text and 休/班 markers (GalleryApp.vue `lunar` / `markers`).
fn labels(iso: &str) -> (String, String) {
    let d: u32 = iso.get(8..10).and_then(|s| s.parse().ok()).unwrap_or(1);
    let digits = ["一", "二", "三", "四", "五", "六", "七", "八", "九", "十"];
    let lunar = match d {
        1 => "九月".to_owned(),
        10 => "寒露".to_owned(),
        _ => format!("{}{}", ["初", "十", "廿"][((d - 1) / 10).min(2) as usize], digits[((d - 1) % 10) as usize]),
    };
    let marker = if ("2026-10-01".."2026-10-08").contains(&iso) {
        "休"
    } else if iso == "2026-10-10" {
        "班"
    } else {
        ""
    };
    (lunar, marker.to_owned())
}

/// Wires the gallery's helpers. `today` fixes the calendars' today (parity runs).
pub fn install(window: &GalleryWindow, today: &str) {
    crate::fluent::install(window, today, Some(Rc::new(labels)));
}
