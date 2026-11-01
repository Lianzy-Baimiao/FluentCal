//! View models of the desk's menus and flyouts (the "+N" day list, search, the Fluent menu):
//! plain data from the document, shared by the live window and the parity snapshots.

use super::menus::Entry;
use crate::theme::color;
use crate::{ChipData, MenuEntry, MenuLeaf, SearchHitData, Swatch};
use fluentcal_core::date::{format_friendly, format_range};
use fluentcal_core::lunar::day_info;
use fluentcal_core::recur::occurrences;
use fluentcal_core::search::{search_tasks, SearchOptions};
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::{task_color, Rgba, Tokens, TASK_COLORS};
use fluentcal_core::week::Chip;
use slint::{ModelRc, VecModel};
use std::collections::HashMap;
use std::sync::LazyLock;

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

/// 0.2's icon names (camelCase, icons.ts) → the glyph, read from ui/icons.slint's `Glyph` so the
/// two can't drift; "svg:…" for the two SVG icons (AnyIcon draws them).
static GLYPHS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let src = include_str!("../../ui/icons.slint");
    let mut map = HashMap::new();
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix("out property <string> ") else { continue };
        let Some((name, value)) = rest.split_once(':') else { continue };
        let value = value.trim().trim_end_matches(';').trim_matches('"');
        let glyph: String = match value.strip_prefix("\\u{").and_then(|v| v.strip_suffix('}')) {
            Some(hex) => u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).map(String::from).unwrap_or_default(),
            None => value.to_owned(),
        };
        let mut camel = String::new();
        let mut up = false;
        for ch in name.trim().chars() {
            if ch == '-' {
                up = true;
            } else if up {
                camel.extend(ch.to_uppercase());
                up = false;
            } else {
                camel.push(ch);
            }
        }
        map.insert(camel, glyph);
    }
    map.insert("gantt".into(), "svg:gantt".into());
    map.insert("milestone".into(), "svg:milestone".into());
    map
});

pub fn glyph(name: &str) -> String {
    GLYPHS.get(name).cloned().unwrap_or_default()
}

/// A palette swatch (task colours, day colours): the colour, its check mark colour.
fn swatch(key: &str, title: &str, selected: bool, t: &Tokens) -> Swatch {
    if key.is_empty() {
        return Swatch { key: "".into(), color: color(Rgba::new(0.0, 0.0, 0.0, 0.0)), fg: color(t.text_primary), title: title.into(), selected, none: true };
    }
    let c = task_color(Some(key), t);
    Swatch { key: key.into(), color: color(c.bar), fg: color(c.on_bar), title: title.into(), selected, none: false }
}

/// The search panel's colour filter (0.2's TASK_COLORS as swatches).
pub fn search_colors(t: &Tokens) -> ModelRc<Swatch> {
    model(TASK_COLORS.iter().map(|c| swatch(c.key, c.label, false, t)).collect())
}

/// Menu entries for the Fluent menu, and whether it needs the check / icon columns (0.2's
/// MenuPanel hasCheck / hasIcon).
pub fn menu(entries: &[Entry], t: &Tokens) -> (ModelRc<MenuEntry>, bool, bool) {
    let has_check = entries.iter().any(|e| e.checked.is_some());
    let has_icon = entries.iter().any(|e| !e.icon.is_empty());
    let rows = entries
        .iter()
        .map(|e| {
            let kind = if e.separator {
                "separator"
            } else if !e.swatches.is_empty() {
                "swatches"
            } else {
                "item"
            };
            let swatches = e.swatches.iter().map(|(k, _, title, on)| swatch(k, title, *on, t)).collect();
            let children: Vec<MenuLeaf> = e
                .children
                .iter()
                .map(|c| MenuLeaf {
                    key: c.id.clone().into(),
                    label: c.label.clone().into(),
                    checked: c.checked == Some(true),
                    checkable: c.checked.is_some(),
                    radio: c.radio,
                    disabled: c.disabled,
                    danger: c.danger,
                })
                .collect();
            MenuEntry {
                key: e.id.clone().into(),
                kind: kind.into(),
                label: e.label.clone().into(),
                icon: glyph(&e.icon).into(),
                shortcut: e.shortcut.clone().into(),
                checkable: e.checked.is_some(),
                checked: e.checked == Some(true),
                radio: e.radio,
                disabled: e.disabled,
                danger: e.danger,
                swatches: model(swatches),
                children_check: e.children.iter().any(|c| c.checked.is_some()),
                children: model(children),
            }
        })
        .collect();
    (model(rows), has_check, has_icon)
}

/// "+N": the day's title, lunar label and tasks (0.2's DayListFlyout: completed ones only
/// when shown, done last, then by time and order).
pub fn day_list(doc: &Doc, date: &str, today: &str, t: &Tokens) -> (String, String, ModelRc<ChipData>) {
    let show_done = doc.settings.tasks.show_completed;
    let mut occs: Vec<_> = doc.tasks.values().flat_map(|task| occurrences(task, date, date, 50)).filter(|o| show_done || !o.done).collect();
    occs.sort_by(|a, b| {
        a.done
            .cmp(&b.done)
            .then_with(|| a.task.time.as_deref().unwrap_or("").cmp(b.task.time.as_deref().unwrap_or("")))
            .then_with(|| a.task.order.total_cmp(&b.task.order))
    });
    let chips = occs
        .into_iter()
        .map(|occ| {
            let ch = Chip { key: format!("{}@{}", occ.task.id, occ.date), occ, col: 0, span: 1, lane: 0, cont_left: false, cont_right: false, multi_day: false, visible: true };
            super::vm::chip_data(doc, &ch, today, t)
        })
        .collect();
    (format_friendly(date, today), day_info(date).lunar_label, model(chips))
}

/// The welcome bar (0.2: not onboarded and sample tasks left).
pub fn welcome(doc: &Doc, read_only: bool) -> bool {
    !read_only && !doc.settings.general.onboarded && fluentcal_core::tasks::sample_count(doc) > 0
}

/// Search results (0.2's SearchPanel: up to 100, only once something is asked) and the text
/// shown when there are none.
pub fn search(doc: &Doc, o: &SearchOptions, today: &str, t: &Tokens) -> (ModelRc<SearchHitData>, String) {
    let asked = !o.q.trim().is_empty() || !o.color.is_empty() || o.status != "all";
    let tasks: Vec<_> = doc.tasks.values().cloned().collect();
    let hits: Vec<SearchHitData> = if asked {
        search_tasks(&tasks, &doc.groups, o, today)
            .into_iter()
            .take(100)
            .map(|h| SearchHitData {
                task_id: h.task.id.clone().into(),
                date: h.date.clone().into(),
                title: h.task.title.clone().into(),
                done: h.task.done,
                color: color(task_color(h.task.color.as_deref(), t).bar),
                when: if h.task.repeat.is_some() { format!("{}（重复）", format_friendly(&h.date, today)) } else { format_range(&h.task.start, &h.task.end, today) }.into(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let empty = if !o.q.trim().is_empty() { "没有找到相关任务" } else { "输入关键字，回车跳转到第一个结果" };
    (model(hits), empty.into())
}
