//! Undo and redo (0.2's src/stores/doc.ts). Each step keeps, per item, the value before and
//! after the write. Writes within 400 ms are one step (deleting a group also rewrites its
//! tasks); a click or key press always starts a new step. An item changed elsewhere since
//! (its current value isn't the step's `next`) is left alone on replay.

use crate::tasks::Edit;
use serde_json::Value;
use std::collections::HashMap;

const BATCH_MS: i64 = 400;
const MAX_STEPS: usize = 50;

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    collection: String,
    id: String,
    prev: Value,
    next: Value,
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Vec<Entry>>,
    redo: Vec<Vec<Entry>>,
    batch: Vec<Entry>,
    batch_until: i64,
}

/// Result of an undo or redo.
#[derive(Debug, Default)]
pub struct Replay {
    /// What to write (without recording it again).
    pub edit: Edit,
    /// Items left as they are because something else changed them meanwhile.
    pub skipped: usize,
    pub done: bool,
}

impl History {
    /// Records an edit before it is written. `current(collection, id)` is the stored value now.
    pub fn record(&mut self, edit: &Edit, current: impl Fn(&str, &str) -> Value, now_ms: i64) {
        if !self.batch.is_empty() && now_ms > self.batch_until {
            self.close_batch();
        }
        // Several writes to one item in a step: each sees the value the previous one left.
        let mut seen: HashMap<(String, String), Value> = HashMap::new();
        for (c, id, v) in &edit.items {
            let key = (c.clone(), id.clone());
            let prev = seen.get(&key).cloned().unwrap_or_else(|| current(c, id));
            let next = v.clone().unwrap_or(Value::Null);
            seen.insert(key, next.clone());
            self.batch.push(Entry { collection: c.clone(), id: id.clone(), prev, next });
        }
        self.redo.clear();
        self.batch_until = now_ms + BATCH_MS;
    }

    /// A click or key press: the next write starts a new step.
    pub fn boundary(&mut self) {
        self.close_batch();
    }

    fn close_batch(&mut self) {
        if !self.batch.is_empty() {
            self.undo.push(std::mem::take(&mut self.batch));
            if self.undo.len() > MAX_STEPS {
                self.undo.remove(0);
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || !self.batch.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Clears everything (after an import or restore).
    pub fn clear(&mut self) {
        *self = History::default();
    }

    pub fn undo(&mut self, current: impl Fn(&str, &str) -> Value) -> Replay {
        self.close_batch();
        let Some(step) = self.undo.pop() else { return Replay::default() };
        let (r, inverse) = replay(step, current);
        if !inverse.is_empty() {
            self.redo.push(inverse);
        }
        r
    }

    pub fn redo(&mut self, current: impl Fn(&str, &str) -> Value) -> Replay {
        self.close_batch();
        let Some(step) = self.redo.pop() else { return Replay::default() };
        let (r, inverse) = replay(step, current);
        if !inverse.is_empty() {
            self.undo.push(inverse);
        }
        r
    }
}

/// Applies `prev` of each entry, newest first. Returns the edit and the inverse step.
fn replay(step: Vec<Entry>, current: impl Fn(&str, &str) -> Value) -> (Replay, Vec<Entry>) {
    let mut out = Replay { done: true, ..Replay::default() };
    let mut inverse = Vec::new();
    let mut now: HashMap<(String, String), Value> = HashMap::new();
    for e in step.into_iter().rev() {
        let key = (e.collection.clone(), e.id.clone());
        let cur = now.get(&key).cloned().unwrap_or_else(|| current(&e.collection, &e.id));
        if cur != e.next {
            out.skipped += 1;
            continue;
        }
        now.insert(key, e.prev.clone());
        let v = (!e.prev.is_null()).then(|| e.prev.clone());
        out.edit.items.push((e.collection.clone(), e.id.clone(), v));
        // In processing order: replaying the inverse (reversed again) runs forward.
        inverse.push(Entry { collection: e.collection, id: e.id, prev: e.next, next: e.prev });
    }
    (out, inverse)
}
