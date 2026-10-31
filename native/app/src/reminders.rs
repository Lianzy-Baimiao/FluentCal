//! Reminder scheduling (0.2's reminder.rs + reminder_popup.rs state). The app computes the
//! next 48 hours of reminders (fluentcal_core::reminders) and hands them over; this thread
//! fires them as Windows notifications and/or reminder cards even when no window is visible.
//! Fired and snoozed reminders are per PC: kept next to the app's state, not in a shared data
//! folder.

use fluentcal_core::reminders::{ReminderItem, GRACE_MS};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
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

impl From<ReminderItem> for Reminder {
    fn from(r: ReminderItem) -> Self {
        Reminder { key: r.key, at_ms: r.at_ms, title: r.title, body: r.body, task_id: r.task_id, date: r.date }
    }
}

/// How a fired reminder is shown: settings.reminders.style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Toast,
    Popup,
    Both,
}

impl Style {
    pub fn parse(s: &str) -> Style {
        match s {
            "toast" => Style::Toast,
            "popup" => Style::Popup,
            _ => Style::Both,
        }
    }
}

#[derive(Default)]
struct Inner {
    items: Vec<Reminder>,
    /// key → time fired (ms). Persisted so a restart doesn't fire the same reminder again.
    fired: HashMap<String, i64>,
    /// "Remind me later" items; kept apart because `items` is replaced on every schedule.
    snoozed: Vec<Reminder>,
    /// Reminders shown on the cards, oldest first.
    popup: Vec<Reminder>,
    style: Option<Style>,
}

pub struct Reminders {
    inner: Mutex<Inner>,
    wake: Condvar,
    file: PathBuf,
    snooze_file: PathBuf,
    /// Called (on the reminder thread) after reminders fired: show toasts / the cards.
    on_fire: Box<dyn Fn(&[Reminder], Style) + Send + Sync>,
}

const KEEP_FIRED_MS: i64 = 25 * 3600 * 1000;

fn now_ms() -> i64 {
    fluentcal_core::date::now_ms()
}

fn load<T: serde::de::DeserializeOwned + Default>(file: &Path) -> T {
    std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

impl Reminders {
    pub fn start(dir: &Path, on_fire: impl Fn(&[Reminder], Style) + Send + Sync + 'static) -> Arc<Reminders> {
        let file = dir.join("fired-reminders.json");
        let snooze_file = dir.join("snoozed-reminders.json");
        let now = now_ms();
        let fired: HashMap<String, i64> = load::<HashMap<String, i64>>(&file).into_iter().filter(|(_, at)| now - at < KEEP_FIRED_MS).collect();
        let snoozed = load(&snooze_file);
        let inner = Inner { fired, snoozed, ..Default::default() };
        let state = Arc::new(Reminders { inner: Mutex::new(inner), wake: Condvar::new(), file, snooze_file, on_fire: Box::new(on_fire) });
        let worker = state.clone();
        std::thread::Builder::new().name("reminders".into()).spawn(move || worker.run()).expect("spawn reminder thread");
        state
    }

    fn run(&self) {
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
                    let _ = std::fs::write(&self.snooze_file, serde_json::to_string(&g.snoozed).unwrap_or_default());
                }
                g.fired.retain(|_, at| now - *at < KEEP_FIRED_MS);
                let style = g.style.unwrap_or(Style::Both);
                if style != Style::Toast {
                    for r in &due {
                        g.popup.retain(|x| x.key != r.key);
                        g.popup.push(r.clone());
                    }
                }
                let text = serde_json::to_string(&g.fired).unwrap_or_default();
                drop(g);
                let _ = std::fs::write(&self.file, text);
                (self.on_fire)(&due, style);
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

    /// The reminders of the next hours (replaces the previous schedule).
    pub fn set(&self, items: Vec<Reminder>, style: Style) {
        let mut g = self.inner.lock().unwrap();
        // Forget fired keys that are no longer scheduled and old enough not to come back.
        let keys: HashSet<&str> = items.iter().map(|r| r.key.as_str()).collect();
        let now = now_ms();
        g.fired.retain(|k, at| keys.contains(k.as_str()) || now - *at < KEEP_FIRED_MS);
        g.items = items;
        g.style = Some(style);
        self.wake.notify_all();
    }

    /// Reminders on the cards, oldest first.
    pub fn popup_list(&self) -> Vec<Reminder> {
        self.inner.lock().unwrap().popup.clone()
    }

    pub fn dismiss(&self, key: &str) {
        self.inner.lock().unwrap().popup.retain(|x| x.key != key);
    }

    /// "Remind me later": fires the same reminder again after `minutes`.
    pub fn snooze(&self, key: &str, minutes: i64) {
        let mut g = self.inner.lock().unwrap();
        let Some(pos) = g.popup.iter().position(|x| x.key == key) else { return };
        let mut r = g.popup.remove(pos);
        let at = now_ms() + minutes.clamp(1, 24 * 60) * 60_000;
        let base = r.key.split("#snooze").next().unwrap_or(&r.key).to_owned();
        r.key = format!("{base}#snooze{at}");
        r.at_ms = at;
        g.snoozed.push(r);
        let _ = std::fs::write(&self.snooze_file, serde_json::to_string(&g.snoozed).unwrap_or_default());
        self.wake.notify_all();
    }
}
