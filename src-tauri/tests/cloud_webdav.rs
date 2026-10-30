//! Cloud sync over WebDAV against an in-memory server: two PCs converge, conflicts are
//! retried, unchanged files aren't uploaded again, foreign files are never overwritten.
mod common;

use common::*;
use fluentcal_lib::cloud::engine::{cycle, Mode, Outcome, SyncState};
use fluentcal_lib::cloud::{http::Http, webdav::WebDav, ErrKind, SyncError};
use fluentcal_lib::store::Store;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
struct Dav {
    folder: bool,
    file: Option<(Vec<u8>, u32)>,
    /// Written by "another PC" right before the next PUT is checked.
    race: Option<Vec<u8>>,
    puts: u32,
}

fn dav_server(dav: Arc<Mutex<Dav>>) -> String {
    let (base, _) = serve(move |req| {
        let mut d = dav.lock().unwrap();
        if req.header("authorization") != Some("Basic dTpwdw==") {
            return Res::new(401);
        }
        let is_file = req.path().ends_with("/fluentcal-data.json");
        match (req.method.as_str(), is_file) {
            ("MKCOL", false) if d.folder => Res::new(405),
            ("MKCOL", false) => {
                d.folder = true;
                Res::new(201)
            }
            ("GET", true) => match &d.file {
                None => Res::new(404),
                Some((_, v)) if req.header("if-none-match") == Some(&format!("\"{v}\"")) => Res::new(304),
                Some((b, v)) => Res::new(200).with("ETag", &format!("\"{v}\"")).body(b.clone()),
            },
            ("PUT", true) if !d.folder => Res::new(409),
            ("PUT", true) => {
                if let Some(b) = d.race.take() {
                    let v = d.file.as_ref().map_or(0, |f| f.1) + 1;
                    d.file = Some((b, v));
                }
                let cur = d.file.as_ref().map(|f| format!("\"{}\"", f.1));
                let ok = match (req.header("if-match"), req.header("if-none-match")) {
                    (Some(m), _) => cur.as_deref() == Some(m),
                    (None, Some("*")) => cur.is_none(),
                    _ => true,
                };
                if !ok {
                    return Res::new(412);
                }
                let v = d.file.as_ref().map_or(0, |f| f.1) + 1;
                d.file = Some((req.body.clone(), v));
                d.puts += 1;
                Res::new(201).with("ETag", &format!("\"{v}\""))
            }
            _ => Res::new(400),
        }
    });
    base
}

fn round(s: &Store, base: &str, st: &mut SyncState, mode: Mode, password: &str) -> Result<Outcome, SyncError> {
    let http = Http::new("direct", Duration::from_secs(10)).unwrap();
    let mut remote = WebDav::new(http, &format!("{base}/dav/"), "u", password, "FluentCal").unwrap();
    cycle(s, &mut remote, st, mode)
}

#[test]
fn two_pcs_converge_with_edits_deletes_and_conflicts() {
    let dav = Arc::new(Mutex::new(Dav::default()));
    let base = dav_server(dav.clone());
    let (a, b) = (store("dav-a"), store("dav-b"));
    let (mut sa, mut sb) = (SyncState::default(), SyncState::default());
    put_task(&a, "a1", "A 的任务");
    tick();
    put_task(&b, "b1", "B 的任务");

    // A uploads first (the folder is created on the way), B merges and uploads both.
    assert!(round(&a, &base, &mut sa, Mode::Normal, "pw").unwrap().uploaded);
    assert!(dav.lock().unwrap().folder);
    let out = round(&b, &base, &mut sb, Mode::Normal, "pw").unwrap();
    assert!(out.downloaded && out.uploaded);
    assert_eq!(titles(&b), ["A 的任务", "B 的任务"]);
    round(&a, &base, &mut sa, Mode::Normal, "pw").unwrap();
    assert_eq!(titles(&a), ["A 的任务", "B 的任务"]);

    // Nothing changed: conditional GET, no upload.
    let puts = dav.lock().unwrap().puts;
    let out = round(&a, &base, &mut sa, Mode::Normal, "pw").unwrap();
    assert!(!out.uploaded && !out.downloaded);
    assert_eq!(dav.lock().unwrap().puts, puts);

    // A deletes its task, B renames its own: both survive the round trip.
    delete_task(&a, "a1");
    tick();
    put_task(&b, "b1", "B 的任务（改）");
    round(&a, &base, &mut sa, Mode::Normal, "pw").unwrap();
    round(&b, &base, &mut sb, Mode::Normal, "pw").unwrap();
    round(&a, &base, &mut sa, Mode::Normal, "pw").unwrap();
    assert_eq!(titles(&a), ["B 的任务（改）"]);
    assert_eq!(titles(&b), ["B 的任务（改）"]);

    // C uploads between B's fetch and B's upload: B's If-Match fails, B merges again.
    let c = store("dav-c");
    put_task(&c, "c1", "C 的任务");
    let other = Arc::new(Mutex::new(Dav::default()));
    let other_base = dav_server(other.clone());
    round(&c, &other_base, &mut SyncState::default(), Mode::Normal, "pw").unwrap();
    dav.lock().unwrap().race = other.lock().unwrap().file.clone().map(|f| f.0);
    tick();
    put_task(&b, "b2", "B 的新任务");
    assert!(round(&b, &base, &mut sb, Mode::Normal, "pw").unwrap().uploaded);
    assert_eq!(titles(&b), ["B 的任务（改）", "B 的新任务", "C 的任务"]);
    round(&a, &base, &mut sa, Mode::Normal, "pw").unwrap();
    assert_eq!(titles(&a), titles(&b));
}

#[test]
fn replace_takes_the_cloud_copy() {
    let dav = Arc::new(Mutex::new(Dav::default()));
    let base = dav_server(dav.clone());
    let (a, b) = (store("dav-ra"), store("dav-rb"));
    put_task(&a, "a1", "云端任务");
    put_task(&b, "x1", "新电脑的示例任务");
    round(&a, &base, &mut SyncState::default(), Mode::Normal, "pw").unwrap();
    let out = round(&b, &base, &mut SyncState::default(), Mode::Replace, "pw").unwrap();
    assert!(out.downloaded && !out.uploaded);
    assert_eq!(titles(&b), ["云端任务"]);
}

#[test]
fn wrong_password_and_foreign_files_are_reported_not_overwritten() {
    let dav = Arc::new(Mutex::new(Dav::default()));
    let base = dav_server(dav.clone());
    let a = store("dav-pw");
    put_task(&a, "a1", "任务");
    let e = round(&a, &base, &mut SyncState::default(), Mode::Normal, "wrong").unwrap_err();
    assert_eq!(e.kind, ErrKind::Auth);

    let foreign = br#"{"some": "other app"}"#.to_vec();
    {
        let mut d = dav.lock().unwrap();
        d.folder = true;
        d.file = Some((foreign.clone(), 1));
    }
    let e = round(&a, &base, &mut SyncState::default(), Mode::Normal, "pw").unwrap_err();
    assert_eq!(e.kind, ErrKind::Remote);
    assert_eq!(dav.lock().unwrap().file.as_ref().unwrap().0, foreign);
}

/// Against a real WebDAV server (wsgidav): FLUENTCAL_DAV_URL=http://127.0.0.1:8085/
/// FLUENTCAL_DAV_ROOT=<its folder> cargo test --test cloud_webdav -- --ignored
#[test]
#[ignore]
fn real_webdav_server() {
    use fluentcal_lib::cloud::{Fetched, Remote};
    let url = std::env::var("FLUENTCAL_DAV_URL").expect("FLUENTCAL_DAV_URL");
    let root = std::env::var("FLUENTCAL_DAV_ROOT").expect("FLUENTCAL_DAV_ROOT");
    let make = || WebDav::new(Http::new("direct", Duration::from_secs(10)).unwrap(), &url, "fc", "fc-pass", "FluentCal/测试").unwrap();
    let (a, b) = (store("real-a"), store("real-b"));
    let (mut sa, mut sb) = (SyncState::default(), SyncState::default());
    put_task(&a, "a1", "A");
    tick();
    put_task(&b, "b1", "B");
    assert!(cycle(&a, &mut make(), &mut sa, Mode::Normal).unwrap().uploaded, "first upload, nested folders created");
    cycle(&b, &mut make(), &mut sb, Mode::Normal).unwrap();
    cycle(&a, &mut make(), &mut sa, Mode::Normal).unwrap();
    assert_eq!(titles(&a), ["A", "B"]);
    assert_eq!(titles(&b), ["A", "B"]);
    let out = cycle(&a, &mut make(), &mut sa, Mode::Normal).unwrap();
    assert!(!out.uploaded && !out.downloaded);

    // Changed on the server after our fetch: an upload with the old ETag must be refused.
    let mut r = make();
    let Fetched::File(_, rev) = r.fetch(None).unwrap() else { panic!("file expected") };
    println!("server ETag: {:?}", rev.etag);
    let file = std::path::Path::new(&root).join("FluentCal").join("测试").join("fluentcal-data.json");
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str("\n ");
    std::fs::write(&file, text).unwrap();
    if rev.etag.as_deref().is_some_and(|e| !e.starts_with("W/")) {
        assert_eq!(r.put(b"{}", Some(&rev)).unwrap_err().kind, ErrKind::Conflict);
    }
    // A wrong password is reported as such.
    let mut bad = WebDav::new(Http::new("direct", Duration::from_secs(10)).unwrap(), &url, "fc", "nope", "FluentCal").unwrap();
    assert_eq!(bad.fetch(None).err().map(|e| e.kind), Some(ErrKind::Auth));
}
