//! The web view's HTTP server (web.rs) on a loopback port with fake assets.
mod common;

use common::*;
use fluentcal_lib::web::{allowed_peer, bind_host, start, Asset, Ctx};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";

/// Like Tauri's asset resolver: unknown paths fall back to index.html.
fn asset(path: &str) -> Option<Asset> {
    let html = Asset { bytes: b"<!doctype html><title>FluentCal</title>".to_vec(), mime: "text/html".into() };
    Some(match path {
        "/assets/app.js" => Asset { bytes: b"console.log(1)".to_vec(), mime: "text/javascript".into() },
        _ => html,
    })
}

struct Got {
    status: u16,
    headers: ureq::http::HeaderMap,
    body: String,
}

fn get(method: &str, url: &str, headers: &[(&str, &str)]) -> Got {
    let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).max_redirects(0).build().into();
    let mut b = ureq::http::Request::builder().method(method).uri(url);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let mut res = agent.run(b.body(()).unwrap()).unwrap();
    let body = res.body_mut().read_to_string().unwrap_or_default();
    Got { status: res.status().as_u16(), headers: res.headers().clone(), body }
}

#[test]
fn serves_the_page_and_guards_the_data() {
    let s = store("web");
    put_task(&s, "t1", "在网页上看");
    s.patch_settings(&json!({ "theme": { "mode": "dark" }, "sync": { "provider": "webdav", "webdav": { "user": "me@example.com" } }, "desk": { "bounds": { "x": 1 } } }));
    let ctx = Arc::new(Ctx { store: s.clone(), token: Arc::new(Mutex::new(TOKEN.into())), asset: Box::new(asset), accent: Box::new(|| "#8764B8".into()), pc: "TEST-PC".into() });
    let running = start(ctx, "127.0.0.1:0").unwrap();
    let base = format!("http://{}", running.addr);
    let bearer = format!("Bearer {TOKEN}");

    // The data needs the access code.
    assert_eq!(get("GET", &format!("{base}/api/doc"), &[]).status, 401);
    assert_eq!(get("GET", &format!("{base}/api/doc"), &[("Authorization", "Bearer wrong")]).status, 401);
    let ok = get("GET", &format!("{base}/api/doc"), &[("Authorization", &bearer)]);
    assert_eq!(ok.status, 200);
    let v: Value = serde_json::from_str(&ok.body).unwrap();
    assert_eq!(v["doc"]["tasks"]["t1"]["title"], "在网页上看");
    assert_eq!(v["doc"]["settings"]["theme"]["mode"], "dark");
    assert_eq!((v["accent"].as_str(), v["pc"].as_str()), (Some("#8764B8"), Some("TEST-PC")));
    for secret in ["sync", "desk", "shortcuts", "general"] {
        assert!(v["doc"]["settings"].get(secret).is_none(), "{secret} must not be served");
    }
    assert!(!ok.body.contains("me@example.com") && v["doc"].get("meta").is_none());
    assert_eq!(ok.headers.get("cache-control").unwrap(), "no-store");

    // Unchanged: 304; changed: a new ETag.
    let etag = ok.headers.get("etag").unwrap().to_str().unwrap().to_owned();
    assert_eq!(get("GET", &format!("{base}/api/doc"), &[("Authorization", &bearer), ("If-None-Match", &etag)]).status, 304);
    put_task(&s, "t2", "新任务");
    let changed = get("GET", &format!("{base}/api/doc"), &[("Authorization", &bearer), ("If-None-Match", &etag)]);
    assert_eq!(changed.status, 200);
    assert!(changed.body.contains("新任务"));

    // The page: redirect to web mode, strict headers; missing files are 404s.
    let root = get("GET", &format!("{base}/"), &[]);
    assert_eq!((root.status, root.headers.get("location").unwrap().to_str().unwrap()), (302, "/?w=web"));
    let page = get("GET", &format!("{base}/?w=web"), &[]);
    assert_eq!(page.status, 200);
    let csp = page.headers.get("content-security-policy").unwrap().to_str().unwrap();
    assert!(csp.contains("default-src 'self'") && csp.contains("frame-ancestors 'none'"));
    assert_eq!(page.headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(page.headers.get("x-frame-options").unwrap(), "DENY");
    let js = get("GET", &format!("{base}/assets/app.js"), &[]);
    assert_eq!((js.status, js.headers.get("content-type").unwrap().to_str().unwrap()), (200, "text/javascript"));
    assert_eq!(get("GET", &format!("{base}/assets/missing.js"), &[]).status, 404, "no index.html for a missing script");
    assert_eq!(get("GET", &format!("{base}/..%2F..%2Fdata.json"), &[]).status, 404);
    assert_eq!(get("GET", &format!("{base}/assets/../../data.json"), &[]).status, 404);
    assert_eq!(get("GET", &format!("{base}/%2E%2E/%2e%2e/data.json"), &[]).status, 404);
    assert_eq!(get("POST", &format!("{base}/api/doc"), &[("Authorization", &bearer)]).status, 405);

    // Stopping frees the port: the same port can be used again at once.
    let addr = running.addr;
    drop(running);
    let again = start(Arc::new(Ctx { store: s, token: Arc::new(Mutex::new(TOKEN.into())), asset: Box::new(asset), accent: Box::new(|| "#000000".into()), pc: "X".into() }), &addr.to_string());
    assert!(again.is_ok(), "port still taken: {:?}", again.err());
}

/// A page polling over one kept-alive connection: 200, then 304s. Every reply says where it
/// ends (Content-Length, no chunked framing on the 304), and the next request on the same
/// connection is answered. With `Transfer-Encoding: chunked` on the 304 (tiny_http's choice
/// when the length isn't given), Edge left every other poll hanging.
#[test]
fn polling_on_one_connection() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::time::Duration;
    let s = store("web-poll");
    put_task(&s, "t1", "x");
    let ctx = Arc::new(Ctx { store: s, token: Arc::new(Mutex::new(TOKEN.into())), asset: Box::new(asset), accent: Box::new(|| "#000000".into()), pc: "X".into() });
    let running = start(ctx, "127.0.0.1:0").unwrap();
    let conn = std::net::TcpStream::connect(running.addr).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(3))).unwrap(); // no answer: fail, don't hang
    let mut w = conn.try_clone().unwrap();
    let mut r = BufReader::new(conn);
    let mut etag = String::new();
    for i in 0..4 {
        let inm = if etag.is_empty() { String::new() } else { format!("If-None-Match: {etag}\r\n") };
        write!(w, "GET /api/doc HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {TOKEN}\r\n{inm}\r\n").unwrap();
        let (mut status, mut len, mut chunked) = (String::new(), None::<usize>, false);
        r.read_line(&mut status).unwrap();
        loop {
            let mut line = String::new();
            r.read_line(&mut line).unwrap();
            let Some((k, v)) = line.trim_end().split_once(':') else { break };
            match k.to_ascii_lowercase().as_str() {
                "content-length" => len = v.trim().parse().ok(),
                "transfer-encoding" => chunked = true,
                "etag" => etag = v.trim().to_owned(),
                _ => {}
            }
        }
        let want = if i == 0 { "200" } else { "304" };
        assert_eq!(status.split(' ').nth(1), Some(want), "request {i}: {status}");
        assert!(!chunked, "request {i}: chunked framing");
        let len = len.unwrap_or_else(|| panic!("request {i}: no Content-Length"));
        assert!(i == 0 || len == 0, "request {i}: a 304 with a body");
        r.by_ref().take(len as u64).read_to_end(&mut Vec::new()).unwrap();
    }
}

#[test]
fn listens_only_where_asked() {
    assert_eq!(bind_host(false, false), None, "nothing asked for: no server");
    assert_eq!(bind_host(false, true), Some("127.0.0.1"), "this PC's browser only: loopback");
    assert_eq!(bind_host(true, false), Some("0.0.0.0"));
    assert_eq!(bind_host(true, true), Some("0.0.0.0"), "all interfaces include loopback");
}

#[test]
fn only_local_network_peers() {
    for ok in ["127.0.0.1", "192.168.1.20", "10.0.0.5", "172.16.3.4", "169.254.1.1", "::1", "::ffff:192.168.1.2"] {
        assert!(allowed_peer(ok.parse().unwrap()), "{ok}");
    }
    for no in ["8.8.8.8", "203.0.113.9", "172.32.0.1", "2001:db8::1"] {
        assert!(!allowed_peer(no.parse().unwrap()), "{no}");
    }
}
