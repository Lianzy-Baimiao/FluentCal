//! Core theme values (fluentcal_core::theme) into the Slint `Theme` global.

use crate::{DeskVars, Palette, ShadowImage, Theme};
use fluentcal_core::theme::{DeskStyle, Rgba, Tokens};
use slint::{Color, ComponentHandle, Global};

/// Straight-alpha RGBA to a Slint colour.
pub fn color(c: Rgba) -> Color {
    let c8 = |v: f64| v.clamp(0.0, 255.0).round() as u8;
    Color::from_argb_u8(c8(c.a * 255.0), c8(c.r), c8(c.g), c8(c.b))
}

macro_rules! palette {
    ($t:expr; $($f:ident,)*) => {
        Palette { $($f: color($t.$f),)* }
    };
}

pub fn palette(t: &Tokens) -> Palette {
    palette! { t;
        accent_fill, accent_fill_hover, accent_fill_pressed, accent_text, accent_text_secondary, text_on_accent,
        text_on_accent_secondary, attention, text_primary, text_secondary, text_tertiary, text_disabled,
        text_inverse, accent_fill_disabled, text_on_accent_disabled, control_fill, control_fill_hover, control_fill_pressed,
        control_fill_disabled, control_fill_input_active, control_alt_fill, control_alt_fill_hover, control_alt_fill_pressed, control_strong_fill,
        control_strong_fill_disabled, control_solid_fill, control_stroke, control_stroke_secondary, control_strong_stroke, control_strong_stroke_disabled,
        elev_top, elev_bottom, accent_elev_top, accent_elev_bottom, subtle_fill_hover, subtle_fill_pressed,
        card_fill, card_fill_secondary, card_stroke, surface_stroke, surface_stroke_flyout, layer_fill,
        layer_fill_alt, solid_base, solid_secondary, solid_tertiary, solid_quarternary, flyout_fill,
        tooltip_fill, smoke, divider, focus_outer, focus_inner, success,
        caution, critical, info_bg, success_bg, caution_bg, critical_bg,
    }
}

pub fn desk_vars(d: &DeskStyle) -> DeskVars {
    DeskVars {
        bg: color(d.bg),
        radius: d.radius as f32,
        pad: d.pad as f32,
        stroke: color(d.stroke),
        panel_bg: color(d.panel_bg),
        cell_bg: color(d.cell_bg),
        cell_gap: d.cell_gap as f32,
        line_gap: d.line_gap as f32,
        fs: d.fs as f32,
        today: color(d.today),
        rest_tint: color(d.rest_tint),
        holiday_text: color(d.holiday_text),
        term_text: color(d.term_text),
        glow: d.glow.is_some(),
        glow_color: d.glow.map_or(Color::default(), color),
    }
}

/// What a window's Theme global shows.
pub struct Look<'a> {
    pub tokens: &'a Tokens,
    pub desk: &'a DeskStyle,
    pub reduce_motion: bool,
    /// Windows 11 fonts (Segoe UI Variable, Segoe Fluent Icons); Windows 10 has the older ones.
    pub win11: bool,
    pub scale: f32,
}

/// Sets the Theme global of one window (each window has its own copy of Slint globals).
pub fn apply<'a, C>(window: &'a C, look: &Look)
where
    C: ComponentHandle,
    Theme<'a>: Global<'a, C>,
{
    let g = window.global::<Theme>();
    g.set_dark(look.tokens.dark);
    g.set_c(palette(look.tokens));
    g.set_desk(desk_vars(look.desk));
    g.set_reduce_motion(look.reduce_motion);
    if !look.win11 {
        g.set_font_ui("Segoe UI".into());
        g.set_font_display("Segoe UI".into());
        g.set_font_icon("Segoe MDL2 Assets".into());
    }
    let sh = |layers: &[fluentcal_core::theme::Shadow], radius: f64| -> ShadowImage {
        crate::shadow::render(layers, radius, look.scale)
    };
    g.set_shadow_desk(sh(&look.desk.shadow, look.desk.radius));
    g.set_shadow_flyout(sh(&look.tokens.shadow_flyout, 8.0));
    g.set_shadow_tooltip(sh(&look.tokens.shadow_tooltip, 4.0));
    g.set_shadow_dialog(sh(&look.tokens.shadow_dialog, 8.0));
}
