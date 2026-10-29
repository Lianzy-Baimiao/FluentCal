//! Watches data.json for changes made by another program (OneDrive/Dropbox syncing a copy
//! edited on another PC) and merges them item by item (see merge.rs). Conflict copies made by
//! sync tools ("data-PCNAME.json", "data (1).json") are merged too and then renamed aside.

use crate::merge;
use crate::store::{now_ms, signature, Store};
use serde_json::Value;
use std::{fs, thread, time::Duration};

const POLL: Duration = Duration::from_secs(3);

/// Shared content only: settings are per PC.
fn shared(v: &Value) -> Value {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("settings");
    }
    v
}

pub fn watch(store: &Store) {
    loop {
        thread::sleep(POLL);
        if sync_once(store) {
            store.emit_reload();
        }
    }
}

/// One check of data.json and conflict copies. Returns true when our document changed.
pub fn sync_once(store: &Store) -> bool {
    let a = check_main(store);
    let b = merge_conflict_copies(store);
    a || b
}

/// Merges data.json if someone else wrote it. Returns true when our document changed.
fn check_main(store: &Store) -> bool {
    let _w = store.write_lock.lock().unwrap();
    let Ok(bytes) = fs::read(&store.file) else { return false };
    let sig = signature(&bytes);
    if *store.last_written.lock().unwrap() == Some(sig) {
        return false;
    }
    let Some(theirs) = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{FEFF}')).ok())
        .filter(Value::is_object)
    else {
        return false; // half-synced file: try again next time
    };
    *store.last_written.lock().unwrap() = Some(sig);
    let mut g = store.inner.lock().unwrap();
    let before = shared(&g.doc);
    let changed = merge::merge(&mut g.doc, &theirs);
    merge::prune(&mut g.doc, now_ms());
    // Write back only if the file lacks something we have, so two PCs don't ping-pong.
    let mut file_view = theirs;
    file_view.as_object_mut().map(|o| o.remove("settings"));
    if shared(&g.doc) != shared(&file_view) {
        store.mark_dirty(&mut g);
    }
    changed && shared(&g.doc) != before
}

fn is_conflict_copy(name: &str) -> bool {
    name.starts_with("data")
        && name.ends_with(".json")
        && name != "data.json"
        && !name.contains(".unreadable-")
        && !name.contains(".corrupt-")
        && !name.contains(".merged-")
        && !name.ends_with(".json.tmp")
}

fn merge_conflict_copies(store: &Store) -> bool {
    let Ok(rd) = fs::read_dir(&store.dir) else { return false };
    let names: Vec<String> = rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).filter(|n| is_conflict_copy(n)).collect();
    let mut changed = false;
    for name in names {
        let path = store.dir.join(&name);
        let Some(theirs) = fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{FEFF}')).ok()).filter(Value::is_object) else {
            continue; // may still be downloading
        };
        {
            let mut g = store.inner.lock().unwrap();
            if merge::merge(&mut g.doc, &theirs) {
                changed = true;
                store.mark_dirty(&mut g);
            }
        }
        let stem = name.trim_end_matches(".json");
        let _ = fs::rename(&path, store.dir.join(format!("{stem}.merged-{}.json", now_ms())));
    }
    changed
}
