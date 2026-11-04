//! Gantt chart geometry (0.2's src/lib/gantt.ts): rows, group summaries, the visible range,
//! date ↔ pixel mapping, the two header tiers and dependency arrows.

use crate::i18n::{tr, trf};

use crate::date::{add_days, add_months, day_num, diff_days, end_of_month, iso_week, parts, start_of_month, start_of_week, weekday};
use crate::model::{Group, Task};
use crate::recur::task_end;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Pixels per day for each zoom level ("day" | "week" | "month").
pub fn day_width(scale: &str) -> f64 {
    match scale {
        "week" => 12.0,
        "month" => 4.0,
        _ => 32.0,
    }
}
pub const ROW_HEIGHT: f64 = 32.0;
pub const UNGROUPED: &str = "__ungrouped__";

pub fn ungrouped_group() -> Group {
    Group { id: UNGROUPED.into(), name: tr("未分组").into(), color: "gray".into(), order: 1e9, ..Default::default() }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Group { group: Group, tasks: Vec<Task>, start: Option<String>, end: Option<String>, progress: f64 },
    Task { task: Task, group_id: String },
    Add { group_id: String },
}

impl Row {
    pub fn key(&self) -> String {
        match self {
            Row::Group { group, .. } => format!("g:{}", group.id),
            Row::Task { task, .. } => format!("t:{}", task.id),
            Row::Add { group_id } => format!("a:{group_id}"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RowOptions {
    pub hide_done: bool,
    pub hide_single_day: bool,
    pub group_filter: Vec<String>,
}

/// True when a task belongs in the Gantt chart under the given options.
pub fn in_gantt(t: &Task, o: &RowOptions) -> bool {
    if o.hide_done && t.done && t.repeat.is_none() {
        return false;
    }
    !(o.hide_single_day && t.group_id.is_none() && t.start == task_end(t) && !t.is_milestone() && t.deps().is_empty())
}

/// Group rows (with summary), their task rows and an "add task" row each.
pub fn build_rows(tasks: &[Task], groups: &[Group], o: &RowOptions) -> Vec<Row> {
    let known: HashSet<&str> = groups.iter().map(|g| g.id.as_str()).collect();
    let mut by_group: HashMap<String, Vec<Task>> = HashMap::new();
    for t in tasks.iter().filter(|t| in_gantt(t, o)) {
        let gid = t.group_id.as_deref().filter(|g| known.contains(g)).unwrap_or(UNGROUPED);
        by_group.entry(gid.to_owned()).or_default().push(t.clone());
    }
    let mut ordered: Vec<Group> = groups.to_vec();
    ordered.sort_by(|a, b| a.order.total_cmp(&b.order));
    if by_group.contains_key(UNGROUPED) {
        ordered.push(ungrouped_group());
    }
    let mut rows = Vec::new();
    for g in ordered {
        if !o.group_filter.is_empty() && !o.group_filter.contains(&g.id) {
            continue;
        }
        let mut list = by_group.remove(&g.id).unwrap_or_default();
        list.sort_by(|a, b| a.start.cmp(&b.start).then(a.order.total_cmp(&b.order)));
        let (start, end, progress) = summarize(&list);
        let collapsed = g.collapsed == Some(true);
        let id = g.id.clone();
        rows.push(Row::Group { group: g, tasks: list.clone(), start, end, progress });
        if collapsed {
            continue;
        }
        for t in list {
            rows.push(Row::Task { task: t, group_id: id.clone() });
        }
        rows.push(Row::Add { group_id: id });
    }
    rows
}

/// Time span and duration-weighted progress of a set of tasks.
pub fn summarize(list: &[Task]) -> (Option<String>, Option<String>, f64) {
    let Some(first) = list.first() else { return (None, None, 0.0) };
    let (mut start, mut end) = (first.start.clone(), task_end(first).to_owned());
    let (mut weight, mut acc) = (0.0, 0.0);
    for t in list {
        if t.start < start {
            start = t.start.clone();
        }
        if task_end(t) > end.as_str() {
            end = task_end(t).to_owned();
        }
        let w = (diff_days(&t.start, task_end(t)) + 1) as f64;
        weight += w;
        acc += w * if t.done { 100.0 } else { t.progress };
    }
    (Some(start), Some(end), if weight > 0.0 { (acc / weight).round() } else { 0.0 })
}

/// Visible date range: covers all tasks and today with margins, aligned to whole months.
pub fn chart_range(tasks: &[Task], now: &str, scale: &str) -> (String, String) {
    let pad = match scale {
        "day" => 2,
        "week" => 4,
        _ => 12,
    };
    let (mut s, mut e) = (add_months(now, -1), add_months(now, pad));
    for t in tasks {
        if t.start < s {
            s = t.start.clone();
        }
        if task_end(t) > e.as_str() {
            e = task_end(t).to_owned();
        }
    }
    (start_of_month(&add_days(&s, -7)), end_of_month(&add_months(&e, 1)))
}

pub fn x_of(date: &str, range_start: &str, day_width: f64) -> f64 {
    diff_days(range_start, date) as f64 * day_width
}

/// Inverse of x_of, rounded down to the day.
pub fn date_at(x: f64, range_start: &str, day_width: f64) -> String {
    add_days(range_start, (x / day_width).floor() as i32)
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeaderCell {
    pub key: String,
    pub label: String,
    pub x: f64,
    pub w: f64,
    /// Day cells: weekend/holiday styling is applied by the view.
    pub date: Option<String>,
    pub today: bool,
}

/// Two header rows: top (months or years) and bottom (days, weeks or months).
pub fn header_tiers(range: (&str, &str), scale: &str, now: &str, week_start: u32) -> (Vec<HeaderCell>, Vec<HeaderCell>) {
    let dw = day_width(scale);
    let (start, end) = range;
    let total = (diff_days(start, end) + 1) as f64;
    let cell = |key: String, label: String, a: &str, b: &str, today: bool| {
        let x = x_of(a, start, dw).max(0.0);
        let w = (total * dw).min(x_of(&add_days(b, 1), start, dw)) - x;
        HeaderCell { key, label, x, w, date: None, today }
    };
    let mut top = Vec::new();
    let mut bottom = Vec::new();
    if scale == "month" {
        for y in parts(start).0..=parts(end).0 {
            top.push(cell(format!("y{y}"), trf("{0}年", &[format!("{}", y)]), &format!("{y:04}-01-01"), &format!("{y:04}-12-31"), false));
        }
    } else {
        let mut m = start_of_month(start);
        while m.as_str() <= end {
            let (y, mm, _) = parts(&m);
            top.push(cell(format!("m{m}"), trf("{0}年{1}月", &[format!("{}", y), format!("{}", mm)]), &m, &end_of_month(&m), false));
            m = add_months(&m, 1);
        }
    }
    match scale {
        "day" => {
            for i in 0..total as i32 {
                let d = add_days(start, i);
                bottom.push(HeaderCell { key: d.clone(), label: parts(&d).2.to_string(), x: i as f64 * dw, w: dw, today: d == now, date: Some(d) });
            }
        }
        "week" => {
            let mut w = start_of_week(start, week_start);
            while w.as_str() <= end {
                let (_, m, d) = parts(&w);
                let contains = day_num(now) >= day_num(&w) && day_num(now) < day_num(&w) + 7;
                bottom.push(cell(format!("w{w}"), format!("{m}/{d}"), &w, &add_days(&w, 6), contains));
                w = add_days(&w, 7);
            }
        }
        _ => {
            let mut m = start_of_month(start);
            while m.as_str() <= end {
                let mm = parts(&m).1;
                bottom.push(cell(format!("b{m}"), trf("{0}月", &[format!("{}", mm)]), &m, &end_of_month(&m), start_of_month(now) == m));
                m = add_months(&m, 1);
            }
        }
    }
    (top, bottom)
}

/// Label for the week header tooltip: "第 44 周".
pub fn week_label(iso: &str) -> String {
    trf("第 {0} 周", &[format!("{}", iso_week(iso))])
}

/// The corners of a finish-to-start dependency arrow between two bars, first to last.
pub fn dep_points(from_x: f64, from_y: f64, to_x: f64, to_y: f64) -> Vec<(f64, f64)> {
    let gap = 8.0;
    if to_x - from_x >= gap * 2.0 {
        let mid = from_x + gap;
        return vec![(from_x, from_y), (mid, from_y), (mid, to_y), (to_x, to_y)];
    }
    // Successor starts before the predecessor ends: route around below the source bar.
    let down = from_y + if to_y > from_y { 1.0 } else { -1.0 } * (ROW_HEIGHT / 2.0);
    vec![(from_x, from_y), (from_x + gap, from_y), (from_x + gap, down), (to_x - gap, down), (to_x - gap, to_y), (to_x, to_y)]
}

/// SVG path for a finish-to-start dependency arrow between two bars.
pub fn dep_path(from_x: f64, from_y: f64, to_x: f64, to_y: f64) -> String {
    let p = dep_points(from_x, from_y, to_x, to_y);
    let mut out = format!("M{},{}", p[0].0, p[0].1);
    for w in p.windows(2) {
        out += &if w[0].1 == w[1].1 { format!(" H{}", w[1].0) } else { format!(" V{}", w[1].1) };
    }
    out
}

/// True when `to` already (transitively) depends on `from`, i.e. adding from → to would loop.
pub fn would_cycle(tasks: &BTreeMap<String, Task>, from: &str, to: &str) -> bool {
    let mut seen = HashSet::new();
    let mut stack = vec![from.to_owned()];
    while let Some(id) = stack.pop() {
        if id == to {
            return true;
        }
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(t) = tasks.get(&id) {
            stack.extend(t.deps().iter().cloned());
        }
    }
    false
}

/// Whether `to` may get `from` as a new predecessor.
pub fn can_link(tasks: &BTreeMap<String, Task>, from: &str, to: &str) -> bool {
    let (Some(a), Some(b)) = (tasks.get(from), tasks.get(to)) else { return false };
    if from == to || a.repeat.is_some() || b.repeat.is_some() || b.deps().iter().any(|d| d == from) {
        return false;
    }
    !would_cycle(tasks, from, to)
}

pub fn is_weekend_col(iso: &str, rest_days: &[u32]) -> bool {
    rest_days.contains(&weekday(iso))
}
