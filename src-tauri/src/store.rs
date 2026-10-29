//! JSON document store. One `data.json` holds tasks, groups, day marks, holiday data and
//! settings. Rust treats the document as opaque JSON; the frontend owns the schema.
//! Mutations mark the store dirty, a background thread writes atomically (temp + rename)
//! after a short debounce, and every change is broadcast so all windows stay in sync.

use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, State, Webview};

pub const COLLECTIONS: [&str; 4] = ["tasks", "groups", "dayMarks", "holidays"];
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);
const DAILY_BACKUPS_KEPT: usize = 14;
const MANUAL_BACKUPS_KEPT: usize = 20;

pub(crate) struct Inner {
    pub(crate) doc: Value,
    pub(crate) dirty: bool,
}

pub struct Store {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) wake: Condvar,
    pub(crate) write_lock: Mutex<()>,
    pub dir: PathBuf,
    pub(crate) file: PathBuf,
    /// Per-PC settings file when the data folder is shared; None = settings in data.json.
    pub(crate) local_settings: Option<PathBuf>,
    /// Signature of the data.json we last wrote or read, to tell external changes apart.
    pub(crate) last_written: Mutex<Option<u64>>,
    /// Set when data.json was unreadable at startup: Some(backup name) or Some("") if reset.
    recovered: Mutex<Option<String>>,
    app: Mutex<Option<AppHandle>>,
    save_failing: AtomicBool,
}

#[derive(Clone, Serialize)]
struct ChangedPayload<'a> {
    origin: &'a str,
    collection: &'a str,
    items: &'a Map<String, Value>,
}

#[derive(Clone, Serialize)]
struct SettingsPayload<'a> {
    origin: &'a str,
    settings: &'a Value,
}

#[derive(Clone, Serialize)]
struct ReloadPayload<'a> {
    origin: &'a str,
}

impl Store {
    pub fn open(dir: PathBuf) -> std::io::Result<Arc<Store>> {
        Store::open_with(dir, None)
    }

    /// `local_settings`: when the data folder is shared (OneDrive…), settings are per PC and
    /// live in this file; data.json then holds only the shared collections.
    pub fn open_with(dir: PathBuf, local_settings: Option<PathBuf>) -> std::io::Result<Arc<Store>> {
        fs::create_dir_all(dir.join("backups"))?;
        let file = dir.join("data.json");
        let (mut doc, recovered) = load(&dir, &file);
        if let Some(p) = &local_settings {
            if let Some(s) = fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(Value::is_object) {
                doc["settings"] = s;
            }
        }
        let dirty = recovered.is_some();
        let sig = fs::read(&file).ok().map(|b| signature(&b));
        let store = Arc::new(Store {
            inner: Mutex::new(Inner { doc, dirty }),
            wake: Condvar::new(),
            write_lock: Mutex::new(()),
            dir,
            file,
            local_settings,
            last_written: Mutex::new(sig),
            recovered: Mutex::new(recovered),
            app: Mutex::new(None),
            save_failing: AtomicBool::new(false),
        });
        store.daily_backup();
        let saver = store.clone();
        thread::Builder::new()
            .name("store-saver".into())
            .spawn(move || saver.saver_loop())?;
        let watcher = store.clone();
        thread::Builder::new()
            .name("store-sync".into())
            .spawn(move || crate::sync::watch(&watcher))?;
        Ok(store)
    }

    fn saver_loop(&self) {
        loop {
            {
                let mut g = self.inner.lock().unwrap();
                while !g.dirty {
                    g = self.wake.wait(g).unwrap();
                }
            }
            thread::sleep(SAVE_DEBOUNCE);
            if !self.flush() {
                thread::sleep(Duration::from_secs(2));
            }
        }
    }

    /// Writes the document if dirty. Returns false when the write failed (store stays dirty).
    /// With a shared data folder, settings go to the local settings file instead.
    pub fn flush(&self) -> bool {
        let _w = self.write_lock.lock().unwrap();
        let (text, settings) = {
            let mut g = self.inner.lock().unwrap();
            if !g.dirty {
                return true;
            }
            g.dirty = false;
            match &self.local_settings {
                Some(_) => {
                    let mut shared = g.doc.clone();
                    let s = shared.as_object_mut().and_then(|o| o.remove("settings"));
                    (serde_json::to_string_pretty(&shared).unwrap_or_default(), s)
                }
                None => (serde_json::to_string_pretty(&g.doc).unwrap_or_default(), None),
            }
        };
        self.daily_backup();
        let res = write_atomic(&self.file, text.as_bytes()).and_then(|()| match (&self.local_settings, settings) {
            (Some(p), Some(s)) => write_atomic(p, serde_json::to_string_pretty(&s).unwrap_or_default().as_bytes()),
            _ => Ok(()),
        });
        if res.is_ok() {
            *self.last_written.lock().unwrap() = Some(signature(text.as_bytes()));
        }
        match res {
            Ok(()) => {
                if self.save_failing.swap(false, Ordering::Relaxed) {
                    self.notify_save(None);
                }
                true
            }
            Err(e) => {
                eprintln!("[store] save failed: {e}");
                self.inner.lock().unwrap().dirty = true;
                if !self.save_failing.swap(true, Ordering::Relaxed) {
                    self.notify_save(Some(e.to_string()));
                }
                false
            }
        }
    }

    /// Lets the store report save failures to the windows (set once the app is running).
    pub fn set_app(&self, app: AppHandle) {
        *self.app.lock().unwrap() = Some(app);
    }

    /// Tells all windows to reload the document (e.g. after merging an external change).
    pub(crate) fn emit_reload(&self) {
        if let Some(app) = self.app.lock().unwrap().as_ref() {
            let _ = app.emit("store://reload", ReloadPayload { origin: "sync" });
        }
    }

    fn notify_save(&self, error: Option<String>) {
        if let Some(app) = self.app.lock().unwrap().as_ref() {
            let _ = app.emit("store://save-status", json!({ "error": error }));
        }
    }

    pub(crate) fn mark_dirty(&self, g: &mut Inner) {
        g.dirty = true;
        self.wake.notify_all();
    }

    pub fn snapshot(&self) -> Value {
        self.inner.lock().unwrap().doc.clone()
    }

    pub fn setting(&self, ptr: &str) -> Option<Value> {
        self.inner.lock().unwrap().doc.pointer(&format!("/settings{ptr}")).cloned()
    }

    pub fn setting_str(&self, ptr: &str, default: &str) -> String {
        self.setting(ptr)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| default.to_owned())
    }

    pub fn setting_bool(&self, ptr: &str, default: bool) -> bool {
        self.setting(ptr).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    /// Deep-merges `patch` into settings. Returns (old, new) settings.
    pub fn patch_settings(&self, patch: &Value) -> (Value, Value) {
        let mut g = self.inner.lock().unwrap();
        let settings = g.doc.get_mut("settings").expect("normalized doc has settings");
        let old = settings.clone();
        deep_merge(settings, patch);
        let new = settings.clone();
        self.mark_dirty(&mut g);
        (old, new)
    }

    pub fn put(&self, collection: &str, items: &Map<String, Value>) {
        let mut g = self.inner.lock().unwrap();
        crate::merge::stamp(&mut g.doc, collection, items.keys(), now_ms());
        if let Some(Value::Object(col)) = g.doc.get_mut(collection) {
            for (id, v) in items {
                if v.is_null() {
                    col.remove(id);
                } else {
                    col.insert(id.clone(), v.clone());
                }
            }
        }
        self.mark_dirty(&mut g);
    }

    /// Replaces the document (import, restore). Its items count as edited now, so a synced
    /// copy elsewhere doesn't undo the import; earlier deletions are remembered.
    fn replace(&self, mut doc: Value) {
        let mut g = self.inner.lock().unwrap();
        let old_meta = g.doc.get("meta").cloned();
        crate::merge::stamp_all(&mut doc, old_meta.as_ref(), now_ms());
        g.doc = doc;
        self.mark_dirty(&mut g);
    }

    /// Recovery notice from startup (taken once).
    pub fn take_recovered(&self) -> Option<String> {
        self.recovered.lock().unwrap().take()
    }

    fn backups_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    /// Copies the on-disk data file once per calendar day (before it is overwritten).
    fn daily_backup(&self) {
        if !self.file.exists() {
            return;
        }
        let dir = self.backups_dir();
        let target = dir.join(format!("daily-{}.json", local_date()));
        if !target.exists() {
            let _ = fs::copy(&self.file, &target);
            prune(&dir, "daily-", DAILY_BACKUPS_KEPT);
        }
    }

    fn manual_backup(&self, prefix: &str) -> std::io::Result<String> {
        let name = format!("{prefix}-{}.json", local_stamp());
        let text = serde_json::to_string_pretty(&self.snapshot()).unwrap_or_default();
        write_atomic(&self.backups_dir().join(&name), text.as_bytes())?;
        prune(&self.backups_dir(), &format!("{prefix}-"), MANUAL_BACKUPS_KEPT);
        Ok(name)
    }
}

/// Reads the file, retrying for a few seconds while another program (antivirus, OneDrive)
/// holds it. NotFound is returned immediately.
fn read_with_retry(file: &Path) -> std::io::Result<Vec<u8>> {
    let mut last = None;
    for _ in 0..10 {
        match fs::read(file) {
            Ok(b) => return Ok(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(e),
            Err(e) => last = Some(e),
        }
        thread::sleep(Duration::from_millis(300));
    }
    Err(last.expect("at least one attempt"))
}

/// Only a missing file means a fresh start. A file that can't be read or parsed is moved
/// aside (so it can't be overwritten or copied into today's backup) and the newest valid
/// backup is used instead; the UI is told either way.
fn load(dir: &Path, file: &Path) -> (Value, Option<String>) {
    let parsed = match read_with_retry(file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (default_doc(), None),
        Err(_) => None,
        Ok(bytes) => std::str::from_utf8(&bytes)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(t.trim_start_matches('\u{FEFF}')).ok())
            .filter(Value::is_object),
    };
    if let Some(v) = parsed {
        return (normalize(v), None);
    }
    let aside = dir.join(format!("data.unreadable-{}.json", local_stamp()));
    if fs::rename(file, &aside).is_err() {
        let _ = fs::copy(file, &aside);
    }
    match newest_valid_backup(&dir.join("backups")) {
        Some((name, v)) => (normalize(v), Some(name)),
        None => {
            // Don't seed sample tasks over a lost data set.
            let mut d = default_doc();
            d["settings"]["general"] = json!({ "seeded": true });
            (d, Some(String::new()))
        }
    }
}

fn newest_valid_backup(dir: &Path) -> Option<(String, Value)> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".json"))
        .collect();
    // newest first by modification time
    names.sort_by_key(|n| {
        std::cmp::Reverse(fs::metadata(dir.join(n)).and_then(|m| m.modified()).ok())
    });
    names.into_iter().find_map(|n| {
        let v: Value = serde_json::from_str(&fs::read_to_string(dir.join(&n)).ok()?).ok()?;
        v.is_object().then_some((n, v))
    })
}

fn default_doc() -> Value {
    normalize(json!({}))
}

pub fn normalize(mut v: Value) -> Value {
    let obj = v.as_object_mut().expect("object");
    obj.entry("version").or_insert(json!(1));
    for key in COLLECTIONS.iter().chain(["settings", "meta"].iter()) {
        if !obj.get(*key).is_some_and(Value::is_object) {
            obj.insert((*key).into(), json!({}));
        }
    }
    v
}

pub fn deep_merge(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                if v.is_null() {
                    t.remove(k);
                } else if v.is_object() && t.get(k).is_some_and(Value::is_object) {
                    deep_merge(t.get_mut(k).unwrap(), v);
                } else {
                    t.insert(k.clone(), v.clone());
                }
            }
        }
        (t, p) => *t = p.clone(),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

fn prune(dir: &Path, prefix: &str, keep: usize) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with(prefix))
        .collect();
    names.sort_unstable_by(|a, b| b.cmp(a)); // names embed the date, so this is newest first
    for n in names.into_iter().skip(keep) {
        let _ = fs::remove_file(dir.join(n));
    }
}

fn local_time() -> windows::Win32::Foundation::SYSTEMTIME {
    unsafe { windows::Win32::System::SystemInformation::GetLocalTime() }
}

fn local_date() -> String {
    let t = local_time();
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

fn local_stamp() -> String {
    let t = local_time();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

/// Content hash of a data file, to recognise our own writes.
pub(crate) fn signature(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn emit_settings(app: &AppHandle, origin: &str, settings: &Value) {
    let _ = app.emit("store://settings", SettingsPayload { origin, settings });
}

/// Patch settings from Rust (tray menu, etc.) and run the same side effects as the UI path.
pub fn patch_settings_from_rust(app: &AppHandle, store: &Store, patch: Value) {
    let (old, new) = store.patch_settings(&patch);
    emit_settings(app, "rust", &new);
    crate::on_settings_changed(app, &old, &new);
}

/// Store bounds and similar values without notifying windows.
pub fn patch_settings_silent(store: &Store, patch: Value) {
    store.patch_settings(&patch);
}

// ---------------------------------------------------------------------------------------
// Commands

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResult {
    doc: Value,
    recovered: Option<String>,
}

#[tauri::command]
pub fn store_get(store: State<'_, Arc<Store>>) -> GetResult {
    GetResult {
        doc: store.snapshot(),
        recovered: store.take_recovered(),
    }
}

#[tauri::command]
pub fn store_put(
    app: AppHandle,
    webview: Webview,
    store: State<'_, Arc<Store>>,
    collection: String,
    items: Map<String, Value>,
) -> Result<(), String> {
    if !COLLECTIONS.contains(&collection.as_str()) {
        return Err(format!("unknown collection: {collection}"));
    }
    store.put(&collection, &items);
    let _ = app.emit(
        "store://changed",
        ChangedPayload { origin: webview.label(), collection: &collection, items: &items },
    );
    Ok(())
}

#[tauri::command]
pub fn store_patch_settings(
    app: AppHandle,
    webview: Webview,
    store: State<'_, Arc<Store>>,
    patch: Value,
) -> Result<(), String> {
    if !patch.is_object() {
        return Err("settings patch must be an object".into());
    }
    let (old, new) = store.patch_settings(&patch);
    emit_settings(&app, webview.label(), &new);
    crate::on_settings_changed(&app, &old, &new);
    Ok(())
}

#[tauri::command]
pub fn store_replace(
    app: AppHandle,
    webview: Webview,
    store: State<'_, Arc<Store>>,
    doc: Value,
) -> Result<(), String> {
    if !doc.is_object() {
        return Err("document must be an object".into());
    }
    store.manual_backup("pre-import").map_err(|e| e.to_string())?;
    let old = store.snapshot()["settings"].clone();
    store.replace(normalize(doc));
    let _ = app.emit("store://reload", ReloadPayload { origin: webview.label() });
    crate::on_settings_changed(&app, &old, &store.snapshot()["settings"]);
    Ok(())
}

#[derive(Serialize)]
pub struct BackupInfo {
    name: String,
    size: u64,
    modified: u64,
}

#[tauri::command]
pub fn backups_list(store: State<'_, Arc<Store>>) -> Vec<BackupInfo> {
    let dir = store.backups_dir();
    let mut list: Vec<BackupInfo> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| {
                let e = e.ok()?;
                let name = e.file_name().into_string().ok()?;
                let meta = e.metadata().ok()?;
                let modified = meta
                    .modified()
                    .ok()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_millis() as u64;
                name.ends_with(".json").then_some(BackupInfo { name, size: meta.len(), modified })
            })
            .collect()
        })
        .unwrap_or_default();
    list.sort_by(|a, b| b.modified.cmp(&a.modified));
    list
}

#[tauri::command]
pub fn backup_create(store: State<'_, Arc<Store>>) -> Result<String, String> {
    store.manual_backup("manual").map_err(|e| e.to_string())
}

#[tauri::command]
pub fn backup_restore(
    app: AppHandle,
    webview: Webview,
    store: State<'_, Arc<Store>>,
    name: String,
) -> Result<(), String> {
    // Backup names are generated by us: daily-2026-10-29.json, manual-20261029-101500.json…
    let safe = name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !safe || name.contains("..") || !name.ends_with(".json") {
        return Err("invalid backup name".into());
    }
    let text = fs::read_to_string(store.backups_dir().join(&name)).map_err(|e| e.to_string())?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| format!("备份文件无法解析：{e}"))?;
    if !doc.is_object() {
        return Err("备份文件格式不正确".into());
    }
    store.manual_backup("pre-restore").map_err(|e| e.to_string())?;
    let old = store.snapshot()["settings"].clone();
    store.replace(normalize(doc));
    let _ = app.emit("store://reload", ReloadPayload { origin: webview.label() });
    crate::on_settings_changed(&app, &old, &store.snapshot()["settings"]);
    Ok(())
}
