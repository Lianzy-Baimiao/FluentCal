// The settings window end to end (0.2's settings pages and their Tauri commands): the real app
// on a test data folder, Slint's headless testing backend, the pages' callbacks as the
// controls call them. Network only to an in-memory WebDAV server on 127.0.0.1.

#[path = "../../core/tests/common/mod.rs"]
mod common;

use fluentcal_app::app::with;
use fluentcal_app::{DeskWindow, SetState, SettingsWindow, Severity};
use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};
use serde_json::{json, Value};
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

struct T {
    _desk: DeskWindow,
    w: SettingsWindow,
    dir: PathBuf,
}

fn wait(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
    slint::platform::update_timers_and_animations();
}

/// Lets deferred work run (answers, jobs, dialogs opened on the next turn).
fn settle() {
    for _ in 0..6 {
        wait(15);
    }
}

fn start(name: &str, doc: Option<Value>, page: &str) -> T {
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions { mock_time: false, threading: false, ..Default::default() }))).expect("platform");
    let dir = std::env::temp_dir().join(format!("fluentcal-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(d) = doc {
        std::fs::write(dir.join("data.json"), d.to_string()).unwrap();
    }
    let desk = fluentcal_app::startup::test_app(dir.clone(), 880.0, 640.0).expect("app");
    settle();
    let page = page.to_owned();
    with(move |a| a.open_settings(Some(&page))).unwrap();
    settle();
    let w = with(|a| a.settings_win.as_ref().map(|s| s.window.clone_strong())).unwrap().expect("settings window");
    T { _desk: desk, w, dir }
}

impl T {
    fn g(&self) -> SetState<'_> {
        self.w.global::<SetState>()
    }
    fn saved(&self) -> Value {
        with(|a| a.store.flush()).unwrap();
        serde_json::from_str(&std::fs::read_to_string(self.dir.join("data.json")).unwrap()).unwrap()
    }
    fn msg(&self) -> (Severity, String) {
        let m = self.g().get_msg();
        assert!(m.shown, "no message");
        (m.severity, m.text.to_string())
    }
    fn answer(&self, r: &str) {
        self.g().invoke_confirm_answer(r.into());
        settle();
    }
    fn pick(&self, path: &std::path::Path) {
        let p = path.display().to_string();
        with(move |a| a.settings_win.as_mut().unwrap().local.test_pick = Some(p)).unwrap();
    }
    /// A real click (pointer events) on the button labelled `label`, e.g. in the confirmation.
    fn click_button(&self, label: &str) {
        use i_slint_backend_testing::{AccessibleRole, ElementHandle};
        use slint::platform::{PointerEventButton, WindowEvent};
        let b = ElementHandle::find_by_accessible_label(&self.w, label).find(|e| e.accessible_role() == Some(AccessibleRole::Button)).unwrap_or_else(|| panic!("no button {label:?}"));
        let (p, s) = (b.absolute_position(), b.size());
        let position = slint::LogicalPosition::new(p.x + s.width / 2.0, p.y + s.height / 2.0);
        let button = PointerEventButton::Left;
        self.w.window().dispatch_event(WindowEvent::PointerMoved { position });
        self.w.window().dispatch_event(WindowEvent::PointerPressed { position, button });
        wait(30);
        self.w.window().dispatch_event(WindowEvent::PointerReleased { position, button });
        settle();
    }
}

impl Drop for T {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn doc_with(tasks: Value) -> Value {
    json!({ "version": 1, "tasks": tasks, "groups": {}, "dayMarks": {}, "holidays": {}, "settings": { "general": { "seeded": true } } })
}

fn task(id: &str, title: &str) -> Value {
    json!({ "id": id, "title": title, "start": "2026-10-14", "end": "2026-10-14", "progress": 0, "done": false, "order": 1, "createdAt": 1, "updatedAt": 1 })
}

#[test]
fn settings_reach_data_json_and_the_pages() {
    let t = start("set", None, "appearance");
    assert_eq!(t.g().get_page(), "appearance");
    t.g().invoke_set_bool("desk.locked".into(), false);
    t.g().invoke_set_str("theme.mode".into(), "dark".into());
    t.g().invoke_set_num("desk.bgOpacity".into(), 40.0);
    t.g().invoke_set_num("calendar.weekStart".into(), 0.0);
    t.g().invoke_set_rest_day(5, true);
    t.g().invoke_action_arg("preset".into(), "glass".into());
    let s = t.saved()["settings"].clone();
    assert_eq!(s["desk"]["locked"], false);
    assert_eq!(s["theme"]["mode"], "dark");
    assert_eq!(s["calendar"]["weekStart"], json!(0));
    assert_eq!(s["calendar"]["restDays"], json!([0, 5, 6]));
    assert_eq!(s["desk"]["bgOpacity"], json!(18), "the preset comes last");
    // the pages show the new state
    assert!(!t.g().get_general().locked);
    assert_eq!(t.g().get_appearance().preset, "glass");
    assert_eq!(t.g().get_appearance().mode, "dark");
    // follow the Windows accent off: a fixed colour
    t.g().invoke_set_bool("follow-accent".into(), false);
    assert!(t.saved()["settings"]["theme"]["accent"].as_str().unwrap().starts_with('#'));
    // page change: the page's message goes
    t.g().invoke_action("backup-now".into());
    assert_eq!(t.msg().0, Severity::Success);
    t.g().invoke_navigate("calendar".into());
    assert_eq!(t.g().get_page(), "calendar");
    assert!(!t.g().get_msg().shown);
}

#[test]
fn groups_reminders_and_sample_data() {
    let t = start("groups", None, "gantt");
    // first start: sample tasks and groups
    assert!(t.g().get_data().samples > 0);
    let n0 = t.g().get_groups().row_count();
    t.g().invoke_add_group("  测试组 ".into());
    let groups = t.saved()["groups"].clone();
    let (id, g) = groups.as_object().unwrap().iter().find(|(_, g)| g["name"] == "测试组").map(|(k, v)| (k.clone(), v.clone())).expect("added");
    assert!(!g["color"].as_str().unwrap().is_empty());
    assert_eq!(t.g().get_groups().row_count(), n0 + 1);
    t.g().invoke_set_group_name(id.as_str().into(), "改名".into());
    t.g().invoke_set_group_color(id.as_str().into(), "red".into());
    assert_eq!(t.saved()["groups"][&id]["name"], "改名");
    assert_eq!(t.saved()["groups"][&id]["color"], "red");
    // delete: 删除分组 lists every group with this one ticked; 取消 keeps it
    let gd = || t.w.global::<fluentcal_app::GroupDelete>();
    t.g().invoke_action_arg("delete-group".into(), id.as_str().into());
    settle();
    assert_eq!(gd().get_title(), "删除分组“改名”？");
    let items: Vec<_> = gd().get_items().iter().collect();
    assert_eq!(items.len(), n0, "every real group is listed; ungrouped is not deletable");
    assert_eq!(items.iter().filter(|p| p.checked).map(|p| p.id.to_string()).collect::<Vec<_>>(), [id.clone()]);
    gd().invoke_answer("close".into());
    settle();
    assert!(t.saved()["groups"].get(&id).is_some_and(|g| !g.is_null()));
    // tick a sample group that has tasks too: both go at once, with its tasks
    t.g().invoke_action_arg("delete-group".into(), id.as_str().into());
    settle();
    assert!(!gd().get_with_tasks(), "tasks are retained by default");
    gd().invoke_with_tasks_toggled(true);
    let other = gd().get_items().iter().find(|p| p.id != id.as_str() && p.desc != "0 个任务").expect("a sample group with tasks").id.to_string();
    let theirs = |v: &Value| v["tasks"].as_object().unwrap().values().filter(|x| x["groupId"] == other.as_str()).count();
    let n = theirs(&t.saved());
    assert!(n > 0);
    let total = t.saved()["tasks"].as_object().unwrap().len();
    gd().invoke_toggled(other.as_str().into(), true);
    settle();
    assert_eq!(gd().get_title(), "删除 2 个分组？");
    assert_eq!(gd().get_with_tasks_label(), format!("同时删除分组里的任务（{n} 个）"));
    assert!(gd().get_can_delete());
    gd().invoke_answer("primary".into());
    settle();
    let saved = t.saved();
    assert!(saved["groups"].get(&id).is_none_or(Value::is_null));
    assert!(saved["groups"].get(&other).is_none_or(Value::is_null));
    assert_eq!(saved["tasks"].as_object().unwrap().len(), total - n, "its tasks are deleted");
    assert_eq!(t.msg(), (Severity::Success, format!("已删除 2 个分组和其中的任务（{n} 个）")));
    assert_eq!(t.g().get_groups().row_count(), n0 - 1, "the page follows");
    // the desk can undo it (one step), as a deletion made there
    with(|a| a.desk_undo(true)).unwrap();
    settle();
    assert!(t.saved()["groups"].get(&other).is_some_and(|g| !g.is_null()));
    assert_eq!(theirs(&t.saved()), n);
    with(|a| a.desk_redo()).unwrap();
    settle();
    assert_eq!(theirs(&t.saved()), 0);
    // default reminders
    t.g().invoke_action("add-offset".into());
    t.g().invoke_action("add-offset".into());
    assert_eq!(t.saved()["settings"]["reminders"]["defaultOffsets"], json!([0, 5]));
    t.g().invoke_set_offset(0, "1440".into());
    assert_eq!(t.saved()["settings"]["reminders"]["defaultOffsets"], json!([5, 1440]));
    t.g().invoke_set_offset(1, "".into());
    assert_eq!(t.saved()["settings"]["reminders"]["defaultOffsets"], json!([5]));
    // clear the sample data
    t.g().invoke_action("clear-samples".into());
    t.answer("primary");
    assert_eq!(t.g().get_data().samples, 0);
    assert_eq!(t.msg().0, Severity::Success);
    assert_eq!(t.saved()["settings"]["general"]["onboarded"], true);
}

#[test]
fn backups_and_restore() {
    let t = start("backup", Some(doc_with(json!({ "a": task("a", "原来的") }))), "data");
    t.g().invoke_action("backup-now".into());
    let (sev, text) = t.msg();
    assert_eq!(sev, Severity::Success);
    let name = text.trim_start_matches("已创建备份 ").to_owned();
    assert!(t.g().get_backups().iter().any(|b| b.name == name.as_str()), "listed");
    // change the data, then restore the backup
    with(|a| {
        let mut e = fluentcal_core::tasks::Edit::default();
        e.put("tasks", "a", task("a", "改过的"));
        a.apply(e, false);
    })
    .unwrap();
    assert_eq!(t.saved()["tasks"]["a"]["title"], "改过的");
    t.g().invoke_action_arg("restore".into(), name.as_str().into());
    assert!(t.w.get_confirm_danger());
    t.answer("primary");
    assert_eq!(t.saved()["tasks"]["a"]["title"], "原来的");
    assert_eq!(with(|a| a.doc.tasks["a"].title.clone()).unwrap(), "原来的");
}

#[test]
fn export_and_import() {
    let t = start("io", Some(doc_with(json!({ "a": task("a", "甲"), "b": task("b", "乙") }))), "data");
    let out = t.dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    t.pick(&out);
    t.g().invoke_action("export-json".into());
    t.g().invoke_action("export-csv".into());
    t.g().invoke_action("export-ics".into());
    let files: Vec<String> = std::fs::read_dir(&out).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(files.len(), 3, "{files:?}");
    let json_file = files.iter().find(|f| f.ends_with(".json")).unwrap();
    assert!(json_file.starts_with("FluentCal-备份-"));
    let backup: Value = serde_json::from_str(&std::fs::read_to_string(out.join(json_file)).unwrap()).unwrap();
    assert_eq!(backup["tasks"].as_object().unwrap().len(), 2);
    assert_eq!(backup["settings"]["calendar"]["view"], "month", "settings with defaults");
    let csv = std::fs::read(out.join(files.iter().find(|f| f.ends_with(".csv")).unwrap())).unwrap();
    assert!(csv.starts_with(b"\xEF\xBB\xBF"), "Excel's BOM");

    // a file with one more task and a changed one: merge
    let mut file = backup.clone();
    file["tasks"]["c"] = task("c", "丙");
    file["tasks"]["a"]["title"] = json!("甲（新）");
    file["tasks"]["a"]["updatedAt"] = json!(9_999_999_999_999i64);
    let path = t.dir.join("in.json");
    std::fs::write(&path, file.to_string()).unwrap();
    t.pick(&path);
    t.g().invoke_action("import-json".into());
    assert_eq!(t.w.get_confirm_primary(), "合并");
    assert!(t.w.get_confirm_content().contains("加入 1 个新任务"));
    t.answer("primary");
    let saved = t.saved();
    assert_eq!(saved["tasks"]["c"]["title"], "丙");
    assert_eq!(saved["tasks"]["a"]["title"], "甲（新）");
    assert!(t.msg().1.starts_with("已合并"));

    // replace: only the file's tasks; this PC's settings stay
    t.g().invoke_set_str("theme.mode".into(), "dark".into());
    let only = doc_with(json!({ "z": task("z", "只有这个") }));
    std::fs::write(&path, only.to_string()).unwrap();
    t.g().invoke_action("import-json".into());
    t.answer("secondary");
    let saved = t.saved();
    assert_eq!(saved["tasks"].as_object().unwrap().keys().collect::<Vec<_>>(), vec!["z"]);
    assert_eq!(saved["settings"]["theme"]["mode"], "dark");
    assert!(t.g().get_backups().iter().any(|b| b.name.starts_with("pre-import")), "backed up first");
}

#[test]
fn data_folder_change_moves_the_data_and_restarts() {
    let t = start("dir", Some(doc_with(json!({ "a": task("a", "要搬走的") }))), "data");
    let target = t.dir.join("shared");
    std::fs::create_dir_all(&target).unwrap();
    t.pick(&target);
    t.g().invoke_action("change-dir".into());
    assert_eq!(t.w.get_confirm_primary(), "更改并重启");
    t.answer("primary");
    assert!(with(|a| a.settings_win.as_ref().unwrap().local.restart_asked).unwrap());
    let moved: Value = serde_json::from_str(&std::fs::read_to_string(target.join("data.json")).unwrap()).unwrap();
    assert_eq!(moved["tasks"]["a"]["title"], "要搬走的");
    assert!(moved.get("settings").is_none(), "settings stay on this PC");
    let pointer = std::fs::read_to_string(t.dir.join("datadir.txt")).unwrap();
    assert_eq!(pointer.trim(), target.display().to_string());
}

/// GitHub #2: clicking 更改并重启 did nothing. The dialog answers "primary" and then "close" as
/// it closes; both went through zero-delay timers, which Slint runs newest first within the
/// same millisecond, so "close" used up the confirmation. Real clicks, as the user does it.
#[test]
fn confirmation_buttons_work_when_clicked() {
    let t = start("dir-click", Some(doc_with(json!({ "a": task("a", "要搬走的") }))), "data");
    let target = t.dir.join("shared");
    std::fs::create_dir_all(&target).unwrap();
    t.pick(&target);
    t.g().invoke_action("change-dir".into());
    settle();
    t.click_button("更改并重启");
    assert!(with(|a| a.settings_win.as_ref().unwrap().local.restart_asked).unwrap(), "the program restarts");
    let pointer = std::fs::read_to_string(t.dir.join("datadir.txt")).expect("the pointer file is written");
    assert_eq!(pointer.trim(), target.display().to_string());
    let moved: Value = serde_json::from_str(&std::fs::read_to_string(target.join("data.json")).unwrap()).unwrap();
    assert_eq!(moved["tasks"]["a"]["title"], "要搬走的");

    // the second button (导入 › 替换) and 取消
    let only = doc_with(json!({ "z": task("z", "只有这个") }));
    let path = t.dir.join("in.json");
    std::fs::write(&path, only.to_string()).unwrap();
    t.pick(&path);
    t.g().invoke_action("import-json".into());
    settle();
    t.click_button("取消");
    assert!(t.saved()["tasks"].get("a").is_some(), "取消 changes nothing");
    t.g().invoke_action("import-json".into());
    settle();
    t.click_button("替换");
    assert_eq!(t.saved()["tasks"].as_object().unwrap().keys().collect::<Vec<_>>(), vec!["z"]);
}

/// An in-memory WebDAV folder (user u, password pw).
fn dav_server() -> (String, Arc<Mutex<(bool, Option<Vec<u8>>)>>) {
    let state = Arc::new(Mutex::new((false, None::<Vec<u8>>)));
    let st = state.clone();
    let (base, _) = common::serve(move |req| {
        let mut d = st.lock().unwrap();
        if req.header("authorization") != Some("Basic dTpwdw==") {
            return common::Res::new(401);
        }
        let is_file = req.path().ends_with("/fluentcal-data.json");
        match (req.method.as_str(), is_file) {
            ("MKCOL", false) if d.0 => common::Res::new(405),
            ("MKCOL", false) => {
                d.0 = true;
                common::Res::new(201)
            }
            ("GET", true) => match &d.1 {
                None => common::Res::new(404),
                Some(b) => common::Res::new(200).with("ETag", "\"1\"").body(b.clone()),
            },
            ("PUT", true) if !d.0 => common::Res::new(409),
            ("PUT", true) => {
                d.1 = Some(req.body.clone());
                common::Res::new(201).with("ETag", "\"1\"")
            }
            _ => common::Res::new(400),
        }
    });
    (base, state)
}

#[test]
fn sync_test_and_enable_against_webdav() {
    let t = start("sync", Some(doc_with(json!({ "a": task("a", "上传我") }))), "sync");
    let (base, dav) = dav_server();
    t.g().invoke_set_str("sync.provider".into(), "webdav".into());
    t.g().invoke_set_str("sync.webdav.url".into(), base.as_str().into());
    t.g().invoke_set_str("sync.webdav.user".into(), "u".into());
    assert!(t.g().get_sync().cloud);
    // the password goes to Credential Manager (test namespace), never into the data
    t.g().invoke_save_secret("webdav".into(), "pw".into());
    assert!(t.g().get_sync().secret_saved);
    assert!(!std::fs::read_to_string(t.dir.join("data.json")).unwrap_or_default().contains("\"pw\""));
    t.g().invoke_action("sync-test".into());
    settle();
    let (sev, text) = t.msg();
    assert_eq!(sev, Severity::Success, "{text}");
    assert!(text.starts_with("连接成功。云端还没有数据"), "{text}");
    t.g().invoke_action("sync-enable".into());
    settle();
    let (sev, text) = t.msg();
    assert_eq!((sev, text.as_str()), (Severity::Success, "已开启同步。"));
    assert_eq!(t.saved()["settings"]["sync"]["enabled"], true);
    assert!(t.g().get_sync().locked, "provider and address locked while on");
    let up: Value = serde_json::from_slice(dav.lock().unwrap().1.as_ref().expect("uploaded")).unwrap();
    assert!(up.to_string().contains("上传我"));
    t.g().invoke_action("sync-disable".into());
    assert_eq!(t.saved()["settings"]["sync"]["enabled"], false);
    fluentcal_core::cloud::secrets::delete("webdav");
}

#[test]
fn mobile_download_and_store_update_controls() {
    let t = start("mobile-update", None, "sync");
    fluentcal_app::webcmd::captured_urls();
    t.g().invoke_action("mobile-download".into());
    assert!(fluentcal_app::webcmd::captured_urls().iter().any(|u| u == fluentcal_core::pairing::MOBILE_DOWNLOAD));
    t.g().invoke_navigate("about".into());
    t.g().invoke_action("store-check-update".into());
    assert!(t.g().get_about().update_checking);
    settle();
    assert!(!t.g().get_about().update_checking);
    assert!(t.g().get_about().update_message.contains("不是 Microsoft Store"));
}

#[test]
fn leaving_pair_page_cancels_session() {
    let t = start("pair-cancel", None, "sync");
    with(|a| {
        let session = fluentcal_core::pairing::start(&json!({}), "127.0.0.1:0", std::time::Duration::from_secs(3)).unwrap();
        a.settings_win.as_mut().unwrap().local.pairing = Some(session);
        a.settings_refresh();
    }).unwrap();
    assert!(t.g().get_pair_active());
    t.g().invoke_navigate("general".into());
    assert!(!t.g().get_pair_active());
    assert!(with(|a| a.settings_win.as_ref().unwrap().local.pairing.is_none()).unwrap());
}

/// One POST to the endpoint for AI assistants on a connection of its own: (status, body).
fn mcp_post(port: u16, token: &str, body: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let mut conn = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    conn.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
    write!(conn, "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut answer = String::new();
    let _ = conn.read_to_string(&mut answer);
    let status = answer.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    (status, answer.split_once("\r\n\r\n").map_or(String::new(), |(_, b)| b.to_owned()))
}

/// A tool called the way an assistant does, from another thread, while this one (the UI
/// thread: the test has no event loop) makes the changes it asks for. Ok(what it told) or
/// Err(why not).
fn mcp_call(port: u16, token: &str, tool: &str, args: Value) -> Result<Value, String> {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": tool, "arguments": args } }).to_string();
    let token = token.to_owned();
    let call = std::thread::spawn(move || mcp_post(port, &token, &body));
    while !call.is_finished() {
        with(|a| a.mcp_drain());
        wait(5);
    }
    let (status, body) = call.join().unwrap();
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or_else(|| panic!("no text in {body}")).to_owned();
    if v["result"]["isError"] == true {
        Err(text)
    } else {
        Ok(serde_json::from_str(&text).unwrap())
    }
}

/// 设置 › AI 助手 (需求文档 4.12): off unless turned on; on, an assistant reads and changes the
/// calendar of the running program through the endpoint, and what it changed is in 最近更改.
#[test]
fn ai_assistants_connect_only_when_allowed() {
    use fluentcal_core::cloud::secrets;
    let free_port = || std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let t = start("mcp", Some(doc_with(json!({ "t1": task("t1", "已有的任务") }))), "mcp");
    secrets::delete("mcp-token"); // (left by a run that did not get to its end)
    let listening = |port: u16| std::net::TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), std::time::Duration::from_millis(300)).is_ok();

    // as installed: off, and nothing of it exists
    let m = t.g().get_mcp();
    assert!(!m.enabled && !m.running && m.write && m.error.is_empty(), "off by default, changes allowed once on");
    assert_eq!(m.port, 18766.0);
    assert!(!listening(18766) || !with(|a| a.mcp_info().running).unwrap());
    assert!(secrets::get("mcp-token").is_none(), "no token before it is turned on");
    assert!(!t.dir.join("mcp.json").exists());

    // turned on: the endpoint, its token, the note for the bridge (`fluentcal.exe mcp`)
    let port = free_port();
    t.g().invoke_set_num("mcp.port".into(), f32::from(port));
    assert!(!listening(port), "a port alone starts nothing");
    t.g().invoke_set_bool("mcp.enabled".into(), true);
    let m = t.g().get_mcp();
    assert!(m.enabled && m.running && m.error.is_empty(), "{}", m.error);
    assert_eq!(t.saved()["settings"]["mcp"], json!({ "enabled": true, "port": port }), "only what was set is written");
    let token = secrets::get("mcp-token").expect("a token was made");
    assert!(token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(fluentcal_core::mcp::bridge::read_state(&t.dir), Some(port));
    // what the page shows to copy: the address, this program, and dots for the token
    let info = with(|a| a.mcp_info()).unwrap();
    assert_eq!((info.url.clone(), info.token.clone()), (format!("http://127.0.0.1:{port}/mcp"), token.clone()));
    assert!(m.url_config.contains(&format!("\"url\": \"http://127.0.0.1:{port}/mcp\"")) && m.url_config.contains("Bearer ••••••••") && !m.url_config.contains(&token));
    assert!(!m.claude_url.contains(&token) && m.claude_url.starts_with("claude mcp add --scope user --transport http fluentcal http://127.0.0.1:"));
    let shown: Value = serde_json::from_str(&m.command_config).expect("the configuration is JSON");
    assert_eq!(shown["mcpServers"]["fluentcal"]["args"], json!(["mcp"]));
    assert_eq!(shown["mcpServers"]["fluentcal"]["command"], info.exe.as_str());
    // (no `--` and no `--mcp`: PowerShell does not pass a lone `--` on to npm's claude.ps1)
    assert_eq!(m.claude_command, format!("claude mcp add --scope user fluentcal \"{}\" mcp", info.exe));
    // the sentence for the assistants that add the server themselves: this program and the
    // same word; what "复制" puts on the clipboard is what the page shows
    assert!(!info.packaged);
    assert!(m.agent_sentence.contains(&format!("\"{}\"", info.exe)) && m.agent_sentence.contains("\"mcp\"") && m.agent_sentence.contains("fluentcal"), "{}", m.agent_sentence);
    assert_eq!(m.agent_sentence, fluentcal_app::mcpcmd::agent_sentence(&info.exe));
    assert!(!m.agent_sentence.contains(&token));
    // (a Store install gets the address and, in the copy only, the token)
    let by_address = fluentcal_app::mcpcmd::agent_sentence_url(&info.url, &info.token);
    assert!(by_address.contains(&info.url) && by_address.contains(&format!("Authorization: Bearer {token}")), "{by_address}");
    let copied: Value = serde_json::from_str(&fluentcal_app::mcpcmd::url_config(&info.url, &info.token)).unwrap();
    assert_eq!(copied["mcpServers"]["fluentcal"]["headers"]["Authorization"], format!("Bearer {token}"));
    assert_eq!(copied["mcpServers"]["fluentcal"]["type"], "http");

    // nobody without the token
    assert_eq!(mcp_post(port, "wrong", "{}").0, 401);
    // an assistant looks…
    let seen = mcp_call(port, &token, "list_tasks", json!({ "from": "2026-10-14", "to": "2026-10-14" })).unwrap();
    assert_eq!(seen["tasks"][0]["title"], "已有的任务");
    // …and changes: at once in the program, and listed in 最近更改 to take back
    let before = with(|a| a.changes.list().len()).unwrap();
    let added = mcp_call(port, &token, "add_tasks", json!({ "tasks": [{ "title": "助手加的任务", "start": "2026-10-15", "time": "9:00" }] })).unwrap();
    let id = added["tasks"][0]["id"].as_str().unwrap().to_owned();
    let found = with({
        let id = id.clone();
        move |a| a.doc.tasks.get(&id).map(|t| (t.title.clone(), t.start.clone(), t.time.clone()))
    })
    .unwrap();
    assert_eq!(found, Some(("助手加的任务".to_owned(), "2026-10-15".to_owned(), Some("09:00".to_owned()))));
    assert_eq!(t.saved()["tasks"][&id]["title"], "助手加的任务");
    let changes = with(|a| a.changes.list()).unwrap();
    assert_eq!(changes.len(), before + 1);
    assert!(changes.iter().any(|c| c.text().contains("助手加的任务")), "{:?}", changes.iter().map(|c| c.text()).collect::<Vec<_>>());
    mcp_call(port, &token, "set_task_done", json!({ "id": "t1" })).unwrap();
    assert!(with(|a| a.doc.tasks["t1"].done).unwrap());
    // nothing to change is not a change
    let n = with(|a| a.changes.list().len()).unwrap();
    assert_eq!(mcp_call(port, &token, "update_task", json!({ "id": id, "title": "助手加的任务" })).unwrap()["unchanged"], true);
    assert_eq!(with(|a| a.changes.list().len()).unwrap(), n);
    // what is wrong is told, and nothing happens
    assert!(mcp_call(port, &token, "update_task", json!({ "id": id, "start": "2026-02-30" })).unwrap_err().contains("YYYY-MM-DD"));
    assert_eq!(with(|a| a.changes.list().len()).unwrap(), n);

    // looking only
    t.g().invoke_set_bool("mcp.write".into(), false);
    assert!(t.g().get_mcp().running && !t.g().get_mcp().write);
    let said = mcp_call(port, &token, "delete_task", json!({ "id": id })).unwrap_err();
    assert!(said.contains("允许修改日历"), "{said}");
    assert!(with({
        let id = id.clone();
        move |a| a.doc.tasks.contains_key(&id)
    })
    .unwrap());
    assert_eq!(mcp_call(port, &token, "get_task", json!({ "id": id })).unwrap()["title"], "助手加的任务");
    t.g().invoke_set_bool("mcp.write".into(), true);

    // a new token: asked first, then the old one is worth nothing
    t.g().invoke_action("mcp-reset-token".into());
    settle();
    assert_eq!(t.w.get_confirm_title(), "重新生成令牌？");
    t.answer("close");
    assert_eq!(secrets::get("mcp-token").as_deref(), Some(token.as_str()), "not without a yes");
    t.g().invoke_action("mcp-reset-token".into());
    settle();
    t.answer("primary");
    let new = secrets::get("mcp-token").unwrap();
    assert!(new != token && new.len() == 32);
    assert_eq!(t.msg().0, Severity::Info);
    assert_eq!(mcp_post(port, &token, "{}").0, 401);
    assert!(mcp_call(port, &new, "list_groups", json!({})).is_ok());

    // another port: the old one is let go of
    let other = free_port();
    t.g().invoke_set_num("mcp.port".into(), f32::from(other));
    assert!(t.g().get_mcp().running && listening(other) && !listening(port));
    assert_eq!(fluentcal_core::mcp::bridge::read_state(&t.dir), Some(other));
    // a port somebody else has: said on the page, and nothing runs
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    t.g().invoke_set_num("mcp.port".into(), f32::from(taken.local_addr().unwrap().port()));
    let m = t.g().get_mcp();
    assert!(m.enabled && !m.running && m.error.contains("占用"), "{}", m.error);
    assert!(!listening(other) && fluentcal_core::mcp::bridge::read_state(&t.dir).is_none());
    t.g().invoke_set_num("mcp.port".into(), f32::from(other));
    assert!(t.g().get_mcp().running && t.g().get_mcp().error.is_empty());

    // turned off: gone at once; the token stays for the next time
    t.g().invoke_set_bool("mcp.enabled".into(), false);
    let m = t.g().get_mcp();
    assert!(!m.enabled && !m.running && m.error.is_empty());
    assert!(!listening(other) && !t.dir.join("mcp.json").exists());
    assert_eq!(secrets::get("mcp-token").as_deref(), Some(new.as_str()));
    t.g().invoke_set_bool("mcp.enabled".into(), true);
    assert!(mcp_call(other, &new, "list_groups", json!({})).is_ok(), "the configuration an assistant was given still works");
    t.g().invoke_set_bool("mcp.enabled".into(), false);
    secrets::delete("mcp-token");
}
