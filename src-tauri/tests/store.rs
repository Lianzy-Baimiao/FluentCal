use fluentcal_lib::store::{deep_merge, normalize, Store, COLLECTIONS};
use serde_json::{json, Map, Value};
use std::fs;

#[test]
fn deep_merge_merges_nested_and_removes_nulls() {
    let mut t = json!({"a": {"b": 1, "c": 2}, "d": [1, 2], "e": 5});
    deep_merge(&mut t, &json!({"a": {"b": 9, "c": null}, "d": [3], "f": {"g": true}}));
    assert_eq!(t, json!({"a": {"b": 9}, "d": [3], "e": 5, "f": {"g": true}}));
}

#[test]
fn normalize_fills_missing_collections() {
    let v = normalize(json!({"tasks": [], "settings": {"x": 1}}));
    for c in COLLECTIONS {
        assert!(v[c].is_object(), "{c} should be an object");
    }
    assert_eq!(v["settings"]["x"], 1);
    assert_eq!(v["version"], 1);
}

#[test]
fn store_roundtrip_and_recovery() {
    let dir = std::env::temp_dir().join(format!("fluentcal-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    {
        let s = Store::open(dir.clone()).unwrap();
        let mut items = Map::new();
        items.insert("t1".into(), json!({"id": "t1", "title": "写周报"}));
        s.put("tasks", &items);
        s.patch_settings(&json!({"theme": {"mode": "dark"}}));
        assert!(s.flush());
        let mut del = Map::new();
        del.insert("missing".into(), Value::Null);
        s.put("tasks", &del); // deleting an unknown id is a no-op
        assert!(s.flush());
    }
    let text = fs::read_to_string(dir.join("data.json")).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["tasks"]["t1"]["title"], "写周报");
    assert_eq!(v["settings"]["theme"]["mode"], "dark");

    // Corrupt the file: the store must recover from the newest backup.
    fs::write(dir.join("backups").join("manual-20990101-000000.json"), &text).unwrap();
    fs::write(dir.join("data.json"), b"{ not json").unwrap();
    let s = Store::open(dir.clone()).unwrap();
    assert_eq!(s.snapshot()["tasks"]["t1"]["title"], "写周报");
    assert!(s.take_recovered().is_some());
    assert!(s.take_recovered().is_none(), "recovery notice is taken once");
    drop(s);
    let _ = fs::remove_dir_all(&dir);
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fluentcal-test-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("backups")).unwrap();
    dir
}

#[test]
fn missing_file_is_a_fresh_start() {
    let dir = temp_dir("fresh");
    let s = Store::open(dir.clone()).unwrap();
    assert!(s.take_recovered().is_none());
    assert!(s.snapshot()["tasks"].as_object().unwrap().is_empty());
    drop(s);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unreadable_bytes_recover_from_backup_and_are_kept_aside() {
    let dir = temp_dir("utf8");
    let good = json!({"tasks": {"a": {"id": "a", "title": "保留"}}, "settings": {}});
    fs::write(dir.join("backups").join("daily-2099-01-01.json"), good.to_string()).unwrap();
    fs::write(dir.join("data.json"), b"{\"tasks\": {\"a\": \"\xE4\xBF"[..].to_vec()).unwrap(); // truncated UTF-8
    let s = Store::open(dir.clone()).unwrap();
    assert_eq!(s.take_recovered().as_deref(), Some("daily-2099-01-01.json"));
    assert_eq!(s.snapshot()["tasks"]["a"]["title"], "保留");
    let aside = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).any(|e| e.file_name().to_string_lossy().starts_with("data.unreadable-"));
    assert!(aside, "the unreadable file is moved aside, not overwritten");
    // Today's daily backup must not be a copy of the broken file.
    for e in fs::read_dir(dir.join("backups")).unwrap().filter_map(|e| e.ok()) {
        let text = fs::read(e.path()).unwrap();
        assert!(std::str::from_utf8(&text).is_ok(), "{:?} holds the corrupt bytes", e.file_name());
    }
    drop(s);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_without_backup_does_not_reseed_samples() {
    let dir = temp_dir("nobackup");
    fs::write(dir.join("data.json"), b"not json").unwrap();
    let s = Store::open(dir.clone()).unwrap();
    assert_eq!(s.take_recovered().as_deref(), Some(""));
    assert_eq!(s.snapshot()["settings"]["general"]["seeded"], true);
    drop(s);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn external_edits_and_conflict_copies_are_merged() {
    use fluentcal_lib::sync::sync_once;
    let dir = temp_dir("sync");
    let s = Store::open_with(dir.clone(), Some(dir.join("local-settings.json"))).unwrap();
    let mut items = Map::new();
    items.insert("mine".into(), json!({"id": "mine", "title": "本机"}));
    s.put("tasks", &items);
    s.patch_settings(&json!({"desk": {"pinMode": "top"}}));
    assert!(s.flush());
    let shared: Value = serde_json::from_str(&fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
    assert!(shared.get("settings").is_none(), "settings stay out of the shared file");
    assert!(!sync_once(&s), "our own write is not an external change");

    // Another PC adds a task (newer timestamp) and its sync tool leaves a conflict copy.
    let mut other = shared.clone();
    other["tasks"]["remote"] = json!({"id": "remote", "title": "另一台"});
    other["meta"]["tasks"]["remote"] = json!(9_000_000_000_000i64);
    fs::write(dir.join("data.json"), other.to_string()).unwrap();
    let mut copy = shared.clone();
    copy["tasks"]["fromcopy"] = json!({"id": "fromcopy", "title": "冲突副本"});
    copy["meta"]["tasks"]["fromcopy"] = json!(9_000_000_000_000i64);
    fs::write(dir.join("data-OTHERPC.json"), copy.to_string()).unwrap();

    assert!(sync_once(&s));
    let d = s.snapshot();
    assert_eq!(d["tasks"]["remote"]["title"], "另一台");
    assert_eq!(d["tasks"]["fromcopy"]["title"], "冲突副本");
    assert_eq!(d["tasks"]["mine"]["title"], "本机");
    assert_eq!(d["settings"]["desk"]["pinMode"], "top", "local settings kept");
    assert!(!dir.join("data-OTHERPC.json").exists(), "conflict copy renamed aside");
    drop(s);
    let _ = fs::remove_dir_all(&dir);
}
