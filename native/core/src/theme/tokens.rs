//! Fluent design tokens for one theme and accent (0.2's src/fluent/tokens.css + applyTheme).

use super::{best_text_on, derive_accent, Rgba, TRANSPARENT};

/// One layer of a CSS box-shadow (no spread): offset, blur radius and colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub x: f64,
    pub y: f64,
    pub blur: f64,
    pub color: Rgba,
}

const fn shadow(y: f64, blur: f64, a: f64) -> Shadow {
    Shadow { x: 0.0, y, blur, color: Rgba::new(0.0, 0.0, 0.0, a) }
}

fn h(s: &str) -> Rgba {
    Rgba::hex(s)
}

const fn rgba(r: f64, g: f64, b: f64, a: f64) -> Rgba {
    Rgba::new(r, g, b, a)
}

/// `derived`: computed from the accent palette in `tokens()`. `fixed`: light value, dark value.
macro_rules! token_table {
    (derived { $($dn:ident),* $(,)? } fixed { $($n:ident: $l:expr, $d:expr;)* }) => {
        /// The CSS custom properties of tokens.css, kebab-case names in snake_case.
        #[derive(Debug, Clone, PartialEq)]
        pub struct Tokens {
            pub dark: bool,
            /// light3, light2, light1, base, dark1, dark2, dark3
            pub accent: [Rgba; 7],
            pub shadow_flyout: Vec<Shadow>,
            pub shadow_dialog: Vec<Shadow>,
            pub shadow_card: Vec<Shadow>,
            pub shadow_tooltip: Vec<Shadow>,
            $(pub $dn: Rgba,)*
            $(pub $n: Rgba,)*
        }

        struct Derived {
            $($dn: Rgba,)*
        }

        impl Tokens {
            fn assemble(dark: bool, accent: [Rgba; 7], d: Derived) -> Tokens {
                let k = if dark { 1 } else { 0 };
                let s = |light: f64, dark: f64| if k == 1 { dark } else { light };
                Tokens {
                    dark,
                    accent,
                    shadow_flyout: vec![shadow(8.0, 16.0, s(0.14, 0.26))],
                    shadow_dialog: vec![shadow(32.0, 64.0, s(0.19, 0.37)), shadow(2.0, 21.0, s(0.15, 0.37))],
                    shadow_card: vec![shadow(2.0, 4.0, s(0.04, 0.13))],
                    shadow_tooltip: vec![shadow(4.0, 8.0, s(0.14, 0.26))],
                    $($dn: d.$dn,)*
                    $($n: if dark { $d } else { $l },)*
                }
            }
        }
    };
}

token_table! {
    derived {
        accent_fill, accent_fill_hover, accent_fill_pressed, accent_text, accent_text_secondary,
        text_on_accent, text_on_accent_secondary, attention,
    }
    fixed {
        text_primary: h("#000000E4"), h("#FFFFFF");
        text_secondary: h("#0000009E"), h("#FFFFFFC5");
        text_tertiary: h("#00000072"), h("#FFFFFF8B");
        text_disabled: h("#0000005C"), h("#FFFFFF5D");
        text_inverse: h("#FFFFFF"), h("#000000E4");
        accent_fill_disabled: h("#00000037"), h("#FFFFFF28");
        text_on_accent_disabled: h("#FFFFFF"), h("#FFFFFF87");
        control_fill: h("#FFFFFFB3"), h("#FFFFFF0F");
        control_fill_hover: h("#F9F9F980"), h("#FFFFFF15");
        control_fill_pressed: h("#F9F9F94D"), h("#FFFFFF08");
        control_fill_disabled: h("#F9F9F94D"), h("#FFFFFF0B");
        control_fill_input_active: h("#FFFFFF"), h("#1E1E1EB3");
        control_alt_fill: h("#00000006"), h("#0000001A");
        control_alt_fill_hover: h("#0000000F"), h("#FFFFFF0B");
        control_alt_fill_pressed: h("#00000018"), h("#FFFFFF12");
        control_strong_fill: h("#00000072"), h("#FFFFFF8B");
        control_strong_fill_disabled: h("#00000051"), h("#FFFFFF3F");
        control_solid_fill: h("#FFFFFF"), h("#454545");
        control_stroke: h("#0000000F"), h("#FFFFFF12");
        control_stroke_secondary: h("#00000029"), h("#FFFFFF18");
        control_strong_stroke: h("#00000072"), h("#FFFFFF8B");
        control_strong_stroke_disabled: h("#00000037"), h("#FFFFFF28");
        elev_top: h("#0000000F"), h("#FFFFFF18");
        elev_bottom: h("#00000029"), h("#FFFFFF12");
        accent_elev_top: h("#FFFFFF14"), h("#FFFFFF14");
        accent_elev_bottom: h("#00000066"), h("#00000023");
        subtle_fill_hover: h("#00000009"), h("#FFFFFF0F");
        subtle_fill_pressed: h("#00000006"), h("#FFFFFF0A");
        card_fill: h("#FFFFFFB3"), h("#FFFFFF0D");
        card_fill_secondary: h("#F6F6F680"), h("#FFFFFF08");
        card_stroke: h("#0000000F"), h("#00000019");
        surface_stroke: h("#75757566"), h("#75757566");
        surface_stroke_flyout: h("#0000000F"), h("#00000033");
        layer_fill: h("#FFFFFF80"), h("#3A3A3A4C");
        layer_fill_alt: h("#FFFFFF"), h("#FFFFFF0D");
        solid_base: h("#F3F3F3"), h("#202020");
        solid_secondary: h("#EEEEEE"), h("#1C1C1C");
        solid_tertiary: h("#F9F9F9"), h("#282828");
        solid_quarternary: h("#FFFFFF"), h("#2C2C2C");
        flyout_fill: rgba(252.0, 252.0, 252.0, 0.85), rgba(44.0, 44.0, 44.0, 0.96);
        tooltip_fill: h("#F9F9F9"), h("#2C2C2C");
        smoke: h("#0000004D"), h("#0000004D");
        divider: h("#00000014"), h("#FFFFFF15");
        focus_outer: h("#000000E4"), h("#FFFFFF");
        focus_inner: h("#FFFFFFB3"), h("#000000B3");
        success: h("#0F7B0F"), h("#6CCB5F");
        caution: h("#9D5D00"), h("#FCE100");
        critical: h("#C42B1C"), h("#FF99A4");
        info_bg: h("#F6F6F680"), h("#FFFFFF08");
        success_bg: h("#DFF6DD"), h("#393D1B");
        caution_bg: h("#FFF4CE"), h("#433519");
        critical_bg: h("#FDE7E9"), h("#442726");
    }
}

/// Tokens for a theme. `palette`: the 7 Windows accent shades (light3 … dark3) when the system
/// accent is used and Windows reported them; otherwise they are derived from `accent`.
pub fn tokens(dark: bool, accent: &str, palette: Option<&[String]>) -> Tokens {
    let given: Option<Vec<Rgba>> = palette
        .filter(|p| p.len() == 7)
        .and_then(|p| p.iter().map(|c| Rgba::parse(c).filter(|_| matches!(c.trim_start_matches('#').len(), 3 | 6))).collect());
    let p: [Rgba; 7] = match given {
        Some(v) => [v[0], v[1], v[2], v[3], v[4], v[5], v[6]],
        None => derive_accent(Rgba::parse(accent).unwrap_or(Rgba::hex("#0078D4"))),
    };
    // Light theme fills with dark1, dark theme with light2 (WinUI AccentFillColorDefault).
    let (on_light, on_dark) = (best_text_on(p[4]), best_text_on(p[1]));
    let fill = if dark { p[1] } else { p[4] };
    let on = if dark { on_dark } else { on_light };
    let d = Derived {
        accent_fill: fill,
        accent_fill_hover: fill.mix(0.9, TRANSPARENT),
        accent_fill_pressed: fill.mix(0.8, TRANSPARENT),
        accent_text: if dark { p[0] } else { p[5] },
        accent_text_secondary: if dark { p[0] } else { p[6] },
        text_on_accent: on,
        text_on_accent_secondary: on.mix(if dark { 0.5 } else { 0.7 }, TRANSPARENT),
        attention: fill,
    };
    Tokens::assemble(dark, p, d)
}
