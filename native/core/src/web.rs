//! Web view (WEB, 需求文档 4.10 / 7.6): a small read-only HTTP server for phones and other
//! computers in the local network. Serves the bundled frontend in web mode (`?w=web`) and the
//! shared data at /api/doc behind an access code. No Tauri here (see webcmd.rs), so tests can
//! run it with fake assets.

use crate::i18n::{tr, trf};

use crate::store::{signature, Store};
use serde_json::{json, Value};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

pub use crate::httpd::Running;
use crate::httpd::{listen, Limits, ListenError, Request, Response};

/// A few devices, each with the handful of connections a browser opens.
const CONNECTIONS: usize = 64;
/// The page only asks (GET): a request has no body worth reading.
const MAX_BODY: usize = 8 * 1024;
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

/// Compares in the same time whatever the two have in common, so that the time taken tells nothing about
/// the token.
pub(crate) fn same(a: &str, b: &str) -> bool {
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
            return Reply::json(401, &json!({ "error": tr("访问码无效") }));
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

fn serve(ctx: &Ctx, req: &Request) -> Response {
    let reply = if allowed_peer(req.peer.ip()) {
        handle(ctx, &req.method, &req.url, req.header("Authorization"), req.header("If-None-Match"))
    } else {
        Reply::new(403)
    };
    let common = [("X-Content-Type-Options", "nosniff"), ("Referrer-Policy", "no-referrer"), ("X-Frame-Options", "DENY")];
    let mut res = Response::new(reply.status);
    res.headers = common.iter().map(|(k, v)| (*k, v.to_string())).chain(reply.headers).collect();
    res.body = reply.body;
    res
}

/// Starts the server (httpd.rs) on `bind`; dropping what is returned stops it and frees the
/// port.
pub fn start(ctx: Arc<Ctx>, bind: &str) -> Result<Running, String> {
    listen(bind, "web", Limits::new(MAX_BODY, CONNECTIONS), move |req| serve(&ctx, req)).map_err(|e| match e {
        ListenError::InUse => tr("端口已被其他程序占用，请换一个端口").to_owned(),
        ListenError::Other(text) if text.is_empty() => tr("无法启动网页服务").to_owned(),
        ListenError::Other(text) => trf("无法启动网页服务：{0}", &[text]),
    })
}
