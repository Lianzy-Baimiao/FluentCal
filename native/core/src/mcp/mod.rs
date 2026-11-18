//! AI assistants (MCP, 需求文档 4.12 / 7.8): FluentCal as a Model Context Protocol server, so an
//! assistant (Claude, Cursor, Codex…) can look at the calendar and change tasks. Off unless
//! the user turns it on (settings `mcp.enabled`).
//!
//! - proto.rs: the protocol, whatever carries it. Both generations of it: the stateless one
//!   (2026-07-28) and the older ones that open with `initialize`.
//! - tools.rs: what an assistant can do.
//! - http.rs: the endpoint on 127.0.0.1 (Streamable HTTP), behind an access token.
//! - bridge.rs: `fluentcal.exe --mcp`, for clients that start a command and talk to it over
//!   stdin/stdout: it passes each message on to the endpoint of the running FluentCal.
//!
//! The specification is kept in C:\Users\30694\tools\mcp-spec (both revisions).

pub mod bridge;
pub mod http;
pub mod proto;
pub mod tools;

use crate::tasks::{Doc, Edit};
use serde_json::Value;

pub const DEFAULT_PORT: u16 = 18766;
/// The access token's key in Windows Credential Manager (cloud::secrets).
pub const TOKEN_KEY: &str = "mcp-token";
/// The endpoint's path: http://127.0.0.1:<port>/mcp
pub const PATH: &str = "/mcp";
/// `<this PC's folder>\mcp.json`: the port of the endpoint while it runs, for the bridge.
pub const STATE_FILE: &str = "mcp.json";

/// A change a tool wants to make. It is called with the document as it is at that moment and
/// answers with what to write and what to tell the assistant, or with why not.
pub type Change = Box<dyn FnOnce(&Doc) -> Result<(Edit, Value), String> + Send>;

/// What the tools work on: the running app (app/src/mcpcmd.rs), or a store in the tests.
pub trait Host: Send + Sync {
    /// The document now.
    fn doc(&self) -> Doc;
    /// Assistants may change it (settings `mcp.write`), not only look.
    fn can_write(&self) -> bool;
    /// Makes a change and writes it; Err when it was not made (a rule broken, the app busy).
    fn change(&self, f: Change) -> Result<Value, String>;
    /// Why no tool can run at all (the bridge without a running FluentCal); None otherwise.
    fn unavailable(&self) -> Option<String> {
        None
    }
}
