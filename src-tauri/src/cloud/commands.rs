//! Commands for the sync settings page, the calendar's sync icon and the tray.

use super::secrets;
use super::service::{Cloud, Status};
use crate::store::{patch_settings_from_rust, Store};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

fn cloud(app: &AppHandle) -> Arc<Cloud> {
    app.state::<Arc<Cloud>>().inner().clone()
}

/// Settings patches touch windows and the tray: run them on the main thread.
fn patch_on_main(app: &AppHandle, patch: Value) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let store = handle.state::<Arc<Store>>().inner().clone();
        patch_settings_from_rust(&handle, &store, patch);
    });
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sync_status(cloud: State<'_, Arc<Cloud>>) -> Status {
    cloud.status()
}

/// "Sync now": runs a round (also after an auth error) and returns the new status.
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Result<Status, String> {
    let c = cloud(&app);
    blocking(move || {
        let _ = c.sync_now();
        c.status()
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    exists: bool,
    tasks: usize,
    groups: usize,
    device: Option<String>,
    saved_at: Option<i64>,
}

/// "Test connection" with the current settings, before sync is turned on.
#[tauri::command]
pub async fn sync_probe(app: AppHandle) -> Result<ProbeResult, String> {
    let c = cloud(&app);
    let p = blocking(move || c.probe()).await?.map_err(|e| e.message)?;
    Ok(ProbeResult { exists: p.exists, tasks: p.tasks, groups: p.groups, device: p.device, saved_at: p.saved_at })
}

/// Turns sync on after a first round: merge both sides, or `replace` this PC's tasks with
/// the cloud copy (backed up first either way).
#[tauri::command]
pub async fn sync_enable(app: AppHandle, replace: bool) -> Result<Status, String> {
    let c = cloud(&app);
    let status = blocking(move || c.enable(replace).map(|_| c.status())).await?.map_err(|e| e.message)?;
    patch_on_main(&app, json!({ "sync": { "enabled": true } }));
    Ok(status)
}

#[tauri::command]
pub fn sync_disable(app: AppHandle, store: State<'_, Arc<Store>>) {
    patch_settings_from_rust(&app, &store, json!({ "sync": { "enabled": false } }));
}

/// Saves a password or token in Windows Credential Manager (empty = delete).
#[tauri::command]
pub fn sync_set_secret(cloud: State<'_, Arc<Cloud>>, key: String, value: String) -> Result<(), String> {
    if !secrets::KEYS.contains(&key.as_str()) {
        return Err("unknown secret".into());
    }
    let value = value.trim();
    if value.is_empty() {
        secrets::delete(&key);
    } else {
        secrets::set(&key, value)?;
    }
    cloud.secrets_changed();
    Ok(())
}

/// Which secrets are saved (never their values).
#[tauri::command]
pub fn sync_secrets() -> Vec<&'static str> {
    secrets::KEYS.iter().copied().filter(|k| secrets::get(k).is_some()).collect()
}
