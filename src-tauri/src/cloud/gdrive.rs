//! Google Drive (SYN-04): FluentCal/fluentcal-data.json via the Drive v3 API with the
//! drive.file scope (only files this app created). Drive has no conditional writes, so the
//! version is checked right before uploading; a lost race is repaired by the next round.

use super::http::{enc, Http, Resp};
use super::{oauth, Fetched, Remote, Rev, SyncError};
use crate::store::now_ms;
use serde_json::{json, Value};

const FOLDER: &str = "FluentCal";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";

pub struct Drive {
    http: Http,
    ep: oauth::Endpoints,
    client_id: String,
    client_secret: String,
    refresh: String,
    token: Option<(String, i64)>,
    folder: Option<String>,
    file: Option<String>,
}

impl Drive {
    /// `location`: ids remembered from earlier rounds ({ folder, file }).
    pub fn new(http: Http, ep: oauth::Endpoints, client_id: &str, client_secret: &str, refresh: &str, location: &Value) -> Drive {
        let id = |k: &str| location.get(k).and_then(Value::as_str).map(str::to_owned);
        Drive {
            http,
            ep,
            client_id: client_id.trim().to_owned(),
            client_secret: client_secret.trim().to_owned(),
            refresh: refresh.to_owned(),
            token: None,
            folder: id("folder"),
            file: id("file"),
        }
    }

    fn token(&mut self) -> Result<String, SyncError> {
        if let Some((t, exp)) = &self.token {
            if *exp > now_ms() + 60_000 {
                return Ok(t.clone());
            }
        }
        let (t, exp) = oauth::refresh(&self.http, &self.ep, &self.client_id, &self.client_secret, &self.refresh)?;
        self.token = Some((t.clone(), exp));
        Ok(t)
    }

    /// Authorized request; a 401 drops the cached token and tries once more.
    fn send(&mut self, method: &str, url: &str, extra: &[(&str, String)], body: Option<Vec<u8>>) -> Result<Resp, SyncError> {
        for attempt in 0..2 {
            let mut h = vec![("Authorization", format!("Bearer {}", self.token()?))];
            h.extend(extra.iter().cloned());
            let r = self.http.send(method, url, &h, body.clone())?;
            if r.status == 401 && attempt == 0 {
                self.token = None;
                continue;
            }
            return Ok(r);
        }
        unreachable!()
    }

    fn fail(r: &Resp) -> SyncError {
        let reason = r.json().and_then(|v| v.pointer("/error/errors/0/reason").and_then(Value::as_str).map(str::to_owned)).unwrap_or_default();
        match (r.status, reason.as_str()) {
            (401, _) => SyncError::auth("Google 授权已失效，请重新登录"),
            (403, "storageQuotaExceeded") => SyncError::server("Google 云端硬盘空间已满"),
            (403, "accessNotConfigured") | (403, "SERVICE_DISABLED") => SyncError::config("这个 Google Cloud 项目还没有启用 Google Drive API"),
            (403, r) if r.contains("ateLimit") => SyncError::server("Google 接口调用太频繁，稍后会自动重试"),
            (403, _) => SyncError::auth("Google 拒绝访问（403）"),
            (s, _) => SyncError::status(s),
        }
    }

    /// First non-trashed item with this name (and mime type) inside `parent`.
    fn find(&mut self, name: &str, parent: &str, folder: bool) -> Result<Option<String>, SyncError> {
        let mut q = format!("name = '{name}' and '{parent}' in parents and trashed = false");
        if folder {
            q.push_str(&format!(" and mimeType = '{FOLDER_MIME}'"));
        }
        let url = format!("{}/drive/v3/files?q={}&fields=files(id)&orderBy=createdTime&spaces=drive", self.ep.api, enc(&q));
        let r = self.send("GET", &url, &[], None)?;
        if r.status != 200 {
            return Err(Self::fail(&r));
        }
        Ok(r.json().and_then(|v| v.pointer("/files/0/id").and_then(Value::as_str).map(str::to_owned)))
    }

    fn folder_id(&mut self) -> Result<String, SyncError> {
        if let Some(f) = self.folder.clone() {
            return Ok(f);
        }
        let id = match self.find(FOLDER, "root", true)? {
            Some(id) => id,
            None => {
                let body = json!({ "name": FOLDER, "mimeType": FOLDER_MIME });
                let r = self.send("POST", &format!("{}/drive/v3/files?fields=id", self.ep.api), &[("Content-Type", "application/json".into())], Some(body.to_string().into_bytes()))?;
                if r.status != 200 {
                    return Err(Self::fail(&r));
                }
                r.json().and_then(|v| v["id"].as_str().map(str::to_owned)).ok_or_else(|| SyncError::server("无法创建 FluentCal 文件夹"))?
            }
        };
        self.folder = Some(id.clone());
        Ok(id)
    }

    /// (file id, version) of the data file, or None.
    fn locate(&mut self) -> Result<Option<(String, String)>, SyncError> {
        for _ in 0..2 {
            let id = match self.file.clone() {
                Some(id) => id,
                None => {
                    let folder = self.folder_id()?;
                    match self.find(super::FILE_NAME, &folder, false)? {
                        Some(id) => id,
                        None => return Ok(None),
                    }
                }
            };
            let r = self.send("GET", &format!("{}/drive/v3/files/{id}?fields=id,version,trashed", self.ep.api), &[], None)?;
            let v = r.json().unwrap_or_default();
            if r.status == 404 || v["trashed"].as_bool() == Some(true) {
                // Deleted or trashed by the user: look again by name (a folder may be gone too).
                self.file = None;
                self.folder = None;
                continue;
            }
            if r.status != 200 {
                return Err(Self::fail(&r));
            }
            let version = v["version"].as_str().map(str::to_owned).or_else(|| v["version"].as_i64().map(|n| n.to_string())).unwrap_or_default();
            self.file = Some(id.clone());
            return Ok(Some((id, version)));
        }
        Ok(None)
    }
}

fn multipart(meta: &Value, bytes: &[u8]) -> (String, Vec<u8>) {
    const B: &str = "fluentcal-boundary-7c1d0a";
    let mut body = format!("--{B}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{meta}\r\n--{B}\r\nContent-Type: application/json\r\n\r\n").into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{B}--\r\n").as_bytes());
    (format!("multipart/related; boundary={B}"), body)
}

fn version_of(r: &Resp) -> Option<String> {
    let v = r.json()?;
    v["version"].as_str().map(str::to_owned).or_else(|| v["version"].as_i64().map(|n| n.to_string()))
}

impl Remote for Drive {
    fn check(&mut self) -> Result<(), SyncError> {
        self.token().map(|_| ())
    }

    fn fetch(&mut self, known: Option<&Rev>) -> Result<Fetched, SyncError> {
        let Some((id, version)) = self.locate()? else { return Ok(Fetched::Missing) };
        if known.and_then(|k| k.version.as_deref()) == Some(version.as_str()) {
            return Ok(Fetched::NotModified);
        }
        let r = self.send("GET", &format!("{}/drive/v3/files/{id}?alt=media", self.ep.api), &[], None)?;
        match r.status {
            200 => Ok(Fetched::File(r.body, Rev { version: Some(version), ..Rev::default() })),
            404 => Ok(Fetched::Missing),
            _ => Err(Self::fail(&r)),
        }
    }

    fn put(&mut self, bytes: &[u8], base: Option<&Rev>) -> Result<Rev, SyncError> {
        let current = self.locate()?;
        let r = match (current, base.and_then(|b| b.version.clone())) {
            (Some((id, version)), Some(expected)) => {
                if version != expected {
                    return Err(super::SyncError::conflict());
                }
                let url = format!("{}/drive/v3/files/{id}?uploadType=media&fields=id,version", self.ep.upload);
                self.send("PATCH", &url, &[("Content-Type", "application/json".into())], Some(bytes.to_vec()))?
            }
            // Appeared since our fetch (another PC created it), or we never saw its version.
            (Some(_), None) => return Err(super::SyncError::conflict()),
            // Not there (or deleted since our fetch): create it.
            (None, _) => self.create(bytes)?,
        };
        if r.status != 200 {
            return Err(Self::fail(&r));
        }
        if let Some(id) = r.json().and_then(|v| v["id"].as_str().map(str::to_owned)) {
            self.file = Some(id);
        }
        Ok(Rev { version: version_of(&r), ..Rev::default() })
    }

    fn location(&self) -> Value {
        json!({ "folder": self.folder, "file": self.file })
    }
}

impl Drive {
    fn create(&mut self, bytes: &[u8]) -> Result<Resp, SyncError> {
        let folder = self.folder_id()?;
        let meta = json!({ "name": super::FILE_NAME, "parents": [folder], "mimeType": "application/json" });
        let (ct, body) = multipart(&meta, bytes);
        let url = format!("{}/drive/v3/files?uploadType=multipart&fields=id,version", self.ep.upload);
        self.send("POST", &url, &[("Content-Type", ct)], Some(body))
    }
}
