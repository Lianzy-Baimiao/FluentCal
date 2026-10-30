//! Cloud file format, content signatures, and the Google OAuth loopback login (PKCE).
mod common;

use base64::Engine as _;
use common::*;
use fluentcal_lib::cloud::http::{dec, Http};
use fluentcal_lib::cloud::oauth::{login, Endpoints};
use fluentcal_lib::cloud::{cloud_view, decode, encode, view_sig, ErrKind};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

/// A pretend browser: opens the redirect with `query` appended.
fn browser(url: &str, query: impl Fn(&str) -> String + Send + 'static) {
    let param = |k: &str| url.split_once('?').unwrap().1.split('&').find_map(|p| p.strip_prefix(&format!("{k}="))).map(dec).unwrap();
    let redirect = param("redirect_uri");
    let state = param("state");
    std::thread::spawn(move || {
        let host = redirect.trim_start_matches("http://");
        std::thread::sleep(Duration::from_millis(100));
        let mut s = std::net::TcpStream::connect(host).unwrap();
        // A stray request first, like a browser asking for the favicon.
        let _ = s.write_all(b"GET /favicon.ico HTTP/1.1\r\n\r\n");
        drop(s);
        let mut s = std::net::TcpStream::connect(host).unwrap();
        s.write_all(format!("GET /?{} HTTP/1.1\r\nHost: {host}\r\n\r\n", query(&state)).as_bytes()).unwrap();
        let mut page = String::new();
        let _ = s.read_to_string(&mut page);
    });
}

#[test]
fn google_login_with_pkce_and_loopback_redirect() {
    let challenge = Arc::new(Mutex::new(String::new()));
    let c2 = challenge.clone();
    let (base, _) = serve(move |req| match req.path() {
        "/token" => {
            let form = String::from_utf8_lossy(&req.body).into_owned();
            let field = |k: &str| form.split('&').find_map(|p| p.strip_prefix(&format!("{k}="))).map(dec).unwrap_or_default();
            let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(field("code_verifier").as_bytes()));
            if field("code") != "the-code" || expected != *c2.lock().unwrap() || field("client_secret") != "sec" {
                return Res::json(400, json!({ "error": "invalid_grant" }));
            }
            Res::json(200, json!({ "access_token": "at", "refresh_token": "rt", "expires_in": 3600 }))
        }
        "/drive/v3/about" => Res::json(200, json!({ "user": { "emailAddress": "me@example.com" } })),
        _ => Res::new(404),
    });
    let ep = Endpoints { auth: format!("{base}/auth"), token: format!("{base}/token"), api: base.clone(), upload: base.clone() };
    let http = Http::new("direct", Duration::from_secs(10)).unwrap();
    let c3 = challenge.clone();
    let open = move |url: &str| {
        let ch = url.split("code_challenge=").nth(1).unwrap().split('&').next().unwrap();
        *c3.lock().unwrap() = ch.to_owned();
        browser(url, |state| format!("code=the-code&state={state}"));
        Ok(())
    };
    let l = login(&http, &ep, "cid", "sec", &open, Duration::from_secs(10)).unwrap();
    assert_eq!(l.refresh_token, "rt");
    assert_eq!(l.email.as_deref(), Some("me@example.com"));

    // Cancelled in the browser, a wrong state, and no answer at all.
    let cancel = |url: &str| {
        browser(url, |state| format!("error=access_denied&state={state}"));
        Ok(())
    };
    assert_eq!(login(&http, &ep, "cid", "sec", &cancel, Duration::from_secs(10)).err().map(|e| e.kind), Some(ErrKind::Auth));
    let forged = |url: &str| {
        browser(url, |_| "code=the-code&state=forged".into());
        Ok(())
    };
    assert!(login(&http, &ep, "cid", "sec", &forged, Duration::from_secs(10)).is_err());
    let silent = |_: &str| Ok(());
    let e = login(&http, &ep, "cid", "sec", &silent, Duration::from_millis(400)).err().unwrap();
    assert!(e.message.contains("超时"));
}
