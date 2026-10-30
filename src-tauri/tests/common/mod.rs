//! Test helpers: a minimal HTTP/1.1 server on 127.0.0.1 (one request per connection) that
//! stands in for WebDAV, GitHub, Gitee and Google, plus store helpers.
#![allow(dead_code)]

use fluentcal_lib::store::Store;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Req {
    pub method: String,
    /// Path and query.
    pub target: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Req {
    pub fn header(&self, k: &str) -> Option<&str> {
        self.headers.get(&k.to_ascii_lowercase()).map(String::as_str)
    }
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or("")
    }
    pub fn query(&self, k: &str) -> Option<String> {
        let q = self.target.split_once('?')?.1;
        q.split('&').filter_map(|p| p.split_once('=')).find(|(n, _)| *n == k).map(|(_, v)| v.to_owned())
    }
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

pub struct Res {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Res {
    pub fn new(status: u16) -> Res {
        Res { status, headers: Vec::new(), body: Vec::new() }
    }
    pub fn json(status: u16, v: Value) -> Res {
        Res { status, headers: vec![("Content-Type".into(), "application/json".into())], body: v.to_string().into_bytes() }
    }
    pub fn with(mut self, k: &str, v: &str) -> Res {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn body(mut self, b: Vec<u8>) -> Res {
        self.body = b;
        self
    }
}

pub type Handler = Arc<Mutex<dyn FnMut(&Req) -> Res + Send>>;

/// Starts the server; returns its base URL (http://127.0.0.1:port) and a log of
/// "METHOD path" lines.
pub fn serve(handler: impl FnMut(&Req) -> Res + Send + 'static) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handler: Handler = Arc::new(Mutex::new(handler));
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let Some(req) = read_req(&mut stream) else { continue };
            log2.lock().unwrap().push(format!("{} {}", req.method, req.path()));
            let res = (handler.lock().unwrap())(&req);
            let mut head = format!("HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n", res.status, res.body.len());
            for (k, v) in &res.headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&res.body);
        }
    });
    (base, log)
}

fn read_req(stream: &mut std::net::TcpStream) -> Option<Req> {
    let mut r = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next()?.to_owned(), parts.next()?.to_owned());
    let mut headers = HashMap::new();
    loop {
        let mut h = String::new();
        r.read_line(&mut h).ok()?;
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_owned());
        }
    }
    let mut body = Vec::new();
    if let Some(n) = headers.get("content-length").and_then(|v| v.parse::<usize>().ok()) {
        body.resize(n, 0);
        r.read_exact(&mut body).ok()?;
    } else if headers.get("transfer-encoding").is_some_and(|v| v.contains("chunked")) {
        loop {
            let mut size = String::new();
            r.read_line(&mut size).ok()?;
            let n = usize::from_str_radix(size.trim(), 16).ok()?;
            let mut chunk = vec![0; n + 2];
            r.read_exact(&mut chunk).ok()?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..n]);
        }
    }
    Some(Req { method, target, headers, body })
}

/// A fresh store in its own temp folder.
pub fn store(name: &str) -> Arc<Store> {
    let dir: PathBuf = std::env::temp_dir().join(format!("fluentcal-cloud-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    Store::open(dir).unwrap()
}

pub fn put_task(s: &Store, id: &str, title: &str) {
    let mut m = Map::new();
    m.insert(id.into(), json!({ "id": id, "title": title, "start": "2026-10-14", "end": "2026-10-14" }));
    s.put("tasks", &m);
}

pub fn delete_task(s: &Store, id: &str) {
    let mut m = Map::new();
    m.insert(id.into(), Value::Null);
    s.put("tasks", &m);
}

pub fn titles(s: &Store) -> Vec<String> {
    let mut t: Vec<String> = s.snapshot()["tasks"].as_object().unwrap().values().map(|v| v["title"].as_str().unwrap_or("").to_owned()).collect();
    t.sort();
    t
}

/// Edits on two "PCs" in the same millisecond would tie.
pub fn tick() {
    std::thread::sleep(std::time::Duration::from_millis(3));
}
