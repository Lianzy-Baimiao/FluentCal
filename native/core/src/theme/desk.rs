//! Task colours (fluent/theme.ts) and the desk calendar's appearance variables (the deskVars
//! of composables/theme.ts).

use super::{best_text_on, contrast, Rgba, Shadow, Tokens, BLACK, WHITE};
use crate::settings::Desk;

/// A task palette entry. `bar` carries white (or black for yellow) text; text_light/text_dark
/// meet 4.5:1 on #FFFFFF / #202020.
#[derive(Debug, Clone, Copy)]
pub struct TaskColor {
    pub key: &'static str,
    pub label: &'static str,
    pub bar: &'static str,
    pub text_light: &'static str,
    pub text_dark: &'static str,
}

const fn tc(key: &'static str, label: &'static str, bar: &'static str, text_light: &'static str, text_dark: &'static str) -> TaskColor {
    TaskColor { key, label, bar, text_light, text_dark }
}

pub const TASK_COLORS: [TaskColor; 10] = [
    tc("red", "红", "#D13438", "#C42B1C", "#FF99A4"),
    tc("orange", "橙", "#B5470D", "#B5470D", "#FFB27D"),
    tc("yellow", "黄", "#FFB900", "#8A6100", "#FCE100"),
    tc("green", "绿", "#107C10", "#0F7B0F", "#6CCB5F"),
    tc("teal", "青", "#03787C", "#03787C", "#4FD6D9"),
    tc("blue", "蓝", "#0078D4", "#0063B1", "#60CDFF"),
    tc("indigo", "靛", "#4F52B2", "#4F52B2", "#A8AAF2"),
    tc("purple", "紫", "#8764B8", "#744DA9", "#D0B3F5"),
    tc("pink", "粉", "#C239B3", "#AD2E9F", "#F7A1E9"),
    tc("gray", "灰", "#7A7574", "#5D5A58", "#BDB8B6"),
];

/// A task colour resolved for the current theme: bar fill, text on the calendar background,
/// text on the bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolved {
    pub bar: Rgba,
    pub text: Rgba,
    pub on_bar: Rgba,
}

fn opaque_hex(s: &str) -> Option<Rgba> {
    matches!(s.trim().trim_start_matches('#').len(), 3 | 6).then(|| Rgba::parse(s)).flatten()
}

/// Colour key (or custom #hex) for the current theme; `None` or unknown → the theme's default
/// text and the accent fill for bars.
pub fn task_color(key: Option<&str>, t: &Tokens) -> Resolved {
    if let Some(k) = key.filter(|k| !k.is_empty()) {
        if let Some(c) = TASK_COLORS.iter().find(|c| c.key == k) {
            let bar = Rgba::hex(c.bar);
            return Resolved { bar, text: Rgba::hex(if t.dark { c.text_dark } else { c.text_light }), on_bar: best_text_on(bar) };
        }
        if let Some(hex) = opaque_hex(k) {
            let bg = if t.dark { Rgba::hex("#202020") } else { WHITE };
            let toward = if t.dark { WHITE } else { BLACK };
            let mut text = hex;
            for i in 1..=10 {
                if contrast(text, bg) >= 4.5 {
                    break;
                }
                text = hex.lerp(toward, f64::from(i) * 0.1);
            }
            return Resolved { bar: hex, text, on_bar: best_text_on(hex) };
        }
    }
    Resolved { bar: t.accent_fill, text: t.text_primary, on_bar: t.text_on_accent }
}

/// Day-mark colour: a palette key's bar colour or a custom hex.
pub fn mark_color(key: &str) -> Option<Rgba> {
    TASK_COLORS.iter().find(|c| c.key == key).map(|c| Rgba::hex(c.bar)).or_else(|| Rgba::parse(key))
}

/// The desk calendar surface, derived from the appearance settings (0.2's deskVars).
#[derive(Debug, Clone, PartialEq)]
pub struct DeskStyle {
    /// Card background.
    pub bg: Rgba,
    pub radius: f64,
    /// Window edge to card: room for the shadow.
    pub pad: f64,
    /// 1px inner border of the card.
    pub stroke: Rgba,
    pub shadow: Vec<Shadow>,
    /// Flyouts and the search panel on the calendar.
    pub panel_bg: Rgba,
    pub cell_bg: Rgba,
    pub cell_gap: f64,
    pub line_gap: f64,
    /// Font scale, 1 = 100 %.
    pub fs: f64,
    pub today: Rgba,
    pub rest_tint: Rgba,
    pub holiday_text: Rgba,
    pub term_text: Rgba,
    /// Halo colour behind text on a see-through calendar, or none.
    pub glow: Option<Rgba>,
}

/// CSS colour from a setting: hex, else the fallback (an invalid value in 0.2 makes the CSS
/// declaration invalid; the fallback here is the "auto" value).
fn setting_color(s: &str, auto: Rgba) -> Rgba {
    Rgba::parse(s).unwrap_or(auto)
}

pub fn desk_style(s: &Desk, t: &Tokens) -> DeskStyle {
    let dark = t.dark;
    let auto_base = Rgba::hex(if dark { "#202020" } else { "#F3F3F3" });
    // alpha()/mixHex() take a hex colour; anything else reads as mid gray there.
    let base = if s.bg_color == "auto" { auto_base } else { Rgba::parse(&s.bg_color).map_or(Rgba::hex("#808080"), |c| c.with_alpha(1.0)) };
    let cell = base.lerp(WHITE, if dark { 0.07 } else { 0.85 });
    let alpha = |c: Rgba, a: f64| c.with_alpha((a.clamp(0.0, 1.0) * 1000.0).round() / 1000.0);
    let glow_on = s.text_shadow == "on" || (s.text_shadow == "auto" && !s.acrylic && s.bg_opacity < 35.0 && s.cell_opacity < 60.0);
    // Border and shadow fade out with the background so a transparent calendar has no frame.
    let k = (s.bg_opacity / 60.0).min(1.0);
    let k3 = |v: f64| ((v * k) * 1000.0).round() / 1000.0;
    let shadow = if s.shadow && !s.acrylic && k > 0.05 {
        vec![
            Shadow { x: 0.0, y: 4.0, blur: 14.0, color: Rgba::new(0.0, 0.0, 0.0, k3(0.22)) },
            Shadow { x: 0.0, y: 0.0, blur: 2.0, color: Rgba::new(0.0, 0.0, 0.0, k3(0.12)) },
        ]
    } else {
        Vec::new()
    };
    let rest_auto = if dark { Rgba::new(0.0, 0.0, 0.0, 0.16) } else { Rgba::new(0.0, 0.0, 0.0, 0.035) };
    DeskStyle {
        bg: alpha(base, s.bg_opacity / 100.0),
        radius: if s.acrylic { 8.0 } else { s.radius },
        pad: if s.acrylic { 0.0 } else if s.shadow { 10.0 } else { 2.0 },
        stroke: if dark { Rgba::new(255.0, 255.0, 255.0, k3(0.1)) } else { Rgba::new(0.0, 0.0, 0.0, k3(0.08)) },
        shadow,
        panel_bg: alpha(base, (s.bg_opacity / 100.0).max(0.82)),
        cell_bg: alpha(cell, s.cell_opacity / 100.0),
        cell_gap: s.cell_gap,
        line_gap: s.line_spacing,
        fs: s.font_scale / 100.0,
        today: if s.today_color == "accent" { t.accent_fill } else { setting_color(&s.today_color, t.accent_fill) },
        rest_tint: if s.rest_bg_color == "auto" {
            rest_auto
        } else {
            Rgba::parse(&s.rest_bg_color).map_or(Rgba::new(128.0, 128.0, 128.0, 0.22), |c| c.with_alpha(0.22))
        },
        holiday_text: setting_color(&s.holiday_text_color, Rgba::hex(if dark { "#FF99A4" } else { "#C42B1C" })),
        term_text: if s.term_text_color == "auto" { t.accent_text } else { setting_color(&s.term_text_color, t.accent_text) },
        glow: glow_on.then(|| if dark { Rgba::new(0.0, 0.0, 0.0, 0.85) } else { Rgba::new(255.0, 255.0, 255.0, 0.9) }),
    }
}
