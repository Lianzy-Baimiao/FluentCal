//! WebDAV (SYN-01): 坚果云, Nextcloud, Synology, Alist… GET/PUT one file with ETag
//! conditions; missing folders are created with MKCOL.

use super::http::{enc, Http, Resp};
use super::{Fetched, Remote, Rev, SyncError};
use base64::Engine as _;

pub struct WebDav {
    http: Http,
    /// Folder URLs from the outermost, for MKCOL.
    folders: Vec<String>,
    file: String,
    auth: String,
}

impl WebDav {
    /// `server`: e.g. https://dav.jianguoyun.com/dav/ ; `folder`: e.g. FluentCal (may be nested).
    pub fn new(http: Http, server: &str, user: &str, password: &str, folder: &str) -> Result<WebDav, SyncError> {
        let server = server.trim();
        if !(server.starts_with("https://") || server.starts_with("http://")) {
            return Err(SyncError::config("服务器地址要以 https:// 开头"));
        }
        let mut url = server.trim_end_matches('/').to_owned();
        let mut folders = Vec::new();
        for seg in folder.split(['/', '\\']).map(str::trim).filter(|s| !s.is_empty()) {
            if seg == "." || seg == ".." {
                return Err(SyncError::config("文件夹名不能是 . 或 .."));
            }
            url = format!("{url}/{}", enc(seg));
            folders.push(format!("{url}/"));
        }
        let auth = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{}:{password}", user.trim())));
        Ok(WebDav { http, folders, file: format!("{url}/{}", super::FILE_NAME), auth })
    }

    fn send(&self, method: &str, url: &str, extra: &[(&str, String)], body: Option<Vec<u8>>) -> Result<Resp, SyncError> {
        let mut headers = vec![("Authorization", self.auth.clone())];
        headers.extend(extra.iter().cloned());
        self.http.send(method, url, &headers, body)
    }

    fn mkcols(&self) -> Result<(), SyncError> {
        for f in &self.folders {
            let r = self.send("MKCOL", f, &[], None)?;
            match r.status {
                // Created, or exists already (405 / 301 on most servers).
                200..=299 | 301 | 405 => {}
                401 => return Err(auth_error()),
                s => return Err(SyncError::server(format!("无法在服务器上创建文件夹（{s}）"))),
            }
        }
        Ok(())
    }
}

fn auth_error() -> SyncError {
    SyncError::auth("用户名或密码不对（坚果云要用“第三方应用密码”，不是登录密码）")
}

fn etag(r: &Resp) -> Option<String> {
    r.header("etag").map(str::to_owned)
}

impl Remote for WebDav {
    fn check(&mut self) -> Result<(), SyncError> {
        Ok(()) // fetch reports wrong passwords and addresses
    }

    fn fetch(&mut self, known: Option<&Rev>) -> Result<Fetched, SyncError> {
        let cond: Vec<(&str, String)> = known.and_then(|k| k.etag.clone()).map(|e| ("If-None-Match", e)).into_iter().collect();
        let r = self.send("GET", &self.file, &cond, None)?;
        match r.status {
            200 => {
                let rev = Rev { etag: etag(&r), ..Rev::default() };
                Ok(Fetched::File(r.body, rev))
            }
            304 => Ok(Fetched::NotModified),
            404 | 409 | 410 => Ok(Fetched::Missing),
            401 => Err(auth_error()),
            403 => Err(SyncError::auth("服务器拒绝访问（403），请检查账号权限和地址")),
            405 | 501 => Err(SyncError::config("这个地址不是 WebDAV 地址（坚果云为 https://dav.jianguoyun.com/dav/）")),
            s => Err(SyncError::status(s)),
        }
    }

    fn put(&mut self, bytes: &[u8], base: Option<&Rev>) -> Result<Rev, SyncError> {
        let mut guard_new = base.is_none();
        let mut made_folders = false;
        for _ in 0..3 {
            let mut cond = vec![("Content-Type", "application/json; charset=utf-8".to_owned())];
            match base.and_then(|b| b.etag.clone()) {
                // A weak ETag can't be used with If-Match; the round's fetch is the check then.
                Some(e) if !e.starts_with("W/") => cond.push(("If-Match", e)),
                _ if guard_new => cond.push(("If-None-Match", "*".to_owned())),
                _ => {}
            }
            let r = self.send("PUT", &self.file, &cond, Some(bytes.to_vec()))?;
            match r.status {
                200..=299 => return Ok(Rev { etag: etag(&r), ..Rev::default() }),
                412 if base.is_some() => return Err(SyncError::conflict()),
                412 => {
                    // Created by someone else just now, or a server that mishandles If-None-Match.
                    match self.fetch(None)? {
                        Fetched::Missing => guard_new = false,
                        _ => return Err(SyncError::conflict()),
                    }
                }
                404 | 409 | 403 if !made_folders => {
                    self.mkcols()?;
                    made_folders = true;
                }
                401 => return Err(auth_error()),
                403 => return Err(SyncError::auth("服务器拒绝写入（403），请检查账号权限")),
                507 => return Err(SyncError::server("网盘空间已满")),
                s => return Err(SyncError::status(s)),
            }
        }
        Err(SyncError::server("无法上传到服务器"))
    }
}
