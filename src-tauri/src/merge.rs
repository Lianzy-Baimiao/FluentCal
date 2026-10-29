//! Merging two copies of the data file (DAT-04): the data folder may live in OneDrive and be
//! edited on two PCs. `doc.meta[collection][id]` holds the last edit time (ms) of every item,
//! also for deleted items (tombstones), so each item can take whichever side changed it last.

use crate::store::COLLECTIONS;
use serde_json::{json, Map, Value};

/// Tombstones older than this are forgotten (a copy that stayed offline longer may bring a
/// deleted item back).
pub const TOMBSTONE_DAYS: i64 = 60;

fn ts(doc: &Value, col: &str, id: &str) -> i64 {
    doc.pointer(&format!("/meta/{col}/{id}")).and_then(Value::as_i64).unwrap_or(0)
}

fn meta_mut<'a>(doc: &'a mut Value, col: &str) -> &'a mut Map<String, Value> {
    let root = doc.as_object_mut().expect("object");
    let meta = root.entry("meta").or_insert_with(|| json!({}));
    if !meta.is_object() {
        *meta = json!({});
    }
    let m = meta.as_object_mut().unwrap().entry(col).or_insert_with(|| json!({}));
    if !m.is_object() {
        *m = json!({});
    }
    m.as_object_mut().unwrap()
}

/// Records `now` as the edit time of the given ids (written or deleted).
pub fn stamp<'a>(doc: &mut Value, col: &str, ids: impl IntoIterator<Item = &'a String>, now: i64) {
    let m = meta_mut(doc, col);
    for id in ids {
        m.insert(id.clone(), json!(now));
    }
}

/// Stamps every item present in `doc` (after an import or restore, which count as edits now);
/// keeps `old_meta` tombstones for items that are absent.
pub fn stamp_all(doc: &mut Value, old_meta: Option<&Value>, now: i64) {
    for col in COLLECTIONS {
        let ids: Vec<String> = doc[col].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        let old: Map<String, Value> = old_meta.and_then(|m| m.get(col)).and_then(Value::as_object).cloned().unwrap_or_default();
        let m = meta_mut(doc, col);
        *m = old;
        for id in &ids {
            m.insert(id.clone(), json!(now));
        }
    }
}

/// Merges `theirs` into `ours`. Per item the newer edit wins (ties keep ours); an item that
/// is absent on the winning side is deleted. Settings stay ours (they are per PC).
/// Returns true when anything changed.
pub fn merge(ours: &mut Value, theirs: &Value) -> bool {
    let mut changed = false;
    for col in COLLECTIONS {
        let their_items = theirs.get(col).and_then(Value::as_object).cloned().unwrap_or_default();
        let their_meta = theirs.pointer(&format!("/meta/{col}")).and_then(Value::as_object).cloned().unwrap_or_default();
        let our_ids: Vec<String> = ours[col].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        let our_meta: Vec<String> = ours.pointer(&format!("/meta/{col}")).and_then(Value::as_object).map(|o| o.keys().cloned().collect()).unwrap_or_default();
        let mut ids: Vec<String> = our_ids.into_iter().chain(our_meta).chain(their_items.keys().cloned()).chain(their_meta.keys().cloned()).collect();
        ids.sort();
        ids.dedup();
        for id in ids {
            let (t_ours, t_theirs) = (ts(ours, col, &id), ts(theirs, col, &id));
            let has_ours = ours[col].get(&id).is_some();
            let theirs_item = their_items.get(&id);
            // Ours wins ties, and an item present on only one side wins when neither has a
            // timestamp (data written before timestamps existed).
            let take_theirs = t_theirs > t_ours || (t_theirs == t_ours && !has_ours && theirs_item.is_some() && t_ours == 0);
            if !take_theirs {
                continue;
            }
            let col_obj = ours[col].as_object_mut().expect("normalized");
            match theirs_item {
                Some(v) if col_obj.get(&id) != Some(v) => {
                    col_obj.insert(id.clone(), v.clone());
                    changed = true;
                }
                None if col_obj.remove(&id).is_some() => changed = true,
                _ => {}
            }
            meta_mut(ours, col).insert(id, json!(t_theirs));
        }
    }
    changed
}

/// Drops tombstones older than TOMBSTONE_DAYS.
pub fn prune(doc: &mut Value, now: i64) {
    let cutoff = now - TOMBSTONE_DAYS * 86_400_000;
    for col in COLLECTIONS {
        let present: Vec<String> = doc[col].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        meta_mut(doc, col).retain(|id, t| present.contains(id) || t.as_i64().unwrap_or(0) >= cutoff);
    }
}
