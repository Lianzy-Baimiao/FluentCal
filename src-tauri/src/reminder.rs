//! Reminder scheduling. The frontend expands recurrence rules and sends the next ~48 hours
//! of reminders; this thread fires them as Windows notifications even when no window is
//! visible (the app lives in the tray).

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_notification::NotificationExt;

/// Reminders more than this late (e.g. the PC was asleep) are dropped instead of fired.
const GRACE_MS: i64 = 10 * 60 * 1000;

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    /// Unique per occurrence, e.g. "taskId@2026-10-30#-15".
    pub key: String,
    pub at_ms: i64,
    pub title: String,
    pub body: String,
    pub task_id: String,
    pub date: String,
}

#[derive(Default)]
pub(crate) struct Inner {
    pub(crate) items: Vec<Reminder>,
    /// key → time fired (ms). Persisted so a restart doesn't fire the same reminder again.
    pub(crate) fired: HashMap<String, i64>,
    /// "Remind me later" items; kept apart because `items` is replaced by the frontend.
    pub(crate) snoozed: Vec<Reminder>,
    /// Reminders shown in the popup window, oldest first.
    pub(crate) popup: Vec<Reminder>,
}

pub struct Reminders {
    pub(crate) inner: Mutex<Inner>,
    pub(crate) wake: Condvar,
    file: PathBuf,
    pub(crate) snooze_file: PathBuf,
}

const KEEP_FIRED_MS: i64 = 25 * 3600 * 1000;

fn load_fired(file: &PathBuf) -> HashMap<String, i64> {
    let now = now_ms();
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<HashMap<String, i64>>(&t).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, at)| now - at < KEEP_FIRED_MS)
        .collect()
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

pub fn start(app: AppHandle, dir: PathBuf) -> Arc<Reminders> {
    let file = dir.join("fired-reminders.json");
    let snooze_file = dir.join("snoozed-reminders.json");
    let fired = load_fired(&file);
    let snoozed = crate::reminder_popup::load_snoozed(&snooze_file);
    let inner = Inner { fired, snoozed, ..Default::default() };
    let state = Arc::new(Reminders { inner: Mutex::new(inner), wake: Condvar::new(), file, snooze_file });
    let worker = state.clone();
    std::thread::Builder::new()
        .name("reminders".into())
        .spawn(move || worker.run(&app))
        .expect("spawn reminder thread");
    state
}

impl Reminders {
    fn run(&self, app: &AppHandle) {
        let mut g = self.inner.lock().unwrap();
        loop {
            let now = now_ms();
            let due: Vec<Reminder> = g
                .items
                .iter()
                .chain(g.snoozed.iter())
                .filter(|r| r.at_ms <= now && r.at_ms > now - GRACE_MS && !g.fired.contains_key(&r.key))
                .cloned()
                .collect();
            if !due.is_empty() {
                for r in &due {
                    g.fired.insert(r.key.clone(), now);
                }
                let before = g.snoozed.len();
                g.snoozed.retain(|s| !due.iter().any(|d| d.key == s.key));
                if g.snoozed.len() != before {
                    crate::reminder_popup::save_snoozed(&self.snooze_file, &g.snoozed);
                }
                g.fired.retain(|_, at| now - *at < KEEP_FIRED_MS);
                let text = serde_json::to_string(&g.fired).unwrap_or_default();
                drop(g);
                let _ = std::fs::write(&self.file, text);
                for r in &due {
                    fire(app, r);
                }
                g = self.inner.lock().unwrap();
                continue;
            }
            // Wake at the next reminder, but at least once a minute so sleep/clock changes
            // are picked up.
            let next = g
                .items
                .iter()
                .chain(g.snoozed.iter())
                .filter(|r| r.at_ms > now && !g.fired.contains_key(&r.key))
                .map(|r| r.at_ms - now)
                .min()
                .unwrap_or(i64::MAX)
                .clamp(50, 60_000);
            g = self.wake.wait_timeout(g, Duration::from_millis(next as u64)).unwrap().0;
        }
    }
}

fn fire(app: &AppHandle, r: &Reminder) {
    let style = crate::reminder_popup::style(app);
    if style != "popup" {
        if let Err(e) = app.notification().builder().title(&r.title).body(&r.body).show() {
            eprintln!("[reminder] notification failed: {e}");
        }
    }
    if style != "toast" {
        crate::reminder_popup::show(app, r, style == "popup");
    }
    let _ = app.emit("reminder://fired", r);
}

#[tauri::command]
pub fn reminders_set(state: State<'_, Arc<Reminders>>, items: Vec<Reminder>) {
    let mut g = state.inner.lock().unwrap();
    // Forget fired keys that are no longer scheduled and old enough not to come back.
    let keys: HashSet<&str> = items.iter().map(|r| r.key.as_str()).collect();
    let now = now_ms();
    g.fired.retain(|k, at| keys.contains(k.as_str()) || now - *at < KEEP_FIRED_MS);
    g.items = items;
    state.wake.notify_all();
}
