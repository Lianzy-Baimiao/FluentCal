//! File dialogs (0.2: tauri-plugin-dialog): save as, open, pick a folder. Modal to `owner`;
//! they run their own message loop, so call them outside `app::run` (the app stays usable for
//! the window's events meanwhile) — see App::save_file.

use windows::core::{Interface, HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH};

fn path_of(d: &IFileDialog) -> Option<String> {
    unsafe {
        let item = d.GetResult().ok()?;
        let p = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

fn show(d: &IFileDialog, owner: Option<HWND>, filter: Option<(&str, &[&str])>, name: Option<&str>, folders: bool) -> Option<String> {
    crate::platform::desk::allow_foreground();
    unsafe {
        let opts = d.GetOptions().ok()?;
        let _ = d.SetOptions(opts | FOS_FORCEFILESYSTEM | if folders { FOS_PICKFOLDERS } else { Default::default() });
        // The strings must live until the dialog closes.
        let label;
        let spec;
        if let Some((title, exts)) = filter {
            label = HSTRING::from(title);
            spec = HSTRING::from(exts.iter().map(|e| format!("*.{e}")).collect::<Vec<_>>().join(";"));
            let f = [COMDLG_FILTERSPEC { pszName: PCWSTR(label.as_ptr()), pszSpec: PCWSTR(spec.as_ptr()) }];
            let _ = d.SetFileTypes(&f);
            if let Some(e) = exts.first() {
                let _ = d.SetDefaultExtension(&HSTRING::from(*e));
            }
        }
        if let Some(n) = name {
            let _ = d.SetFileName(&HSTRING::from(n));
        }
        d.Show(owner).ok()?;
        path_of(d)
    }
}

fn init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

/// "另存为": the chosen path, None when cancelled.
pub fn save_file(owner: Option<HWND>, default_name: &str, filter_name: &str, exts: &[&str]) -> Option<String> {
    init();
    let d: IFileSaveDialog = unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).ok()? };
    show(&d.cast().ok()?, owner, Some((filter_name, exts)), Some(default_name), false)
}

pub fn open_file(owner: Option<HWND>, filter_name: &str, exts: &[&str]) -> Option<String> {
    init();
    let d: IFileOpenDialog = unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()? };
    show(&d.cast().ok()?, owner, Some((filter_name, exts)), None, false)
}

pub fn pick_folder(owner: Option<HWND>) -> Option<String> {
    init();
    let d: IFileOpenDialog = unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()? };
    show(&d.cast().ok()?, owner, None, None, true)
}

/// Opens a folder in Explorer.
pub fn open_folder(path: &str) {
    use windows::core::w;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    crate::platform::desk::allow_foreground();
    unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(path), None, None, SW_SHOWNORMAL);
    }
}
