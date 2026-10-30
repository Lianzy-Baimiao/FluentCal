//! Where the desk window goes when screens change (layout.rs), with the user's real setup:
//! 3840×2160 primary and 2560×1600 secondary to its right, both at 125%.

use fluentcal_lib::layout::{default_rect, moved, target, Home, Mon, Rect};
use serde_json::json;

fn mon(name: &str, x: i32, w: u32, h: u32, scale: f64, primary: bool) -> Mon {
    let rect = Rect { x, y: 0, w, h };
    Mon { name: name.into(), rect, work: Rect { h: h - 40, ..rect }, scale, primary }
}

fn both() -> Vec<Mon> {
    vec![mon(r"\\.\DISPLAY1", 0, 3840, 2160, 1.25, true), mon(r"\\.\DISPLAY2", 3840, 2560, 1600, 1.25, false)]
}

/// Primary turned off: the secondary becomes the only screen and moves to (0,0).
fn secondary_only() -> Vec<Mon> {
    vec![mon(r"\\.\DISPLAY2", 0, 2560, 1600, 1.25, true)]
}

/// The calendar at the top right of the primary screen, as the user put it.
fn home_on_primary() -> Home {
    Home { x: 2860, y: 30, w: 950, h: 700, monitor: Some((r"\\.\DISPLAY1".into(), Rect { x: 0, y: 0, w: 3840, h: 2160 }, 1.25)) }
}

#[test]
fn primary_off_then_on_returns_to_the_users_place() {
    let home = home_on_primary();
    // Off: home's screen is gone and the window is off-screen, so it goes to the remaining one.
    let off = target(&secondary_only(), Some(&home), Some(home.rect())).unwrap();
    assert_eq!(off, Rect { x: 1580, y: 30, w: 950, h: 700 });
    // On again: back home (the bug was coming back at 1580,30, where it had been parked).
    assert_eq!(target(&both(), Some(&home), Some(off)), Some(home.rect()));
}

#[test]
fn home_on_the_secondary_follows_it_when_the_primary_goes() {
    let home = Home { x: 5000, y: 100, w: 950, h: 700, monitor: Some((r"\\.\DISPLAY2".into(), Rect { x: 3840, y: 0, w: 2560, h: 1600 }, 1.25)) };
    // The secondary is now at (0,0): same place on that screen.
    assert_eq!(target(&secondary_only(), Some(&home), None), Some(Rect { x: 1160, y: 100, w: 950, h: 700 }));
    assert_eq!(target(&both(), Some(&home), None), Some(home.rect()));
}

#[test]
fn startup_without_home_and_a_window_left_out_of_reach() {
    assert_eq!(target(&both(), None, None), default_rect(&both()));
    assert_eq!(default_rect(&both()), Some(Rect { x: 2860, y: 30, w: 950, h: 700 }));
    // Display change, no home, window still on a screen: left where it is.
    let c = Rect { x: 100, y: 100, w: 950, h: 700 };
    assert_eq!(target(&both(), None, Some(c)), Some(c));
    // Window nowhere: default place.
    assert_eq!(target(&both(), None, Some(Rect { x: 9000, y: 100, w: 950, h: 700 })), default_rect(&both()));
}

#[test]
fn resolution_or_scale_change_keeps_place_and_logical_size() {
    let home = home_on_primary();
    let small = vec![mon(r"\\.\DISPLAY1", 0, 1920, 1080, 1.0, true)];
    let r = target(&small, Some(&home), None).unwrap();
    assert_eq!((r.w, r.h), (760, 560), "logical size kept: 950×700 at 125% = 760×560 at 100%");
    assert!(r.x + r.w as i32 <= 1920 && r.y >= 0 && r.y + r.h as i32 <= 1040, "inside the work area: {r:?}");
    assert!(r.x > 1000, "still on the right: {r:?}");
}

#[test]
fn old_bounds_without_screen_and_parsing() {
    let old = Home::parse(&json!({ "x": 1580.0, "y": 30, "w": 950, "h": 700 })).unwrap();
    assert_eq!(old.monitor, None);
    assert_eq!(target(&both(), Some(&old), None), Some(old.rect()));
    assert_eq!(target(&secondary_only(), Some(&Home { x: 5000, ..old.clone() }), None), default_rect(&secondary_only()));
    assert!(Home::parse(&json!({ "x": 1, "y": 1, "w": 10, "h": 10 })).is_none(), "implausible size");
    let h = home_on_primary();
    assert_eq!(Home::parse(&serde_json::to_value(&h).unwrap()), Some(h));
}

#[test]
fn moving_to_another_screen_and_back() {
    let all = both();
    let r = Rect { x: 2860, y: 30, w: 950, h: 700 };
    let there = moved(r, &all[0], &all[1]);
    assert_eq!((there.w, there.h), (950, 700));
    assert!(there.x + there.w as i32 >= 6400 - 40 && there.y <= 40, "top right of the secondary: {there:?}");
    let back = moved(there, &all[1], &all[0]);
    assert!((back.x - r.x).abs() <= 2 && (back.y - r.y).abs() <= 2, "{back:?}");
}
