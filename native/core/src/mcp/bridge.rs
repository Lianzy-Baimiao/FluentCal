//! `fluentcal.exe --mcp`: for the clients that start a command and talk to it over its
//! standard input and output (one JSON message per line) instead of calling a web address.
//!
//! This is not a second calendar: every message is passed on to the endpoint of the FluentCal
//! that is running (http.rs) and the answer is passed back, so what an assistant changes shows
//! in the calendar at once and there is one writer of the data. When FluentCal is not running
//! (or the connection is turned off in its settings), this program still answers what does
//! not need the calendar (which tools there are) and tells every tool call why it cannot be
//! done. It never starts the calendar by itself.
//!
//! Both generations of the protocol pass through unchanged: nothing is remembered between
//! messages here either.

use super::proto::{self, Outcome};
use super::{Change, Host, PATH, STATE_FILE};
use crate::i18n::tr;
use crate::tasks::Doc;
use base64::Engine as _;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Duration;

/// Where the running endpoint is, and the token it asks for.
pub struct Target {
    pub port: u16,
    pub token: String,
}

/// The running program notes its port in `<dir>\mcp.json` while the endpoint is up.
pub fn write_state(dir: &Path, port: u16) -> std::io::Result<()> {
    std::fs::write(dir.join(STATE_FILE), json!({ "port": port, "pid": std::process::id() }).to_string())
}

pub fn clear_state(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(STATE_FILE));
}

/// The port noted by the running program, if any.
pub fn read_state(dir: &Path) -> Option<u16> {
    let v: Value = serde_json::from_slice(&std::fs::read(dir.join(STATE_FILE)).ok()?).ok()?;
    u16::try_from(v.get("port")?.as_u64()?).ok().filter(|p| *p != 0)
}

/// No calendar to work on: every tool call is answered with why.
struct Offline(String);

impl Host for Offline {
    fn doc(&self) -> Doc {
        Doc::default()
    }
    fn can_write(&self) -> bool {
        false
    }
    fn change(&self, _: Change) -> Result<Value, String> {
        Err(self.0.clone())
    }
    fn unavailable(&self) -> Option<String> {
        Some(self.0.clone())
    }
}

fn not_running() -> String {
    format!(
        "FluentCal is not running, or it is not letting assistants connect. Ask the user to start FluentCal and turn on \"{}\" under {} → {}, then try again.",
        tr("允许 AI 助手连接"),
        tr("设置"),
        tr("AI 助手")
    )
}

fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        // this PC only: no proxy, whatever the environment says
        .proxy(None)
        .http_status_as_error(false)
        .max_redirects(0)
        // a connection per message: one left over from before FluentCal was restarted would
        // fail in a way that cannot be told from a message that was lost half-way
        .max_idle_connections(0)
        .max_idle_connections_per_host(0)
        .timeout_connect(Some(Duration::from_secs(3)))
        .timeout_global(Some(Duration::from_secs(30)))
        .user_agent(concat!("FluentCal-mcp/", env!("CARGO_PKG_VERSION")))
        .build();
    ureq::Agent::new_with_config(config)
}

/// A value for the `Mcp-Name` header: as it is when plain ASCII can carry it, else wrapped
/// as `=?base64?…?=`.
pub fn header_value(s: &str) -> String {
    let plain = !s.is_empty() && s.bytes().all(|b| (0x20..=0x7e).contains(&b)) && s.trim() == s && !(s.starts_with("=?base64?") && s.ends_with("?="));
    if plain {
        s.to_owned()
    } else {
        format!("=?base64?{}?=", base64::engine::general_purpose::STANDARD.encode(s))
    }
}

enum Posted {
    Answer(Value),
    /// Taken, nothing to answer (a notification).
    Nothing,
    /// FluentCal is there but did not answer the message.
    Refused(String),
}

/// Passes a message on. None: nobody is listening there.
fn post(agent: &ureq::Agent, target: &Target, msg: &Value) -> Option<Posted> {
    let mut b = ureq::http::Request::builder()
        .method("POST")
        .uri(format!("http://127.0.0.1:{}{PATH}", target.port))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .header("Authorization", format!("Bearer {}", target.token));
    // the stateless versions repeat parts of the message in headers (http.rs checks them)
    if let Some(version) = proto::meta_version(msg).filter(|v| proto::is_modern(v)) {
        b = b.header("MCP-Protocol-Version", version);
        if let Some(method) = msg.get("method").and_then(Value::as_str) {
            b = b.header("Mcp-Method", method);
            if let Some(name) = msg.pointer("/params/name").and_then(Value::as_str).filter(|_| method == "tools/call") {
                b = b.header("Mcp-Name", header_value(name));
            }
        }
    }
    let request = match b.body(msg.to_string()) {
        Ok(r) => r,
        Err(_) => return Some(Posted::Refused("the message could not be passed on to FluentCal (its method is not a name a header can carry)".into())),
    };
    let mut res = match agent.run(request) {
        Ok(res) => res,
        Err(ureq::Error::Timeout(_)) => return Some(Posted::Refused("FluentCal did not answer in time. Try again in a moment.".into())),
        Err(_) => return None,
    };
    let status = res.status().as_u16();
    if status == 202 {
        return Some(Posted::Nothing);
    }
    let body = res.body_mut().with_config().limit(16 << 20).read_to_vec().unwrap_or_default();
    match serde_json::from_slice::<Value>(&body) {
        Ok(v) if v.get("jsonrpc").is_some() => Some(Posted::Answer(v)),
        _ => Some(Posted::Refused(match status {
            401 => format!("FluentCal did not accept the access token. Ask the user to turn \"{}\" off and on again in FluentCal's settings.", tr("允许 AI 助手连接")),
            413 => "the message is too large for FluentCal (1 MB at most)".into(),
            other => format!("FluentCal answered with HTTP status {other}"),
        })),
    }
}

/// The answer given here, without the calendar.
fn local(msg: &Value, why: String) -> Option<Value> {
    match proto::handle(&Offline(why), msg) {
        Outcome::Reply { body, .. } => Some(body),
        Outcome::Accepted => None,
    }
}

/// Reads messages from `input` until it ends and writes the answers to `output`, one per
/// line. `target` is asked again for every message: FluentCal may be started, stopped or given
/// a new token while the client keeps this program running.
pub fn run(mut input: impl BufRead, mut output: impl Write, target: impl Fn() -> Option<Target>) {
    let agent = agent();
    let mut line = Vec::new();
    loop {
        line.clear();
        match input.read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return, // the client closed its end
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&line);
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Value>(text) {
            Err(_) => Some(proto::error_body(&Value::Null, proto::PARSE_ERROR, "Parse error: the line is not JSON", None)),
            Ok(msg) => match target().and_then(|t| post(&agent, &t, &msg)) {
                Some(Posted::Answer(v)) => Some(v),
                Some(Posted::Nothing) => None,
                Some(Posted::Refused(why)) => local(&msg, why),
                None => local(&msg, not_running()),
            },
        };
        if let Some(a) = answer {
            // Value prints on one line
            if writeln!(output, "{a}").is_err() || output.flush().is_err() {
                return;
            }
        }
    }
}
