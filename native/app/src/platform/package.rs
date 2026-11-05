//! Running from the MSIX package (Microsoft Store). Package identity changes two things:
//! autostart is the manifest's startup task (not the Run value) and notifications use the
//! package's app ID. %APPDATA% is redirected copy-on-write to the package's own folder, so the
//! first start reads 0.2's data where it is, and later writes stay in the package.

use windows::core::HSTRING;

/// The startup task's id in package/AppxManifest.xml.
const STARTUP_TASK: &str = "FluentCalStartup";

/// Runs on a worker thread; only a successful Store API response means no update.
pub fn check_store_update(hwnd: isize) -> windows::core::Result<Option<u32>> {
    if !is_packaged() { return Ok(None); }
    use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
    unsafe { RoInitialize(RO_INIT_MULTITHREADED)?; }
    let result = (|| {
        let store = windows::Services::Store::StoreContext::GetDefault()?;
        if hwnd != 0 {
            use windows::core::Interface;
            let init: windows::Win32::UI::Shell::IInitializeWithWindow = store.cast()?;
            unsafe { init.Initialize(windows::Win32::Foundation::HWND(hwnd as *mut _))?; }
        }
        let updates = store.GetAppAndOptionalStorePackageUpdatesAsync()?.join()?;
        Ok(Some(updates.Size()?))
    })();
    unsafe { RoUninitialize(); }
    result
}
pub fn open_store() {
    use windows::{core::w, Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}};
    unsafe { ShellExecuteW(None, w!("open"), w!("ms-windows-store://pdp/?ProductId=9NTT06453DG5"), None, None, SW_SHOWNORMAL); }
}

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
