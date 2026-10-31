//! Task and group operations (0.2's src/lib/tasks.ts). Each takes the current document and
//! returns an Edit (the items to write); the app writes it with the store and records it for
//! undo. Times are passed in, so everything here is plain data and testable.

use crate::date::{add_days, diff_days};
use crate::model::{self, Group, Task};
use crate::recur::{task_end, Occurrence};
use crate::settings::Settings;
use crate::store::{Store, StoreEvent};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// A typed copy of the document for the UI and the operations here.
#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub tasks: BTreeMap<String, Task>,
    pub groups: BTreeMap<String, Group>,
    /// Day colours: date → colour key.
    pub day_marks: BTreeMap<String, String>,
    pub settings: Settings,
}

impl Doc {
    pub fn from_value(v: &Value) -> Doc {
        Doc {
            tasks: model::tasks(v).into_iter().map(|t| (t.id.clone(), t)).collect(),
            groups: model::groups(v).into_iter().map(|g| (g.id.clone(), g)).collect(),
            day_marks: model::day_marks(v),
            settings: Settings::of_doc(v),
        }
    }

    /// Follows a store change without re-reading everything. Returns false for a Reload (the
    /// caller re-reads the snapshot).
    pub fn apply(&mut self, e: &StoreEvent) -> bool {
        match e {
            StoreEvent::Changed { collection, items, .. } => {
                for (id, v) in items {
                    match collection.as_str() {
                        "tasks" => match serde_json::from_value::<Task>(v.clone()) {
                            Ok(t) if !v.is_null() => drop(self.tasks.insert(id.clone(), t)),
                            _ => drop(self.tasks.remove(id)),
                        },
                        "groups" => match serde_json::from_value::<Group>(v.clone()) {
                            Ok(g) if !v.is_null() => drop(self.groups.insert(id.clone(), g)),
                            _ => drop(self.groups.remove(id)),
                        },
                        "dayMarks" => match v.get("color").and_then(Value::as_str) {
                            Some(c) => drop(self.day_marks.insert(id.clone(), c.to_owned())),
                            None => drop(self.day_marks.remove(id)),
                        },
                        _ => {}
                    }
                }
                true
            }
            StoreEvent::Settings { new, .. } => {
                self.settings = Settings::from_value(new);
                true
            }
            StoreEvent::Reload { .. } => false,
            StoreEvent::SaveStatus(_) => true,
        }
    }

    /// The JSON currently stored for an item (Null = none), for undo.
    pub fn raw(&self, collection: &str, id: &str) -> Value {
        match collection {
            "tasks" => self.tasks.get(id).map_or(Value::Null, Task::to_value),
            "groups" => self.groups.get(id).map_or(Value::Null, Group::to_value),
            "dayMarks" => self.day_marks.get(id).map_or(Value::Null, |c| serde_json::json!({ "color": c })),
            _ => Value::Null,
        }
    }
}

/// Items to write: (collection, id, new value; None = delete), in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Edit {
    pub items: Vec<(String, String, Option<Value>)>,
}

impl Edit {
    pub fn task(t: &Task) -> Edit {
        let mut e = Edit::default();
        e.put_task(t);
        e
    }
    pub fn put_task(&mut self, t: &Task) {
        self.items.push(("tasks".into(), t.id.clone(), Some(t.to_value())));
    }
    pub fn delete(&mut self, collection: &str, id: &str) {
        self.items.push((collection.into(), id.into(), None));
    }
    pub fn put(&mut self, collection: &str, id: &str, v: Value) {
        self.items.push((collection.into(), id.into(), Some(v)));
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn extend(&mut self, other: Edit) {
        self.items.extend(other.items);
    }

    /// Writes the edit, one store write per collection (in the order they first appear).
    pub fn write(&self, store: &Store, origin: &str) -> Result<(), String> {
        let mut by: Vec<(String, Map<String, Value>)> = Vec::new();
        for (c, id, v) in &self.items {
            let i = match by.iter().position(|(n, _)| n == c) {
                Some(i) => i,
                None => {
                    by.push((c.clone(), Map::new()));
                    by.len() - 1
                }
            };
            by[i].1.insert(id.clone(), v.clone().unwrap_or(Value::Null));
        }
        for (c, m) in by {
            store.write(origin, &c, m)?;
        }
        Ok(())
    }
}

/// A random version 4 UUID, like crypto.randomUUID() in 0.2.
pub fn uid() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("OS random generator");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

fn next_order(doc: &Doc) -> f64 {
    doc.tasks.values().map(|t| t.order).fold(0.0, f64::max) + 1.0
}

/// New task with defaults (default colour and reminders from the settings when `p` has none);
/// gets a new id, `order` (or the next free one) and timestamps.
pub fn make_task(doc: &Doc, mut p: Task, order: Option<f64>, now_ms: i64) -> Task {
    if p.end.is_empty() {
        p.end = p.start.clone();
    }
    if p.color.is_none() && !doc.settings.tasks.default_color.is_empty() {
        p.color = Some(doc.settings.tasks.default_color.clone());
    }
    if p.reminders.is_none() && !doc.settings.reminders.default_offsets.is_empty() {
        p.reminders = Some(doc.settings.reminders.default_offsets.clone());
    }
    p.id = uid();
    p.order = order.unwrap_or_else(|| next_order(doc));
    p.created_at = now_ms;
    p.updated_at = now_ms;
    p
}

pub fn save_task(doc: &Doc, mut t: Task, now_ms: i64) -> Edit {
    // A renamed sample task is the user's own now: "清除示例数据" must not take it.
    if t.is_sample() && doc.tasks.get(&t.id).is_some_and(|old| old.title != t.title) {
        t.sample = None;
    }
    t.updated_at = now_ms;
    Edit::task(&t)
}

pub fn add_task(doc: &Doc, p: Task, order: Option<f64>, now_ms: i64) -> (Edit, Task) {
    let t = make_task(doc, p, order, now_ms);
    (Edit::task(&t), t)
}

/// Deletes a task and removes it from other tasks' predecessors.
pub fn delete_task(doc: &Doc, id: &str, now_ms: i64) -> Edit {
    let mut e = Edit::default();
    e.delete("tasks", id);
    for t in doc.tasks.values().filter(|t| t.deps().iter().any(|d| d == id)) {
        let mut t = t.clone();
        t.deps = Some(t.deps().iter().filter(|d| *d != id).cloned().collect());
        t.updated_at = now_ms;
        e.put_task(&t);
    }
    e
}

pub fn set_occurrence_done(doc: &Doc, t: &Task, date: &str, done: bool, now_ms: i64) -> Edit {
    let mut t = t.clone();
    if t.repeat.is_some() {
        let mut set: BTreeSet<String> = t.done_dates.take().unwrap_or_default().into_iter().collect();
        if done {
            set.insert(date.to_owned());
        } else {
            set.remove(date);
        }
        t.done_dates = Some(set.into_iter().collect());
    } else {
        t.progress = if done { 100.0 } else if t.progress == 100.0 { 0.0 } else { t.progress };
        t.done = done;
    }
    save_task(doc, t, now_ms)
}

pub fn set_progress(doc: &Doc, t: &Task, progress: f64, now_ms: i64) -> Edit {
    let p = progress.round().clamp(0.0, 100.0);
    let mut t = t.clone();
    t.done = if t.repeat.is_some() { t.done } else { p >= 100.0 };
    t.progress = p;
    save_task(doc, t, now_ms)
}

pub fn set_color(doc: &Doc, t: &Task, color: &str, now_ms: i64) -> Edit {
    let mut t = t.clone();
    t.color = (!color.is_empty()).then(|| color.to_owned());
    save_task(doc, t, now_ms)
}

/// Moves a task (keeping its duration) so it starts on `start`.
pub fn move_task(doc: &Doc, t: &Task, start: &str, now_ms: i64) -> Edit {
    let mut t = t.clone();
    let len = diff_days(&t.start, task_end(&t));
    t.end = add_days(start, len);
    t.start = start.to_owned();
    save_task(doc, t, now_ms)
}

/// Moves one occurrence. For a repeating task the occurrence is detached: it is excluded from
/// the series and a standalone copy is created on the new date.
pub fn move_occurrence(doc: &Doc, occ: &Occurrence, start: &str, now_ms: i64) -> Edit {
    let t = occ.task;
    if t.repeat.is_none() {
        return move_task(doc, t, start, now_ms);
    }
    if start == occ.start {
        return Edit::default();
    }
    let mut copy = t.clone();
    copy.repeat = None;
    copy.done_dates = None;
    copy.ex_dates = None;
    copy.start = start.to_owned();
    copy.end = add_days(start, diff_days(&occ.start, &occ.end));
    copy.done = occ.done;
    copy.progress = if occ.done { 100.0 } else { 0.0 };
    let copy = make_task(doc, copy, Some(t.order), now_ms);
    let mut series = t.clone();
    let mut ex: BTreeSet<String> = series.ex_dates.take().unwrap_or_default().into_iter().collect();
    ex.insert(occ.date.clone());
    series.ex_dates = Some(ex.into_iter().collect());
    series.updated_at = now_ms;
    let mut e = Edit::task(&series);
    e.put_task(&copy);
    e
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    This,
    Future,
    All,
}

pub fn delete_occurrence(doc: &Doc, t: &Task, date: &str, mode: DeleteMode, now_ms: i64) -> Edit {
    let Some(rule) = &t.repeat else { return delete_task(doc, &t.id, now_ms) };
    if mode == DeleteMode::All || (mode == DeleteMode::Future && date <= t.start.as_str()) {
        return delete_task(doc, &t.id, now_ms);
    }
    let mut t = t.clone();
    if mode == DeleteMode::This {
        let mut ex: BTreeSet<String> = t.ex_dates.take().unwrap_or_default().into_iter().collect();
        ex.insert(date.to_owned());
        t.ex_dates = Some(ex.into_iter().collect());
    } else {
        let mut r = rule.clone();
        r.count = None;
        r.until = Some(add_days(date, -1));
        t.repeat = Some(r);
    }
    save_task(doc, t, now_ms)
}

pub fn duplicate_task(doc: &Doc, t: &Task, now_ms: i64) -> (Edit, Task) {
    let mut p = t.clone();
    p.title = format!("{}（副本）", t.title);
    p.sample = None;
    add_task(doc, p, None, now_ms)
}

/// Moves overdue single-day tasks (late by at most 30 days) to today. Not undoable (0.2 kept
/// it out of the history too).
pub fn auto_postpone(doc: &Doc, now: &str, now_ms: i64) -> Edit {
    let mut e = Edit::default();
    for t in doc.tasks.values() {
        if t.done || t.repeat.is_some() || t.start != task_end(t) {
            continue;
        }
        if !t.auto_postpone.unwrap_or(doc.settings.tasks.auto_postpone) {
            continue;
        }
        let late = diff_days(task_end(t), now);
        if late <= 0 || late > 30 {
            continue;
        }
        let mut t = t.clone();
        t.start = now.to_owned();
        t.end = now.to_owned();
        t.updated_at = now_ms;
        e.put_task(&t);
    }
    e
}

pub fn make_group(doc: &Doc, name: &str, color: &str) -> Group {
    let order = doc.groups.values().map(|g| g.order).fold(0.0, f64::max) + 1.0;
    Group { id: uid(), name: name.into(), color: color.into(), order, ..Default::default() }
}

pub fn save_group(doc: &Doc, mut g: Group) -> Edit {
    if g.sample == Some(true) && doc.groups.get(&g.id).is_some_and(|old| old.name != g.name) {
        g.sample = None;
    }
    let mut e = Edit::default();
    e.put("groups", &g.id.clone(), g.to_value());
    e
}

/// First-run sample tasks and groups still there.
pub fn sample_count(doc: &Doc) -> usize {
    doc.tasks.values().filter(|t| t.is_sample()).count() + doc.groups.values().filter(|g| g.sample == Some(true)).count()
}

/// "清除示例数据": removes the sample tasks and groups (one undo step). The user's own tasks in a
/// sample group become ungrouped; predecessors pointing at sample tasks are dropped.
pub fn clear_sample_data(doc: &Doc, now_ms: i64) -> (Edit, usize) {
    let gone: HashSet<&str> = doc.tasks.values().filter(|t| t.is_sample()).map(|t| t.id.as_str()).collect();
    let groups: HashSet<&str> = doc.groups.values().filter(|g| g.sample == Some(true)).map(|g| g.id.as_str()).collect();
    let mut e = Edit::default();
    for id in doc.tasks.keys().filter(|id| gone.contains(id.as_str())) {
        e.delete("tasks", id);
    }
    for t in doc.tasks.values().filter(|t| !gone.contains(t.id.as_str())) {
        let ungroup = t.group_id.as_deref().is_some_and(|g| groups.contains(g));
        let deps = t.deps().iter().any(|d| gone.contains(d.as_str()));
        if ungroup || deps {
            let mut t = t.clone();
            if ungroup {
                t.group_id = None;
            }
            if deps {
                t.deps = Some(t.deps().iter().filter(|d| !gone.contains(d.as_str())).cloned().collect());
            }
            t.updated_at = now_ms;
            e.put_task(&t);
        }
    }
    for id in doc.groups.keys().filter(|id| groups.contains(id.as_str())) {
        e.delete("groups", id);
    }
    (e, gone.len())
}

/// Deletes a group; its tasks become ungrouped.
pub fn delete_group(doc: &Doc, id: &str, now_ms: i64) -> Edit {
    let mut e = Edit::default();
    for t in doc.tasks.values().filter(|t| t.group_id.as_deref() == Some(id)) {
        let mut t = t.clone();
        t.group_id = None;
        t.updated_at = now_ms;
        e.put_task(&t);
    }
    e.delete("groups", id);
    e
}

pub fn set_day_mark(date: &str, color: Option<&str>) -> Edit {
    let mut e = Edit::default();
    match color {
        Some(c) => e.put("dayMarks", date, serde_json::json!({ "color": c })),
        None => e.delete("dayMarks", date),
    }
    e
}

/// Three-way merge of an edited task (0.2's lib/merge.ts mergeEdit): fields the user changed
/// (`draft` vs `base`, the task as the editor opened it) win; every other field keeps the
/// current stored value, so edits made elsewhere meanwhile (ticked done, dragged in the
/// Gantt chart) survive.
pub fn merge_edit(base: &Task, draft: &Task, current: &Task) -> Task {
    let (b, d, c) = (base.to_value(), draft.to_value(), current.to_value());
    let empty = Map::new();
    let (bm, dm) = (b.as_object().unwrap_or(&empty), d.as_object().unwrap_or(&empty));
    let mut out = c.as_object().cloned().unwrap_or_default();
    let keys: BTreeSet<&String> = bm.keys().chain(dm.keys()).collect();
    for k in keys {
        let (dv, bv) = (dm.get(k).unwrap_or(&Value::Null), bm.get(k).unwrap_or(&Value::Null));
        if dv != bv {
            if dv.is_null() {
                out.remove(k);
            } else {
                out.insert(k.clone(), dv.clone());
            }
        }
    }
    let mut t: Task = serde_json::from_value(Value::Object(out)).unwrap_or_else(|_| draft.clone());
    if t.is_milestone() || t.end < t.start {
        t.end = t.start.clone();
    }
    t
}
