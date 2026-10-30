//! Cloud file format, content signatures, and cleaning up after the removed Google Drive sync.
mod common;

use common::*;
use fluentcal_core::cloud::service::Config;
use fluentcal_core::cloud::{cloud_view, decode, drop_google_drive, encode, view_sig, ErrKind};
use serde_json::json;

#[test]
fn format_refuses_foreign_and_newer_files() {
    let view = cloud_view(&json!({ "tasks": { "t": { "title": "x" } }, "meta": { "tasks": { "t": 5 } } }));
    let f = decode(&encode(&view, 7)).unwrap().unwrap();
    assert_eq!(f.view, view);
    assert_eq!(f.saved_at, Some(7));
    assert!(decode(b"").unwrap().is_none(), "empty file = no data yet");
    assert!(decode("\u{FEFF}  ".as_bytes()).unwrap().is_none());
    for bad in [&br#"{"tasks": {}}"#[..], b"not json", br#"{"app": "Other", "data": {}}"#] {
        assert_eq!(decode(bad).err().map(|e| e.kind), Some(ErrKind::Remote));
    }
    let newer = json!({ "app": "FluentCal", "format": 99, "data": {} }).to_string();
    assert!(decode(newer.as_bytes()).err().unwrap().message.contains("升级"));
}

#[test]
fn signature_ignores_key_order_and_settings() {
    let a = json!({ "tasks": { "x": { "a": 1, "b": 2 }, "y": {} }, "settings": { "theme": "dark" } });
    let b = json!({ "tasks": { "y": {}, "x": { "b": 2, "a": 1 } }, "holidays": { "2026": {} } });
    assert_eq!(view_sig(&cloud_view(&a)), view_sig(&cloud_view(&b)));
    assert!(cloud_view(&a).get("settings").is_none());
}

/// Settings left by 0.2.0, where Google Drive was a sync provider.
#[test]
fn google_drive_settings_are_cleaned_up() {
    // Syncing to Google Drive: sync goes off and the Google settings go; the rest stays.
    let s = store("drop-gdrive");
    s.patch_settings(&json!({ "sync": { "enabled": true, "provider": "gdrive", "interval": 15, "gdrive": { "clientId": "cid", "email": "me@example.com" }, "webdav": { "url": "https://dav.example.com/" } } }));
    let patch = drop_google_drive(&s.snapshot()["settings"]).expect("needs cleaning up");
    s.patch_settings(&patch);
    let sync = s.snapshot()["settings"]["sync"].clone();
    assert!(sync.get("gdrive").is_none(), "{sync}");
    assert_eq!((sync["provider"].as_str(), sync["enabled"].as_bool()), (Some(""), Some(false)));
    assert_eq!((sync["interval"].as_u64(), sync["webdav"]["url"].as_str()), (Some(15), Some("https://dav.example.com/")));
    assert!(!Config::from_settings(&s.snapshot()["settings"]).active());
    assert!(drop_google_drive(&s.snapshot()["settings"]).is_none(), "done once");

    // Tried Google Drive, then moved to WebDAV: only the leftovers go.
    let left = json!({ "sync": { "enabled": true, "provider": "webdav", "gdrive": { "clientId": "cid" } } });
    assert_eq!(drop_google_drive(&left), Some(json!({ "sync": { "gdrive": null } })));
    // Never used it.
    assert!(drop_google_drive(&json!({ "sync": { "provider": "github" } })).is_none());
    assert!(drop_google_drive(&json!({})).is_none());
}
