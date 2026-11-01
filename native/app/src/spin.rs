//! The sync button's spinning icon (0.2: `animation: sync-spin 1s linear infinite`). The
//! software renderer can't rotate (Slint #6068), so the glyph's outline is read from the icon
//! font and drawn as a Path, one precomputed frame per step of the turn. Built on first use.

use std::fmt::Write;
use std::sync::OnceLock;

/// Frames per turn (1 s): 10° a step.
pub const FRAMES: usize = 36;

struct Pen {
    out: String,
    /// glyph units → the 16-unit icon box, with the turn applied
    map: Box<dyn Fn(f32, f32) -> (f32, f32)>,
}

impl Pen {
    fn pt(&mut self, x: f32, y: f32) {
        let (x, y) = (self.map)(x, y);
        let _ = write!(self.out, " {x:.3} {y:.3}");
    }
}

impl ttf_parser::OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.out.push_str(" M");
        self.pt(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.out.push_str(" L");
        self.pt(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.out.push_str(" Q");
        self.pt(x1, y1);
        self.pt(x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.out.push_str(" C");
        self.pt(x1, y1);
        self.pt(x2, y2);
        self.pt(x, y);
    }
    fn close(&mut self) {
        self.out.push_str(" Z");
    }
}

/// The frames of `glyph` (one character of the icon font) turning about the icon box's
/// centre, as SVG path data in a 16×16 viewbox laid out like the Text the Icon component draws
/// (advance centred, line box centred). Empty if the font isn't there.
pub fn frames(glyph: char) -> &'static [String] {
    static FR: OnceLock<Vec<String>> = OnceLock::new();
    FR.get_or_init(|| build(glyph).unwrap_or_default())
}

fn build(glyph: char) -> Option<Vec<String>> {
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    // Windows 11: Segoe Fluent Icons; Windows 10: Segoe MDL2 Assets (theme.rs picks the same)
    let data = ["SegoeIcons.ttf", "segmdl2.ttf"].iter().find_map(|f| std::fs::read(format!(r"{dir}\Fonts\{f}")).ok())?;
    let face = ttf_parser::Face::parse(&data, 0).ok()?;
    let id = face.glyph_index(glyph)?;
    let em = f32::from(face.units_per_em());
    let s = 16.0 / em;
    let adv = f32::from(face.glyph_hor_advance(id).unwrap_or(face.units_per_em()));
    let (asc, desc) = (f32::from(face.ascender()), f32::from(face.descender()));
    // baseline: the line box (ascent − descent) centred in the 16px box
    let base = 8.0 + (asc + desc) / 2.0 * s;
    let left = 8.0 - adv / 2.0 * s;
    let mut out = Vec::with_capacity(FRAMES);
    for i in 0..FRAMES {
        let a = (i as f32) * std::f32::consts::TAU / FRAMES as f32;
        let (sin, cos) = a.sin_cos();
        let map = Box::new(move |x: f32, y: f32| {
            let (px, py) = (left + x * s - 8.0, base - y * s - 8.0);
            (8.0 + px * cos - py * sin, 8.0 + px * sin + py * cos)
        });
        let mut pen = Pen { out: String::new(), map };
        face.outline_glyph(id, &mut pen)?;
        out.push(pen.out.trim().to_owned());
    }
    Some(out)
}
