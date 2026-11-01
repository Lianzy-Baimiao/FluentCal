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
    // delete: asked first; 取消 keeps it
    t.g().invoke_action_arg("delete-group".into(), id.as_str().into());
    assert_eq!(t.w.get_confirm_title(), "删除分组“改名”？");
    t.answer("close");
    assert!(t.saved()["groups"].get(&id).is_some_and(|g| !g.is_null()));
    t.g().invoke_action_arg("delete-group".into(), id.as_str().into());
    t.answer("primary");
    assert!(t.saved()["groups"].get(&id).is_none_or(Value::is_null));
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
