//! Web view (WEB, 需求文档 4.10 / 7.6): a small read-only HTTP server for phones and other
//! computers in the local network. Serves the bundled frontend in web mode (`?w=web`) and the
//! shared data at /api/doc behind an access code. No Tauri here (see webcmd.rs), so tests can
//! run it with fake assets.

use crate::store::{signature, Store};
use serde_json::{json, Value};
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Method, Request, Response, Server};

const WORKERS: usize = 4;
const CSP: &str = "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

pub struct Asset {
    pub bytes: Vec<u8>,
    pub mime: String,
}

pub struct Ctx {
    pub store: Arc<Store>,
    /// Access code; replaced when the user asks for a new one.
    pub token: Arc<Mutex<String>>,
    /// Bundled frontend files by path ("/assets/x.js").
    pub asset: Box<dyn Fn(&str) -> Option<Asset> + Send + Sync>,
    /// Windows accent colour (#RRGGBB), for theme.accent = "system".
    pub accent: Box<dyn Fn() -> String + Send + Sync>,
    pub pc: String,
}

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    fn new(status: u16) -> Reply {
        Reply { status, headers: Vec::new(), body: Vec::new() }
    }
    fn json(status: u16, v: &Value) -> Reply {
        let mut r = Reply::new(status);
        r.headers.push(("Content-Type", "application/json; charset=utf-8".into()));
        r.headers.push(("Cache-Control", "no-store".into()));
        r.body = v.to_string().into_bytes();
        r
    }
}

/// Where the server listens. `lan`: the setting "other devices may view" (all interfaces,
/// Windows Firewall asks once). `local`: this PC's own browser asked for the page, so loopback
/// only, which reaches no other device and needs no firewall rule. Neither: no server at all.
pub fn bind_host(lan: bool, local: bool) -> Option<&'static str> {
    match (lan, local) {
        (true, _) => Some("0.0.0.0"),
        (false, true) => Some("127.0.0.1"),
        (false, false) => None,
    }
}

/// Local network and this PC only.
pub fn allowed_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => v.is_loopback() || v.is_private() || v.is_link_local(),
        IpAddr::V6(v) => v.is_loopback() || v.to_ipv4_mapped().is_some_and(|m| m.is_loopback() || m.is_private()),
    }
}

/// What the page may see: tasks, groups, day colours, holidays and display settings. No sync
/// configuration, window position, shortcuts or reminder settings.
pub fn public_view(doc: &Value, accent: &str, pc: &str) -> Value {
    let pick = |k: &str| doc.get(k).cloned().unwrap_or_else(|| json!({}));
    let s = doc.get("settings").cloned().unwrap_or(Value::Null);
    let mut settings = json!({});
    for k in ["calendar", "tasks", "gantt"] {
        if let Some(v) = s.get(k) {
            settings[k] = v.clone();
        }
    }
    settings["theme"] = json!({ "mode": s.pointer("/theme/mode").cloned().unwrap_or(json!("system")), "accent": s.pointer("/theme/accent").cloned().unwrap_or(json!("system")) });
    let doc = json!({ "version": 1, "tasks": pick("tasks"), "groups": pick("groups"), "dayMarks": pick("dayMarks"), "holidays": pick("holidays"), "settings": settings });
    json!({ "doc": doc, "accent": accent, "pc": pc })
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn handle(ctx: &Ctx, method: &str, url: &str, auth: Option<&str>, if_none_match: Option<&str>) -> Reply {
    if method != "GET" && method != "HEAD" {
        return Reply::new(405);
    }
    let path = url.split(['?', '#']).next().unwrap_or("/");
    if path == "/" && !url.contains("w=web") {
        let mut r = Reply::new(302);
        r.headers.push(("Location", "/?w=web".into()));
        return r;
    }
    if path == "/api/doc" {
        let token = ctx.token.lock().unwrap().clone();
        let given = auth.and_then(|a| a.strip_prefix("Bearer ")).unwrap_or("");
        if token.is_empty() || !same(given.trim(), &token) {
            return Reply::json(401, &json!({ "error": "访问码无效" }));
        }
        let view = public_view(&ctx.store.snapshot(), &(ctx.accent)(), &ctx.pc);
        let mut body = Reply::json(200, &view);
        let etag = format!("\"{:x}\"", signature(&body.body));
        if if_none_match == Some(etag.as_str()) {
            let mut r = Reply::new(304);
            r.headers.push(("ETag", etag));
            return r;
        }
        body.headers.push(("ETag", etag));
        return body;
    }
    let decoded = crate::cloud::http::dec(path);
    if decoded.contains("..") || decoded.contains('\\') || decoded.contains('\0') {
        return Reply::new(404);
    }
    let Some(a) = (ctx.asset)(if path == "/" { "/index.html" } else { path }) else { return Reply::new(404) };
    let html = a.mime.starts_with("text/html");
    // The asset resolver answers index.html for unknown paths; a missing script must be a 404.
    if html && path != "/" && !path.ends_with(".html") {
        return Reply::new(404);
    }
    let mut r = Reply::new(200);
    r.headers.push(("Content-Type", a.mime));
    if html {
        r.headers.push(("Content-Security-Policy", CSP.into()));
        r.headers.push(("Cache-Control", "no-cache".into()));
    } else if path.starts_with("/assets/") {
        r.headers.push(("Cache-Control", "public, max-age=31536000, immutable".into())); // hashed names
    }
    r.body = a.bytes;
    r
}

fn serve(ctx: &Ctx, req: Request) {
    let peer_ok = req.remote_addr().is_some_and(|a: &SocketAddr| allowed_peer(a.ip()));
    let header = |name: &'static str| req.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_owned());
    let reply = if peer_ok {
        let method = match req.method() {
            Method::Get => "GET",
            Method::Head => "HEAD",
            _ => "OTHER",
        };
        handle(ctx, method, req.url(), header("Authorization").as_deref(), header("If-None-Match").as_deref())
    } else {
        Reply::new(403)
    };
    let len = reply.body.len();
    let mut res = Response::new(reply.status.into(), Vec::new(), Cursor::new(reply.body), Some(len), None);
    let common = [("X-Content-Type-Options", "nosniff"), ("Referrer-Policy", "no-referrer"), ("X-Frame-Options", "DENY")];
    for (k, v) in common.iter().map(|(k, v)| (*k, v.to_string())).chain(reply.headers) {
        if let Ok(h) = Header::from_bytes(k.as_bytes(), v.as_bytes()) {
            res = res.with_header(h);
        }
    }
    let _ = req.respond(res);
}

/// The running server; dropping it stops it and frees the port.
pub struct Running {
    server: Option<Arc<Server>>,
    stop: Arc<AtomicBool>,
    pub addr: SocketAddr,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let Some(server) = self.server.take() else { return };
        for _ in 0..WORKERS {
            server.unblock(); // one per worker
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while Arc::strong_count(&server) > 1 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        drop(server);
        // tiny_http wakes its accept thread by connecting to the listening address, which
        // fails for 0.0.0.0 on Windows; without this the port would stay taken.
        let _ = std::net::TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], self.addr.port())), std::time::Duration::from_millis(300));
    }
}

pub fn start(ctx: Arc<Ctx>, bind: &str) -> Result<Running, String> {
    let server = Arc::new(Server::http(bind).map_err(|e| {
        let text = e.to_string();
        if text.contains("10048") || text.to_lowercase().contains("in use") {
            "端口已被其他程序占用，请换一个端口".to_owned()
        } else {
            format!("无法启动网页服务：{text}")
        }
    })?);
    let addr = server.server_addr().to_ip().ok_or("无法启动网页服务")?;
    let stop = Arc::new(AtomicBool::new(false));
    for i in 0..WORKERS {
        let (server, stop, ctx) = (server.clone(), stop.clone(), ctx.clone());
        let _ = std::thread::Builder::new().name(format!("web-{i}")).spawn(move || {
            while let Ok(req) = server.recv() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                serve(&ctx, req);
            }
        });
    }
    Ok(Running { server: Some(server), stop, addr })
}
