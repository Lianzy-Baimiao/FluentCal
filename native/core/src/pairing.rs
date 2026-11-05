//! One-use LAN handoff of cloud configuration. The QR carries a random AES key; HTTP
//! carries only authenticated ciphertext. No credential or calendar data is logged/written.
use aes_gcm::{aead::{Aead, KeyInit, Payload}, Aes256Gcm, Nonce};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use std::{net::{Ipv4Addr, SocketAddr}, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
use tiny_http::{Header, Method, Response, Server};

pub const MOBILE_DOWNLOAD: &str = "https://github.com/Lianzy-Baimiao/FluentCal/releases/tag/android-latest";
pub const TTL: Duration = Duration::from_secs(300);

pub struct Session {
    pub port: u16,
    pub expires: i64,
    id: String,
    key: String,
    stop: Arc<AtomicBool>,
    used: Arc<AtomicBool>,
    until: Instant,
}
impl Session {
    pub fn active(&self) -> bool { !self.used.load(Ordering::Acquire) && !self.stop.load(Ordering::Acquire) && Instant::now() < self.until }
    pub fn used(&self) -> bool { self.used.load(Ordering::Acquire) }
    pub fn seconds_left(&self) -> u64 { self.until.saturating_duration_since(Instant::now()).as_secs() }
    pub fn qr(&self, ip: &str) -> Result<String, String> {
        let addr = ip.parse::<Ipv4Addr>().map_err(|_| "Invalid LAN address")?;
        if !addr.is_private() { return Err("Invalid LAN address".into()); }
        Ok(format!("fluentcal://pair?v=1&host={ip}&port={}&id={}&key={}&expires={}", self.port, self.id, self.key, self.expires))
    }
}
impl Drop for Session {
    fn drop(&mut self) { self.stop.store(true, Ordering::Release); }
}
pub fn payload(sync: &Value, secret: &str, device: &str) -> Result<Value, String> {
    let provider = sync["provider"].as_str().unwrap_or("");
    if !sync["enabled"].as_bool().unwrap_or(false) || !["webdav", "github", "gitee"].contains(&provider) {
        return Err("Enable WebDAV, GitHub or Gitee sync first".into());
    }
    if provider == "webdav" && !sync[provider]["url"].as_str().unwrap_or("").starts_with("https://") {
        return Err("Android requires an HTTPS WebDAV address".into());
    }
    let mut config = json!({ "provider": provider, "enabled": false, "interval": sync["interval"].as_u64().unwrap_or(5), "proxy": "" });
    config[provider] = sync[provider].clone();
    Ok(json!({ "app": "FluentCal", "format": 1, "device": device, "sync": config, "secret": secret }))
}
pub fn start(data: &Value, bind: &str, ttl: Duration) -> Result<Session, String> {
    let mut key = [0u8; 32]; let mut id = [0u8; 16]; let mut iv = [0u8; 12];
    getrandom::fill(&mut key).map_err(|_| "Random generator failed")?;
    getrandom::fill(&mut id).map_err(|_| "Random generator failed")?;
    getrandom::fill(&mut iv).map_err(|_| "Random generator failed")?;
    let id: String = id.iter().map(|v| format!("{v:02x}")).collect();
    let expires = crate::date::now_ms() + ttl.as_millis() as i64;
    let mut data = data.clone(); data["expires"] = json!(expires);
    let plain = serde_json::to_vec(&data).map_err(|_| "Invalid pairing data")?;
    if plain.len() > 12_000 { return Err("Pairing data too large".into()); }
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "Encryption failed")?;
    let encrypted = cipher.encrypt(Nonce::from_slice(&iv), Payload { msg: &plain, aad: id.as_bytes() }).map_err(|_| "Encryption failed")?;
    let body = json!({ "v": 1, "iv": URL_SAFE_NO_PAD.encode(iv), "data": URL_SAFE_NO_PAD.encode(encrypted) }).to_string();
    let server = Server::http(bind).map_err(|_| "Cannot start pairing service")?;
    let addr = server.server_addr().to_ip().ok_or("Cannot get pairing port")?;
    let stop = Arc::new(AtomicBool::new(false)); let used = Arc::new(AtomicBool::new(false));
    let session = Session { port: addr.port(), expires, id: id.clone(), key: URL_SAFE_NO_PAD.encode(key), stop: stop.clone(), used: used.clone(), until: Instant::now() + ttl };
    let until = session.until;
    std::thread::Builder::new().name("mobile-pair".into()).spawn(move || {
        while !stop.load(Ordering::Acquire) && Instant::now() < until {
            let Ok(Some(req)) = server.recv_timeout(Duration::from_millis(150)) else { continue };
            let local_peer = req.remote_addr().is_some_and(|a| match a.ip() { std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback(), _ => false });
            let allowed = local_peer && req.method() == &Method::Get && req.url() == format!("/pair/{id}") &&
                !req.headers().iter().any(|h| h.field.equiv("Origin"));
            let response = if allowed && !used.load(Ordering::Acquire) && Instant::now() < until && !stop.load(Ordering::Acquire) {
                Response::from_string(body.clone()).with_status_code(200)
            } else { Response::from_string("").with_status_code(if used.load(Ordering::Acquire) { 410 } else { 403 }) };
            let success = response.status_code().0 == 200;
            let response = response.with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
                .with_header(Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap());
            if req.respond(response).is_ok() && success { used.store(true, Ordering::Release); }
        }
        drop(server);
        // Wake tiny_http's accept thread when bound to 0.0.0.0 on Windows.
        let _ = std::net::TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], addr.port())), Duration::from_millis(100));
    }).map_err(|_| "Cannot start pairing service")?;
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decrypts_shared_webcrypto_fixture() {
        let fixture: Value = serde_json::from_str(include_str!("../tests/fixtures/pairing-v1.json")).unwrap();
        let envelope = &fixture["envelope"];
        let iv = URL_SAFE_NO_PAD.decode(envelope["iv"].as_str().unwrap()).unwrap();
        let data = URL_SAFE_NO_PAD.decode(envelope["data"].as_str().unwrap()).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&[7u8;32]).unwrap();
        let plain = cipher.decrypt(Nonce::from_slice(&iv), Payload { msg: &data, aad: "a".repeat(32).as_bytes() }).unwrap();
        let parsed: Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(parsed["device"], "TEST-PC");
        assert_eq!(parsed["secret"], "test-only-not-a-real-password");
    }
    #[test]
    fn encrypted_once_and_no_plaintext_in_qr() {
        let s = start(&json!({"secret":"test-secret","sync":{"provider":"webdav"}}), "127.0.0.1:0", Duration::from_secs(3)).unwrap();
        let qr = s.qr("192.168.1.2").unwrap();
        assert!(!qr.contains("test-secret"));
        assert!(s.qr("8.8.8.8").is_err());
        let url = format!("http://127.0.0.1:{}/pair/{}", s.port, s.id);
        let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        let body: Value = serde_json::from_str(&agent.get(&url).call().unwrap().body_mut().read_to_string().unwrap()).unwrap();
        let cipher = Aes256Gcm::new_from_slice(&URL_SAFE_NO_PAD.decode(&s.key).unwrap()).unwrap();
        let iv = URL_SAFE_NO_PAD.decode(body["iv"].as_str().unwrap()).unwrap();
        let encrypted = URL_SAFE_NO_PAD.decode(body["data"].as_str().unwrap()).unwrap();
        let plain = cipher.decrypt(Nonce::from_slice(&iv), Payload { msg: &encrypted, aad: s.id.as_bytes() }).unwrap();
        assert!(String::from_utf8(plain).unwrap().contains("test-secret"));
        assert!(cipher.decrypt(Nonce::from_slice(&iv), Payload { msg: &encrypted, aad: b"wrong" }).is_err());
        assert_eq!(agent.get(&url).call().unwrap().status(), 410);
        assert!(s.used()); assert!(!s.active());
    }
    #[test]
    fn rejects_browser_origins_and_wrong_ids_without_consuming() {
        let s = start(&json!({}), "127.0.0.1:0", Duration::from_secs(3)).unwrap();
        let url = format!("http://127.0.0.1:{}/pair/{}", s.port, s.id);
        let agent: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).build().into();
        assert_eq!(agent.get(&url).header("Origin", "https://other.example").call().unwrap().status(), 403);
        assert_eq!(agent.get(format!("{url}wrong")).call().unwrap().status(), 403);
        assert!(s.active());
    }
    #[test]
    fn only_transfers_selected_cloud_config() {
        let data = payload(&json!({"enabled":true,"provider":"github","github":{"repo":"a/b"},"webdav":{"user":"unused"},"proxy":"private-proxy"}), "test", "PC").unwrap();
        assert!(data["sync"].get("webdav").is_none());
        assert_eq!(data["sync"]["proxy"], "");
        assert!(payload(&json!({"enabled":false}), "", "PC").is_err());
    }
    #[test]
    fn expiry_disables_qr() {
        let s = start(&json!({}), "127.0.0.1:0", Duration::ZERO).unwrap();
        assert!(!s.active());
    }
}
