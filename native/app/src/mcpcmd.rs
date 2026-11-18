//! AI assistants (MCP, 需求文档 4.12 / 7.8; the server itself is fluentcal_core::mcp): the
//! endpoint inside the running program, following the settings mcp.enabled / mcp.port /
//! mcp.write, with its access token in Windows Credential Manager; and `fluentcal.exe --mcp`,
//! the bridge for assistants that start a command instead of calling an address.
//!
//! Off (as it is unless the user turns it on): no thread, no port, no token.

use crate::app::{post, App};
use fluentcal_core::cloud::secrets;
use fluentcal_core::httpd::Running;
use fluentcal_core::mcp::{bridge, http, Change, Host, DEFAULT_PORT, PATH, TOKEN_KEY};
use fluentcal_core::store::Store;
use fluentcal_core::tasks::Doc;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// A change waits this long for the UI thread (which may be showing a file dialog).
const WAIT: Duration = Duration::from_secs(10);
const WAITING: u8 = 0;
const TAKEN: u8 = 1;
const GIVEN_UP: u8 = 2;

const BUSY: &str = "FluentCal is busy (a dialog may be open) and did not make the change. Nothing was changed; try again in a moment.";
const TURNED_OFF: &str = "The user has just turned this off in FluentCal's settings. Nothing was changed.";

/// A change an assistant asked for, on its way to the UI thread, where the document is.
struct Job {
    change: Change,
    /// WAITING until the UI thread takes it or its caller gives up, whichever is first: a
    /// change whose caller was told "not made" is never made later.
    state: Arc<AtomicU8>,
    done: Sender<Result<Value, String>>,
}

type Queue = Arc<Mutex<Vec<Job>>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The running program as the tools see it, from the threads of the endpoint.
struct AppHost {
    store: Arc<Store>,
    write: Arc<AtomicBool>,
    queue: Queue,
}

impl Host for AppHost {
    fn doc(&self) -> Doc {
        Doc::from_value(&self.store.snapshot())
    }

    fn can_write(&self) -> bool {
        self.write.load(Ordering::Relaxed)
    }

    /// Changes are made by the UI thread, like the user's own: they show at once, and they
    /// are listed in 最近更改 to take back.
    fn change(&self, change: Change) -> Result<Value, String> {
        let (done, result) = channel();
        let state = Arc::new(AtomicU8::new(WAITING));
        lock(&self.queue).push(Job { change, state: state.clone(), done });
        post(|app| app.mcp_drain());
        match result.recv_timeout(WAIT) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) if state.compare_exchange(WAITING, GIVEN_UP, Ordering::AcqRel, Ordering::Acquire).is_ok() => Err(BUSY.into()),
            // taken in the last moment: it is being made, the answer is on its way
            Err(RecvTimeoutError::Timeout) => result.recv().unwrap_or_else(|_| Err(TURNED_OFF.into())),
            Err(RecvTimeoutError::Disconnected) => Err(TURNED_OFF.into()),
        }
    }
}

#[derive(Default)]
pub struct McpState {
    running: Option<Running>,
    pub error: Option<String>,
    token: Arc<Mutex<String>>,
    write: Arc<AtomicBool>,
    queue: Queue,
    /// Where the bridge finds the port (mcp.json in this folder), while the endpoint runs.
    note: Option<PathBuf>,
}

impl Drop for McpState {
    fn drop(&mut self) {
        if let Some(dir) = self.note.take() {
            bridge::clear_state(&dir);
        }
    }
}

/// For the settings page.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct McpInfo {
    pub enabled: bool,
    pub write: bool,
    pub running: bool,
    pub port: u16,
    pub error: Option<String>,
    /// http://127.0.0.1:<port>/mcp
    pub url: String,
    pub token: String,
    /// This program's file, for the assistants that start `fluentcal.exe --mcp`.
    pub exe: String,
    /// Installed from the Microsoft Store: other programs cannot start this file.
    pub packaged: bool,
}

fn new_token() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| e.to_string())?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// What goes into an assistant's MCP settings (the "mcpServers" form most of them read) to
/// start this program as the server.
pub fn command_config(exe: &str) -> String {
    format!("{{\n  \"mcpServers\": {{\n    \"fluentcal\": {{\n      \"command\": {},\n      \"args\": [\"--mcp\"]\n    }}\n  }}\n}}", quoted(exe))
}

/// The same for assistants that call an address.
pub fn url_config(url: &str, token: &str) -> String {
    format!(
        "{{\n  \"mcpServers\": {{\n    \"fluentcal\": {{\n      \"type\": \"http\",\n      \"url\": {},\n      \"headers\": {{\n        \"Authorization\": {}\n      }}\n    }}\n  }}\n}}",
        quoted(url),
        quoted(&format!("Bearer {token}"))
    )
}

/// The command that adds the server to Claude Code, for every project of this user.
pub fn claude_command(exe: &str) -> String {
    format!("claude mcp add --scope user fluentcal -- \"{exe}\" --mcp")
}

pub fn claude_url(url: &str, token: &str) -> String {
    format!("claude mcp add --scope user --transport http fluentcal {url} --header \"Authorization: Bearer {token}\"")
}

impl App {
    fn mcp_port(&self) -> u16 {
        match self.doc.settings.mcp.port {
            0 => DEFAULT_PORT,
            p => p.clamp(1024, 65535),
        }
    }

    /// The access token: made the first time assistants are let in, kept in Windows
    /// Credential Manager (where the bridge reads it).
    fn mcp_token(&mut self) -> Result<String, String> {
        let mut t = lock(&self.mcp.token);
        if t.is_empty() {
            *t = secrets::get(TOKEN_KEY).unwrap_or_default();
        }
        if t.is_empty() {
            let new = new_token()?;
            secrets::set(TOKEN_KEY, &new)?;
            *t = new;
        }
        Ok(t.clone())
    }

    fn mcp_stop(&mut self) {
        self.mcp.running = None;
        if let Some(dir) = self.mcp.note.take() {
            bridge::clear_state(&dir);
        }
        // whoever still waits for a change is told that it was not made
        self.mcp_drain();
    }

    /// Starts, restarts or stops the endpoint to match the settings.
    pub fn mcp_apply(&mut self) {
        let (enabled, write) = (self.doc.settings.mcp.enabled, self.doc.settings.mcp.write);
        self.mcp.write.store(write, Ordering::Relaxed);
        if !enabled {
            self.mcp_stop();
            self.mcp.error = None;
            // (a note left behind by a run that ended badly)
            bridge::clear_state(&self.local_dir);
            return;
        }
        let port = self.mcp_port();
        if self.mcp.running.as_ref().is_some_and(|r| r.addr.port() == port) {
            return;
        }
        self.mcp_stop(); // frees the old port first
        if let Err(e) = self.mcp_token() {
            self.mcp.error = Some(e);
            return;
        }
        let host = Arc::new(AppHost { store: self.store.clone(), write: self.mcp.write.clone(), queue: self.mcp.queue.clone() });
        match http::start(Arc::new(http::Ctx { host, token: self.mcp.token.clone() }), port) {
            Ok(r) => {
                if bridge::write_state(&self.local_dir, r.addr.port()).is_ok() {
                    self.mcp.note = Some(self.local_dir.clone());
                }
                self.mcp.running = Some(r);
                self.mcp.error = None;
            }
            Err(e) => self.mcp.error = Some(e),
        }
    }

    /// Makes the changes assistants asked for (posted by the endpoint's threads; the tests,
    /// which have no event loop to post to, call it themselves).
    pub fn mcp_drain(&mut self) {
        let jobs: Vec<Job> = std::mem::take(&mut *lock(&self.mcp.queue));
        for Job { change, state, done } in jobs {
            if state.compare_exchange(WAITING, TAKEN, Ordering::AcqRel, Ordering::Acquire).is_err() {
                continue; // its caller gave up waiting and said so
            }
            let allowed = self.mcp.running.is_some() && self.doc.settings.mcp.write;
            let result = if !allowed {
                Err(TURNED_OFF.into())
            } else {
                // A slip in a tool must not take the calendar down with it.
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| change(&self.doc))) {
                    Ok(Ok((edit, told))) => {
                        // "nothing to change" is not a change: nothing for 最近更改
                        if !edit.is_empty() {
                            self.apply(edit, false);
                        }
                        Ok(told)
                    }
                    Ok(Err(why)) => Err(why),
                    Err(_) => Err("FluentCal could not make this change (an internal error). Nothing was changed.".into()),
                }
            };
            let _ = done.send(result);
        }
    }

    pub fn mcp_info(&self) -> McpInfo {
        let running = self.mcp.running.as_ref().map(|r| r.addr.port());
        let port = running.unwrap_or_else(|| self.mcp_port());
        McpInfo {
            enabled: self.doc.settings.mcp.enabled,
            write: self.doc.settings.mcp.write,
            running: running.is_some(),
            port,
            error: self.mcp.error.clone(),
            url: format!("http://127.0.0.1:{port}{PATH}"),
            token: lock(&self.mcp.token).clone(),
            exe: std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default(),
            packaged: crate::platform::package::is_packaged(),
        }
    }

    /// A new access token: assistants that were given the old one (in an address's
    /// configuration) are shut out at once; those that start `--mcp` read the new one.
    pub fn mcp_reset_token(&mut self) -> Result<(), String> {
        let t = new_token()?;
        secrets::set(TOKEN_KEY, &t)?;
        *lock(&self.mcp.token) = t;
        Ok(())
    }
}

/// The language the user chose for FluentCal, read from the settings without opening the
/// data (the running program has it open).
fn language_preference(local: &Path) -> String {
    let (dir, local_settings, _) = fluentcal_core::datadir::resolve(local);
    let read = |p: &Path| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{FEFF}')).ok());
    let settings = match local_settings {
        Some(p) => read(&p),
        None => read(&dir.join("data.json")).and_then(|d| d.get("settings").cloned()),
    };
    settings.and_then(|s| s.pointer("/general/language").and_then(Value::as_str).map(str::to_owned)).unwrap_or_else(|| "system".into())
}

/// `fluentcal.exe --mcp`: passes the messages of an assistant (standard input and output) on
/// to the FluentCal that is running, until the assistant closes its end. `local`: this PC's
/// folder of the program (where the running one notes its port). Opens no window and does not
/// start the calendar.
pub fn bridge_main(local: PathBuf) {
    if crate::platform::isolated() {
        secrets::set_namespace("FluentCal-dev");
    }
    // the names of settings in what the assistant is told, as the user sees them
    fluentcal_core::i18n::set_language(fluentcal_core::i18n::resolve(&language_preference(&local), &crate::i18n::system_language()));
    let stdin = std::io::stdin();
    bridge::run(stdin.lock(), std::io::stdout(), || Some(bridge::Target { port: bridge::read_state(&local)?, token: secrets::get(TOKEN_KEY)? }));
}
