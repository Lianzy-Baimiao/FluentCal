//! Tauri side of the web view (web.rs): settings web.enabled / web.port, the access code in
//! Windows Credential Manager, this PC's local network addresses, and the settings page's
//! commands.

use crate::cloud::secrets;
use crate::store::Store;
use crate::web::{self, Asset, Ctx, Running};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};

pub const DEFAULT_PORT: u16 = 18765;
const TOKEN_KEY: &str = "web-token";

pub struct WebState {
    running: Mutex<Option<Running>>,
    error: Mutex<Option<String>>,
    token: Arc<Mutex<String>>,
    accent: Arc<Mutex<String>>,
}

fn new_token() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("OS random generator");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn state() -> WebState {
    let token = secrets::get(TOKEN_KEY).unwrap_or_default();
    WebState { running: Mutex::new(None), error: Mutex::new(None), token: Arc::new(Mutex::new(token)), accent: Arc::new(Mutex::new("#0078D4".into())) }
}

/// The access code, made the first time the web view is used.
fn ensure_token(st: &WebState) -> String {
    let mut t = st.token.lock().unwrap();
    if t.is_empty() {
        *t = new_token();
        let _ = secrets::set(TOKEN_KEY, &t);
    }
    t.clone()
}

fn port(store: &Store) -> u16 {
    store.setting("/web/port").and_then(|v| v.as_u64()).map_or(DEFAULT_PORT, |p| p.clamp(1024, 65535) as u16)
}

/// Starts, restarts or stops the server to match the settings (at startup and when they change).
pub fn apply(app: &AppHandle) {
    let store = app.state::<Arc<Store>>().inner().clone();
    let st = app.state::<WebState>();
    *st.accent.lock().unwrap() = crate::desktop::system_info().accent.unwrap_or_else(|| "#0078D4".into());
    let enabled = store.setting_bool("/web/enabled", false);
    let port = port(&store);
    let mut running = st.running.lock().unwrap();
    if !enabled {
        *running = None;
        *st.error.lock().unwrap() = None;
        return;
    }
    if running.as_ref().is_some_and(|r| r.addr.port() == port) {
        return;
    }
    *running = None; // frees the old port first
    ensure_token(&st);
    // Tests bind to 127.0.0.1 so Windows Firewall doesn't ask.
    let host = if std::env::var("FLUENTCAL_WEB_BIND").is_ok_and(|v| v == "127.0.0.1") { "127.0.0.1" } else { "0.0.0.0" };
    let handle = app.clone();
    let accent = st.accent.clone();
    let ctx = Arc::new(Ctx {
        store,
        token: st.token.clone(),
        asset: Box::new(move |path| handle.asset_resolver().get(path.to_owned()).map(|a| Asset { bytes: a.bytes, mime: a.mime_type })),
        accent: Box::new(move || accent.lock().unwrap().clone()),
        pc: crate::cloud::device_name(),
    });
    match web::start(ctx, &format!("{host}:{port}")) {
        Ok(r) => {
            *running = Some(r);
            *st.error.lock().unwrap() = None;
        }
        Err(e) => *st.error.lock().unwrap() = Some(e),
    }
}

#[derive(Serialize, Clone)]
pub struct Address {
    ip: String,
    label: String,
    /// Virtual adapter (VM, VPN): usually not reachable from a phone.
    virt: bool,
    url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebInfo {
    enabled: bool,
    running: bool,
    port: u16,
    error: Option<String>,
    addresses: Vec<Address>,
    /// Link for this PC itself.
    local_url: String,
}

fn link(ip: &str, port: u16, token: &str) -> String {
    format!("http://{ip}:{port}/?w=web#k={token}")
}

#[tauri::command]
pub fn web_info(store: State<'_, Arc<Store>>, st: State<'_, WebState>) -> WebInfo {
    let port = port(&store);
    let token = ensure_token(&st);
    let running = st.running.lock().unwrap().as_ref().map(|r| r.addr.port());
    let addresses = local_addresses()
        .into_iter()
        .map(|(ip, label, virt)| Address { url: link(&ip, port, &token), ip, label, virt })
        .collect();
    WebInfo {
        enabled: store.setting_bool("/web/enabled", false),
        running: running.is_some(),
        port: running.unwrap_or(port),
        error: st.error.lock().unwrap().clone(),
        addresses,
        local_url: link("127.0.0.1", port, &token),
    }
}

/// New access code: links shared before stop working at once.
#[tauri::command]
pub fn web_reset_token(st: State<'_, WebState>) -> Result<(), String> {
    let t = new_token();
    secrets::set(TOKEN_KEY, &t)?;
    *st.token.lock().unwrap() = t;
    Ok(())
}

/// Private IPv4 addresses of connected adapters: physical ones with a gateway first, virtual
/// adapters (Hyper-V, VMware, VPN, TUN…) last.
pub fn local_addresses() -> Vec<(String, String, bool)> {
    use windows::Win32::NetworkManagement::IpHelper::*;
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER | GAA_FLAG_INCLUDE_GATEWAYS;
    let mut size = 16 * 1024u32;
    let mut buf: Vec<u64> = Vec::new();
    for _ in 0..3 {
        buf = vec![0u64; size as usize / 8 + 1];
        let r = unsafe { GetAdaptersAddresses(AF_INET.0 as u32, flags, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size) };
        if r == 0 {
            break;
        }
        if r != 111 {
            return Vec::new(); // 111 = ERROR_BUFFER_OVERFLOW: retry with `size`
        }
    }
    let mut out: Vec<(String, String, bool, bool)> = Vec::new();
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        let a = unsafe { &*p };
        p = a.Next;
        if a.OperStatus != IfOperStatusUp || a.IfType == IF_TYPE_SOFTWARE_LOOPBACK {
            continue;
        }
        let name = unsafe { a.FriendlyName.to_string() }.unwrap_or_default();
        let desc = unsafe { a.Description.to_string() }.unwrap_or_default();
        let hay = format!("{name} {desc}").to_lowercase();
        const VIRTUAL: [&str; 12] = ["virtual", "vmware", "virtualbox", "hyper-v", "vethernet", "tap-", "tun", "wintun", "wireguard", "clash", "zerotier", "tailscale"];
        let virt = VIRTUAL.iter().any(|k| hay.contains(k));
        let gateway = !a.FirstGatewayAddress.is_null();
        let mut u = a.FirstUnicastAddress;
        while !u.is_null() {
            let ua = unsafe { &*u };
            u = ua.Next;
            let sa = ua.Address.lpSockaddr;
            if sa.is_null() || unsafe { (*sa).sa_family } != AF_INET {
                continue;
            }
            let b = unsafe { (*(sa as *const SOCKADDR_IN)).sin_addr.S_un.S_un_b };
            let ip = std::net::Ipv4Addr::new(b.s_b1, b.s_b2, b.s_b3, b.s_b4);
            if ip.is_private() {
                out.push((ip.to_string(), name.clone(), virt, gateway));
            }
        }
    }
    out.sort_by_key(|(_, _, virt, gw)| (*virt, !*gw));
    out.dedup_by(|a, b| a.0 == b.0);
    out.into_iter().map(|(ip, label, virt, _)| (ip, label, virt)).collect()
}
