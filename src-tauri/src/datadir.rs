//! Choosing the data folder (DAT-04). `<app data>\datadir.txt` points at a custom folder
//! (e.g. in OneDrive). Settings then stay per PC in `<app data>\settings.json`; data.json in
//! the folder holds the shared tasks. Switching folders merges and restarts the app.

use crate::store::{normalize, Store};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

const POINTER: &str = "datadir.txt";
const LOCAL_SETTINGS: &str = "settings.json";

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DataDir {
    pub path: String,
    pub custom: bool,
    /// The custom folder configured but not reachable at start (offline drive…).
    pub missing: Option<String>,
}

/// (data folder, local settings file if the folder is shared, status for the UI)
pub fn resolve(default: &Path) -> (PathBuf, Option<PathBuf>, DataDir) {
    let configured = std::fs::read_to_string(default.join(POINTER)).ok().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    match configured {
        Some(p) if Path::new(&p).is_dir() => (PathBuf::from(&p), Some(default.join(LOCAL_SETTINGS)), DataDir { path: p, custom: true, missing: None }),
        Some(p) => (default.to_path_buf(), None, DataDir { path: default.display().to_string(), custom: false, missing: Some(p) }),
        None => (default.to_path_buf(), None, DataDir { path: default.display().to_string(), custom: false, missing: None }),
    }
}

fn read_doc(path: &Path) -> Option<Value> {
    let t = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(t.trim_start_matches('\u{FEFF}')).ok().filter(Value::is_object).map(normalize)
}

fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(v).unwrap_or_default()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn data_dir_info(state: State<'_, DataDir>) -> DataDir {
    state.inner().clone()
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle, window: tauri::Window) -> Option<String> {
    app.dialog().file().set_parent(&window).blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

/// Moves the data to `path` (None = back to the default folder). Data already in the target
/// is merged with ours, never overwritten. Restarts the app.
#[tauri::command]
pub fn data_dir_set(app: AppHandle, store: State<'_, Arc<Store>>, path: Option<String>) -> Result<(), String> {
    let default = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let target = path.map(PathBuf::from).unwrap_or_else(|| default.clone());
    if !target.is_dir() {
        return Err("文件夹不存在".into());
    }
    std::fs::create_dir_all(target.join("backups")).map_err(|e| e.to_string())?;
    store.flush();
    let mut doc = store.snapshot();
    let to_default = target == default;
    let file = target.join("data.json");
    if let Some(theirs) = read_doc(&file) {
        crate::merge::merge(&mut doc, &theirs);
    }
    if to_default {
        write_json(&file, &doc)?;
        let _ = std::fs::remove_file(default.join(POINTER));
        let _ = std::fs::remove_file(default.join(LOCAL_SETTINGS));
    } else {
        let settings = doc.as_object_mut().and_then(|o| o.remove("settings")).unwrap_or(Value::Null);
        write_json(&file, &doc)?;
        write_json(&default.join(LOCAL_SETTINGS), &settings)?;
        std::fs::write(default.join(POINTER), target.display().to_string()).map_err(|e| e.to_string())?;
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        crate::restart(&handle);
    });
    Ok(())
}
