//! Blocking HTTP for cloud sync: ureq with native-tls (Windows schannel and the system
//! certificate store). Uses the Windows system proxy unless one is given. Error texts never
//! contain URLs, because Gitee takes its token as a query parameter.

use super::{ErrKind, SyncError};
use std::time::Duration;
use ureq::http::{HeaderMap, Method, Request};
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::{Agent, Proxy};

/// Larger cloud files are refused (the data file is normally well under 1 MB).
const MAX_BODY: u64 = 32 * 1024 * 1024;

pub struct Resp {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Resp {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub struct Http {
    agent: Agent,
    /// Without proxy, for addresses in the local network (a NAS, tests).
    direct: Agent,
}

fn agent(proxy: Option<Proxy>, timeout: Duration) -> Agent {
    let tls = TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build();
    let config = Agent::config_builder()
        .tls_config(tls)
        .proxy(proxy)
        .http_status_as_error(false)
        // WebDAV needs MKCOL.
        .allow_non_standard_methods(true)
        .timeout_global(Some(timeout))
        .timeout_connect(Some(timeout.min(Duration::from_secs(10))))
        .user_agent(concat!("FluentCal/", env!("CARGO_PKG_VERSION")))
        .build();
    Agent::new_with_config(config)
}

/// localhost, private IPv4 ranges, single-label and .local names.
fn is_local(url: &str) -> bool {
    let host = url.split_once("://").map_or("", |(_, r)| r).split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host.rsplit_once(':').map_or(host, |(h, _)| h),
    };
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(std::net::IpAddr::V6(ip)) => ip.is_loopback(),
        Err(_) => host.eq_ignore_ascii_case("localhost") || !host.contains('.') || host.ends_with(".local") || host.ends_with(".lan"),
    }
}

impl Http {
    /// `proxy`: "" = Windows system proxy, "direct" = none, otherwise http://host:port.
    /// Addresses in the local network never go through a proxy.
    pub fn new(proxy: &str, timeout: Duration) -> Result<Http, SyncError> {
        let proxy = match proxy.trim() {
            "" => Proxy::try_from_env(),
            "direct" => None,
            p if p.starts_with("socks") => return Err(SyncError::config("只支持 HTTP 代理（http://地址:端口）")),
            p => {
                let p = if p.contains("://") { p.to_owned() } else { format!("http://{p}") };
                Some(Proxy::new(&p).map_err(|_| SyncError::config("代理地址格式不对，应为 http://地址:端口"))?)
            }
        };
        Ok(Http { agent: agent(proxy, timeout), direct: agent(None, timeout) })
    }

    pub fn send(&self, method: &str, url: &str, headers: &[(&str, String)], body: Option<Vec<u8>>) -> Result<Resp, SyncError> {
        let method = Method::from_bytes(method.as_bytes()).map_err(|_| SyncError::config("bad method"))?;
        let mut b = Request::builder().method(method).uri(url);
        for (k, v) in headers {
            b = b.header(*k, v.as_str());
        }
        let agent = if is_local(url) { &self.direct } else { &self.agent };
        let result = match body {
            Some(bytes) => b.body(bytes).map(|r| agent.run(r)),
            None => b.body(()).map(|r| agent.run(r)),
        };
        let mut res = result.map_err(|_| SyncError::config("地址格式不对"))?.map_err(network)?;
        let status = res.status().as_u16();
        let headers = res.headers().clone();
        let body = res.body_mut().with_config().limit(MAX_BODY).read_to_vec().map_err(network)?;
        Ok(Resp { status, headers, body })
    }
}

/// Network errors in plain Chinese, without the URL.
fn network(e: ureq::Error) -> SyncError {
    let msg = match &e {
        ureq::Error::HostNotFound => "找不到服务器，请检查网络和地址".to_owned(),
        ureq::Error::Timeout(_) => "连接超时，请检查网络（访问 GitHub、Google 可能需要代理）".to_owned(),
        ureq::Error::ConnectionFailed => "无法连接服务器，请检查网络和地址".to_owned(),
        ureq::Error::ConnectProxyFailed(_) | ureq::Error::InvalidProxyUrl => "无法通过代理连接".to_owned(),
        ureq::Error::BodyExceedsLimit(_) => "云端文件太大（超过 32 MB）".to_owned(),
        ureq::Error::BadUri(_) => return SyncError::config("地址格式不对"),
        ureq::Error::Io(io) => format!("网络错误：{io}"),
        ureq::Error::NativeTls(t) => format!("安全连接失败：{t}"),
        ureq::Error::Tls(t) => format!("安全连接失败：{t}"),
        other => format!("网络错误：{}", scrub(&other.to_string())),
    };
    SyncError::new(ErrKind::Network, msg)
}

/// Cuts anything after "access_token=" (Gitee's token travels in the query string).
fn scrub(s: &str) -> String {
    match s.find("access_token=") {
        Some(i) => format!("{}access_token=***", &s[..i]),
        None => s.to_owned(),
    }
}

/// Percent-encodes one path segment or query value (RFC 3986 unreserved characters stay).
pub fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decodes %XX and '+' in a query value.
pub fn dec(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
