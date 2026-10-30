//! Google OAuth 2.0 for installed apps: browser login with PKCE and a loopback redirect
//! (http://127.0.0.1:<random port>), token refresh. The user brings their own "Desktop app"
//! OAuth client (an open-source app can't ship a verified one).

use super::http::{dec, enc, Http};
use super::SyncError;
use crate::store::now_ms;
use base64::Engine as _;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

pub const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

#[derive(Clone)]
pub struct Endpoints {
    pub auth: String,
    pub token: String,
    pub api: String,
    pub upload: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Endpoints {
            auth: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            api: "https://www.googleapis.com".into(),
            upload: "https://www.googleapis.com/upload".into(),
        }
    }
}

fn form(pairs: &[(&str, &str)]) -> Vec<u8> {
    pairs.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&").into_bytes()
}

fn post_form(http: &Http, url: &str, pairs: &[(&str, &str)]) -> Result<super::http::Resp, SyncError> {
    http.send("POST", url, &[("Content-Type", "application/x-www-form-urlencoded".into())], Some(form(pairs)))
}

fn random(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("OS random generator");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// New access token and its expiry (ms).
pub fn refresh(http: &Http, ep: &Endpoints, client_id: &str, secret: &str, refresh: &str) -> Result<(String, i64), SyncError> {
    if client_id.is_empty() || secret.is_empty() {
        return Err(SyncError::config("还没有填写 Google OAuth 客户端 ID 和密钥"));
    }
    if refresh.is_empty() {
        return Err(SyncError::auth("还没有登录 Google 账号"));
    }
    let r = post_form(http, &ep.token, &[("client_id", client_id), ("client_secret", secret), ("refresh_token", refresh), ("grant_type", "refresh_token")])?;
    let v = r.json().unwrap_or_default();
    if r.status == 200 {
        let token = v["access_token"].as_str().ok_or_else(|| SyncError::server("Google 返回了无法解析的结果"))?;
        return Ok((token.to_owned(), now_ms() + v["expires_in"].as_i64().unwrap_or(3600) * 1000));
    }
    Err(match v["error"].as_str().unwrap_or("") {
        "invalid_grant" => SyncError::auth("Google 授权已失效（被撤销，或同意屏幕仍是“测试”状态导致 7 天后过期），请重新登录"),
        "invalid_client" | "unauthorized_client" => SyncError::config("Google OAuth 客户端 ID 或密钥不对"),
        _ => SyncError::status(r.status),
    })
}

pub struct Login {
    pub refresh_token: String,
    pub email: Option<String>,
}

const PAGE_OK: &str = "<!doctype html><meta charset=utf-8><title>FluentCal</title><body style=\"font:16px 'Segoe UI','Microsoft YaHei';padding:48px\"><h2>已登录</h2><p>可以关闭这个页面，回到 FluentCal。</p>";
const PAGE_FAIL: &str = "<!doctype html><meta charset=utf-8><title>FluentCal</title><body style=\"font:16px 'Segoe UI','Microsoft YaHei';padding:48px\"><h2>没有完成登录</h2><p>可以关闭这个页面，回到 FluentCal 重试。</p>";

/// Opens the consent page with `open`, waits up to `timeout` for the redirect, exchanges the
/// code. The listener only accepts 127.0.0.1 and closes when done.
pub fn login(http: &Http, ep: &Endpoints, client_id: &str, secret: &str, open: &dyn Fn(&str) -> Result<(), String>, timeout: Duration) -> Result<Login, SyncError> {
    let (client_id, secret) = (client_id.trim(), secret.trim());
    if client_id.is_empty() || secret.is_empty() {
        return Err(SyncError::config("请先填写 Google OAuth 客户端 ID 和密钥"));
    }
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| SyncError::config(format!("无法开始登录：{e}")))?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr().map_err(|e| SyncError::config(e.to_string()))?.port());
    let verifier = random(48);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random(16);
    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={challenge}&code_challenge_method=S256&state={state}&access_type=offline&prompt=consent",
        ep.auth, enc(client_id), enc(&redirect), enc(SCOPE)
    );
    open(&url).map_err(|e| SyncError::config(format!("无法打开浏览器：{e}")))?;
    let code = wait_for_code(&listener, &state, timeout)?;
    let r = post_form(http, &ep.token, &[
        ("client_id", client_id), ("client_secret", secret), ("code", &code), ("code_verifier", &verifier),
        ("grant_type", "authorization_code"), ("redirect_uri", &redirect),
    ])?;
    let v = r.json().unwrap_or_default();
    if r.status != 200 {
        return Err(match v["error"].as_str().unwrap_or("") {
            "invalid_client" | "unauthorized_client" => SyncError::config("Google OAuth 客户端 ID 或密钥不对（客户端类型要选“桌面应用”）"),
            _ => SyncError::status(r.status),
        });
    }
    let refresh_token = v["refresh_token"].as_str().map(str::to_owned).ok_or_else(|| {
        SyncError::auth("Google 没有返回刷新令牌。请在 Google 账号的“第三方访问”里移除 FluentCal 后重新登录")
    })?;
    let access = v["access_token"].as_str().unwrap_or_default();
    Ok(Login { refresh_token, email: email(http, ep, access) })
}

fn wait_for_code(listener: &TcpListener, state: &str, timeout: Duration) -> Result<String, SyncError> {
    listener.set_nonblocking(true).ok();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let Ok((mut s, peer)) = listener.accept() else {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };
        if !peer.ip().is_loopback() {
            continue;
        }
        s.set_nonblocking(false).ok();
        s.set_read_timeout(Some(Duration::from_secs(5))).ok();
        let mut buf = [0u8; 8192];
        let n = s.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let target = req.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("");
        let query = target.split_once('?').map_or("", |(_, q)| q);
        let param = |k: &str| query.split('&').filter_map(|p| p.split_once('=')).find(|(n, _)| *n == k).map(|(_, v)| dec(v));
        if param("state").is_none() && param("error").is_none() {
            let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue; // favicon and the like
        }
        let ok = param("state").as_deref() == Some(state) && param("code").is_some();
        let page = if ok { PAGE_OK } else { PAGE_FAIL };
        let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len()).as_bytes());
        if ok {
            return Ok(param("code").unwrap_or_default());
        }
        return Err(match param("error").as_deref() {
            Some("access_denied") => SyncError::auth("已取消 Google 登录"),
            Some(e) => SyncError::auth(format!("Google 登录失败：{e}")),
            None => SyncError::auth("Google 登录失败（state 不匹配）"),
        });
    }
    Err(SyncError::auth("登录超时，请重试"))
}

/// The account's email address, for display.
fn email(http: &Http, ep: &Endpoints, access: &str) -> Option<String> {
    let url = format!("{}/drive/v3/about?fields=user(emailAddress)", ep.api);
    let r = http.send("GET", &url, &[("Authorization", format!("Bearer {access}"))], None).ok()?;
    r.json().and_then(|v: Value| v.pointer("/user/emailAddress").and_then(Value::as_str).map(str::to_owned))
}

/// Best effort: tells Google to forget the authorization.
pub fn revoke(http: &Http, token: &str) {
    let _ = post_form(http, "https://oauth2.googleapis.com/revoke", &[("token", token)]);
}
