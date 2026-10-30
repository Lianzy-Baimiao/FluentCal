//! Passwords and tokens for cloud sync live in Windows Credential Manager (per user, protected
//! by DPAPI), never in data.json, backups, exports or error messages.

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

/// Keys the UI may set (see commands.rs).
pub const KEYS: [&str; 5] = ["webdav", "github", "gitee", "google-secret", "google-refresh"];

fn target(key: &str) -> HSTRING {
    HSTRING::from(format!("FluentCal/sync/{key}"))
}

pub fn set(key: &str, value: &str) -> Result<(), String> {
    if value.len() > 2048 {
        return Err("内容太长".into());
    }
    let target = target(key);
    let mut blob = value.as_bytes().to_vec();
    let cred = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_ptr() as *mut u16),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        ..Default::default()
    };
    unsafe { CredWriteW(&cred, 0) }.map_err(|e| format!("无法写入 Windows 凭据管理器：{}", e.message()))
}

pub fn get(key: &str) -> Option<String> {
    let mut p: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(&target(key), CRED_TYPE_GENERIC, None, &mut p).ok()?;
        let c = &*p;
        let bytes = if c.CredentialBlob.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec()
        };
        CredFree(p as *const _);
        String::from_utf8(bytes).ok().filter(|s| !s.is_empty())
    }
}

pub fn delete(key: &str) {
    let _ = unsafe { CredDeleteW(&target(key), CRED_TYPE_GENERIC, None) };
}
