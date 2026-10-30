//! Task search (0.2's src/lib/search.ts).

use crate::date::{add_months, diff_days};
use crate::model::{Group, Task};
use crate::recur::{occurrences, task_end};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub q: String,
    /// "all" | "1m" | "3m" | "1y"
    pub range: String,
    /// "all" | "done" | "undone"
    pub status: String,
    /// Colour key/hex; "" = any.
    pub color: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub task: Task,
    /// Date to jump to: the task start, or for a repeating task its occurrence nearest today.
    pub date: String,
}

/// Occurrence of `t` closest to `now` within [lo, hi].
fn nearest(t: &Task, now: &str, lo: &str, hi: &str) -> Option<String> {
    if t.repeat.is_none() {
        return (task_end(t) >= lo && t.start.as_str() <= hi).then(|| t.start.clone());
    }
    let mut best: Option<String> = None;
    for o in occurrences(t, lo, hi, 1000) {
        if best.as_ref().is_none_or(|b| diff_days(now, &o.start).abs() < diff_days(now, b).abs()) {
            best = Some(o.start.clone());
        }
        if o.start.as_str() >= now {
            break; // occurrences are ordered; later ones are farther away
        }
    }
    best
}

/// Tasks matching the query (title, notes, group name), nearest to today first.
pub fn search_tasks(tasks: &[Task], groups: &BTreeMap<String, Group>, o: &SearchOptions, now: &str) -> Vec<SearchHit> {
    let q = o.q.trim().to_lowercase();
    let months = match o.range.as_str() {
        "1m" => 1,
        "3m" => 3,
        "1y" => 12,
        _ => 0,
    };
    // "All time" still needs a window for repeating tasks: ten years around today.
    let span = if months == 0 { 120 } else { months };
    let (lo, hi) = (add_months(now, -span), add_months(now, span));
    let mut hits = Vec::new();
    for t in tasks {
        if !q.is_empty() {
            let group = t.group_id.as_ref().and_then(|g| groups.get(g)).map_or("", |g| g.name.as_str());
            let hay = format!("{}\n{}\n{}", t.title, t.notes.as_deref().unwrap_or(""), group).to_lowercase();
            if !q.split_whitespace().all(|w| hay.contains(w)) {
                continue;
            }
        }
        match o.status.as_str() {
            "done" if !t.done => continue,
            "undone" if t.done => continue,
            _ => {}
        }
        if !o.color.is_empty() && t.color.as_deref().unwrap_or("") != o.color {
            continue;
        }
        let date = if months != 0 || t.repeat.is_some() { nearest(t, now, &lo, &hi) } else { Some(t.start.clone()) };
        if let Some(date) = date {
            hits.push(SearchHit { task: t.clone(), date });
        }
    }
    hits.sort_by(|a, b| {
        diff_days(now, &a.date).abs().cmp(&diff_days(now, &b.date).abs()).then(a.task.order.total_cmp(&b.task.order))
    });
    hits
}
