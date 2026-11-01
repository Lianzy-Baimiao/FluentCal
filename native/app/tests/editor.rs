// End-to-end tests of the task editor window (0.2's editor flows): the real app on a test
// data folder, the editor opened as the calendar opens it, driven through Slint's headless
// testing backend. Nothing appears on screen; each test is its own thread.

use fluentcal_app::app::{run, with};
use fluentcal_app::EditorWindow;
use i_slint_backend_testing::{ElementHandle, TestingBackend, TestingBackendOptions};
use serde_json::{json, Value};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::path::PathBuf;

struct T {
    dir: PathBuf,
}

fn start(name: &str, tasks: &[Value]) -> T {
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions { mock_time: false, threading: false, ..Default::default() }))).expect("platform");
    let dir = std::env::temp_dir().join(format!("fluentcal-editor-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("data.json"), doc_with(tasks).to_string()).unwrap();
    fluentcal_app::startup::test_app(dir.clone(), 880.0, 640.0).expect("app");
    settle();
    T { dir }
}

fn settle() {
    for _ in 0..5 {
        wait(20);
    }
}

fn wait(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
    slint::platform::update_timers_and_animations();
}

fn doc_with(tasks: &[Value]) -> Value {
    let mut m = serde_json::Map::new();
    for (i, t) in tasks.iter().enumerate() {
        let mut t = t.clone();
        let id = t["id"].as_str().map(str::to_owned).unwrap_or_else(|| format!("t{i}"));
        t["id"] = json!(id);
        if t.get("end").is_none() {
            t["end"] = t["start"].clone();
        }
        for (k, v) in [("progress", json!(0)), ("done", json!(false)), ("order", json!(i + 1)), ("createdAt", json!(0)), ("updatedAt", json!(0))] {
            if t.get(k).is_none() {
                t[k] = v;
            }
        }
        m.insert(id, t);
    }
    let s = json!({ "general": { "seeded": true, "onboarded": true }, "holidays": { "autoUpdate": false } });
    json!({ "version": 1, "tasks": m, "groups": {}, "dayMarks": {}, "holidays": {}, "settings": s })
}

impl T {
    /// Opens the editor as the calendar does and returns its window.
    fn open(&self, f: impl FnOnce(&mut fluentcal_app::app::App) + 'static) -> EditorWindow {
        run(f);
        settle();
        self.editor().expect("editor window")
    }

    fn editor(&self) -> Option<EditorWindow> {
        with(|a| a.editor_window()).flatten()
    }

    fn find(&self, w: &EditorWindow, label: &str) -> ElementHandle {
        ElementHandle::find_by_accessible_label(w, label).next().unwrap_or_else(|| panic!("no element labelled {label:?}"))
    }

    fn click(&self, w: &EditorWindow, e: &ElementHandle) {
        let p = e.absolute_position();
        let s = e.size();
        let position = LogicalPosition::new(p.x + s.width / 2.0, p.y + s.height / 2.0);
        let button = PointerEventButton::Left;
        w.window().dispatch_event(WindowEvent::PointerMoved { position });
        w.window().dispatch_event(WindowEvent::PointerPressed { position, button });
        wait(30);
        w.window().dispatch_event(WindowEvent::PointerReleased { position, button });
        settle();
    }

    fn key(&self, w: &EditorWindow, text: &str) {
        w.window().dispatch_event(WindowEvent::KeyPressed { text: text.into() });
        w.window().dispatch_event(WindowEvent::KeyReleased { text: text.into() });
        settle();
    }

    fn ctrl(&self, w: &EditorWindow, k: &str) {
        let c: slint::SharedString = Key::Control.into();
        w.window().dispatch_event(WindowEvent::KeyPressed { text: c.clone() });
        self.key(w, k);
        w.window().dispatch_event(WindowEvent::KeyReleased { text: c });
        settle();
    }

    fn type_text(&self, w: &EditorWindow, s: &str) {
        self.key(w, &slint::SharedString::from(Key::End));
        for ch in s.chars() {
            self.key(w, &ch.to_string());
        }
    }

    fn task(&self, id: &str) -> Option<Value> {
        with(|a| a.store.snapshot()["tasks"].get(id).cloned()).flatten()
    }

    fn tasks(&self) -> Vec<Value> {
        with(|a| a.store.snapshot()["tasks"].as_object().map(|m| m.values().cloned().collect()).unwrap_or_default()).unwrap()
    }

    fn today(&self) -> String {
        with(|a| a.today()).unwrap()
    }
}

impl Drop for T {
    fn drop(&mut self) {
        if let Some(app) = fluentcal_app::app::uninstall() {
            app.store.flush();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn new_task_is_created_and_saved() {
    let t = start("new", &[]);
    let today = t.today();
    let w = t.open(|a| a.open_editor_new(None));
    assert_eq!(w.get_title_text(), "");
    // The title box has the focus: typing goes there.
    t.type_text(&w, "写周报");
    t.click(&w, &t.find(&w, "创建"));
    assert!(t.editor().is_none(), "the editor closes after saving");
    let tasks = t.tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!((tasks[0]["title"].as_str(), tasks[0]["start"].as_str()), (Some("写周报"), Some(today.as_str())));
    // and it is in data.json
    with(|a| a.store.flush());
    let file: Value = serde_json::from_str(&std::fs::read_to_string(t.dir.join("data.json")).unwrap()).unwrap();
    assert!(file["tasks"].as_object().unwrap().values().any(|x| x["title"] == "写周报"));
}

#[test]
fn empty_title_is_not_saved() {
    let t = start("empty", &[]);
    let w = t.open(|a| a.open_editor_new(None));
    t.ctrl(&w, "s");
    assert!(t.editor().is_some() && w.get_title_error());
    assert!(t.find(&w, "请输入任务名称").accessible_label().is_some());
    assert!(t.tasks().is_empty());
}

#[test]
fn edit_and_save_with_ctrl_s() {
    let t = start("edit", &[json!({ "id": "a", "title": "周报", "start": "2026-10-14" })]);
    let w = t.open(|a| a.open_editor_task("a", None));
    assert_eq!(w.get_title_text(), "周报");
    t.type_text(&w, "初稿");
    t.ctrl(&w, "s");
    assert!(t.editor().is_none());
    assert_eq!(t.task("a").unwrap()["title"], "周报初稿");
}

#[test]
fn save_keeps_what_changed_elsewhere_meanwhile() {
    let t = start("merge", &[json!({ "id": "a", "title": "周报", "start": "2026-10-14" })]);
    let w = t.open(|a| a.open_editor_task("a", None));
    t.type_text(&w, "初稿");
    // Meanwhile the task is ticked done in the calendar.
    run(|a| {
        let task = a.doc.tasks["a"].clone();
        let edit = fluentcal_core::tasks::set_occurrence_done(&a.doc, &task, "2026-10-14", true, 5);
        a.apply(edit, true);
    });
    settle();
    t.ctrl(&w, "\n");
    let a = t.task("a").unwrap();
    assert_eq!((a["title"].as_str(), a["done"].as_bool()), (Some("周报初稿"), Some(true)));
}

#[test]
fn delete_one_occurrence_or_all() {
    let rule = json!({ "freq": "weekly", "weekdays": [5] });
    let t = start("delete", &[json!({ "id": "w", "title": "交周报", "start": "2026-10-02", "repeat": rule })]);
    let w = t.open(|a| a.open_editor_task("w", Some("2026-10-16".into())));
    t.click(&w, &t.find(&w, "删除"));
    assert!(t.find(&w, "“交周报”是重复任务，要删除哪些？").accessible_label().is_some());
    t.click(&w, &t.find(&w, "仅这一次"));
    assert!(t.editor().is_none());
    assert_eq!(t.task("w").unwrap()["exDates"], json!(["2026-10-16"]));
    let w = t.open(|a| a.open_editor_task("w", None));
    t.click(&w, &t.find(&w, "删除"));
    t.click(&w, &t.find(&w, "全部删除"));
    assert!(t.task("w").is_none());
}

#[test]
fn delete_asks_and_cancel_keeps() {
    let t = start("delete1", &[json!({ "id": "a", "title": "周报", "start": "2026-10-14" })]);
    let w = t.open(|a| a.open_editor_task("a", None));
    t.click(&w, &t.find(&w, "删除"));
    assert!(t.find(&w, "“周报”将被永久删除。").accessible_label().is_some());
    t.key(&w, &slint::SharedString::from(Key::Escape));
    assert!(t.editor().is_some() && t.task("a").is_some());
}

#[test]
fn unsaved_changes_ask_before_closing_and_switching() {
    let t = start("pending", &[json!({ "id": "a", "title": "周报", "start": "2026-10-14" }), json!({ "id": "b", "title": "买菜", "start": "2026-10-15" })]);
    let w = t.open(|a| a.open_editor_task("a", None));
    // unchanged: Esc closes at once
    t.key(&w, &slint::SharedString::from(Key::Escape));
    assert!(t.editor().is_none());
    let w = t.open(|a| a.open_editor_task("a", None));
    t.type_text(&w, "初稿");
    t.key(&w, &slint::SharedString::from(Key::Escape));
    t.click(&w, &t.find(&w, "继续编辑"));
    assert!(t.editor().is_some() && w.get_title_text() == "周报初稿");
    // Opened on b while a has changes: asks, then shows b in the same window.
    run(|a| a.open_editor_task("b", None));
    settle();
    t.click(&w, &t.find(&w, "放弃"));
    assert_eq!(w.get_title_text(), "买菜");
    assert_eq!(t.task("a").unwrap()["title"], "周报");
    // The window's close button asks too.
    t.type_text(&w, "和水果");
    w.window().dispatch_event(WindowEvent::CloseRequested);
    settle();
    assert!(t.editor().is_some());
    t.click(&w, &t.find(&w, "放弃"));
    assert!(t.editor().is_none());
}

#[test]
fn predecessor_tags_and_repeat_days() {
    let t = start("deps", &[json!({ "id": "a", "title": "需求调研", "start": "2026-10-09" }), json!({ "id": "b", "title": "视觉设计", "start": "2026-10-12", "deps": ["a"] })]);
    let w = t.open(|a| a.open_editor_task("b", None));
    let tag_x = t.find(&w, "移除 需求调研");
    assert_eq!((tag_x.size().width, tag_x.size().height), (20.0, 20.0));
    // inside the tag, after its title
    let title = ElementHandle::find_by_accessible_label(&w, "需求调研").next().unwrap();
    assert!(tag_x.absolute_position().x >= title.absolute_position().x + title.size().width);
    t.click(&w, &tag_x);
    t.ctrl(&w, "s");
    assert!(t.task("b").unwrap().get("deps").is_none());
    // a weekly rule, then one more day
    let w = t.open(|a| a.open_editor_task("a", None));
    run(|a| a.editor_msg(fluentcal_app::windows::editor::Msg::Edit("freq".into(), "weekly".into())));
    settle();
    t.click(&w, &t.find(&w, "周一"));
    t.ctrl(&w, "s");
    assert_eq!(t.task("a").unwrap()["repeat"]["weekdays"], json!([1, 5]));
}
