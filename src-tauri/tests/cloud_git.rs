//! Cloud sync through the GitHub and Gitee contents APIs, against an in-memory server.
mod common;

use base64::Engine as _;
use common::*;
use fluentcal_lib::cloud::engine::{cycle, Mode, Outcome, SyncState};
use fluentcal_lib::cloud::git::{Flavor, Git};
use fluentcal_lib::cloud::{cloud_view, encode, http::Http, ErrKind, SyncError};
use fluentcal_lib::store::Store;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TOKEN: &str = "tok-secret-123";

struct Repo {
    private: bool,
    push: bool,
    file: Option<(Vec<u8>, String)>,
    /// Written by "another PC" right before the next upload is checked.
    race: Option<Vec<u8>>,
    puts: u32,
    gitee_error: bool,
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn git_server(flavor: Flavor, repo: Arc<Mutex<Repo>>) -> String {
    let (base, _) = serve(move |req| {
        let mut r = repo.lock().unwrap();
        let authed = match flavor {
            Flavor::GitHub => req.header("authorization") == Some(&format!("Bearer {TOKEN}")),
            Flavor::Gitee => req.query("access_token").as_deref() == Some(TOKEN) || req.json()["access_token"] == TOKEN,
        };
        if !authed {
            return Res::json(401, json!({ "message": "Bad credentials" }));
        }
        let path = req.path().trim_start_matches("/api/v5");
        if path == "/repos/o/r" {
            let perm = if flavor == Flavor::GitHub { "permissions" } else { "permission" };
            return Res::json(200, json!({ "private": r.private, perm: { "push": r.push } }));
        }
        if !path.ends_with("/contents/fluentcal-data.json") {
            return Res::new(404);
        }
        match req.method.as_str() {
            "GET" => match &r.file {
                None if flavor == Flavor::Gitee => Res::json(200, json!([])),
                None => Res::json(404, json!({ "message": "Not Found" })),
                Some((_, sha)) if req.header("if-none-match") == Some(&format!("\"e{sha}\"")) => Res::new(304),
                Some((b, sha)) => {
                    // GitHub wraps base64 at 60 columns.
                    let wrapped: Vec<String> = b64(b).as_bytes().chunks(60).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
                    Res::json(200, json!({ "sha": sha, "encoding": "base64", "size": b.len(), "content": wrapped.join("\n") })).with("ETag", &format!("\"e{sha}\""))
                }
            },
            "PUT" | "POST" => {
                if r.gitee_error {
                    return Res::json(400, json!({ "message": format!("token {TOKEN} is not allowed here") }));
                }
                if let Some(b) = r.race.take() {
                    r.file = Some((b, "raced".into()));
                }
                let body = req.json();
                let sent = body["sha"].as_str().map(str::to_owned);
                let current = r.file.as_ref().map(|f| f.1.clone());
                if sent != current {
                    return match (flavor, &sent) {
                        (Flavor::GitHub, Some(_)) => Res::json(409, json!({ "message": "is at 1 but expected 2" })),
                        (Flavor::GitHub, None) => Res::json(422, json!({ "message": "Invalid request. \"sha\" wasn't supplied." })),
                        (Flavor::Gitee, _) => Res::json(400, json!({ "message": "sha does not match" })),
                    };
                }
                let bytes = base64::engine::general_purpose::STANDARD.decode(body["content"].as_str().unwrap_or("")).unwrap();
                r.puts += 1;
                let sha = format!("s{}", r.puts);
                r.file = Some((bytes, sha.clone()));
                Res::json(201, json!({ "content": { "sha": sha } }))
            }
            _ => Res::new(405),
        }
    });
    base
}

fn repo() -> Arc<Mutex<Repo>> {
    Arc::new(Mutex::new(Repo { private: true, push: true, file: None, race: None, puts: 0, gitee_error: false }))
}

fn round(s: &Store, flavor: Flavor, base: &str, st: &mut SyncState) -> Result<Outcome, SyncError> {
    let http = Http::new("direct", Duration::from_secs(10)).unwrap();
    let api = if flavor == Flavor::Gitee { format!("{base}/api/v5") } else { base.to_owned() };
    let mut remote = Git::new(http, flavor, &api, "https://github.com/o/r.git", "", "", TOKEN).unwrap();
    cycle(s, &mut remote, st, Mode::Normal)
}

#[test]
fn github_round_trip_conflict_and_not_modified() {
    let r = repo();
    let base = git_server(Flavor::GitHub, r.clone());
    let (a, b) = (store("gh-a"), store("gh-b"));
    let (mut sa, mut sb) = (SyncState::default(), SyncState::default());
    put_task(&a, "a1", "A");
    tick();
    put_task(&b, "b1", "B");
    round(&a, Flavor::GitHub, &base, &mut sa).unwrap();
    round(&b, Flavor::GitHub, &base, &mut sb).unwrap();
    round(&a, Flavor::GitHub, &base, &mut sa).unwrap();
    assert_eq!(titles(&a), ["A", "B"]);
    let puts = r.lock().unwrap().puts;
    assert!(!round(&a, Flavor::GitHub, &base, &mut sa).unwrap().uploaded, "unchanged: no commit");
    assert_eq!(r.lock().unwrap().puts, puts);

    // Another PC commits between B's fetch and B's upload.
    let c = store("gh-c");
    put_task(&c, "c1", "C");
    r.lock().unwrap().race = Some(encode(&cloud_view(&c.snapshot()), 1));
    tick();
    put_task(&b, "b2", "B2");
    assert!(round(&b, Flavor::GitHub, &base, &mut sb).unwrap().uploaded);
    assert_eq!(titles(&b), ["A", "B", "B2", "C"]);
}

#[test]
fn github_refuses_public_and_read_only_repos() {
    let r = repo();
    let base = git_server(Flavor::GitHub, r.clone());
    let a = store("gh-pub");
    put_task(&a, "a1", "A");
    r.lock().unwrap().private = false;
    assert_eq!(round(&a, Flavor::GitHub, &base, &mut SyncState::default()).unwrap_err().kind, ErrKind::Config);
    r.lock().unwrap().private = true;
    r.lock().unwrap().push = false;
    assert_eq!(round(&a, Flavor::GitHub, &base, &mut SyncState::default()).unwrap_err().kind, ErrKind::Auth);
    assert_eq!(r.lock().unwrap().puts, 0);
}

#[test]
fn gitee_create_update_and_no_token_in_errors() {
    let r = repo();
    let base = git_server(Flavor::Gitee, r.clone());
    let (a, b) = (store("ge-a"), store("ge-b"));
    let (mut sa, mut sb) = (SyncState::default(), SyncState::default());
    put_task(&a, "a1", "A");
    tick();
    put_task(&b, "b1", "B");
    round(&a, Flavor::Gitee, &base, &mut sa).unwrap(); // POST (create)
    round(&b, Flavor::Gitee, &base, &mut sb).unwrap(); // PUT with sha
    round(&a, Flavor::Gitee, &base, &mut sa).unwrap();
    assert_eq!(titles(&a), ["A", "B"]);
    assert!(!round(&a, Flavor::Gitee, &base, &mut sa).unwrap().uploaded);

    r.lock().unwrap().gitee_error = true;
    tick();
    put_task(&a, "a2", "A2");
    let e = round(&a, Flavor::Gitee, &base, &mut sa).unwrap_err();
    assert!(!e.message.contains(TOKEN), "token leaked: {}", e.message);
    assert!(e.message.contains("***"), "server message kept, token masked: {}", e.message);
}

/// Against the real GitHub API, in a private test repository:
/// FLUENTCAL_GH_TOKEN=… FLUENTCAL_GH_REPO=owner/name cargo test --test cloud_git -- --ignored
#[test]
#[ignore]
fn real_github() {
    use fluentcal_lib::cloud::{Fetched, Remote};
    let token = std::env::var("FLUENTCAL_GH_TOKEN").expect("FLUENTCAL_GH_TOKEN");
    let repo_name = std::env::var("FLUENTCAL_GH_REPO").expect("FLUENTCAL_GH_REPO");
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    let path = format!("runs/{ms}.json");
    let make = |repo: &str| Git::new(Http::new("", Duration::from_secs(30)).unwrap(), Flavor::GitHub, "https://api.github.com", repo, "", &path, &token).unwrap();
    let pause = || std::thread::sleep(Duration::from_millis(1500));
    let (a, b) = (store("rgh-a"), store("rgh-b"));
    let (mut sa, mut sb) = (SyncState::default(), SyncState::default());
    put_task(&a, "a1", "A");
    tick();
    put_task(&b, "b1", "B");
    assert!(cycle(&a, &mut make(&repo_name), &mut sa, Mode::Normal).unwrap().uploaded);
    pause();
    cycle(&b, &mut make(&repo_name), &mut sb, Mode::Normal).unwrap();
    pause();
    cycle(&a, &mut make(&repo_name), &mut sa, Mode::Normal).unwrap();
    assert_eq!(titles(&a), ["A", "B"]);
    assert_eq!(titles(&b), ["A", "B"]);
    pause();
    let mut r = make(&repo_name);
    let out = cycle(&a, &mut r, &mut sa, Mode::Normal).unwrap();
    assert!(!out.uploaded && !out.downloaded, "unchanged: no commit");

    // Someone commits after our fetch: an upload with the old sha is refused.
    let Fetched::File(_, rev) = r.fetch(None).unwrap() else { panic!("file expected") };
    let mut other = make(&repo_name);
    other.check().unwrap();
    other.put(&fluentcal_lib::cloud::encode(&cloud_view(&b.snapshot()), 1), Some(&rev)).unwrap();
    pause();
    assert_eq!(r.put(b"{}", Some(&rev)).unwrap_err().kind, ErrKind::Conflict);

    // A public repository is refused before anything is written.
    let e = cycle(&a, &mut make("Lianzy-Baimiao/FluentCal"), &mut SyncState::default(), Mode::Normal).unwrap_err();
    assert_eq!(e.kind, ErrKind::Config, "{}", e.message);
}
