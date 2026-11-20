//! Task and group operations (0.2's src/lib/tasks.ts). Each takes the current document and
//! returns an Edit (the items to write); the app writes it with the store and records it for
//! undo. Times are passed in, so everything here is plain data and testable.

use crate::i18n::{trf};

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
    // made done (the editor's 已完成 box, an assistant): done today
    stamp_done(None, &mut p, &day_of(now_ms));
    p
}

/// The local day of a time.
fn day_of(now_ms: i64) -> String {
    crate::date::local_of_ms(now_ms, crate::date::utc_offset_minutes()).0
}

/// Keeps the day a task was ticked done (GitHub #15) in step with its done state, whoever
/// writes it (the calendar, the Gantt chart, the editor, a reminder, an assistant):
/// - ticked: `today`, unless this very edit names the day (the editor's 完成日期);
/// - unticked: no day, so ticking it again is a new day (a day set by hand goes too);
/// - done before and after: the day as the edit has it (unchanged, or corrected by hand). A
///   task done before 0.3.17 has none, and none is made up for it.
///
/// Repeating tasks keep a day per occurrence, the same way. To-dos without a day have their
/// own record (`flex_done`). `old`: the task as stored (None: a new one).
fn stamp_done(old: Option<&Task>, t: &mut Task, today: &str) {
    let days = t.done_on_dates.take();
    if crate::flex::is_flex(t) {
        t.done_on = None;
        return;
    }
    if t.repeat.is_some() {
        let mut days = days.unwrap_or_default();
        days.retain(|k, _| t.done_dates.iter().flatten().any(|d| d == k));
        // (a new task that comes with times done is a copy: they were done when the task's
        // were, and a day that isn't known is not made up for the copy either)
        if let Some(old) = old {
            let was: BTreeSet<&str> = old.done_dates.iter().flatten().map(String::as_str).collect();
            for d in t.done_dates.iter().flatten().filter(|d| !was.contains(d.as_str())) {
                days.entry(d.clone()).or_insert_with(|| today.to_owned());
            }
        }
        t.done_on_dates = (!days.is_empty()).then_some(days);
    }
    // (a repeating task keeps `done` and its day for the day it stops repeating)
    if !t.done {
        t.done_on = None;
        return;
    }
    let was_done = old.is_some_and(|o| o.done);
    let named = t.done_on.is_some() && old.is_none_or(|o| o.done_on != t.done_on);
    if !was_done && !named {
        t.done_on = Some(today.to_owned());
    }
}

/// The day an occurrence was ticked done, when it is done and the day is known. `date` as in
/// `set_occurrence_done`: the occurrence, or a to-do's slot or the day it was done.
pub fn done_day<'a>(t: &'a Task, date: &str) -> Option<&'a str> {
    if crate::flex::is_flex(t) {
        if crate::flex::is_slot(date) {
            return crate::flex::slot_done(t, date);
        }
        return t.flex_done.iter().flatten().map(|(_, d)| d.as_str()).find(|d| *d == date);
    }
    if t.repeat.is_some() {
        let done = t.done_dates.iter().flatten().any(|d| d == date);
        return t.done_on_dates.as_ref().filter(|_| done)?.get(date).map(String::as_str);
    }
    t.done_on.as_deref().filter(|_| t.done)
}

/// Corrects the day an occurrence was done (右键 › 修改完成日期, GitHub #15: a tick or an untick
/// by mistake). Nothing for one that isn't done, or for a day after today. `date` as in
/// `done_day`.
pub fn set_done_day(doc: &Doc, t: &Task, date: &str, day: &str, now_ms: i64) -> Edit {
    // (nothing was done on a day that has not come)
    if !crate::date::is_iso_date(day) || day > day_of(now_ms).as_str() {
        return Edit::default();
    }
    let mut t = t.clone();
    if crate::flex::is_flex(&t) {
        if crate::flex::is_slot(date) {
            if crate::flex::slot_done(&t, date).is_none() {
                return Edit::default();
            }
            crate::flex::set_slot(&mut t, date, Some(day));
        } else {
            crate::flex::move_day(&mut t, date, day);
        }
    } else if t.repeat.is_some() {
        if !t.done_dates.iter().flatten().any(|d| d == date) {
            return Edit::default();
        }
        t.done_on_dates.get_or_insert_with(Default::default).insert(date.to_owned(), day.to_owned());
    } else {
        if !t.done {
            return Edit::default();
        }
        t.done_on = Some(day.to_owned());
    }
    if doc.tasks.get(&t.id) == Some(&t) {
        return Edit::default();
    }
    save_task(doc, t, now_ms)
}

pub fn save_task(doc: &Doc, mut t: Task, now_ms: i64) -> Edit {
    let old = doc.tasks.get(&t.id);
    // A renamed sample task is the user's own now: "清除示例数据" must not take it.
    if t.is_sample() && old.is_some_and(|old| old.title != t.title) {
        t.sample = None;
    }
    stamp_done(old, &mut t, &day_of(now_ms));
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

/// `date`: the occurrence; for a to-do without a day (flex.rs) its slot (done today), or the
/// day it was done (to untick it there).
pub fn set_occurrence_done(doc: &Doc, t: &Task, date: &str, done: bool, now_ms: i64) -> Edit {
    let mut t = t.clone();
    if crate::flex::is_flex(&t) {
        if crate::flex::is_slot(date) {
            let today = crate::date::today();
            crate::flex::set_slot(&mut t, date, done.then_some(today.as_str()));
        } else if !done {
            crate::flex::clear_day(&mut t, date);
        }
    } else if t.repeat.is_some() {
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
    if crate::flex::is_flex(t) {
        // the day it was done
        let mut t = t.clone();
        crate::flex::move_day(&mut t, &occ.date, start);
        return save_task(doc, t, now_ms);
    }
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
    let mut copy = make_task(doc, copy, Some(t.order), now_ms);
    // done on the day the occurrence was (not known: not made up)
    copy.done_on = done_day(t, &occ.date).map(str::to_owned);
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
    p.title = trf("{0}（副本）", &[format!("{}", t.title)]);
    p.sample = None;
    let mut copy = make_task(doc, p, None, now_ms);
    // a copy of a done task was done when the task was (not known: not made up)
    copy.done_on = t.done_on.clone().filter(|_| copy.done);
    (Edit::task(&copy), copy)
}

/// Moves overdue single-day tasks (late by at most 30 days) to today. Not undoable (0.2 kept
/// it out of the history too).
pub fn auto_postpone(doc: &Doc, now: &str, now_ms: i64) -> Edit {
    let mut e = Edit::default();
    for t in doc.tasks.values() {
        if t.done || t.repeat.is_some() || t.flex.is_some() || t.start != task_end(t) {
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

/// Groups there from the start (GitHub #14: 生日、工作、还款、纪念日), as choices wherever a task's
/// group is chosen. One is made the first time it is chosen, so nobody's Gantt chart fills
/// with empty groups. The name is a translation key: the group gets it in the language it is
/// made in. With its colour.
pub const PRESET_GROUPS: [(&str, &str); 4] = [("生日", "pink"), ("工作", "blue"), ("还款", "orange"), ("纪念日", "purple")];

/// A preset among group ids (a Select's value, a menu entry): its key behind this.
pub const PRESET_PREFIX: &str = "__preset__:";

/// The group that is preset `key` already: one with the preset's name in any of the app's
/// languages. A group is named in the language it was made in and keeps that name, so one
/// made while the calendar was in English (or on another PC, synced here) is 生日 here too,
/// not offered a second time. The first in the groups' order when several have such a name.
fn preset_owner<'a>(doc: &'a Doc, key: &str) -> Option<&'a Group> {
    use crate::i18n::{translate, Language};
    let names = [Language::SimplifiedChinese, Language::TraditionalChinese, Language::English].map(|l| translate(l, key));
    doc.groups.values().filter(|g| names.iter().any(|n| n.eq_ignore_ascii_case(g.name.trim()))).min_by(|a, b| a.order.total_cmp(&b.order).then_with(|| a.id.cmp(&b.id)))
}

/// The presets no group has the name of yet: (key, name).
pub fn open_presets(doc: &Doc) -> Vec<(&'static str, &'static str)> {
    PRESET_GROUPS.iter().filter(|(k, _)| preset_owner(doc, k).is_none()).map(|(k, _)| (*k, crate::i18n::tr(k))).collect()
}

/// The group of preset `key` and what makes it: nothing when a group has that name already.
pub fn preset_group(doc: &Doc, key: &str) -> Option<(String, Edit)> {
    let (k, color) = PRESET_GROUPS.iter().find(|(k, _)| *k == key)?;
    if let Some(g) = preset_owner(doc, k) {
        return Some((g.id.clone(), Edit::default()));
    }
    let g = make_group(doc, crate::i18n::tr(k), color);
    Some((g.id.clone(), save_group(doc, g)))
}

/// 分组 in a task's menu: `group` None is 未分组, a preset (`PRESET_PREFIX` and its key) is made
/// with it.
pub fn set_group(doc: &Doc, t: &Task, group: Option<&str>, now_ms: i64) -> Edit {
    let (group, mut e) = match group.and_then(|g| g.strip_prefix(PRESET_PREFIX)) {
        Some(key) => match preset_group(doc, key) {
            Some((id, e)) => (Some(id), e),
            None => return Edit::default(),
        },
        None => (group.map(str::to_owned), Edit::default()),
    };
    if t.group_id != group {
        e.extend(save_task(doc, Task { group_id: group, ..t.clone() }, now_ms));
    }
    e
}

/// The to-do column's order, set by hand (GitHub #7): the to-do `id` goes in front of
/// `before`, or behind `after` when there is nothing to go in front of (the end of its part).
/// The column lists by `order`: the one moved gets a number between its new neighbours', so
/// one task is written. Two with the same number (a copy has its original's) leave none
/// between them: the ones behind it that have that number are spread out with it. When the
/// numbers between two are used up (some fifty moves into the same gap), all are numbered
/// anew.
///
/// The neighbours are looked up among all to-dos of its kind, also the ones the column
/// doesn't show now (done, or of a group the filter leaves out): those keep their places.
pub fn reorder_flex(doc: &Doc, id: &str, after: Option<&str>, before: Option<&str>, now_ms: i64) -> Edit {
    let mut e = Edit::default();
    let Some(t) = doc.tasks.get(id) else { return e };
    let Some(kind) = crate::flex::kind(t) else { return e };
    let by_order = |a: &Task, b: &Task| a.order.total_cmp(&b.order).then_with(|| a.id.cmp(&b.id));
    let mut all: Vec<&Task> = doc.tasks.values().filter(|o| o.id != id && crate::flex::kind(o) == Some(kind)).collect();
    all.sort_by(|a, b| by_order(a, b));
    let index = |x: &str| all.iter().position(|o| o.id == x);
    let at = match (before.and_then(index), after.and_then(index)) {
        (Some(i), _) => i,
        (None, Some(i)) => i + 1,
        (None, None) => return e,
    };
    // there already
    if all.iter().filter(|o| by_order(o, t).is_lt()).count() == at {
        return e;
    }
    let low = at.checked_sub(1).map(|i| all[i].order);
    // the ones behind it that have the number of the one before it need new ones as well
    let tied = all[at..].iter().take_while(|o| low.is_some_and(|l| o.order <= l)).count();
    let high = all.get(at + tied).map(|o| o.order);
    let (first, step) = match (low, high) {
        (Some(l), Some(h)) => (l + (h - l) / (tied as f64 + 2.0), (h - l) / (tied as f64 + 2.0)),
        (Some(l), None) => (l + 1.0, 1.0),
        (None, Some(h)) => (h - 1.0, 1.0),
        (None, None) => return e,
    };
    let numbers: Vec<f64> = (0..=tied).map(|i| first + step * i as f64).collect();
    let rising = low.is_none_or(|l| l < numbers[0]) && numbers.windows(2).all(|w| w[0] < w[1]) && high.is_none_or(|h| numbers[tied] < h);
    let placed: Vec<(&Task, f64)> = if rising {
        std::iter::once(t).chain(all[at..at + tied].iter().copied()).zip(numbers).collect()
    } else {
        // no number left between two of them: 1, 2, 3… for all (the moved one first: it names
        // the change in 最近更改)
        let number = |i: usize| (i + 1) as f64;
        std::iter::once((t, number(at))).chain(all.iter().enumerate().map(|(i, o)| (*o, number(if i < at { i } else { i + 1 })))).collect()
    };
    for (task, order) in placed {
        if task.order != order {
            e.extend(save_task(doc, Task { order, ..task.clone() }, now_ms));
        }
    }
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

/// The tasks of these groups.
pub fn tasks_in_groups(doc: &Doc, ids: &[String]) -> usize {
    doc.tasks.values().filter(|t| t.group_id.as_ref().is_some_and(|g| ids.contains(g))).count()
}

/// Deletes the groups (one undo step). With `with_tasks` their tasks are deleted too (other
/// tasks drop them as predecessors); otherwise the tasks become ungrouped. Ids that are not
/// groups (deleted meanwhile, e.g. by sync) are skipped.
pub fn delete_groups(doc: &Doc, ids: &[String], with_tasks: bool, now_ms: i64) -> Edit {
    let gone: BTreeSet<&str> = ids.iter().map(String::as_str).filter(|id| doc.groups.contains_key(*id)).collect();
    let in_gone = |t: &Task| t.group_id.as_deref().is_some_and(|g| gone.contains(g));
    let mut e = Edit::default();
    if with_tasks {
        let dropped: BTreeSet<&str> = doc.tasks.values().filter(|t| in_gone(t)).map(|t| t.id.as_str()).collect();
        for id in &dropped {
            e.delete("tasks", id);
        }
        for t in doc.tasks.values().filter(|t| !in_gone(t) && t.deps().iter().any(|d| dropped.contains(d.as_str()))) {
            let mut t = t.clone();
            t.deps = Some(t.deps().iter().filter(|d| !dropped.contains(d.as_str())).cloned().collect());
            t.updated_at = now_ms;
            e.put_task(&t);
        }
    } else {
        for t in doc.tasks.values().filter(|t| in_gone(t)) {
            let mut t = t.clone();
            t.group_id = None;
            t.updated_at = now_ms;
            e.put_task(&t);
        }
    }
    for id in gone {
        e.delete("groups", id);
    }
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
