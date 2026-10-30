//! Cloud sync (SYN, 需求文档 4.9 / 7.5): one JSON file on the user's own WebDAV server,
//! GitHub/Gitee private repository or Google Drive, merged item by item with merge.rs.
//! `engine` is one sync round (testable without Tauri), `service` schedules rounds, the
//! providers implement `Remote`.

pub mod commands;
pub mod engine;
pub mod gdrive;
pub mod git;
pub mod http;
pub mod oauth;
pub mod secrets;
pub mod service;
pub mod webdav;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Collections that are synced. Holiday data is per PC (each one downloads it itself).
pub const SYNCED: [&str; 3] = ["tasks", "groups", "dayMarks"];
pub const FILE_NAME: &str = "fluentcal-data.json";
/// Cloud file format version written by this build.
pub const FORMAT: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrKind {
    /// No connection, timeout, DNS: retried with backoff.
    Network,
    /// Wrong password or token, revoked authorization: not retried automatically.
    Auth,
    /// Bad address, public repository, missing setting: not retried automatically.
    Config,
    /// The cloud file is not FluentCal data or comes from a newer version: never overwritten.
    Remote,
    /// Unexpected answer from the server (5xx, rate limit).
    Server,
    /// Someone else uploaded in between: fetch, merge and try again.
    Conflict,
}

#[derive(Debug, Clone)]
pub struct SyncError {
    pub kind: ErrKind,
    pub message: String,
}

impl SyncError {
    pub fn new(kind: ErrKind, message: impl Into<String>) -> Self {
        SyncError { kind, message: message.into() }
    }
    pub fn auth(m: impl Into<String>) -> Self {
        Self::new(ErrKind::Auth, m)
    }
    pub fn config(m: impl Into<String>) -> Self {
        Self::new(ErrKind::Config, m)
    }
    pub fn server(m: impl Into<String>) -> Self {
        Self::new(ErrKind::Server, m)
    }
    pub fn conflict() -> Self {
        Self::new(ErrKind::Conflict, "云端文件刚被另一台电脑修改")
    }
    /// For status codes a provider has no special message for.
    pub fn status(code: u16) -> Self {
        match code {
            500..=599 => Self::server(format!("服务器出错（{code}），稍后会自动重试")),
            _ => Self::server(format!("服务器返回了意外的结果（{code}）")),
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Version of the cloud file as the provider knows it: WebDAV ETag, Git blob sha (+ ETag),
/// Drive version number.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rev {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

pub enum Fetched {
    /// The file hasn't changed since `known`.
    NotModified,
    Missing,
    File(Vec<u8>, Rev),
}

pub trait Remote: Send {
    /// Checks the account and target once per session (repository is private, etc.).
    fn check(&mut self) -> Result<(), SyncError>;
    /// Gets the file; with `known`, may answer NotModified.
    fn fetch(&mut self, known: Option<&Rev>) -> Result<Fetched, SyncError>;
    /// Writes the file if it is still at `base` (None = it must not exist yet). Returns the
    /// new version, or ErrKind::Conflict.
    fn put(&mut self, bytes: &[u8], base: Option<&Rev>) -> Result<Rev, SyncError>;
    /// Provider-specific ids to remember across restarts (Drive folder and file id).
    fn location(&self) -> Value {
        Value::Null
    }
}

/// The synced part of the document: the collections in SYNCED and their edit times.
pub fn cloud_view(doc: &Value) -> Value {
    let mut data = json!({});
    let mut meta = json!({});
    for col in SYNCED {
        data[col] = doc.get(col).filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({}));
        meta[col] = doc.pointer(&format!("/meta/{col}")).filter(|v| v.is_object()).cloned().unwrap_or_else(|| json!({}));
    }
    data["meta"] = meta;
    data
}

/// Content signature of a cloud view, independent of key order.
pub fn view_sig(view: &Value) -> u64 {
    fn canon(v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                Value::Object(keys.into_iter().map(|k| (k.clone(), canon(&m[k]))).collect())
            }
            Value::Array(a) => Value::Array(a.iter().map(canon).collect()),
            v => v.clone(),
        }
    }
    crate::store::signature(canon(view).to_string().as_bytes())
}

pub fn device_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "此电脑".into())
}

/// The cloud file: `{ app, format, device, savedAt, data: cloud_view }`.
pub fn encode(view: &Value, now: i64) -> Vec<u8> {
    let file = json!({ "app": "FluentCal", "format": FORMAT, "device": device_name(), "savedAt": now, "data": view });
    serde_json::to_vec_pretty(&file).unwrap_or_default()
}

pub struct CloudFile {
    pub view: Value,
    pub device: Option<String>,
    pub saved_at: Option<i64>,
}

/// Parses a cloud file. Anything that isn't FluentCal data is refused, so it is never
/// overwritten (SYN-13). An empty file counts as missing.
pub fn decode(bytes: &[u8]) -> Result<Option<CloudFile>, SyncError> {
    let text = std::str::from_utf8(bytes).map_err(|_| SyncError::new(ErrKind::Remote, "云端文件不是 FluentCal 数据（无法读取），为安全起见没有覆盖它"))?;
    let text = text.trim_start_matches('\u{FEFF}').trim();
    if text.is_empty() {
        return Ok(None);
    }
    let v: Value = serde_json::from_str(text).map_err(|_| SyncError::new(ErrKind::Remote, "云端文件已损坏或不是 FluentCal 数据，为安全起见没有覆盖它"))?;
    if v.get("app").and_then(Value::as_str) != Some("FluentCal") || !v.get("data").is_some_and(Value::is_object) {
        return Err(SyncError::new(ErrKind::Remote, "云端同名文件不是 FluentCal 同步数据，为安全起见没有覆盖它。请换一个文件夹或文件路径"));
    }
    if v.get("format").and_then(Value::as_i64).unwrap_or(0) > FORMAT {
        return Err(SyncError::new(ErrKind::Remote, "云端数据来自更新版本的 FluentCal，请先升级本机的 FluentCal"));
    }
    Ok(Some(CloudFile {
        view: cloud_view(&v["data"]),
        device: v.get("device").and_then(Value::as_str).map(str::to_owned),
        saved_at: v.get("savedAt").and_then(Value::as_i64),
    }))
}
