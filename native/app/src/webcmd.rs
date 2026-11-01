//! The web view's server (0.2's webcmd.rs; the server itself is fluentcal_core::web): settings
//! web.enabled / web.port, "在浏览器中查看" on this PC, the access code in Windows Credential
//! Manager. Runs only when used: loopback for this PC's browser, all interfaces only while
//! the LAN switch is on.

use crate::app::App;
use fluentcal_core::cloud::secrets;
use fluentcal_core::web::{self, Asset, Ctx, Running};
use std::sync::{Arc, Mutex};

mod assets {
    include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));
}

pub const DEFAULT_PORT: u16 = 18765;
const TOKEN_KEY: &str = "web-token";

#[derive(Default)]
pub struct WebState {
    running: Option<Running>,
    pub error: Option<String>,
    token: Arc<Mutex<String>>,
    accent: Arc<Mutex<String>>,
    /// This PC's browser asked for the page; until FluentCal exits.
    local: bool,
}

/// For the settings page.
pub struct WebInfo {
    pub enabled: bool,
    pub running: bool,
    pub port: u16,
    pub error: Option<String>,
    /// (ip, adapter, virtual, link)
    pub addresses: Vec<(String, String, bool, String)>,
    pub local_url: String,
}

fn new_token() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("OS random generator");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn link(ip: &str, port: u16, token: &str) -> String {
    format!("http://{ip}:{port}/?w=web#k={token}")
}

fn asset(path: &str) -> Option<Asset> {
    assets::FILES.iter().find(|(p, _, _)| *p == path).map(|(_, mime, bytes)| Asset { bytes: bytes.to_vec(), mime: (*mime).to_owned() })
}

impl App {
    fn web_port(&self) -> u16 {
        self.doc.settings.web.port.clamp(1024, 65535)
    }

    fn web_token(&mut self) -> String {
        let mut t = self.web.token.lock().unwrap();
        if t.is_empty() {
            *t = secrets::get(TOKEN_KEY).unwrap_or_default();
        }
        if t.is_empty() {
            *t = new_token();
            let _ = secrets::set(TOKEN_KEY, &t);
        }
        t.clone()
    }

    /// Starts, restarts or stops the server to match the settings: all interfaces when other
    /// devices may view, loopback when only this PC's browser asked, otherwise none.
    pub fn web_apply(&mut self) {
        *self.web.accent.lock().unwrap() = self.sys.accent.clone().unwrap_or_else(|| "#0078D4".into());
        let port = self.web_port();
        let Some(host) = web::bind_host(self.doc.settings.web.enabled, self.web.local) else {
            self.web.running = None;
            self.web.error = None;
            return;
        };
        // Tests bind to 127.0.0.1 so Windows Firewall doesn't ask.
        let host = if std::env::var("FLUENTCAL_WEB_BIND").is_ok_and(|v| v == "127.0.0.1") { "127.0.0.1" } else { host };
        let want: std::net::SocketAddr = format!("{host}:{port}").parse().expect("ip:port");
        if self.web.running.as_ref().is_some_and(|r| r.addr == want) {
            return;
        }
        self.web.running = None; // frees the old port first
        self.web_token();
        let accent = self.web.accent.clone();
        let ctx = Arc::new(Ctx {
            store: self.store.clone(),
            token: self.web.token.clone(),
            asset: Box::new(asset),
            accent: Box::new(move || accent.lock().unwrap().clone()),
            pc: fluentcal_core::cloud::device_name(),
        });
        match web::start(ctx, &format!("{host}:{port}")) {
            Ok(r) => {
                self.web.running = Some(r);
                self.web.error = None;
            }
            Err(e) => self.web.error = Some(e),
        }
    }

    /// "在浏览器中查看": the read-only web view in this PC's default browser.
    pub fn open_in_browser(&mut self) {
        self.web.local = true;
        self.web_apply();
        if let Some(e) = self.web.error.clone() {
            self.notify(&format!("无法在浏览器中查看：{e}（端口在 设置 › 网页查看 中修改）"), false);
            return;
        }
        let Some(port) = self.web.running.as_ref().map(|r| r.addr.port()) else { return };
        let token = self.web_token();
        open_url(&link("127.0.0.1", port, &token));
    }

    pub fn web_info(&self) -> WebInfo {
        let port = self.web_port();
        let token = self.web.token.lock().unwrap().clone();
        let running = self.web.running.as_ref().map(|r| r.addr.port());
        WebInfo {
            enabled: self.doc.settings.web.enabled,
            running: running.is_some(),
            port: running.unwrap_or(port),
            error: self.web.error.clone(),
            addresses: crate::platform::net::local_addresses().into_iter().map(|(ip, label, virt)| (ip.clone(), label, virt, link(&ip, port, &token))).collect(),
            local_url: link("127.0.0.1", port, &token),
        }
    }

    /// New access code: links shared before stop working at once.
    pub fn web_reset_token(&mut self) -> Result<(), String> {
        let t = new_token();
        secrets::set(TOKEN_KEY, &t)?;
        *self.web.token.lock().unwrap() = t;
        Ok(())
    }
}

thread_local! {
    /// Tests: links are recorded here instead of opening a browser.
    static CAPTURED: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Tests: from now on links opened on this thread are recorded, not opened; returns them so far.
#[doc(hidden)]
pub fn captured_urls() -> Vec<String> {
    CAPTURED.with(|c| c.borrow_mut().get_or_insert_with(Vec::new).clone())
}

/// Opens a link in the default browser (which may come to the front: the calendar doesn't
/// activate, so it hands the right on).
pub fn open_url(url: &str) {
    use windows::core::{w, HSTRING};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return;
    }
    if CAPTURED.with(|c| c.borrow_mut().as_mut().map(|v| v.push(url.to_owned())).is_some()) {
        return;
    }
    crate::platform::desk::allow_foreground();
    unsafe {
        ShellExecuteW(None, w!("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL);
    }
}
