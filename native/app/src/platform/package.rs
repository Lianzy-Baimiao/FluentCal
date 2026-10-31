//! Running from the MSIX package (Microsoft Store). Package identity changes two things:
//! autostart is the manifest's startup task (not the Run value) and notifications use the
//! package's app ID. %APPDATA% is redirected copy-on-write to the package's own folder, so the
//! first start reads 0.2's data where it is, and later writes stay in the package.

use windows::core::HSTRING;

/// The startup task's id in package/AppxManifest.xml.
const STARTUP_TASK: &str = "FluentCalStartup";

pub fn is_packaged() -> bool {
    use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
    let mut len = 0u32;
    // ERROR_INSUFFICIENT_BUFFER (122) with a package; APPMODEL_ERROR_NO_PACKAGE (15700) without.
    unsafe { GetCurrentPackageFullName(&mut len, None) }.0 == 122
}

/// The packaged app's AppUserModelID (PackageFamilyName!App), None outside a package.
pub fn app_user_model_id() -> Option<String> {
    use windows::Win32::Storage::Packaging::Appx::GetCurrentApplicationUserModelId;
    if !is_packaged() {
        return None;
    }
    let mut len = 0u32;
    unsafe {
        let _ = GetCurrentApplicationUserModelId(&mut len, None);
        let mut buf = vec![0u16; len as usize];
        if GetCurrentApplicationUserModelId(&mut len, Some(windows::core::PWSTR(buf.as_mut_ptr()))).0 != 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]))
    }
}

/// Enables or disables the startup task (off the UI thread: the WinRT calls wait).
pub fn sync_startup_task(want: bool) {
    std::thread::spawn(move || {
        use windows::ApplicationModel::{StartupTask, StartupTaskState};
        let Ok(task) = StartupTask::GetAsync(&HSTRING::from(STARTUP_TASK)).and_then(|op| op.join()) else { return };
        match (want, task.State()) {
            (true, Ok(StartupTaskState::Disabled)) => {
                let _ = task.RequestEnableAsync().and_then(|op| op.join());
            }
            (false, Ok(StartupTaskState::Enabled)) => {
                let _ = task.Disable();
            }
            _ => {}
        }
    });
}
