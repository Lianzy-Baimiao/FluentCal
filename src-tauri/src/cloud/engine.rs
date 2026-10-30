//! One sync round (需求文档 7.5): conditional fetch → merge into the store → conditional
//! upload, retried on conflict. No Tauri here, so tests can drive it with a mock server.

use super::{cloud_view, decode, encode, view_sig, ErrKind, Fetched, Remote, Rev, SyncError};
use crate::store::{now_ms, Store};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ATTEMPTS: usize = 3;

/// Per-PC sync state, saved in cloud-state.json (outside a shared data folder).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    /// Which target this state belongs to (provider + address); reset when it changes.
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub rev: Option<Rev>,
    /// Signature of the synced content after the last successful round.
    #[serde(default)]
    pub synced_sig: Option<u64>,
    #[serde(default)]
    pub last_sync: Option<i64>,
    #[serde(default)]
    pub remote_device: Option<String>,
    #[serde(default)]
    pub remote_saved_at: Option<i64>,
    /// Provider ids (Drive folder and file).
    #[serde(default)]
    pub location: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    /// First sync with "use the cloud data": the cloud copy replaces this PC's tasks.
    Replace,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Outcome {
    /// This PC's data changed (merged or replaced from the cloud).
    pub downloaded: bool,
    pub uploaded: bool,
}

fn has_data(view: &Value) -> bool {
    super::SYNCED.iter().any(|c| view[c].as_object().is_some_and(|o| !o.is_empty()) || view["meta"][c].as_object().is_some_and(|o| !o.is_empty()))
}

/// Drops expired tombstones from a cloud view, the same way merge::prune does locally, so
/// the two sides compare equal after a merge.
fn pruned(mut view: Value) -> Value {
    let cutoff = now_ms() - crate::merge::TOMBSTONE_DAYS * 86_400_000;
    for col in super::SYNCED {
        let present: Vec<String> = view[col].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        if let Some(m) = view["meta"][col].as_object_mut() {
            m.retain(|id, t| present.contains(id) || t.as_i64().unwrap_or(0) >= cutoff);
        }
    }
    view
}

pub fn cycle(store: &Store, remote: &mut dyn Remote, st: &mut SyncState, mode: Mode) -> Result<Outcome, SyncError> {
    remote.check()?;
    let mut out = Outcome::default();
    let mut mode = mode;
    let mut last = SyncError::conflict();
    for _ in 0..ATTEMPTS {
        // A version without a matching content signature is useless for "not modified".
        let known = st.rev.clone().filter(|_| st.synced_sig.is_some() && mode == Mode::Normal);
        let (remote_view, rev) = match remote.fetch(known.as_ref())? {
            Fetched::NotModified => {
                let local = cloud_view(&store.snapshot());
                let sig = view_sig(&local);
                if Some(sig) == st.synced_sig {
                    finish(st, remote, None);
                    return Ok(out);
                }
                // The cloud is still what we synced last time: upload our edits on top.
                match upload(remote, &local, known.as_ref(), st) {
                    Ok(()) => {
                        out.uploaded = true;
                        return Ok(out);
                    }
                    Err(e) if e.kind == ErrKind::Conflict => {
                        st.synced_sig = None;
                        last = e;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            Fetched::Missing => (None, None),
            Fetched::File(bytes, rev) => match decode(&bytes)? {
                Some(f) => {
                    st.remote_device = f.device;
                    st.remote_saved_at = f.saved_at;
                    (Some(pruned(f.view)), Some(rev))
                }
                None => (None, Some(rev)),
            },
        };
        if let Some(v) = &remote_view {
            if mode == Mode::Replace {
                store.replace_cloud(v).map_err(SyncError::config)?;
                out.downloaded = true;
                mode = Mode::Normal;
            } else if store.absorb_cloud(v) {
                out.downloaded = true;
            }
        }
        let merged = cloud_view(&store.snapshot());
        let same = remote_view.as_ref().is_some_and(|v| *v == merged);
        if same || (remote_view.is_none() && !has_data(&merged)) {
            st.rev = rev;
            st.synced_sig = Some(view_sig(&merged));
            finish(st, remote, None);
            return Ok(out);
        }
        match upload(remote, &merged, rev.as_ref(), st) {
            Ok(()) => {
                out.uploaded = true;
                return Ok(out);
            }
            Err(e) if e.kind == ErrKind::Conflict => last = e,
            Err(e) => return Err(e),
        }
    }
    Err(SyncError::new(ErrKind::Server, format!("{}，重试 {ATTEMPTS} 次仍未成功，稍后会再试", last.message)))
}

fn upload(remote: &mut dyn Remote, view: &Value, base: Option<&Rev>, st: &mut SyncState) -> Result<(), SyncError> {
    let now = now_ms();
    let rev = remote.put(&encode(view, now), base)?;
    st.rev = Some(rev);
    st.synced_sig = Some(view_sig(view));
    st.remote_device = Some(super::device_name());
    st.remote_saved_at = Some(now);
    finish(st, remote, Some(now));
    Ok(())
}

fn finish(st: &mut SyncState, remote: &dyn Remote, at: Option<i64>) {
    st.last_sync = Some(at.unwrap_or_else(now_ms));
    let loc = remote.location();
    if !loc.is_null() {
        st.location = loc;
    }
}

/// What the cloud holds, for the "test connection" result and the first-sync dialog.
pub struct Probe {
    pub exists: bool,
    pub tasks: usize,
    pub groups: usize,
    pub device: Option<String>,
    pub saved_at: Option<i64>,
}

pub fn probe(remote: &mut dyn Remote) -> Result<Probe, SyncError> {
    remote.check()?;
    let file = match remote.fetch(None)? {
        Fetched::File(bytes, _) => decode(&bytes)?,
        _ => None,
    };
    let count = |f: &super::CloudFile, c: &str| f.view[c].as_object().map_or(0, |o| o.len());
    Ok(match file {
        Some(f) => Probe { exists: true, tasks: count(&f, "tasks"), groups: count(&f, "groups"), device: f.device, saved_at: f.saved_at },
        None => Probe { exists: false, tasks: 0, groups: 0, device: None, saved_at: None },
    })
}
