//! Cloud sync through the Google Drive v3 API (token refresh, folder and file lookup,
//! multipart create, media update with a version check), against an in-memory server.
mod common;

use common::*;
use fluentcal_lib::cloud::engine::{cycle, Mode, Outcome, SyncState};
use fluentcal_lib::cloud::{cloud_view, encode, gdrive::Drive, http::{dec, Http}, oauth::Endpoints, ErrKind, SyncError};
use fluentcal_lib::store::Store;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct File {
    id: String,
    name: String,
    parent: String,
    folder: bool,
    content: Vec<u8>,
    version: u32,
    trashed: bool,
}

#[derive(Default)]
struct DriveState {
    files: Vec<File>,
    next: u32,
    /// Another PC uploads on the n-th metadata read from now (1 = the next one).
    race_at: u32,
    race: Option<Vec<u8>>,
    revoked: bool,
}

/// (metadata, media) of a multipart/related upload.
fn multipart(body: &[u8]) -> (Value, Vec<u8>) {
    let s = String::from_utf8_lossy(body).into_owned();
    let mut parts = s.split("\r\n\r\n");
    parts.next();
    let meta = parts.next().unwrap_or("").split("\r\n--").next().unwrap_or("");
    let media = parts.next().unwrap_or("").rsplit_once("\r\n--").map_or("", |(m, _)| m);
    (serde_json::from_str(meta).unwrap_or(Value::Null), media.as_bytes().to_vec())
}

fn drive_server(st: Arc<Mutex<DriveState>>) -> String {
    let (base, _) = serve(move |req| {
        let mut d = st.lock().unwrap();
        if req.path() == "/token" {
            let body = String::from_utf8_lossy(&req.body).into_owned();
            return if d.revoked || !body.contains("refresh_token=r1") {
                Res::json(400, json!({ "error": "invalid_grant" }))
            } else {
                Res::json(200, json!({ "access_token": "at", "expires_in": 3600 }))
            };
        }
        if req.header("authorization") != Some("Bearer at") {
            return Res::new(401);
        }
        let new_id = |d: &mut DriveState| {
            d.next += 1;
            format!("id{}", d.next)
        };
        match (req.method.as_str(), req.path()) {
            ("GET", "/drive/v3/files") => {
                let q = dec(&req.query("q").unwrap_or_default());
                let quoted: Vec<&str> = q.split('\'').collect();
                let (name, parent) = (quoted.get(1).copied().unwrap_or(""), quoted.get(3).copied().unwrap_or(""));
                let hit = d.files.iter().find(|f| f.name == name && f.parent == parent && !f.trashed);
                Res::json(200, json!({ "files": hit.map(|f| vec![json!({ "id": f.id })]).unwrap_or_default() }))
            }
            ("POST", "/drive/v3/files") => {
                let meta = req.json();
                let id = new_id(&mut d);
                d.files.push(File { id: id.clone(), name: meta["name"].as_str().unwrap_or("").into(), parent: "root".into(), folder: true, content: vec![], version: 1, trashed: false });
                Res::json(200, json!({ "id": id }))
            }
            ("POST", "/upload/drive/v3/files") => {
                let (meta, media) = multipart(&req.body);
                let id = new_id(&mut d);
                let parent = meta["parents"][0].as_str().unwrap_or("").to_owned();
                d.files.push(File { id: id.clone(), name: meta["name"].as_str().unwrap_or("").into(), parent, folder: false, content: media, version: 1, trashed: false });
                Res::json(200, json!({ "id": id, "version": "1" }))
            }
            (m, p) => {
                let id = p.rsplit('/').next().unwrap_or("").to_owned();
                let race = if m == "GET" && req.query("fields").is_some() && d.race_at > 0 {
                    d.race_at -= 1;
                    d.race_at == 0
                } else {
                    false
                };
                let injected = if race { d.race.take() } else { None };
                let Some(f) = d.files.iter_mut().find(|f| f.id == id) else { return Res::new(404) };
                if let Some(b) = injected {
                    f.content = b;
                    f.version += 1;
                }
                match m {
                    "GET" if req.query("alt").as_deref() == Some("media") => Res::new(200).body(f.content.clone()),
                    "GET" => Res::json(200, json!({ "id": f.id, "version": f.version.to_string(), "trashed": f.trashed })),
                    "PATCH" => {
                        f.content = req.body.clone();
                        f.version += 1;
                        Res::json(200, json!({ "id": f.id, "version": f.version.to_string() }))
                    }
                    _ => Res::new(405),
                }
            }
        }
    });
    base
}

fn round(s: &Store, base: &str, st: &mut SyncState) -> Result<Outcome, SyncError> {
    let http = Http::new("direct", Duration::from_secs(10)).unwrap();
    let ep = Endpoints { auth: String::new(), token: format!("{base}/token"), api: base.into(), upload: format!("{base}/upload") };
    let mut remote = Drive::new(http, ep, "cid", "secret", "r1", &st.location);
    cycle(s, &mut remote, st, Mode::Normal)
}

#[test]
fn drive_two_pcs_race_and_trashed_file() {
    let d = Arc::new(Mutex::new(DriveState::default()));
    let base = drive_server(d.clone());
    let (a, b) = (store("gd-a"), store("gd-b"));
    let (mut sa, mut sb) = (SyncState::default(), SyncState::default());
    put_task(&a, "a1", "A");
    tick();
    put_task(&b, "b1", "B");
    round(&a, &base, &mut sa).unwrap();
    round(&b, &base, &mut sb).unwrap();
    round(&a, &base, &mut sa).unwrap();
    assert_eq!(titles(&a), ["A", "B"]);
    assert!(!round(&a, &base, &mut sa).unwrap().uploaded, "unchanged: no upload");
    assert_eq!(d.lock().unwrap().files.iter().filter(|f| f.folder).count(), 1, "one FluentCal folder");

    // Another PC uploads between B's version check and B's update.
    let c = store("gd-c");
    put_task(&c, "c1", "C");
    {
        let mut s = d.lock().unwrap();
        s.race = Some(encode(&cloud_view(&c.snapshot()), 1));
        s.race_at = 2;
    }
    tick();
    put_task(&b, "b2", "B2");
    assert!(round(&b, &base, &mut sb).unwrap().uploaded);
    assert_eq!(titles(&b), ["A", "B", "B2", "C"]);

    // The user trashed the file: it is created again.
    for f in d.lock().unwrap().files.iter_mut().filter(|f| !f.folder) {
        f.trashed = true;
    }
    tick();
    put_task(&b, "b3", "B3");
    assert!(round(&b, &base, &mut sb).unwrap().uploaded);
    round(&a, &base, &mut sa).unwrap();
    assert_eq!(titles(&a), ["A", "B", "B2", "B3", "C"]);
    let live = d.lock().unwrap().files.iter().filter(|f| !f.folder && !f.trashed).count();
    assert_eq!(live, 1);

    d.lock().unwrap().revoked = true;
    let e = round(&a, &base, &mut SyncState::default()).unwrap_err();
    assert_eq!(e.kind, ErrKind::Auth);
}
