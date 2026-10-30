//! Schedules sync rounds (SYN-07): after start, after local edits (debounced), every N
//! minutes, after waking from sleep, on demand, and a short last push on exit. Reports status
//! to the windows as `sync://status`.

use super::engine::{self, Mode, SyncState};
use super::git::{Flavor, Git};
use super::{cloud_view, gdrive::Drive, http::Http, oauth, secrets, view_sig, webdav::WebDav, ErrKind, Remote, SyncError};
use crate::store::{now_ms, Store};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const POLL: Duration = Duration::from_secs(3);
const TIMEOUT: Duration = Duration::from_secs(30);
const QUIT_TIMEOUT: Duration = Duration::from_secs(4);
pub const GITHUB_API: &str = "https://api.github.com";
pub const GITEE_API: &str = "https://gitee.com/api/v5";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Provider {
    WebDav { url: String, user: String, folder: String },
    Git { site: String, repo: String, branch: String, path: String },
    Drive { client_id: String },
}

/// settings.sync, as far as Rust needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub enabled: bool,
    pub provider: Option<Provider>,
    /// Minutes between pulls; 0 = only after start and edits.
    pub interval: u64,
    pub proxy: String,
}

impl Config {
    pub fn from_settings(s: &Value) -> Config {
        let s = s.get("sync").cloned().unwrap_or(Value::Null);
        let str_at = |p: &str| s.pointer(p).and_then(Value::as_str).unwrap_or("").to_owned();
        let git = |site: &str| Provider::Git { site: site.into(), repo: str_at(&format!("/{site}/repo")), branch: str_at(&format!("/{site}/branch")), path: str_at(&format!("/{site}/path")) };
        let provider = match str_at("/provider").as_str() {
            "webdav" => {
                // Absent = the default; an empty string = the server's root folder.
                let folder = s.pointer("/webdav/folder").and_then(Value::as_str).unwrap_or("FluentCal").to_owned();
                Some(Provider::WebDav { url: str_at("/webdav/url"), user: str_at("/webdav/user"), folder })
            }
            "github" => Some(git("github")),
            "gitee" => Some(git("gitee")),
            "gdrive" => Some(Provider::Drive { client_id: str_at("/gdrive/clientId") }),
            _ => None, // "" or "folder" (a shared data folder, handled by sync.rs)
        };
        Config {
            enabled: s.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            provider,
            interval: s.get("interval").and_then(Value::as_u64).unwrap_or(5),
            proxy: str_at("/proxy"),
        }
    }

    pub fn active(&self) -> bool {
        self.enabled && self.provider.is_some()
    }

    /// Which target the sync state belongs to.
    pub fn fingerprint(&self) -> String {
        serde_json::to_string(&self.provider).unwrap_or_default()
    }

    fn is_git(&self) -> bool {
        matches!(self.provider, Some(Provider::Git { .. }))
    }

    fn name(&self) -> &'static str {
        match &self.provider {
            Some(Provider::WebDav { .. }) => "WebDAV",
            Some(Provider::Git { site, .. }) if site == "gitee" => "Gitee",
            Some(Provider::Git { .. }) => "GitHub",
            Some(Provider::Drive { .. }) => "Google 云端硬盘",
            None => "",
        }
    }
}

pub fn make_remote(cfg: &Config, timeout: Duration, location: &Value) -> Result<Box<dyn Remote>, SyncError> {
    let http = Http::new(&cfg.proxy, timeout)?;
    let missing = |what: &str| SyncError::auth(format!("还没有填写{what}"));
    Ok(match cfg.provider.as_ref().ok_or_else(|| SyncError::config("还没有选择同步方式"))? {
        Provider::WebDav { url, user, folder } => {
            let pw = secrets::get("webdav").ok_or_else(|| missing("密码"))?;
            Box::new(WebDav::new(http, url, user, &pw, folder)?)
        }
        Provider::Git { site, repo, branch, path } => {
            let (flavor, api) = if site == "gitee" { (Flavor::Gitee, GITEE_API) } else { (Flavor::GitHub, GITHUB_API) };
            let token = secrets::get(site).ok_or_else(|| missing(" Token"))?;
            Box::new(Git::new(http, flavor, api, repo, branch, path, &token)?)
        }
        Provider::Drive { client_id } => {
            let secret = secrets::get("google-secret").ok_or_else(|| missing(" Google OAuth 客户端密钥"))?;
            let refresh = secrets::get("google-refresh").ok_or_else(|| SyncError::auth("还没有登录 Google 账号"))?;
            Box::new(Drive::new(http, oauth::Endpoints::default(), client_id, &secret, &refresh, location))
        }
    })
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub provider: String,
    pub syncing: bool,
    pub last_sync: Option<i64>,
    pub error: Option<String>,
    pub error_kind: Option<ErrKind>,
    /// Automatic sync stopped until settings change or "sync now" (wrong password etc.).
    pub halted: bool,
    /// Local edits not uploaded yet.
    pub pending: bool,
    pub remote_device: Option<String>,
    pub remote_saved_at: Option<i64>,
}

struct Runtime {
    cfg: Config,
    status: Status,
    gen_seen: u64,
    pending_since: Option<i64>,
    push_at: Option<i64>,
    pull_at: i64,
    failures: u32,
    halted: bool,
}

struct Slot {
    remote: Option<(String, Box<dyn Remote>)>,
    state: SyncState,
}

pub struct Cloud {
    store: Arc<Store>,
    app: Option<AppHandle>,
    state_file: PathBuf,
    rt: Mutex<Runtime>,
    wake: Condvar,
    slot: Mutex<Slot>,
    /// Bumped when a secret changes, so the remote is rebuilt.
    secrets_gen: AtomicU64,
    /// synced_sig of the last round, readable without waiting for a running round.
    synced: AtomicU64,
}
fn load_state(path: &Path) -> SyncState {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_state(path: &Path, st: &SyncState) {
    let Ok(text) = serde_json::to_string_pretty(st) else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Upload delay after an edit, (quiet time, at most) in ms. Every GitHub/Gitee upload is a
/// commit, so they wait longer.
fn debounce(cfg: &Config) -> (i64, i64) {
    if cfg.is_git() { (60_000, 300_000) } else { (15_000, 90_000) }
}

/// Retry delay after the n-th failure in a row.
fn backoff(failures: u32) -> i64 {
    [30_000, 60_000, 120_000, 300_000][(failures.max(1) as usize - 1).min(3)]
}

fn interval_ms(cfg: &Config) -> i64 {
    if cfg.interval == 0 { i64::MAX / 4 } else { cfg.interval as i64 * 60_000 }
}

impl Cloud {
    pub fn start(store: Arc<Store>, app: Option<AppHandle>, state_file: PathBuf) -> Arc<Cloud> {
        let cfg = Config::from_settings(&store.snapshot()["settings"]);
        let state = load_state(&state_file);
        let status = Status {
            enabled: cfg.active(),
            provider: cfg.name().into(),
            last_sync: state.last_sync,
            remote_device: state.remote_device.clone(),
            remote_saved_at: state.remote_saved_at,
            ..Status::default()
        };
        let rt = Runtime { cfg, status, gen_seen: u64::MAX, pending_since: None, push_at: None, pull_at: now_ms() + 5_000, failures: 0, halted: false };
        let synced = AtomicU64::new(state.synced_sig.unwrap_or(0));
        let slot = Mutex::new(Slot { remote: None, state });
        let cloud = Arc::new(Cloud { store, app, state_file, rt: Mutex::new(rt), wake: Condvar::new(), slot, secrets_gen: AtomicU64::new(0), synced });
        let c = cloud.clone();
        let _ = std::thread::Builder::new().name("cloud-sync".into()).spawn(move || c.run_loop());
        cloud
    }
    fn run_loop(&self) {
        loop {
            self.poll_local();
            let due = {
                let rt = self.rt.lock().unwrap();
                let now = now_ms();
                let active = rt.cfg.active() && !rt.halted;
                // After a failure only the backoff (pull_at) counts, not new edits.
                let push_due = rt.failures == 0 && rt.push_at.is_some_and(|t| now >= t);
                if active && (now >= rt.pull_at || push_due) {
                    true
                } else {
                    let _ = self.wake.wait_timeout(rt, POLL).unwrap();
                    false
                }
            };
            if due {
                let cfg = self.rt.lock().unwrap().cfg.clone();
                let _ = self.run_with(&cfg, Mode::Normal, false);
            }
        }
    }

    /// Notices local edits and schedules a debounced upload.
    fn poll_local(&self) {
        let gen = self.store.data_gen();
        let mut rt = self.rt.lock().unwrap();
        if gen == rt.gen_seen || !rt.cfg.active() {
            return;
        }
        rt.gen_seen = gen;
        let pending = view_sig(&cloud_view(&self.store.snapshot())) != self.synced.load(Ordering::Relaxed);
        if self.set_pending(&mut rt, pending) {
            self.emit(&rt.status);
        }
    }

    /// Returns true when the status changed.
    fn set_pending(&self, rt: &mut Runtime, pending: bool) -> bool {
        let now = now_ms();
        if pending {
            let (quiet, cap) = debounce(&rt.cfg);
            let since = *rt.pending_since.get_or_insert(now);
            rt.push_at = Some((now + quiet).min(since + cap));
        } else {
            rt.pending_since = None;
            rt.push_at = None;
        }
        let changed = rt.status.pending != pending;
        rt.status.pending = pending;
        changed
    }
    /// One round with `cfg`; `manual` also runs after auth errors. Updates the status.
    fn run_with(&self, cfg: &Config, mode: Mode, manual: bool) -> Result<engine::Outcome, SyncError> {
        let mut slot = self.slot.lock().unwrap();
        {
            let mut rt = self.rt.lock().unwrap();
            if !manual && (rt.halted || !rt.cfg.active() || rt.cfg != *cfg) {
                return Err(SyncError::config("同步已关闭"));
            }
            rt.status.syncing = true;
            self.emit(&rt.status);
        }
        let result = self.round(&mut slot, cfg, mode, TIMEOUT);
        let synced = slot.state.synced_sig.unwrap_or(0);
        self.synced.store(synced, Ordering::Relaxed);
        let mut rt = self.rt.lock().unwrap();
        let now = now_ms();
        rt.status.syncing = false;
        rt.gen_seen = self.store.data_gen();
        let pending = view_sig(&cloud_view(&self.store.snapshot())) != synced;
        rt.pending_since = None;
        self.set_pending(&mut rt, pending);
        match &result {
            Ok(_) => {
                rt.failures = 0;
                rt.halted = false;
                rt.pull_at = now.saturating_add(interval_ms(cfg));
                rt.status.error = None;
                rt.status.error_kind = None;
                rt.status.halted = false;
                rt.status.last_sync = slot.state.last_sync;
                rt.status.remote_device = slot.state.remote_device.clone();
                rt.status.remote_saved_at = slot.state.remote_saved_at;
            }
            Err(e) => {
                rt.failures += 1;
                rt.halted = matches!(e.kind, ErrKind::Auth | ErrKind::Config | ErrKind::Remote);
                rt.pull_at = now + backoff(rt.failures);
                rt.status.error = Some(e.message.clone());
                rt.status.error_kind = Some(e.kind);
                rt.status.halted = rt.halted;
            }
        }
        self.emit(&rt.status);
        result
    }

    /// Builds (or reuses) the remote and runs the engine; saves the state.
    fn round(&self, slot: &mut Slot, cfg: &Config, mode: Mode, timeout: Duration) -> Result<engine::Outcome, SyncError> {
        let fp = cfg.fingerprint();
        if slot.state.fingerprint != fp {
            slot.state = SyncState { fingerprint: fp.clone(), ..SyncState::default() };
            slot.remote = None;
        }
        let key = format!("{fp}|{}|{}|{timeout:?}", cfg.proxy, self.secrets_gen.load(Ordering::Relaxed));
        if slot.remote.as_ref().map_or(true, |(k, _)| *k != key) {
            slot.remote = Some((key, make_remote(cfg, timeout, &slot.state.location)?));
        }
        let Slot { remote, state } = slot;
        let remote = &mut remote.as_mut().expect("just built").1;
        let result = engine::cycle(&self.store, remote.as_mut(), state, mode);
        save_state(&self.state_file, state);
        result
    }
    pub fn status(&self) -> Status {
        self.rt.lock().unwrap().status.clone()
    }

    fn emit(&self, s: &Status) {
        if let Some(app) = &self.app {
            let _ = app.emit("sync://status", s);
        }
    }

    /// settings.sync changed (on_settings_changed).
    pub fn reconfigure(&self, settings: &Value) {
        let cfg = Config::from_settings(settings);
        let mut rt = self.rt.lock().unwrap();
        if cfg == rt.cfg {
            return;
        }
        if cfg.fingerprint() != rt.cfg.fingerprint() || cfg.enabled != rt.cfg.enabled || cfg.proxy != rt.cfg.proxy {
            rt.failures = 0;
            rt.halted = false;
            rt.status.error = None;
            rt.status.error_kind = None;
            rt.status.halted = false;
            rt.pull_at = now_ms() + 1_000;
        } else {
            rt.pull_at = rt.pull_at.min(now_ms().saturating_add(interval_ms(&cfg)));
        }
        rt.status.enabled = cfg.active();
        rt.status.provider = cfg.name().into();
        if !cfg.active() {
            self.set_pending(&mut rt, false);
        }
        rt.gen_seen = u64::MAX;
        rt.cfg = cfg;
        self.emit(&rt.status);
        self.wake.notify_all();
    }

    /// Tray "sync now": a round soon, also after an error.
    pub fn request(&self) {
        let mut rt = self.rt.lock().unwrap();
        rt.halted = false;
        rt.failures = 0;
        rt.pull_at = 0;
        self.wake.notify_all();
    }

    /// Back from sleep: sync once the network is up again.
    pub fn nudge_after_wake(&self) {
        let mut rt = self.rt.lock().unwrap();
        if !rt.halted {
            rt.failures = 0;
            rt.pull_at = rt.pull_at.min(now_ms() + 8_000);
        }
        self.wake.notify_all();
    }

    /// A password, token or Google login changed: rebuild the remote, retry an auth error.
    pub fn secrets_changed(&self) {
        self.secrets_gen.fetch_add(1, Ordering::Relaxed);
        let mut rt = self.rt.lock().unwrap();
        if rt.halted {
            rt.halted = false;
            rt.failures = 0;
            rt.status.halted = false;
            rt.pull_at = now_ms() + 1_000;
        }
        self.wake.notify_all();
    }

    pub fn sync_now(&self) -> Result<engine::Outcome, SyncError> {
        let cfg = self.rt.lock().unwrap().cfg.clone();
        if !cfg.active() {
            return Err(SyncError::config("同步没有开启"));
        }
        self.run_with(&cfg, Mode::Normal, true)
    }

    /// "Test connection" with the current settings (sync may still be off).
    pub fn probe(&self) -> Result<engine::Probe, SyncError> {
        let cfg = self.rt.lock().unwrap().cfg.clone();
        let mut remote = make_remote(&cfg, TIMEOUT, &Value::Null)?;
        engine::probe(remote.as_mut())
    }

    /// Turns sync on: first round merging both sides, or replacing this PC's tasks with the
    /// cloud copy. This PC's data is backed up first.
    pub fn enable(&self, replace: bool) -> Result<engine::Outcome, SyncError> {
        let mut cfg = self.rt.lock().unwrap().cfg.clone();
        if cfg.provider.is_none() {
            return Err(SyncError::config("还没有选择同步方式"));
        }
        cfg.enabled = true;
        if !replace {
            self.store.backup("pre-sync").map_err(SyncError::config)?;
        }
        let out = self.run_with(&cfg, if replace { Mode::Replace } else { Mode::Normal }, true)?;
        let mut rt = self.rt.lock().unwrap();
        rt.status.enabled = true;
        rt.status.provider = cfg.name().into();
        rt.pull_at = now_ms().saturating_add(interval_ms(&cfg));
        rt.cfg = cfg;
        rt.gen_seen = u64::MAX;
        self.emit(&rt.status);
        Ok(out)
    }

    /// On quit: uploads edits that haven't been synced yet, waiting at most ~5 s.
    pub fn quit_flush(self: &Arc<Self>) {
        let cfg = {
            let rt = self.rt.lock().unwrap();
            if !rt.cfg.active() || rt.halted {
                return;
            }
            rt.cfg.clone()
        };
        if view_sig(&cloud_view(&self.store.snapshot())) == self.synced.load(Ordering::Relaxed) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let me = self.clone();
        std::thread::spawn(move || {
            if let Ok(mut slot) = me.slot.try_lock() {
                let _ = me.round(&mut slot, &cfg, Mode::Normal, QUIT_TIMEOUT);
            }
            let _ = tx.send(());
        });
        let _ = rx.recv_timeout(QUIT_TIMEOUT + Duration::from_secs(1));
    }
}
