//! Colour helpers (0.2's src/lib/color.ts).

/// #RGB / #RRGGBB → (r, g, b); invalid input gives mid gray.
pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim().trim_start_matches('#');
    let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_owned() };
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return (128, 128, 128);
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(128);
    (p(0), p(2), p(4))
}

pub fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let c = |v: f64| v.clamp(0.0, 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", c(r), c(g), c(b))
}

/// Linear mix: t = 0 → a, t = 1 → b.
pub fn mix_hex(a: &str, b: &str, t: f64) -> String {
    let (x, y) = (hex_to_rgb(a), hex_to_rgb(b));
    let m = |p: u8, q: u8| p as f64 + (q as f64 - p as f64) * t;
    rgb_to_hex(m(x.0, y.0), m(x.1, y.1), m(x.2, y.2))
}

pub fn is_hex(s: &str) -> bool {
    let h = match s.strip_prefix('#') {
        Some(h) => h,
        None => return false,
    };
    (h.len() == 3 || h.len() == 6) && h.chars().all(|c| c.is_ascii_hexdigit())
}
