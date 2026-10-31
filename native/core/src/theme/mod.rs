//! Theme colours (0.2's src/fluent/theme.ts, tokens.css and the deskVars of
//! composables/theme.ts): the Windows accent palette, the Fluent design tokens for light and
//! dark, task colours and the desk calendar's appearance settings, as plain RGBA values for the
//! UI layer.

mod desk;
mod tokens;

pub use desk::{desk_style, mark_color, task_color, DeskStyle, Resolved, TaskColor, TASK_COLORS};
pub use tokens::{tokens, Shadow, Tokens};

/// sRGB colour with straight (not premultiplied) alpha. Channels 0–255, alpha 0–1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

pub const TRANSPARENT: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
pub const WHITE: Rgba = Rgba { r: 255.0, g: 255.0, b: 255.0, a: 1.0 };
pub const BLACK: Rgba = Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };

impl Rgba {
    pub const fn new(r: f64, g: f64, b: f64, a: f64) -> Rgba {
        Rgba { r, g, b, a }
    }

    /// `#RGB`, `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
    pub fn parse(s: &str) -> Option<Rgba> {
        let h = s.trim().trim_start_matches('#');
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let h: String = match h.len() {
            3 => h.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => h.to_owned(),
            _ => return None,
        };
        let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map(f64::from).ok();
        let a = if h.len() == 8 { p(6)? / 255.0 } else { 1.0 };
        Some(Rgba { r: p(0)?, g: p(2)?, b: p(4)?, a })
    }

    /// For the colour tables: a literal that must parse.
    pub fn hex(s: &str) -> Rgba {
        Rgba::parse(s).unwrap_or_else(|| panic!("bad colour {s}"))
    }

    pub fn with_alpha(self, a: f64) -> Rgba {
        Rgba { a: a.clamp(0.0, 1.0), ..self }
    }

    /// Channels rounded to whole numbers, as a `#RRGGBB` string would hold them.
    pub fn rounded(self) -> Rgba {
        let c = |v: f64| v.clamp(0.0, 255.0).round();
        Rgba { r: c(self.r), g: c(self.g), b: c(self.b), a: self.a }
    }

    /// `#RRGGBB` (alpha dropped).
    pub fn to_hex(self) -> String {
        let c = self.rounded();
        format!("#{:02X}{:02X}{:02X}", c.r as u8, c.g as u8, c.b as u8)
    }

    /// Linear blend of the colour channels, `t` = 0 → self, 1 → `other` (fluent/theme.ts `mix`,
    /// lib/color.ts `mixHex`); rounded like their hex results.
    pub fn lerp(self, other: Rgba, t: f64) -> Rgba {
        let m = |p: f64, q: f64| p + (q - p) * t;
        Rgba { r: m(self.r, other.r), g: m(self.g, other.g), b: m(self.b, other.b), a: 1.0 }.rounded()
    }

    /// CSS `color-mix(in srgb, self p, other)` with p in 0–1: premultiplied interpolation, so
    /// mixing with `transparent` only scales the alpha.
    pub fn mix(self, p: f64, other: Rgba) -> Rgba {
        let q = 1.0 - p;
        let a = self.a * p + other.a * q;
        if a <= 0.0 {
            return TRANSPARENT;
        }
        let c = |x: f64, y: f64| (x * self.a * p + y * other.a * q) / a;
        Rgba { r: c(self.r, other.r), g: c(self.g, other.g), b: c(self.b, other.b), a }
    }

    /// `self` painted over `below` (source-over).
    pub fn over(self, below: Rgba) -> Rgba {
        let a = self.a + below.a * (1.0 - self.a);
        if a <= 0.0 {
            return TRANSPARENT;
        }
        let c = |x: f64, y: f64| (x * self.a + y * below.a * (1.0 - self.a)) / a;
        Rgba { r: c(self.r, below.r), g: c(self.g, below.g), b: c(self.b, below.b), a }
    }

    fn luminance(self) -> f64 {
        let c = self.rounded();
        let lin = |v: f64| {
            let v = v / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
    }
}

/// WCAG 2 contrast ratio between two opaque colours.
pub fn contrast(a: Rgba, b: Rgba) -> f64 {
    let (x, y) = (a.luminance(), b.luminance());
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// White unless it fails AA (4.5:1) and black reads better — matches Windows, which keeps
/// white text on mid-tone accents such as #0078D4.
pub fn best_text_on(fill: Rgba) -> Rgba {
    let white = contrast(fill, WHITE);
    if white >= 4.5 || white >= contrast(fill, BLACK) { WHITE } else { BLACK }
}

fn rgb_to_hsl(c: Rgba) -> (f64, f64, f64) {
    let (r, g, b) = (c.r / 255.0, c.g / 255.0, c.b / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Rgba {
    let k = |n: f64| (n + h / 30.0) % 12.0;
    let a = s * l.min(1.0 - l);
    let f = |n: f64| l - a * (-1.0f64).max((k(n) - 3.0).min((9.0 - k(n)).min(1.0)));
    Rgba::new(f(0.0) * 255.0, f(8.0) * 255.0, f(4.0) * 255.0, 1.0).rounded()
}

/// Windows-like accent shades: [light3, light2, light1, base, dark1, dark2, dark3]. Lightness
/// moves a fixed share of the remaining distance to white/black, calibrated against the
/// default Windows 11 blue palette.
pub fn derive_accent(base: Rgba) -> [Rgba; 7] {
    let base = base.rounded().with_alpha(1.0);
    let (h, s, l) = rgb_to_hsl(base);
    let up = [0.658, 0.399, 0.121].map(|t| hsl_to_rgb(h, s, l + (1.0 - l) * t));
    let down = [0.094, 0.311, 0.509].map(|t| hsl_to_rgb(h, s, l * (1.0 - t)));
    [up[0], up[1], up[2], base, down[0], down[1], down[2]]
}

/// The 48 Windows 11 accent colours, in Settings order.
pub const WIN_ACCENTS: [&str; 48] = [
    "#FFB900", "#FF8C00", "#F7630C", "#CA5010", "#DA3B01", "#EF6950", "#D13438", "#FF4343",
    "#E74856", "#E81123", "#EA005E", "#C30052", "#E3008C", "#BF0077", "#C239B3", "#9A0089",
    "#0078D4", "#0063B1", "#8E8CD8", "#6B69D6", "#8764B8", "#744DA9", "#B146C2", "#881798",
    "#0099BC", "#2D7D9A", "#00B7C3", "#038387", "#00B294", "#018574", "#00CC6A", "#10893E",
    "#7A7574", "#5D5A58", "#68768A", "#515C6B", "#567C73", "#486860", "#498205", "#107C10",
    "#767676", "#4C4A48", "#69797E", "#4A5459", "#647C64", "#525E54", "#847545", "#7E735F",
];
