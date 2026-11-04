//! JSON document store. One `data.json` holds tasks, groups, day marks, holiday data and
//! settings. Rust treats the document as opaque JSON; the frontend owns the schema.
//! Mutations mark the store dirty, a background thread writes atomically (temp + rename)
//! after a short debounce, and every change is broadcast so all windows stay in sync.

use crate::i18n::{tr, trf};

use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

/// What the store tells the app (the native replacement of 0.2's `store://…` events).
#[derive(Debug, Clone)]
pub enum StoreEvent {
    /// Items written (`origin`: who wrote them, e.g. "desk", "editor", "sync").
    Changed { origin: String, collection: String, items: Map<String, Value> },
    /// Settings changed; `old` and `new` are the whole settings objects.
    Settings { origin: String, old: Value, new: Value },
    /// The whole document changed (import, restore, external merge): reload everything.
    Reload { origin: String },
    /// Saving failed (Some) or works again (None).
    SaveStatus(Option<String>),
}

pub type Listener = Box<dyn Fn(&StoreEvent) + Send + Sync>;

pub const COLLECTIONS: [&str; 4] = ["tasks", "groups", "dayMarks", "holidays"];
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);
const DAILY_BACKUPS_KEPT: usize = 14;
/// In backups/: the last saved content. Recovery takes the newest valid file there, which is
/// this one; the backup list in Settings leaves it out (it equals the current data).
pub const LATEST_COPY: &str = "last-saved.json";
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
    listeners: Mutex<Vec<Arc<Listener>>>,
    save_failing: AtomicBool,
    data_gen: AtomicU64,
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
            listeners: Mutex::new(Vec::new()),
            save_failing: AtomicBool::new(false),
            data_gen: AtomicU64::new(0),
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
            // A copy of what was just saved, so a data.json damaged later (disk, another
            // program) comes back without losing anything since the daily backup (REL-04).
            // Not in a shared data folder: the sync tool would upload it on every save, and
            // keeps file versions itself.
            if self.local_settings.is_none() {
                if let Err(e) = write_atomic(&self.backups_dir().join(LATEST_COPY), text.as_bytes()) {
                    eprintln!("[store] copy of the last save failed: {e}");
                }
            }
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

    /// Called (on the thread that made the change) for every StoreEvent.
    pub fn listen(&self, f: impl Fn(&StoreEvent) + Send + Sync + 'static) {
        self.listeners.lock().unwrap().push(Arc::new(Box::new(f)));
    }

    pub(crate) fn emit(&self, e: StoreEvent) {
        // Copy the list first: a listener may read the store.
        let ls: Vec<Arc<Listener>> = self.listeners.lock().unwrap().clone();
        for l in ls {
            l(&e);
        }
    }

    /// Tells the app to reload the document (e.g. after merging an external change).
    pub(crate) fn emit_reload(&self) {
        self.emit(StoreEvent::Reload { origin: "sync".into() });
    }

    fn notify_save(&self, error: Option<String>) {
        self.emit(StoreEvent::SaveStatus(error));
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

    /// Counts changes to the collections (not settings), so cloud sync can tell edits apart
    /// from window moves.
    pub fn data_gen(&self) -> u64 {
        self.data_gen.load(Ordering::Relaxed)
    }

    pub(crate) fn bump_data(&self) {
        self.data_gen.fetch_add(1, Ordering::Relaxed);
    }

    /// Merges the synced collections of a cloud copy (see cloud::cloud_view) into ours.
    /// Returns true when our data changed; windows are told to reload.
    pub fn absorb_cloud(&self, view: &Value) -> bool {
        let changed = {
            let mut g = self.inner.lock().unwrap();
            let before = crate::cloud::cloud_view(&g.doc);
            let changed = crate::merge::merge_cols(&mut g.doc, view, &crate::cloud::SYNCED);
            crate::merge::prune(&mut g.doc, now_ms());
            // Also new tombstones alone: they must reach the disk, or a deletion could come back.
            if crate::cloud::cloud_view(&g.doc) != before {
                self.mark_dirty(&mut g);
            }
            changed
        };
        if changed {
            self.bump_data();
            self.emit_reload();
        }
        changed
    }

    /// "Use the cloud data": replaces the synced collections with the cloud copy (backed up
    /// first). Settings and holiday data stay.
    pub fn replace_cloud(&self, view: &Value) -> Result<(), String> {
        self.manual_backup("pre-sync").map_err(|e| e.to_string())?;
        {
            let mut g = self.inner.lock().unwrap();
            for col in crate::cloud::SYNCED {
                g.doc[col] = view.get(col).cloned().unwrap_or_else(|| json!({}));
                g.doc["meta"][col] = view.pointer(&format!("/meta/{col}")).cloned().unwrap_or_else(|| json!({}));
            }
            self.mark_dirty(&mut g);
        }
        self.bump_data();
        self.emit_reload();
        Ok(())
    }

    /// Backup before a sync merges into this PC's data for the first time.
    pub fn backup(&self, prefix: &str) -> Result<String, String> {
        self.manual_backup(prefix).map_err(|e| e.to_string())
    }

    pub fn put(&self, collection: &str, items: &Map<String, Value>) {
        self.bump_data();
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
    /// This PC's cloud sync settings are kept: they go with passwords in the Windows
    /// credential store, which a backup from another PC doesn't have.
    fn replace(&self, mut doc: Value) {
        self.bump_data();
        let mut g = self.inner.lock().unwrap();
        let old_meta = g.doc.get("meta").cloned();
        crate::merge::stamp_all(&mut doc, old_meta.as_ref(), now_ms());
        let sync = g.doc.pointer("/settings/sync").cloned();
        if let (Some(s), Some(settings)) = (sync, doc.get_mut("settings").and_then(Value::as_object_mut)) {
            settings.insert("sync".into(), s);
        }
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
    // newest first by modification time; on a tie (same millisecond) the copy of the last
    // save, which is never older than the others
    names.sort_by_key(|n| {
        (std::cmp::Reverse(fs::metadata(dir.join(n)).and_then(|m| m.modified()).ok()), n != LATEST_COPY)
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

/// Store bounds and similar values without notifying anyone.
pub fn patch_settings_silent(store: &Store, patch: Value) {
    store.patch_settings(&patch);
}

// ---------------------------------------------------------------------------------------
// What 0.2's Tauri commands did, as methods (the app calls them directly).

impl Store {
    /// Writes items (null = delete) and tells the listeners.
    pub fn write(&self, origin: &str, collection: &str, items: Map<String, Value>) -> Result<(), String> {
        if !COLLECTIONS.contains(&collection) {
            return Err(format!("unknown collection: {collection}"));
        }
        self.put(collection, &items);
        self.emit(StoreEvent::Changed { origin: origin.into(), collection: collection.into(), items });
        Ok(())
    }

    /// Deep-merges a settings patch (null removes a key) and tells the listeners.
    pub fn update_settings(&self, origin: &str, patch: &Value) -> Result<(), String> {
        if !patch.is_object() {
            return Err("settings patch must be an object".into());
        }
        let (old, new) = self.patch_settings(patch);
        self.emit(StoreEvent::Settings { origin: origin.into(), old, new });
        Ok(())
    }

    /// Import: replaces the document (backed up first) and tells the listeners.
    pub fn import(&self, origin: &str, doc: Value) -> Result<(), String> {
        if !doc.is_object() {
            return Err("document must be an object".into());
        }
        self.manual_backup("pre-import").map_err(|e| e.to_string())?;
        let old = self.snapshot()["settings"].clone();
        self.replace(normalize(doc));
        self.emit(StoreEvent::Reload { origin: origin.into() });
        self.emit(StoreEvent::Settings { origin: origin.into(), old, new: self.snapshot()["settings"].clone() });
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub name: String,
    pub size: u64,
    pub modified: u64,
}

impl Store {
pub fn backups_list(&self) -> Vec<BackupInfo> {
    let dir = self.backups_dir();
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
                (name.ends_with(".json") && name != LATEST_COPY).then_some(BackupInfo { name, size: meta.len(), modified })
            })
            .collect()
        })
        .unwrap_or_default();
    list.sort_by(|a, b| b.modified.cmp(&a.modified));
    list
}

pub fn backup_create(&self) -> Result<String, String> {
    self.manual_backup("manual").map_err(|e| e.to_string())
}

/// Restores a backup (this PC's data backed up first) and tells the listeners.
pub fn backup_restore(&self, origin: &str, name: &str) -> Result<(), String> {
    // Backup names are generated by us: daily-2026-10-29.json, manual-20261029-101500.json…
    let safe = name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !safe || name.contains("..") || !name.ends_with(".json") {
        return Err("invalid backup name".into());
    }
    let text = fs::read_to_string(self.backups_dir().join(name)).map_err(|e| e.to_string())?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| trf("备份文件无法解析：{0}", &[format!("{}", e)]))?;
    if !doc.is_object() {
        return Err(tr("备份文件格式不正确").into());
    }
    self.manual_backup("pre-restore").map_err(|e| e.to_string())?;
    let old = self.snapshot()["settings"].clone();
    self.replace(normalize(doc));
    self.emit(StoreEvent::Reload { origin: origin.into() });
    self.emit(StoreEvent::Settings { origin: origin.into(), old, new: self.snapshot()["settings"].clone() });
    Ok(())
}
}
