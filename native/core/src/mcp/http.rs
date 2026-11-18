//! The endpoint assistants connect to: MCP over Streamable HTTP, on 127.0.0.1 only, behind an
//! access token. One POST carries one message and gets one JSON answer; there are no streams
//! and no sessions, which the 2026-07-28 revision does not have and the older ones leave to
//! the server.
//!
//! Who gets in is decided before the message is looked at (`admit`): only this PC (the peer,
//! and the name it was called by: a page elsewhere can make a browser call 127.0.0.1 under its
//! own name, "DNS rebinding"), no web page from elsewhere (`Origin`), and only with the token.

use super::proto::{self, Outcome};
use super::{Host, PATH};
use crate::httpd::{listen, Limits, ListenError, Request, Response, Running};
use crate::i18n::{tr, trf};
use crate::web::same;
use base64::Engine as _;
use serde_json::{json, Value};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

/// An assistant asks one thing at a time, on one connection or on a new one each time; this
/// leaves room for a few of them.
const CONNECTIONS: usize = 16;
/// A message is a few kilobytes; fifty tasks with long notes stay far below this. A longer
/// one is refused by the server (httpd.rs) before any of it is read.
pub const MAX_BODY: usize = 1 << 20;

pub struct Ctx {
    pub host: Arc<dyn Host>,
    /// The access token; replaced when the user asks for a new one. Empty: nobody gets in.
    pub token: Arc<Mutex<String>>,
}

fn reply(status: u16, body: Vec<u8>) -> Response {
    let mut res = Response::new(status);
    res.headers = vec![("Content-Type", "application/json".into()), ("Cache-Control", "no-store".into()), ("X-Content-Type-Options", "nosniff".into())];
    res.body = body;
    res
}

/// Not an answer to a message: who or what was wrong with the request itself.
fn refuse(status: u16, why: &str) -> Response {
    reply(status, json!({ "error": why }).to_string().into_bytes())
}

fn rpc(status: u16, body: &Value) -> Response {
    reply(status, body.to_string().into_bytes())
}

/// What this PC is called from itself: "localhost", 127.0.0.1, [::1], with or without a port.
fn local_name(host: &str) -> bool {
    let h = host.trim();
    let name = match h.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => match h.rsplit_once(':') {
            Some((name, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => name,
            _ => h,
        },
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// Programs send no `Origin`; browsers send the page's. A page of this PC may ask (it still
/// needs the token); a page from anywhere else, or from nowhere ("null"), may not. Schemes
/// other than http(s) are programs with a web view (vscode-webview://…), not pages.
fn origin_allowed(origin: &str) -> bool {
    let Some((scheme, rest)) = origin.trim().split_once("://") else { return false };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return true;
    }
    local_name(rest.split('/').next().unwrap_or(""))
}

/// The token of `Authorization: Bearer <token>`.
fn bearer(auth: &str) -> Option<&str> {
    let (scheme, token) = auth.trim().split_once(' ')?;
    scheme.eq_ignore_ascii_case("Bearer").then(|| token.trim())
}

/// Decides whether a request is looked at at all.
pub fn admit(ctx: &Ctx, req: &Request) -> Result<(), Response> {
    let loopback = match req.peer.ip() {
        IpAddr::V4(v) => v.is_loopback(),
        IpAddr::V6(v) => v.is_loopback() || v.to_ipv4_mapped().is_some_and(|m| m.is_loopback()),
    };
    if !loopback {
        return Err(refuse(403, "only this PC"));
    }
    if req.header("Host").is_some_and(|h| !local_name(h)) {
        return Err(refuse(403, "unexpected Host"));
    }
    if req.header("Origin").is_some_and(|o| !origin_allowed(o)) {
        return Err(refuse(403, "this origin may not connect"));
    }
    if req.url.split(['?', '#']).next().unwrap_or("") != PATH {
        return Err(refuse(404, "the MCP endpoint is /mcp"));
    }
    let token = ctx.token.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if token.is_empty() || !req.header("Authorization").and_then(bearer).is_some_and(|given| same(given, &token)) {
        let mut res = refuse(401, "the access token is missing or wrong: send \"Authorization: Bearer <token>\" (FluentCal shows the token in its settings)");
        res.headers.push(("WWW-Authenticate", "Bearer realm=\"FluentCal\"".into()));
        return Err(res);
    }
    if req.method != "POST" {
        // no stream to listen on (GET) and no session to end (DELETE)
        let mut res = refuse(405, "POST only");
        res.headers.push(("Allow", "POST".into()));
        return Err(res);
    }
    Ok(())
}

/// A header value as the client meant it: `=?base64?…?=` wraps what plain ASCII cannot carry.
fn decoded(value: &str) -> Option<String> {
    match value.strip_prefix("=?base64?").and_then(|v| v.strip_suffix("?=")) {
        Some(b64) => String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b64).ok()?).ok(),
        None => Some(value.to_owned()),
    }
}

fn mismatch(id: &Value, what: String) -> Response {
    rpc(400, &proto::error_body(id, proto::HEADER_MISMATCH, &format!("Header mismatch: {what}"), None))
}

/// The modern versions repeat parts of the message in headers (for whatever sits between
/// client and server), and what the headers say must be what the message says.
fn check_headers(msg: &Value, req: &Request) -> Result<(), Response> {
    let (Some(method), Some(id)) = (msg.get("method").and_then(Value::as_str), msg.get("id").filter(|v| v.is_string() || v.is_number())) else {
        return Ok(()); // not a request: proto.rs says what it is
    };
    let said = req.header("MCP-Protocol-Version").map(str::trim).filter(|v| !v.is_empty());
    match proto::meta_version(msg) {
        Some(version) if proto::is_modern(version) => {
            match said {
                Some(h) if h == version => {}
                Some(h) => return Err(mismatch(id, format!("MCP-Protocol-Version header value '{h}' does not match body value '{version}'"))),
                None => return Err(mismatch(id, "the MCP-Protocol-Version header is missing".into())),
            }
            match req.header("Mcp-Method").map(str::trim) {
                Some(h) if h == method => {}
                Some(h) => return Err(mismatch(id, format!("Mcp-Method header value '{h}' does not match body value '{method}'"))),
                None => return Err(mismatch(id, "the Mcp-Method header is missing".into())),
            }
            // a call without a name is answered by proto.rs
            if let Some(name) = msg.pointer("/params/name").and_then(Value::as_str).filter(|_| method == "tools/call") {
                match req.header("Mcp-Name").map(|h| decoded(h.trim())) {
                    Some(Some(h)) if h == name => {}
                    Some(Some(h)) => return Err(mismatch(id, format!("Mcp-Name header value '{h}' does not match body value '{name}'"))),
                    Some(None) => return Err(mismatch(id, "the Mcp-Name header is not valid Base64".into())),
                    None => return Err(mismatch(id, "the Mcp-Name header is missing".into())),
                }
            }
        }
        // a version this server does not speak: proto.rs answers with the ones it does
        Some(_) => {}
        None if method != "initialize" => match said {
            Some(h) if proto::is_modern(h) => {
                let text = format!("Invalid params: _meta[\"{}\"] is required (the MCP-Protocol-Version header says {h})", proto::META_VERSION);
                return Err(rpc(400, &proto::error_body(id, proto::INVALID_PARAMS, &text, None)));
            }
            Some(h) if !proto::LEGACY.contains(&h) => match proto::unsupported_version(id, h) {
                Outcome::Reply { body, status } => return Err(rpc(status, &body)),
                Outcome::Accepted => {}
            },
            // an older client after its handshake (those before 2025-06-18 send no header)
            _ => {}
        },
        None => {}
    }
    Ok(())
}

/// The answer to the message of a request that was admitted.
pub fn answer(ctx: &Ctx, req: &Request) -> Response {
    let Ok(msg) = serde_json::from_slice::<Value>(&req.body) else {
        return rpc(400, &proto::error_body(&Value::Null, proto::PARSE_ERROR, "Parse error: the body is not JSON", None));
    };
    if let Err(res) = check_headers(&msg, req) {
        return res;
    }
    // A slip in a tool must not take the calendar down with it, and the assistant is told in
    // the words of the protocol.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| proto::handle(ctx.host.as_ref(), &msg)));
    match outcome {
        Ok(Outcome::Reply { body, status }) => rpc(status, &body),
        Ok(Outcome::Accepted) => reply(202, Vec::new()),
        Err(_) => {
            let id = msg.get("id").filter(|v| v.is_string() || v.is_number()).cloned().unwrap_or(Value::Null);
            rpc(500, &proto::error_body(&id, proto::INTERNAL_ERROR, "Internal error", None))
        }
    }
}

fn serve(ctx: &Ctx, req: &Request) -> Response {
    match admit(ctx, req) {
        Ok(()) => answer(ctx, req),
        Err(res) => res,
    }
}

/// Starts the endpoint on 127.0.0.1:`port` (0: any free port, for the tests). Dropping what
/// is returned stops it and frees the port.
pub fn start(ctx: Arc<Ctx>, port: u16) -> Result<Running, String> {
    listen(&format!("127.0.0.1:{port}"), "mcp", Limits::new(MAX_BODY, CONNECTIONS), move |req| serve(&ctx, req)).map_err(|e| match e {
        ListenError::InUse => tr("端口已被其他程序占用，请换一个端口").to_owned(),
        ListenError::Other(text) if text.is_empty() => tr("无法启动 AI 助手服务").to_owned(),
        ListenError::Other(text) => trf("无法启动 AI 助手服务：{0}", &[text]),
    })
}
