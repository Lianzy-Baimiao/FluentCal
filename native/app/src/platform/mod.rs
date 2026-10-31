//! Windows integration: the desk window's layer, screens, the hidden message window
//! (single instance, hotkeys), autostart, notifications, system theme.

pub mod autostart;
pub mod desk;
pub mod dialog;
pub mod msgwin;
pub mod net;
pub mod screens;
pub mod sysinfo;
pub mod toast;
pub mod tray;
pub mod window;

/// A development build or a test run (FLUENTCAL_DATA_DIR): its own instance, credentials,
/// notification ID; never the installed program's Run value. Release test runs need this
/// too: with the installed program's instance name they would hand over to it and exit.
pub fn isolated() -> bool {
    cfg!(debug_assertions) || std::env::var_os("FLUENTCAL_DATA_DIR").is_some()
}
