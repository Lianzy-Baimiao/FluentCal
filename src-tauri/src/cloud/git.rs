//! GitHub and Gitee (SYN-02, SYN-03): one file in a private repository through the contents
//! API. The blob sha makes writes conditional; every upload is a commit.

use super::http::{enc, Http, Resp};
use super::{ErrKind, Fetched, Remote, Rev, SyncError};
use base64::Engine as _;
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    GitHub,
    Gitee,
}

pub struct Git {
    http: Http,
    flavor: Flavor,
    api: String,
    owner: String,
    repo: String,
    /// Empty = the repository's default branch.
    branch: String,
    path: String,
    token: String,
    checked: bool,
}

fn valid_name(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) && s != "." && s != ".."
}

impl Git {
    /// `api`: https://api.github.com or https://gitee.com/api/v5 (tests pass a mock server).
    pub fn new(http: Http, flavor: Flavor, api: &str, repo: &str, branch: &str, path: &str, token: &str) -> Result<Git, SyncError> {
        let repo = repo.trim().trim_end_matches(".git");
        let repo = repo.rsplit_once("github.com/").or_else(|| repo.rsplit_once("gitee.com/")).map_or(repo, |(_, r)| r);
        let (owner, name) = repo.split_once('/').ok_or_else(|| SyncError::config("仓库要写成“用户名/仓库名”"))?;
        if !valid_name(owner) || !valid_name(name) {
            return Err(SyncError::config("仓库要写成“用户名/仓库名”"));
        }
        let segs: Vec<&str> = path.split(['/', '\\']).map(str::trim).filter(|s| !s.is_empty()).collect();
        if segs.iter().any(|s| *s == "." || *s == "..") {
            return Err(SyncError::config("文件路径不能包含 . 或 .."));
        }
        let path = if segs.is_empty() { super::FILE_NAME.to_owned() } else { segs.iter().map(|s| enc(s)).collect::<Vec<_>>().join("/") };
        let branch = branch.trim();
        if branch.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(SyncError::config("分支名不能有空格"));
        }
        if token.trim().is_empty() {
            return Err(SyncError::auth("还没有填写 Token"));
        }
        let (owner, repo, branch) = (owner.to_owned(), name.to_owned(), branch.to_owned());
        Ok(Git { http, flavor, api: api.trim_end_matches('/').to_owned(), owner, repo, branch, path, token: token.trim().to_owned(), checked: false })
    }

    fn site(&self) -> &'static str {
        if self.flavor == Flavor::GitHub { "GitHub" } else { "Gitee" }
    }

    fn headers(&self, accept: &str) -> Vec<(&'static str, String)> {
        let mut h = vec![("Accept", accept.to_owned())];
        if self.flavor == Flavor::GitHub {
            h.push(("Authorization", format!("Bearer {}", self.token)));
            h.push(("X-GitHub-Api-Version", "2022-11-28".to_owned()));
        }
        h
    }

    /// Query string: branch, and for Gitee the token (it takes no Authorization header).
    fn query(&self, with_ref: bool) -> String {
        let mut q = Vec::new();
        if with_ref && !self.branch.is_empty() {
            q.push(format!("ref={}", enc(&self.branch)));
        }
        if self.flavor == Flavor::Gitee {
            q.push(format!("access_token={}", enc(&self.token)));
        }
        if q.is_empty() { String::new() } else { format!("?{}", q.join("&")) }
    }

    fn contents(&self) -> String {
        format!("{}/repos/{}/{}/contents/{}", self.api, self.owner, self.repo, self.path)
    }

    /// Server message without the token.
    fn message(&self, r: &Resp) -> String {
        let m = r.json().and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned)).unwrap_or_default();
        m.replace(&self.token, "***")
    }

    fn fail(&self, r: &Resp) -> SyncError {
        let site = self.site();
        match r.status {
            401 => SyncError::auth(format!("{site} Token 无效或已过期")),
            403 if r.header("x-ratelimit-remaining") == Some("0") => SyncError::server(format!("{site} 接口调用次数已用完，稍后会自动重试")),
            403 => SyncError::auth(format!("{site} Token 没有这个仓库的读写权限（需要 Contents 读写）")),
            404 => SyncError::config(format!("找不到仓库 {}/{}，或者 Token 无权访问它", self.owner, self.repo)),
            s => {
                let m = self.message(r);
                if m.is_empty() { SyncError::status(s) } else { SyncError::server(format!("{site}：{m}（{s}）")) }
            }
        }
    }

    fn decode_content(&self, v: &Value) -> Option<Vec<u8>> {
        let c = v.get("content").and_then(Value::as_str)?;
        let clean: String = c.chars().filter(|ch| !ch.is_whitespace()).collect();
        base64::engine::general_purpose::STANDARD.decode(clean).ok()
    }

    /// Files over 1 MB come without inline content.
    fn raw(&self) -> Result<Vec<u8>, SyncError> {
        let url = match self.flavor {
            Flavor::GitHub => format!("{}{}", self.contents(), self.query(true)),
            Flavor::Gitee => format!("{}/repos/{}/{}/raw/{}{}", self.api, self.owner, self.repo, self.path, self.query(true)),
        };
        let r = self.http.send("GET", &url, &self.headers("application/vnd.github.raw+json"), None)?;
        if r.status == 200 { Ok(r.body) } else { Err(self.fail(&r)) }
    }
}

impl Remote for Git {
    /// The repository must exist, be private and writable with this token.
    fn check(&mut self) -> Result<(), SyncError> {
        if self.checked {
            return Ok(());
        }
        let url = format!("{}/repos/{}/{}{}", self.api, self.owner, self.repo, self.query(false));
        let r = self.http.send("GET", &url, &self.headers("application/json"), None)?;
        if r.status != 200 {
            return Err(self.fail(&r));
        }
        let v = r.json().unwrap_or_default();
        let private = v.get("private").and_then(Value::as_bool).unwrap_or(false);
        if !private {
            return Err(SyncError::config(format!("{}/{} 是公开仓库，日程会被所有人看到。请换成私有仓库", self.owner, self.repo)));
        }
        let push = v.pointer("/permissions/push").or_else(|| v.pointer("/permission/push")).and_then(Value::as_bool);
        if push == Some(false) {
            return Err(SyncError::auth(format!("这个 {} Token 只能读、不能写这个仓库", self.site())));
        }
        self.checked = true;
        Ok(())
    }

    fn fetch(&mut self, known: Option<&Rev>) -> Result<Fetched, SyncError> {
        let mut headers = self.headers("application/vnd.github+json");
        if let Some(e) = known.and_then(|k| k.etag.clone()).filter(|_| self.flavor == Flavor::GitHub) {
            headers.push(("If-None-Match", e));
        }
        let r = self.http.send("GET", &format!("{}{}", self.contents(), self.query(true)), &headers, None)?;
        match r.status {
            200 => {}
            304 => return Ok(Fetched::NotModified),
            // 404: no such file (or branch); Gitee may answer 404 with a message instead.
            404 => return Ok(Fetched::Missing),
            _ => return Err(self.fail(&r)),
        }
        let v = r.json().ok_or_else(|| SyncError::server(format!("{} 返回了无法解析的结果", self.site())))?;
        // Gitee answers [] for a missing file; a directory is an array too.
        if let Some(a) = v.as_array() {
            return if a.is_empty() { Ok(Fetched::Missing) } else { Err(SyncError::config("文件路径指向的是一个文件夹，请填写文件路径")) };
        }
        let sha = v.get("sha").and_then(Value::as_str).map(str::to_owned);
        if sha.is_none() {
            return Ok(Fetched::Missing);
        }
        if self.flavor == Flavor::Gitee && known.is_some_and(|k| k.sha == sha) {
            return Ok(Fetched::NotModified);
        }
        let bytes = match self.decode_content(&v) {
            Some(b) if !b.is_empty() || v.get("size").and_then(Value::as_u64) == Some(0) => b,
            _ => self.raw()?,
        };
        let rev = Rev { etag: r.header("etag").map(str::to_owned), sha, version: None };
        Ok(Fetched::File(bytes, rev))
    }

    fn put(&mut self, bytes: &[u8], base: Option<&Rev>) -> Result<Rev, SyncError> {
        let sha = base.and_then(|b| b.sha.clone());
        let mut body = json!({
            "message": format!("FluentCal 同步（{}）", super::device_name()),
            "content": base64::engine::general_purpose::STANDARD.encode(bytes),
        });
        if !self.branch.is_empty() {
            body["branch"] = json!(self.branch);
        }
        if let Some(s) = &sha {
            body["sha"] = json!(s);
        }
        let method = match (self.flavor, &sha) {
            (Flavor::Gitee, None) => "POST",
            _ => "PUT",
        };
        if self.flavor == Flavor::Gitee {
            body["access_token"] = json!(self.token);
        }
        let mut headers = self.headers("application/vnd.github+json");
        headers.push(("Content-Type", "application/json".to_owned()));
        let url = format!("{}{}", self.contents(), if self.flavor == Flavor::Gitee { String::new() } else { self.query(false) });
        let r = self.http.send(method, &url, &headers, Some(serde_json::to_vec(&body).unwrap_or_default()))?;
        match r.status {
            200 | 201 => {
                let v = r.json().unwrap_or_default();
                let sha = v.pointer("/content/sha").and_then(Value::as_str).map(str::to_owned);
                Ok(Rev { etag: None, sha, version: None })
            }
            // Stale or missing sha: the file changed (or appeared) since our fetch.
            409 => Err(SyncError::conflict()),
            422 | 400 => {
                let m = self.message(&r);
                let lower = m.to_lowercase();
                if lower.contains("sha") || m.contains("已存在") || lower.contains("exist") || lower.contains("match") {
                    Err(SyncError::new(ErrKind::Conflict, format!("{} 上的文件刚被修改（{m}）", self.site())))
                } else {
                    Err(self.fail(&r))
                }
            }
            _ => Err(self.fail(&r)),
        }
    }
}
