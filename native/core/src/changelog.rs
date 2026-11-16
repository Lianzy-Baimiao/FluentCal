//! 最近更改 (GitHub #11): the last changes to tasks, groups and day colours, each of which can
//! be taken back later, also after the toast with 撤销 is gone or the app was restarted. Kept
//! in this PC's folder (changes.json), read when first needed. A change is taken back item by
//! item, as undo does: an item changed again since is left alone.

use crate::i18n::{tr, trf};
use crate::tasks::Edit;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Changes kept.
pub const MAX_CHANGES: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub collection: String,
    pub id: String,
    /// Null = it didn't exist.
    pub prev: Value,
    /// Null = deleted.
    pub next: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub id: u64,
    /// Epoch ms.
    pub at: i64,
    /// "add" | "delete" | "edit"
    pub kind: String,
    /// What was changed: the task's title, "分组 工作", "日期颜色 2026-11-14".
    pub what: String,
    /// What was done to it, for "edit": "done" | "undone" | "moved" | "progress" | "".
    #[serde(default)]
    pub detail: String,
    /// Other items changed with it.
    #[serde(default)]
    pub more: usize,
    pub items: Vec<Item>,
    #[serde(default)]
    pub undone: bool,
}

impl Change {
    /// "删除“周报”", "完成“周报”等 3 项".
    pub fn text(&self) -> String {
        let what = self.what.clone();
        let s = match (self.kind.as_str(), self.detail.as_str()) {
            ("add", _) => trf("添加“{0}”", &[what]),
            ("delete", _) => trf("删除“{0}”", &[what]),
            (_, "done") => trf("完成“{0}”", &[what]),
            (_, "undone") => trf("取消完成“{0}”", &[what]),
            (_, "moved") => trf("改期“{0}”", &[what]),
            (_, "progress") => trf("改进度“{0}”", &[what]),
            _ => trf("修改“{0}”", &[what]),
        };
        if self.more > 0 { trf("{0}等 {1} 项", &[s, (self.more + 1).to_string()]) } else { s }
    }
}

#[derive(Debug, Default)]
pub struct ChangeLog {
    path: Option<PathBuf>,
    loaded: bool,
    changes: Vec<Change>,
    next_id: u64,
}

/// Result of taking a change back (or doing it again).
#[derive(Debug, Default)]
pub struct Reverted {
    pub edit: Edit,
    pub skipped: usize,
}

fn title_of(collection: &str, id: &str, v: &Value) -> String {
    let name = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    match collection {
        "groups" => trf("分组 {0}", &[name("name")]),
        "dayMarks" => trf("日期颜色 {0}", &[id.to_owned()]),
        _ => name("title"),
    }
}

fn detail_of(prev: &Value, next: &Value) -> &'static str {
    let differs = |k: &str| prev.get(k) != next.get(k);
    if differs("done") || differs("doneDates") {
        let was = prev.get("done").and_then(Value::as_bool).unwrap_or(false);
        let is = next.get("done").and_then(Value::as_bool).unwrap_or(false);
        let n = |v: &Value| v.get("doneDates").and_then(Value::as_array).map_or(0, Vec::len);
        return if (is && !was) || n(next) > n(prev) { "done" } else { "undone" };
    }
    if differs("start") || differs("end") {
        return "moved";
    }
    if differs("progress") {
        return "progress";
    }
    ""
}

impl ChangeLog {
    /// A log kept in `path` (None: in memory only).
    pub fn new(path: Option<PathBuf>) -> ChangeLog {
        ChangeLog { path, ..Default::default() }
    }

    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let read = self.path.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice::<Vec<Change>>(&b).ok());
        self.changes = read.unwrap_or_default();
        self.next_id = self.changes.iter().map(|c| c.id).max().unwrap_or(0) + 1;
    }

    fn save(&self) {
        if let Some(p) = &self.path {
            if let Ok(b) = serde_json::to_vec(&self.changes) {
                let _ = std::fs::write(p, b);
            }
        }
    }

    /// Records an edit before it is written. `current(collection, id)` is the stored value now.
    /// The first item names the change (deleting a group: the group, not its tasks, which
    /// come first in the edit, so groups go before tasks).
    pub fn record(&mut self, edit: &Edit, current: impl Fn(&str, &str) -> Value, now_ms: i64) {
        self.load();
        let mut seen: HashMap<(String, String), usize> = HashMap::new();
        let mut items: Vec<Item> = Vec::new();
        for (c, id, v) in &edit.items {
            let next = v.clone().unwrap_or(Value::Null);
            match seen.get(&(c.clone(), id.clone())) {
                Some(&i) => items[i].next = next,
                None => {
                    seen.insert((c.clone(), id.clone()), items.len());
                    items.push(Item { collection: c.clone(), id: id.clone(), prev: current(c, id), next });
                }
            }
        }
        items.retain(|i| i.prev != i.next);
        let Some(main) = items.iter().find(|i| i.collection == "groups" && (i.prev.is_null() || i.next.is_null())).or(items.first()) else { return };
        let kind = if main.prev.is_null() { "add" } else if main.next.is_null() { "delete" } else { "edit" };
        let shown = if main.next.is_null() { &main.prev } else { &main.next };
        let change = Change {
            id: self.next_id,
            at: now_ms,
            kind: kind.into(),
            what: title_of(&main.collection, &main.id, shown),
            detail: if kind == "edit" && main.collection == "tasks" { detail_of(&main.prev, &main.next).into() } else { String::new() },
            more: items.len() - 1,
            items,
            undone: false,
        };
        self.next_id += 1;
        self.changes.push(change);
        if self.changes.len() > MAX_CHANGES {
            let cut = self.changes.len() - MAX_CHANGES;
            self.changes.drain(..cut);
        }
        self.save();
    }

    /// The changes, newest first.
    pub fn list(&mut self) -> Vec<Change> {
        self.load();
        self.changes.iter().rev().cloned().collect()
    }

    /// Takes change `id` back, or does it again when it was taken back. Items changed since
    /// are skipped; with nothing left to write the change stays as it is.
    pub fn toggle(&mut self, id: u64, current: impl Fn(&str, &str) -> Value) -> Option<Reverted> {
        self.load();
        let c = self.changes.iter_mut().find(|c| c.id == id)?;
        let mut out = Reverted::default();
        for i in c.items.iter().rev() {
            let (from, to) = if c.undone { (&i.prev, &i.next) } else { (&i.next, &i.prev) };
            if current(&i.collection, &i.id) != *from {
                out.skipped += 1;
                continue;
            }
            out.edit.items.push((i.collection.clone(), i.id.clone(), (!to.is_null()).then(|| to.clone())));
        }
        if !out.edit.is_empty() {
            c.undone = !c.undone;
            self.save();
        }
        Some(out)
    }

    /// 清空记录.
    pub fn clear(&mut self) {
        self.loaded = true;
        self.changes.clear();
        self.save();
    }
}

/// "刚刚", "5 分钟前", "今天 14:05", "昨天 09:30", "11-14 09:30".
pub fn when_text(at: i64, now_ms: i64, offset_min: i32) -> String {
    let mins = (now_ms - at) / 60_000;
    if mins < 1 {
        return tr("刚刚").into();
    }
    if mins < 60 {
        return trf("{0} 分钟前", &[mins.to_string()]);
    }
    let (day, m) = crate::date::local_of_ms(at, offset_min);
    let today = crate::date::local_of_ms(now_ms, offset_min).0;
    let time = format!("{:02}:{:02}", m / 60, m % 60);
    match crate::date::diff_days(&day, &today) {
        0 => trf("今天 {0}", &[time]),
        1 => trf("昨天 {0}", &[time]),
        _ => format!("{} {time}", &day[5..]),
    }
}
