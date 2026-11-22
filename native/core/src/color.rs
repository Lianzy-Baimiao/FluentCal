//! Colour helpers (0.2's src/lib/color.ts).

/// CSS #RGB[A] / #RRGGBB[AA] → (r, g, b); invalid input gives mid gray.
pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    crate::theme::Rgba::parse(hex).map_or((128,128,128), |c| (c.r as u8,c.g as u8,c.b as u8))
}

pub fn normalize_hex(hex: &str) -> Option<String> {
    let h = hex.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if !matches!(h.len(),3|4|6|8) || !h.bytes().all(|c|c.is_ascii_hexdigit()) { return None; }
    let h = if h.len() < 5 { h.chars().flat_map(|c|[c,c]).collect::<String>() } else { h.to_owned() };
    Some(format!("#{}",h.to_ascii_uppercase()))
}

pub fn with_alpha(hex: &str, alpha: f64) -> String {
    crate::theme::Rgba::parse(hex).unwrap_or(crate::theme::BLACK).with_alpha(alpha).to_hex_alpha()
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
    let s = s.trim();
    let h = match s.strip_prefix('#') {
        Some(h) => h,
        None => return false,
    };
    matches!(h.len(),3|4|6|8) && h.chars().all(|c| c.is_ascii_hexdigit())
}

/// HSV controls use degrees and percentages; storage remains RGB hex.
pub fn hsv_to_hex(h: f64, s: f64, v: f64) -> String {
    let h = h.rem_euclid(360.0) / 60.0;
    let s = s.clamp(0.0, 100.0) / 100.0;
    let v = v.clamp(0.0, 100.0) / 100.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as i32 { 0 => (c,x,0.0), 1 => (x,c,0.0), 2 => (0.0,c,x), 3 => (0.0,x,c), 4 => (x,0.0,c), _ => (c,0.0,x) };
    let m = v - c;
    rgb_to_hex((r+m)*255.0, (g+m)*255.0, (b+m)*255.0)
}

pub fn hex_to_hsv(hex: &str) -> (f64, f64, f64) {
    let (r,g,b) = hex_to_rgb(hex);
    let (r,g,b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let hi = r.max(g).max(b); let lo = r.min(g).min(b); let d = hi-lo;
    let h = if d == 0.0 { 0.0 } else if hi == r { ((g-b)/d).rem_euclid(6.0) } else if hi == g { (b-r)/d+2.0 } else { (r-g)/d+4.0 };
    (h*60.0, if hi == 0.0 { 0.0 } else { d/hi*100.0 }, hi*100.0)
}
