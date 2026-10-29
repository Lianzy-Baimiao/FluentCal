use fluentcal_lib::merge::{merge, prune, stamp, stamp_all};
use fluentcal_lib::store::normalize;
use serde_json::{json, Value};

fn doc(tasks: Value, meta: Value) -> Value {
    normalize(json!({ "tasks": tasks, "meta": { "tasks": meta } }))
}

#[test]
fn newer_edit_wins_per_item() {
    let mut ours = doc(json!({ "a": {"title": "我改的"}, "b": {"title": "旧"} }), json!({ "a": 300, "b": 100 }));
    let theirs = doc(json!({ "a": {"title": "另一台旧的"}, "b": {"title": "另一台新的"}, "c": {"title": "另一台新增"} }), json!({ "a": 200, "b": 250, "c": 260 }));
    assert!(merge(&mut ours, &theirs));
    assert_eq!(ours["tasks"]["a"]["title"], "我改的");
    assert_eq!(ours["tasks"]["b"]["title"], "另一台新的");
    assert_eq!(ours["tasks"]["c"]["title"], "另一台新增");
    assert_eq!(ours["meta"]["tasks"]["b"], 250);
}

#[test]
fn deletions_win_when_newer() {
    // We deleted "a" (tombstone 300); the other copy still has its older version.
    let mut ours = doc(json!({}), json!({ "a": 300 }));
    let theirs = doc(json!({ "a": {"title": "旧"} }), json!({ "a": 100 }));
    assert!(!merge(&mut ours, &theirs));
    assert!(ours["tasks"].get("a").is_none());
    // The other PC deleted "b" after our last edit.
    let mut ours = doc(json!({ "b": {"title": "x"} }), json!({ "b": 100 }));
    let theirs = doc(json!({}), json!({ "b": 200 }));
    assert!(merge(&mut ours, &theirs));
    assert!(ours["tasks"].get("b").is_none());
}

#[test]
fn ties_keep_ours_and_merging_twice_changes_nothing() {
    let mut ours = doc(json!({ "a": {"title": "我的"} }), json!({ "a": 100 }));
    let theirs = doc(json!({ "a": {"title": "他的"}, "n": {"title": "无时间戳"} }), json!({ "a": 100 }));
    assert!(merge(&mut ours, &theirs));
    assert_eq!(ours["tasks"]["a"]["title"], "我的");
    assert_eq!(ours["tasks"]["n"]["title"], "无时间戳"); // data from before timestamps existed
    assert!(!merge(&mut ours, &theirs), "merge is idempotent");
}

#[test]
fn settings_stay_local() {
    let mut ours = normalize(json!({ "settings": { "desk": { "pinMode": "desktop" } } }));
    let theirs = normalize(json!({ "settings": { "desk": { "pinMode": "top" } } }));
    merge(&mut ours, &theirs);
    assert_eq!(ours["settings"]["desk"]["pinMode"], "desktop");
}

#[test]
fn stamping_and_pruning() {
    let mut d = doc(json!({ "a": {} }), json!({ "gone-old": 1, "gone-new": 9_000_000_000_000i64 }));
    stamp(&mut d, "tasks", [&"a".to_string()], 500);
    assert_eq!(d["meta"]["tasks"]["a"], 500);
    prune(&mut d, 9_000_000_000_000);
    assert!(d["meta"]["tasks"].get("gone-old").is_none());
    assert!(d["meta"]["tasks"].get("gone-new").is_some());
    let old = d["meta"].clone();
    stamp_all(&mut d, Some(&old), 777);
    assert_eq!(d["meta"]["tasks"]["a"], 777);
    assert_eq!(d["meta"]["tasks"]["gone-new"], 9_000_000_000_000i64, "tombstones survive a restore");
}
