//! The web view's server (0.2's webcmd.rs): "在浏览器中查看" on this PC, the LAN page.

use crate::app::App;

impl App {
    /// "在浏览器中查看": the read-only web view in this PC's default browser.
    pub fn open_in_browser(&mut self) {
        self.notify("网页查看还在移植中", false);
    }
}
