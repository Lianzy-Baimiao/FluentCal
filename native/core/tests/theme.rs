// Theme colours: 0.2's fluent/theme.test.ts, plus values 0.2's TypeScript computes for the
// same inputs (accent palettes, custom task colours, the desk surface).

use fluentcal_core::settings::Desk;
use fluentcal_core::theme::*;

fn hexes(p: &[Rgba]) -> Vec<String> {
    p.iter().map(|c| c.to_hex()).collect()
}

fn lightness(c: Rgba) -> f64 {
    let v = [c.r, c.g, c.b].map(|x| x / 255.0);
    (v.iter().cloned().fold(0.0, f64::max) + v.iter().cloned().fold(1.0, f64::min)) / 2.0
}

#[test]
fn accent_palette_matches_typescript() {
    assert_eq!(hexes(&derive_accent(Rgba::hex("#0078D4"))), ["#99D3FF", "#4CB1FF", "#008CF8", "#0078D4", "#006DC0", "#005392", "#003B68"]);
    assert_eq!(hexes(&derive_accent(Rgba::hex("#8764B8"))), ["#D6CAE7", "#B7A2D4", "#9677C1", "#8764B8", "#7952B0", "#5C3D86", "#412C60"]);
    assert_eq!(hexes(&derive_accent(Rgba::hex("#FFB900"))), ["#FFE7A8", "#FFD566", "#FFC11F", "#FFB900", "#E7A800", "#B07F00", "#7D5B00"]);
    for base in ["#0078D4", "#FFB900", "#107C10", "#8764B8", "#767676"] {
        let p = derive_accent(Rgba::hex(base));
        for i in 1..7 {
            assert!(lightness(p[i]) < lightness(p[i - 1]), "{base} {i}");
        }
    }
}

#[test]
fn colour_math() {
    assert!((contrast(BLACK, WHITE) - 21.0).abs() < 1e-9);
    assert_eq!(BLACK.lerp(WHITE, 0.5).to_hex(), "#808080");
    assert_eq!(Rgba::parse("#abc").unwrap().to_hex(), "#AABBCC");
    assert_eq!(Rgba::parse("0078d4").unwrap().to_hex(), "#0078D4");
    assert!(Rgba::parse("nope").is_none());
    assert_eq!(best_text_on(Rgba::hex("#0078D4")), WHITE);
    assert_eq!(best_text_on(Rgba::hex("#FFB900")), BLACK);
    assert_eq!(best_text_on(Rgba::hex("#4CC2FF")), BLACK);
    // color-mix with transparent only scales alpha
    let m = Rgba::hex("#0067C0").mix(0.9, TRANSPARENT);
    assert_eq!(m.to_hex(), "#0067C0");
    assert!((m.a - 0.9).abs() < 1e-9);
}

#[test]
fn task_colours() {
    let light = tokens(false, "#0078D4", None);
    let dark = tokens(true, "#0078D4", None);
    for c in TASK_COLORS {
        assert!(contrast(Rgba::hex(c.text_light), WHITE) >= 4.5, "{}", c.key);
        assert!(contrast(Rgba::hex(c.text_dark), Rgba::hex("#202020")) >= 4.5, "{}", c.key);
        let bar = Rgba::hex(c.bar);
        assert!(contrast(best_text_on(bar), bar) >= 4.5, "{}", c.key);
    }
    assert_eq!(task_color(Some("red"), &light).text.to_hex(), "#C42B1C");
    assert_eq!(task_color(Some("red"), &dark).text.to_hex(), "#FF99A4");
    let c = task_color(Some("#FFFF00"), &light);
    assert_eq!((c.bar.to_hex(), c.text.to_hex(), c.on_bar), ("#FFFF00".into(), "#666600".into(), BLACK));
    let c = task_color(Some("#333399"), &dark);
    assert_eq!((c.text.to_hex(), c.on_bar), ("#8585C2".into(), WHITE));
    assert_eq!(task_color(None, &dark).bar, dark.accent_fill);
    assert_eq!(WIN_ACCENTS.len(), 48);
}

#[test]
fn tokens_follow_theme_and_accent() {
    let win: Vec<String> = ["#99EBFF", "#4CC2FF", "#0091F8", "#0078D4", "#0067C0", "#003E92", "#001A68"].map(String::from).to_vec();
    let light = tokens(false, "#0078D4", Some(&win));
    assert_eq!(light.accent_fill.to_hex(), "#0067C0");
    assert_eq!(light.text_on_accent, WHITE);
    assert_eq!(light.accent_text.to_hex(), "#003E92");
    let dark = tokens(true, "#0078D4", Some(&win));
    assert_eq!(dark.accent_fill.to_hex(), "#4CC2FF");
    assert_eq!(dark.text_on_accent, BLACK);
    assert_eq!(dark.text_primary, WHITE);
    assert_eq!(dark.solid_base.to_hex(), "#202020");
    // a bad palette falls back to deriving from the accent
    let bad = vec!["x".to_string(); 7];
    let t = tokens(true, "#FFB900", Some(&bad));
    assert_eq!(t.accent[3].to_hex(), "#FFB900");
    assert_eq!(t.text_on_accent, BLACK);
}

#[test]
fn desk_surface() {
    let light = tokens(false, "#0078D4", None);
    let s = desk_style(&Desk::default(), &light);
    assert_eq!((s.bg.to_hex(), s.bg.a), ("#F3F3F3".into(), 0.72));
    assert_eq!((s.cell_bg.to_hex(), s.cell_bg.a), ("#FDFDFD".into(), 0.55));
    assert_eq!((s.pad, s.radius), (10.0, 8.0));
    assert_eq!(s.shadow.len(), 2);
    assert!((s.shadow[0].color.a - 0.22).abs() < 1e-9);
    assert!(s.glow.is_none());
    assert_eq!(s.holiday_text.to_hex(), "#C42B1C");
    // the "transparent glass" preset of the README's dark screenshot
    let dark = tokens(true, "#8764B8", None);
    let glass = Desk { bg_opacity: 18.0, cell_opacity: 38.0, shadow: false, ..Desk::default() };
    let s = desk_style(&glass, &dark);
    assert_eq!(s.cell_bg.to_hex(), "#303030");
    assert!((s.stroke.a - 0.03).abs() < 1e-9);
    assert!(s.shadow.is_empty());
    assert_eq!(s.pad, 2.0);
    assert_eq!(s.glow, Some(Rgba::new(0.0, 0.0, 0.0, 0.85)));
    assert_eq!(s.term_text, dark.accent_text);
    assert_eq!(mark_color("orange").unwrap().to_hex(), "#B5470D");
}
