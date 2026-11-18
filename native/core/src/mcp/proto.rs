//! The Model Context Protocol itself: one JSON-RPC message in, an answer out. Nothing here
//! knows how the message travelled (http.rs, bridge.rs) and nothing is remembered between
//! messages.
//!
//! Two generations of clients are served side by side (the specification's "dual-era" server):
//! - modern, 2026-07-28: every request says its protocol version and the client's
//!   capabilities in `params._meta`; there is no handshake. `server/discover` tells what the
//!   server offers.
//! - legacy, 2025-11-25 and earlier: the client opens with `initialize` and may `ping`.
//!
//! A request with the modern `_meta` is answered the modern way, everything else the legacy
//! way. The only difference in the answers: modern results carry `resultType` and who
//! answered, and the modern rules say which HTTP status goes with which error.

use super::{tools, Host};
use crate::i18n::tr;
use serde_json::{json, Map, Value};

/// Stateless versions.
pub const MODERN: [&str; 1] = ["2026-07-28"];
/// Versions with the `initialize` handshake, newest first.
pub const LEGACY: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

pub const SERVER_NAME: &str = "fluentcal";
pub const SERVER_TITLE: &str = "FluentCal";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const META_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const META_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
pub const META_SERVER: &str = "io.modelcontextprotocol/serverInfo";

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
/// The HTTP headers disagree with the body (http.rs).
pub const HEADER_MISMATCH: i64 = -32020;
pub const UNSUPPORTED_VERSION: i64 = -32022;

/// What an assistant is told about this server before it uses the tools. The name of the
/// place where changes are taken back is given as the user sees it.
fn instructions() -> String {
    format!(
        "FluentCal is the user's desktop calendar and task list on this Windows PC. \
Dates are local calendar days written YYYY-MM-DD and end dates are inclusive; times are HH:MM (24-hour) and a task without a time lasts all day. \
Call list_tasks first: its answer says what today is. \
A repeating task is one task with many occurrences, each identified by its date; say which occurrence you mean with that date. \
To-dos without a day (once a month, or someday) are not on the calendar: read them with list_todos. \
Changes appear in the calendar at once and the user can take them back in FluentCal under \"{}\", so do what was asked without asking twice, but never delete or rewrite tasks that were not mentioned. \
Write task titles and notes in the language the user writes in.",
        tr("最近更改")
    )
}

/// The answer to one message.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// A JSON-RPC response, and the HTTP status it travels with.
    Reply { body: Value, status: u16 },
    /// Nothing to answer (a notification): HTTP 202, nothing on stdout.
    Accepted,
}

/// Every version this server speaks, newest first.
pub fn supported() -> Vec<&'static str> {
    MODERN.iter().chain(LEGACY.iter()).copied().collect()
}

pub fn is_modern(version: &str) -> bool {
    MODERN.contains(&version)
}

/// The protocol version a message declares in `params._meta` (modern requests do).
pub fn meta_version(msg: &Value) -> Option<&str> {
    msg.pointer("/params/_meta")?.get(META_VERSION)?.as_str()
}

/// A JSON-RPC error response.
pub fn error_body(id: &Value, code: i64, message: &str, data: Option<Value>) -> Value {
    let mut e = json!({ "code": code, "message": message });
    if let Some(d) = data {
        e["data"] = d;
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": e })
}

fn fail(id: &Value, code: i64, message: &str, status: u16) -> Outcome {
    Outcome::Reply { body: error_body(id, code, message, None), status }
}

/// "Unsupported protocol version", with the versions that are supported.
pub fn unsupported_version(id: &Value, requested: &str) -> Outcome {
    let data = json!({ "supported": supported(), "requested": requested });
    Outcome::Reply { body: error_body(id, UNSUPPORTED_VERSION, "Unsupported protocol version", Some(data)), status: 400 }
}

fn ok(id: &Value, mut result: Value, modern: bool) -> Outcome {
    if modern {
        result["resultType"] = json!("complete");
        result["_meta"] = json!({ META_SERVER: { "name": SERVER_NAME, "version": SERVER_VERSION } });
    }
    Outcome::Reply { body: json!({ "jsonrpc": "2.0", "id": id, "result": result }), status: 200 }
}

fn capabilities() -> Value {
    // the list of tools never changes while the program runs
    json!({ "tools": { "listChanged": false } })
}

/// Answers one message.
pub fn handle(host: &dyn Host, msg: &Value) -> Outcome {
    let Some(obj) = msg.as_object() else {
        // not a message, or a batch (never part of the versions served here over HTTP, and
        // removed from the protocol since)
        return fail(&Value::Null, INVALID_REQUEST, "Invalid request: expected one JSON-RPC message (an object)", 400);
    };
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // a response: this server never asks the client anything, so there is nothing it answers
        if obj.contains_key("result") || obj.contains_key("error") {
            return Outcome::Accepted;
        }
        let id = obj.get("id").filter(|v| v.is_string() || v.is_number()).cloned().unwrap_or(Value::Null);
        return fail(&id, INVALID_REQUEST, "Invalid request: no method", 400);
    };
    let Some(id) = obj.get("id") else { return Outcome::Accepted }; // a notification
    if !(id.is_string() || id.is_number()) {
        return fail(&Value::Null, INVALID_REQUEST, "Invalid request: id must be a string or a number", 400);
    }
    let empty = Map::new();
    let params = match obj.get("params") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(p)) => p,
        Some(_) => return fail(id, INVALID_PARAMS, "Invalid params: params must be an object", 400),
    };

    if method == "initialize" {
        return initialize(id, params);
    }
    let meta = params.get("_meta").and_then(Value::as_object);
    match meta.and_then(|m| m.get(META_VERSION)) {
        Some(Value::String(version)) => {
            if !is_modern(version) {
                // an unknown (newer) version, or an older one: those open with initialize
                return unsupported_version(id, version);
            }
            if !meta.is_some_and(|m| m.get(META_CAPABILITIES).is_some_and(Value::is_object)) {
                return fail(id, INVALID_PARAMS, &format!("Invalid params: _meta[\"{META_CAPABILITIES}\"] is required"), 400);
            }
            match method {
                "server/discover" => ok(id, discover(), true),
                "tools/list" => list(id, params, true),
                "tools/call" => call(host, id, params, true),
                _ => fail(id, METHOD_NOT_FOUND, &format!("Method not found: {method}"), 404),
            }
        }
        Some(_) => fail(id, INVALID_PARAMS, &format!("Invalid params: _meta[\"{META_VERSION}\"] must be a string"), 400),
        None => match method {
            // only the stateless versions have it, and they say their version
            "server/discover" => fail(id, INVALID_PARAMS, &format!("Invalid params: _meta[\"{META_VERSION}\"] is required"), 400),
            "ping" => ok(id, json!({}), false),
            "tools/list" => list(id, params, false),
            "tools/call" => call(host, id, params, false),
            _ => fail(id, METHOD_NOT_FOUND, &format!("Method not found: {method}"), 200),
        },
    }
}

/// The legacy handshake: the version asked for when this server speaks it, else its newest
/// legacy one (the client decides whether it can live with that).
fn initialize(id: &Value, params: &Map<String, Value>) -> Outcome {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
    let version = LEGACY.iter().find(|v| **v == asked).unwrap_or(&LEGACY[0]);
    let result = json!({
        "protocolVersion": version,
        "capabilities": capabilities(),
        "serverInfo": { "name": SERVER_NAME, "title": SERVER_TITLE, "version": SERVER_VERSION },
        "instructions": instructions(),
    });
    ok(id, result, false)
}

fn discover() -> Value {
    json!({
        "supportedVersions": supported(),
        "capabilities": capabilities(),
        "instructions": instructions(),
        // the same for everybody, and for as long as this version of the program runs
        "ttlMs": 3_600_000,
        "cacheScope": "public",
    })
}

fn list(id: &Value, params: &Map<String, Value>, modern: bool) -> Outcome {
    // one page: no cursor was ever handed out
    if params.get("cursor").is_some_and(|c| !c.is_null()) {
        return fail(id, INVALID_PARAMS, "Invalid params: unknown cursor", if modern { 400 } else { 200 });
    }
    let mut result = json!({ "tools": tools::list() });
    if modern {
        result["ttlMs"] = json!(3_600_000);
        result["cacheScope"] = json!("public");
    }
    ok(id, result, modern)
}

fn call(host: &dyn Host, id: &Value, params: &Map<String, Value>, modern: bool) -> Outcome {
    let status = if modern { 400 } else { 200 };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return fail(id, INVALID_PARAMS, "Invalid params: name (the tool to call) is required", status);
    };
    if !tools::exists(name) {
        return fail(id, INVALID_PARAMS, &format!("Unknown tool: {name}"), status);
    }
    let empty = Map::new();
    let args = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(a)) => a,
        Some(_) => return fail(id, INVALID_PARAMS, "Invalid params: arguments must be an object", status),
    };
    // What goes wrong inside a tool is told to the assistant in the result (isError), so it
    // can read why and try again; a protocol error it would never see.
    let (text, is_error) = match host.unavailable() {
        Some(why) => (why, true),
        None => match tools::call(host, name, args) {
            Ok(v) => (v.to_string(), false),
            Err(why) => (why, true),
        },
    };
    let mut result = json!({ "content": [{ "type": "text", "text": text }] });
    if is_error {
        result["isError"] = json!(true);
    }
    ok(id, result, modern)
}
