//! The settings window (0.2's src/windows/settings).

use crate::app::App;

impl App {
    /// Opens the settings window (on `page`, e.g. "appearance", "web").
    pub fn open_settings(&mut self, _page: Option<&str>) {
        self.notify("设置窗口还在移植中", false);
    }
}
