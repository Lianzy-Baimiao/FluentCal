//! Helpers the Fluent controls call through the `Fluent` global (ui/inputs.slint): date
//! arithmetic and parsing for the date and time pickers (0.2's src/fluent/dates.ts), keyboard
//! shortcut text (0.2's src/fluent/shortcut.ts).

use crate::{CalDay, Fluent, Swatch};
use slint::Model;
use fluentcal_core::date::{add_days, add_months, days_in_month, parts, weekday, ymd};
use slint::{ComponentHandle, Global, ModelRc, SharedString, VecModel};
use std::rc::Rc;

/// Lunar text and 休/班 marker for a date (the calendar views show them when asked).
pub type DayLabels = Rc<dyn Fn(&str) -> (String, String)>;

/// Wires the `Fluent` global of one window. `today`: the calendars' today ("" = the clock's);
/// `labels`: lunar text and markers (None = none).
pub fn install<'a, C>(window: &'a C, today: &str, labels: Option<DayLabels>)
where
    C: ComponentHandle,
    Fluent<'a>: Global<'a, C>,
{
    let g = window.global::<Fluent>();
    let today = if today.is_empty() { fluentcal_core::date::today() } else { today.to_owned() };
    g.set_today(today.clone().into());
    let (_, minutes, _) = fluentcal_core::date::local_now();
    g.set_now_minutes(minutes as i32);
    g.on_add_days(|d, n| add_days(&d, n).into());
    g.on_add_months(|d, n| add_months(&d, n).into());
    g.on_make_date(|y, m, d| ymd(y, m.clamp(1, 12) as u32, (d.max(1) as u32).min(days_in_month(y, m.clamp(1, 12) as u32))).into());
    g.on_year_of(|d| parts(&d).0);
    g.on_month_of(|d| parts(&d).1 as i32);
    g.on_day_of(|d| parts(&d).2 as i32);
    g.on_weekday_of(|d| weekday(&d) as i32);
    let base = today.clone();
    g.on_parse_date(move |t| parse_date(&t, &base).unwrap_or_default().into());
    g.on_parse_time(|t| parse_time(&t).unwrap_or_default().into());
    g.on_time_minutes(|t| parse_time(&t).map_or(-1, |s| minutes_of(&s)));
    g.on_trim(|t| t.trim().into());
    g.on_normalize_hex(|t| normalize_hex(&t).unwrap_or_default().into());
    g.on_text_on(|t| {
        let c = normalize_hex(&t).and_then(|h| fluentcal_core::theme::Rgba::parse(&h)).unwrap_or(fluentcal_core::theme::BLACK);
        crate::theme::color(fluentcal_core::theme::best_text_on(c))
    });
    g.on_custom_value(|v, list: ModelRc<Swatch>| {
        if v.is_empty() || list.iter().any(|s| s.key == v) {
            return SharedString::new();
        }
        normalize_hex(&v).unwrap_or_default().into()
    });
    g.on_hex_color(|t| {
        normalize_hex(&t).and_then(|h| fluentcal_core::theme::Rgba::parse(&h)).map_or(slint::Color::from_argb_u8(0, 0, 0, 0), crate::theme::color)
    });
    g.on_shortcut_labels(|s| strings(shortcut_labels(&s)));
    g.on_shortcut_from(|key, c, a, s, m| shortcut_from(&key, c, a, s, m).unwrap_or_default().into());
    g.on_month_grid(move |y, m, first, lunar, markers| {
        let first_of = ymd(y, m as u32, 1);
        let lead = (weekday(&first_of) as i32 - first + 7) % 7;
        let start = add_days(&first_of, -lead);
        let days: Vec<CalDay> = (0..42)
            .map(|i| {
                let iso = add_days(&start, i);
                let (_, dm, dd) = parts(&iso);
                let (lt, mk) = match (&labels, lunar || markers) {
                    (Some(f), true) => f(&iso),
                    _ => (String::new(), String::new()),
                };
                CalDay {
                    day: dd as i32,
                    out: dm as i32 != m,
                    lunar: if lunar { lt.into() } else { SharedString::new() },
                    marker: if markers { mk.into() } else { SharedString::new() },
                    iso: iso.into(),
                }
            })
            .collect();
        ModelRc::new(VecModel::from(days))
    });
}

/// theme.ts normalizeHex: "#abc" / "0078d4" → "#AABBCC".
pub fn normalize_hex(s: &str) -> Option<String> {
    let h = s.trim().trim_start_matches('#');
    if !(h.len() == 3 || h.len() == 6) || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_owned() };
    Some(format!("#{}", full.to_ascii_uppercase()))
}

fn strings(v: Vec<String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(v.into_iter().map(SharedString::from).collect::<Vec<_>>()))
}

fn minutes_of(hhmm: &str) -> i32 {
    let (h, m) = hhmm.split_once(':').unwrap_or(("0", "0"));
    h.parse::<i32>().unwrap_or(0) * 60 + m.parse::<i32>().unwrap_or(0)
}

/// Leading run of ASCII digits (at most `max`), and the rest.
fn digits(s: &str, max: usize) -> Option<(u32, &str)> {
    let n = s.bytes().take_while(u8::is_ascii_digit).count().min(max);
    if n == 0 {
        return None;
    }
    Some((s[..n].parse().ok()?, &s[n..]))
}

/// Typed dates: 2026-10-30, 2026/10/30, 2026.10.30, 2026年10月30日, 10/30, 10-30, 10月30日
/// (this year), 20261030, 今天/明天/后天/昨天/前天. None when invalid.
pub fn parse_date(text: &str, today: &str) -> Option<String> {
    let normalized = text.trim().to_lowercase();
    let t = normalized.as_str();
    let rel = [("今天", 0), ("今日", 0), ("明天", 1), ("后天", 2), ("昨天", -1), ("前天", -2), ("後天", 2), ("today", 0), ("tomorrow", 1), ("yesterday", -1)];
    if let Some((_, n)) = rel.iter().find(|(k, _)| *k == t) {
        return Some(add_days(today, *n));
    }
    let sep = |s: &str, set: &[char]| -> Option<String> {
        let s = s.trim_start();
        let c = s.chars().next()?;
        set.contains(&c).then(|| s[c.len_utf8()..].trim_start().to_owned())
    };
    let end_day = |s: &str| {
        let s = s.trim();
        s.is_empty() || s == "日"
    };
    let (y, mo, d);
    if t.len() == 8 && t.bytes().all(|b| b.is_ascii_digit()) {
        (y, mo, d) = (t[..4].parse::<i32>().ok()?, t[4..6].parse().ok()?, t[6..].parse().ok()?);
    } else if let Some((yy, rest)) = digits(t, 4).filter(|(_, r)| t.len() - r.len() == 4) {
        let rest = sep(rest, &['-', '/', '.', '年'])?;
        let (m1, rest) = digits(&rest, 2)?;
        let rest = sep(rest, &['-', '/', '.', '月'])?;
        let (d1, rest) = digits(&rest, 2)?;
        if !end_day(rest) {
            return None;
        }
        (y, mo, d) = (yy as i32, m1, d1);
    } else {
        let (m1, rest) = digits(t, 2)?;
        let rest = sep(rest, &['-', '/', '.', '月'])?;
        let (d1, rest) = digits(&rest, 2)?;
        if !end_day(rest) {
            return None;
        }
        (y, mo, d) = (parts(today).0, m1, d1);
    }
    if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) {
        return None;
    }
    Some(ymd(y, mo, d))
}

/// Typed times: 9, 930, 0930, 9:30, 21.05, 9点, 9点半, 9点15分, 下午3点. "HH:mm" or None.
pub fn parse_time(text: &str) -> Option<String> {
    let mut t = text.trim().replace('：', ":");
    let mut pm = false;
    let lower = t.to_lowercase();
    for p in ["下午", "晚上", "pm"] {
        if lower.starts_with(p) {
            pm = true;
            t = t[p.len()..].trim_start().to_owned();
            break;
        }
    }
    if !pm {
        for p in ["上午", "早上", "am"] {
            if t.to_lowercase().starts_with(p) {
                t = t[p.len()..].trim_start().to_owned();
                break;
            }
        }
    }
    let (mut h, mi);
    if let Some(i) = t.find('点') {
        let (hs, rest) = (&t[..i], t[i + '点'.len_utf8()..].trim());
        h = hs.trim().parse::<u32>().ok().filter(|_| hs.trim().len() <= 2)?;
        mi = if rest.is_empty() {
            0
        } else if rest == "半" {
            30
        } else {
            let r = rest.strip_suffix('分').unwrap_or(rest).trim();
            if r.is_empty() || r.len() > 2 || !r.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            r.parse().ok()?
        };
    } else if let Some((a, b)) = t.split_once([':', '.']) {
        let ok = |s: &str| (1..=2).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit());
        if !ok(a) || !ok(b) {
            return None;
        }
        (h, mi) = (a.parse().ok()?, b.parse().ok()?);
    } else {
        if t.is_empty() || t.len() > 4 || !t.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if t.len() <= 2 {
            (h, mi) = (t.parse().ok()?, 0);
        } else {
            (h, mi) = (t[..t.len() - 2].parse().ok()?, t[t.len() - 2..].parse().ok()?);
        }
    }
    if pm && h < 12 {
        h += 12;
    }
    if h > 23 || mi > 59 {
        return None;
    }
    Some(format!("{h:02}:{mi:02}"))
}

const LABELS: [(&str, &str); 20] = [
    ("Super", "Win"), ("Minus", "-"), ("Equal", "="), ("BracketLeft", "["), ("BracketRight", "]"),
    ("Backslash", "\\"), ("Semicolon", ";"), ("Quote", "'"), ("Backquote", "`"), ("Comma", ","),
    ("Period", "."), ("Slash", "/"), ("Up", "↑"), ("Down", "↓"), ("Left", "←"), ("Right", "→"),
    ("Escape", "Esc"), ("Delete", "Del"), ("PageUp", "PgUp"), ("PageDown", "PgDn"),
];

/// Keycap labels: "Ctrl+Alt+D" → ["Ctrl", "Alt", "D"].
pub fn shortcut_labels(s: &str) -> Vec<String> {
    s.split('+').filter(|t| !t.is_empty()).map(|t| LABELS.iter().find(|(k, _)| *k == t).map_or(t, |(_, v)| v).to_owned()).collect()
}

/// The key token of a key press (shortcut.ts codeToKey, from the text Slint gives the key):
/// letters, digits (also when Shift turned them into symbols), F1–F24, punctuation, named keys.
pub fn key_token(text: &str) -> Option<String> {
    use slint::platform::Key;
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let named: [(Key, &str); 17] = [
        (Key::Space, "Space"), (Key::Return, "Enter"), (Key::Tab, "Tab"), (Key::Escape, "Escape"),
        (Key::Backspace, "Backspace"), (Key::Delete, "Delete"), (Key::Insert, "Insert"), (Key::Home, "Home"),
        (Key::End, "End"), (Key::PageUp, "PageUp"), (Key::PageDown, "PageDown"), (Key::UpArrow, "Up"),
        (Key::DownArrow, "Down"), (Key::LeftArrow, "Left"), (Key::RightArrow, "Right"), (Key::Pause, "Pause"),
        (Key::ScrollLock, "ScrollLock"),
    ];
    for (k, name) in named {
        if char::from(k) == c {
            return Some(name.into());
        }
    }
    let f = [
        Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12,
        Key::F13, Key::F14, Key::F15, Key::F16, Key::F17, Key::F18, Key::F19, Key::F20, Key::F21, Key::F22, Key::F23, Key::F24,
    ];
    if let Some(i) = f.iter().position(|k| char::from(k.clone()) == c) {
        return Some(format!("F{}", i + 1));
    }
    // Ctrl+letter can arrive as a control character.
    if ('\u{1}'..='\u{1a}').contains(&c) {
        return Some(char::from(c as u8 + 0x40).to_string());
    }
    if c.is_ascii_alphabetic() {
        return Some(c.to_ascii_uppercase().to_string());
    }
    if c.is_ascii_digit() {
        return Some(c.to_string());
    }
    let shifted = [(')', '0'), ('!', '1'), ('@', '2'), ('#', '3'), ('$', '4'), ('%', '5'), ('^', '6'), ('&', '7'), ('*', '8'), ('(', '9')];
    if let Some((_, d)) = shifted.iter().find(|(s, _)| *s == c) {
        return Some(d.to_string());
    }
    let punct = [
        ('-', "Minus"), ('_', "Minus"), ('=', "Equal"), ('+', "Equal"), ('[', "BracketLeft"), ('{', "BracketLeft"),
        (']', "BracketRight"), ('}', "BracketRight"), ('\\', "Backslash"), ('|', "Backslash"), (';', "Semicolon"),
        (':', "Semicolon"), ('\'', "Quote"), ('"', "Quote"), ('`', "Backquote"), ('~', "Backquote"), (',', "Comma"),
        ('<', "Comma"), ('.', "Period"), ('>', "Period"), ('/', "Slash"), ('?', "Slash"),
    ];
    punct.iter().find(|(p, _)| *p == c).map(|(_, n)| (*n).to_owned())
}

/// "Ctrl+Alt+D" from a key press with modifiers; None until a key comes with a modifier.
pub fn shortcut_from(text: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Option<String> {
    let key = key_token(text)?;
    let mods: Vec<&str> = [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift"), (meta, "Super")].into_iter().filter(|(on, _)| *on).map(|(_, n)| n).collect();
    if mods.is_empty() {
        return None;
    }
    Some(format!("{}+{key}", mods.join("+")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_like_dates_ts() {
        let base = "2026-10-14";
        assert_eq!(parse_date("2026-10-30", base).as_deref(), Some("2026-10-30"));
        assert_eq!(parse_date("2026/1/5", base).as_deref(), Some("2026-01-05"));
        assert_eq!(parse_date("2026.10.30", base).as_deref(), Some("2026-10-30"));
        assert_eq!(parse_date("2026年10月30日", base).as_deref(), Some("2026-10-30"));
        assert_eq!(parse_date("10/30", base).as_deref(), Some("2026-10-30"));
        assert_eq!(parse_date("10月30日", base).as_deref(), Some("2026-10-30"));
        assert_eq!(parse_date("20261030", base).as_deref(), Some("2026-10-30"));
        assert_eq!(parse_date("明天", base).as_deref(), Some("2026-10-15"));
        assert_eq!(parse_date("前天", base).as_deref(), Some("2026-10-12"));
        assert_eq!(parse_date("2026-02-30", base), None);
        assert_eq!(parse_date("13/1", base), None);
        assert_eq!(parse_date("abc", base), None);
    }

    #[test]
    fn times_like_dates_ts() {
        assert_eq!(parse_time("9").as_deref(), Some("09:00"));
        assert_eq!(parse_time("930").as_deref(), Some("09:30"));
        assert_eq!(parse_time("0930").as_deref(), Some("09:30"));
        assert_eq!(parse_time("9:30").as_deref(), Some("09:30"));
        assert_eq!(parse_time("21.05").as_deref(), Some("21:05"));
        assert_eq!(parse_time("9点").as_deref(), Some("09:00"));
        assert_eq!(parse_time("9点半").as_deref(), Some("09:30"));
        assert_eq!(parse_time("9点15分").as_deref(), Some("09:15"));
        assert_eq!(parse_time("下午3点").as_deref(), Some("15:00"));
        assert_eq!(parse_time("9：30").as_deref(), Some("09:30"));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("9:60"), None);
        assert_eq!(parse_time("x"), None);
    }

    #[test]
    fn shortcuts_like_shortcut_ts() {
        assert_eq!(shortcut_labels("Ctrl+Alt+D"), ["Ctrl", "Alt", "D"]);
        assert_eq!(shortcut_labels("Super+Minus"), ["Win", "-"]);
        assert!(shortcut_labels("").is_empty());
        assert_eq!(shortcut_from("d", true, true, false, false).as_deref(), Some("Ctrl+Alt+D"));
        assert_eq!(shortcut_from("\u{4}", true, false, false, false).as_deref(), Some("Ctrl+D"));
        assert_eq!(shortcut_from("!", true, false, true, false).as_deref(), Some("Ctrl+Shift+1"));
        assert_eq!(shortcut_from("d", false, false, false, false), None);
        let f5: SharedString = slint::platform::Key::F5.into();
        assert_eq!(shortcut_from(&f5, false, true, false, false).as_deref(), Some("Alt+F5"));
    }
}
