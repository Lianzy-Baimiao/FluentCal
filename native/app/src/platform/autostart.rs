//! Start with Windows: the HKCU Run value "FluentCal" (the same name 0.2's autostart plugin
//! used, so installing the native version over 0.2 takes the value over) and its Task Manager
//! approval. Debug builds never write it.

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::*;

const RUN: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
const APPROVED: PCWSTR = w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run");
const NAME: PCWSTR = w!("FluentCal");
/// StartupApproved value meaning "enabled" (Task Manager writes 03 … when disabled).
const ENABLED: [u8; 12] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(format!("\"{}\" --autostart", exe.display()))
}

fn read_run() -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe { RegGetValueW(HKEY_CURRENT_USER, RUN, NAME, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut size)) };
    r.is_ok().then(|| String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1)]))
}

pub fn is_enabled() -> bool {
    read_run().is_some()
}

/// Makes the Run value (or, from the Store package, the startup task) match the setting.
/// The installed program only, never a test run.
pub fn sync(want: bool) {
    if super::isolated() {
        return;
    }
    if super::package::is_packaged() {
        return super::package::sync_startup_task(want);
    }
    let Some(cmd) = command() else { return };
    unsafe {
        if want {
            if read_run().as_deref() == Some(cmd.as_str()) {
                return;
            }
            let wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            let _ = RegSetKeyValueW(HKEY_CURRENT_USER, RUN, NAME, REG_SZ.0, Some(bytes.as_ptr() as *const _), bytes.len() as u32);
            let mut key = HKEY::default();
            if RegOpenKeyExW(HKEY_CURRENT_USER, APPROVED, Some(0), KEY_SET_VALUE, &mut key).is_ok() {
                let _ = RegSetValueExW(key, NAME, Some(0), REG_BINARY, Some(&ENABLED));
                let _ = RegCloseKey(key);
            }
        } else if read_run().is_some() {
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, NAME);
            let _ = RegDeleteKeyValueW(HKEY_CURRENT_USER, APPROVED, NAME);
        }
    }
}
