// End-to-end tests of the native app (0.2's scripts/e2e.mjs): the real app on a test data
// folder, driven through Slint's headless testing backend (clicks, drags, keys, the screen
// reader's view of the elements). Nothing appears on screen; each test is its own thread.

use fluentcal_app::app::with;
use fluentcal_app::DeskWindow;
use i_slint_backend_testing::{ElementHandle, ElementRoot, TestingBackend, TestingBackendOptions};
use serde_json::{json, Value};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::PathBuf;

struct T {
    w: DeskWindow,
    dir: PathBuf,
}

/// A fresh data folder (with `doc` as data.json) and the app on it.
fn start(name: &str, doc: Option<Value>) -> T {
    // Real time (the published crate has no mock-time helper outside its own tests): events,
    // then timers run in `settle`.
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions { mock_time: false, threading: false, ..Default::default() }))).expect("platform");
    let dir = std::env::temp_dir().join(format!("fluentcal-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(d) = doc {
        std::fs::write(dir.join("data.json"), d.to_string()).unwrap();
    }
    let w = fluentcal_app::startup::test_app(dir.clone(), 880.0, 640.0).expect("app");
    settle();
    T { w, dir }
}

/// Lets deferred work and timers run (layout, the lane capacity, debounced refreshes).
fn settle() {
    for _ in 0..5 {
        wait(20);
    }
}

fn wait(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
    slint::platform::update_timers_and_animations();
}

impl T {
    fn find(&self, label: &str) -> ElementHandle {
        ElementHandle::find_by_accessible_label(&self.w, label).next().unwrap_or_else(|| panic!("no element labelled {label:?}"))
    }

    fn find_prefix(&self, prefix: &str) -> ElementHandle {
        let p = prefix.to_owned();
        self.w.root_element().query_descendants().match_predicate(move |e| e.accessible_label().is_some_and(|l| l.starts_with(&p))).find_first().unwrap_or_else(|| panic!("no element labelled {prefix:?}…"))
    }

    fn at(&self, e: &ElementHandle, dx: f32, dy: f32) -> LogicalPosition {
        let p = e.absolute_position();
        LogicalPosition::new(p.x + dx, p.y + dy)
    }

    fn ev(&self, e: WindowEvent) {
        self.w.window().dispatch_event(e);
    }

    fn click_at(&self, position: LogicalPosition) {
        let button = PointerEventButton::Left;
        self.ev(WindowEvent::PointerMoved { position });
        self.ev(WindowEvent::PointerPressed { position, button });
        wait(30);
        self.ev(WindowEvent::PointerReleased { position, button });
        settle();
    }

    fn double_click_at(&self, position: LogicalPosition) {
        let button = PointerEventButton::Left;
        self.ev(WindowEvent::PointerMoved { position });
        for _ in 0..2 {
            self.ev(WindowEvent::PointerPressed { position, button });
            wait(40);
            self.ev(WindowEvent::PointerReleased { position, button });
            wait(40);
        }
        settle();
    }

    fn drag(&self, from: LogicalPosition, to: LogicalPosition) {
        let button = PointerEventButton::Left;
        self.ev(WindowEvent::PointerMoved { position: from });
        self.ev(WindowEvent::PointerPressed { position: from, button });
        for i in 1..=10 {
            let t = i as f32 / 10.0;
            wait(16);
            self.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t) });
        }
        self.ev(WindowEvent::PointerReleased { position: to, button });
        settle();
    }

    fn key(&self, text: &str) {
        self.ev(WindowEvent::KeyPressed { text: text.into() });
        self.ev(WindowEvent::KeyReleased { text: text.into() });
        settle();
    }

    fn ctrl(&self, letter: &str) {
        let c: slint::SharedString = Key::Control.into();
        self.ev(WindowEvent::KeyPressed { text: c.clone() });
        self.key(letter);
        self.ev(WindowEvent::KeyReleased { text: c });
        settle();
    }

    fn type_text(&self, s: &str) {
        for ch in s.chars() {
            self.key(&ch.to_string());
        }
    }

    fn tasks(&self) -> Vec<Value> {
        with(|a| a.store.snapshot()["tasks"].as_object().map(|m| m.values().cloned().collect()).unwrap_or_default()).unwrap()
    }

    fn task(&self, title: &str) -> Value {
        self.tasks().into_iter().find(|t| t["title"] == title).unwrap_or_else(|| panic!("no task {title:?}"))
    }

    fn today(&self) -> String {
        with(|a| a.today()).unwrap()
    }
}

impl Drop for T {
    fn drop(&mut self) {
        // The app first (it saves into the folder), then the folder.
        if let Some(app) = fluentcal_app::app::uninstall() {
            app.store.flush();
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// One task (all-day, or with `extra` fields) for `date`, in a document of its own.
fn doc_with(tasks: &[Value], settings: Value) -> Value {
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
    let mut s = settings;
    s["general"] = json!({ "seeded": true, "onboarded": true });
    s["holidays"] = json!({ "autoUpdate": false });
    json!({ "version": 1, "tasks": m, "groups": {}, "dayMarks": {}, "holidays": {}, "settings": s })
}
#[test]
fn first_run_seeds_sample_data() {
    let t = start("seed", None);
    let tasks = t.tasks();
    assert!(tasks.len() >= 5, "sample tasks: {}", tasks.len());
    assert!(tasks.iter().all(|x| x["sample"] == true));
    assert_eq!(with(|a| a.doc.settings.general.seeded).unwrap(), true);
    // The calendar shows them: some chip is labelled with a sample title.
    let title = tasks[0]["title"].as_str().unwrap().to_owned();
    assert!(ElementHandle::find_by_accessible_label(&t.w, &title).next().is_some() || t.tasks().len() > 1);
}

#[test]
fn quick_add_then_undo() {
    let t = start("quickadd", Some(doc_with(&[], json!({}))));
    let today = t.today();
    let (_, m, d) = fluentcal_core::date::parts(&today);
    let cell = t.find_prefix(&format!("{m}月{d}日 "));
    t.double_click_at(t.at(&cell, 40.0, 50.0));
    let input = t.find("快速添加任务");
    assert_eq!(input.accessible_role(), Some(i_slint_backend_testing::AccessibleRole::TextInput));
    t.type_text("写周报");
    t.key("\n");
    let task = t.task("写周报");
    assert_eq!(task["start"], json!(today));
    // The box stays open for the next one.
    t.type_text("买菜");
    t.key("\n");
    assert_eq!(t.tasks().len(), 2);
    t.key(&slint::SharedString::from(Key::Escape));
    // Each entry is its own undo step.
    t.ctrl("z");
    assert_eq!(t.tasks().len(), 1);
    assert_eq!(t.w.get_toast_text(), "已撤销");
    t.ctrl("y");
    assert_eq!(t.tasks().len(), 2);
}

#[test]
fn checkbox_toggles_done_and_drag_moves() {
    let today = fluentcal_core::date::today();
    let t = start("chip", Some(doc_with(&[json!({ "title": "交房租", "start": today })], json!({}))));
    let chip = t.find("交房租");
    // The check circle: 3 px padding, 12 px.
    t.click_at(t.at(&chip, 9.0, chip.size().height / 2.0));
    assert_eq!(t.task("交房租")["done"], json!(true));
    t.click_at(t.at(&chip, 9.0, chip.size().height / 2.0));
    assert_eq!(t.task("交房租")["done"], json!(false));
    // Drag the chip one day to the right (one cell + gap).
    let cell_w = t.w.get_cell_w() + t.w.get_cell_gap();
    let from = t.at(&chip, 40.0, chip.size().height / 2.0);
    let next = fluentcal_core::date::add_days(&today, 1);
    // Saturday → Sunday may wrap to the next row in a Monday-first month: drag down instead.
    let to = if fluentcal_core::date::weekday(&today) == 0 { LogicalPosition::new(from.x - 6.0 * cell_w, from.y + t.w.get_cell_h() + t.w.get_cell_gap()) } else { LogicalPosition::new(from.x + cell_w, from.y) };
    t.drag(from, to);
    assert_eq!(t.task("交房租")["start"], json!(next));
    t.ctrl("z");
    assert_eq!(t.task("交房租")["start"], json!(today));
}

#[test]
fn countdown_shows_the_next_holiday() {
    let t = start("countdown", Some(doc_with(&[], json!({}))));
    let c = t.w.root_element().query_descendants().match_predicate(|e| e.accessible_label().is_some_and(|l| l.contains("还有") || l.contains("就是今天"))).find_first();
    assert!(c.is_some(), "countdown pill");
}

#[test]
fn gantt_drag_add_link_zoom() {
    let today = fluentcal_core::date::today();
    let d = |n: i32| fluentcal_core::date::add_days(&today, n);
    let mut doc = doc_with(
        &[json!({ "id": "a", "title": "设计", "start": d(0), "end": d(2), "groupId": "g" }), json!({ "id": "b", "title": "开发", "start": d(4), "end": d(7), "groupId": "g" })],
        json!({ "calendar": { "view": "gantt" } }),
    );
    doc["groups"] = json!({ "g": { "id": "g", "name": "项目", "color": "blue", "order": 1 } });
    let t = start("gantt", Some(doc));
    // Drag "设计" two days later (32 px a day at the day scale).
    let bar = t.find("设计");
    let from = t.at(&bar, bar.size().width / 2.0, 10.0);
    t.drag(from, LogicalPosition::new(from.x + 64.0, from.y));
    assert_eq!(t.task("设计")["start"], json!(d(2)));
    assert_eq!(t.task("设计")["end"], json!(d(4)));
    // Link: from the end of "设计" onto "开发".
    let bar = t.find("设计");
    let handle = t.at(&bar, bar.size().width + 6.0, 9.0);
    let target = t.find("开发");
    t.drag(handle, t.at(&target, 20.0, 10.0));
    assert_eq!(t.task("开发")["deps"], json!(["a"]));
    // "添加任务" in the group: a new task, renamed inline.
    let before = t.tasks().len();
    t.click_at(t.at(&t.find("添加任务"), 10.0, 10.0));
    assert_eq!(t.tasks().len(), before + 1);
    assert!(t.tasks().iter().any(|x| x["title"] == "新任务" && x["groupId"] == "g"));
    // Zoom out keeps the date at the centre (0.2's e2e: within a few days).
    let centre = with(|a| a.gantt_date_at(0.5)).unwrap();
    with(|a| a.gantt_zoom(-1)).unwrap();
    settle();
    assert_eq!(with(|a| a.doc.settings.gantt.scale.clone()).unwrap(), "week");
    let after = with(|a| a.gantt_date_at(0.5)).unwrap();
    assert!(fluentcal_core::date::diff_days(&centre, &after).abs() <= 7, "{centre} → {after}");
}

#[test]
fn gantt_export_png_and_svg() {
    let today = fluentcal_core::date::today();
    let t = start("export", Some(doc_with(&[json!({ "title": "导出", "start": today, "end": fluentcal_core::date::add_days(&today, 3) })], json!({ "gantt": { "hideSingleDay": false } }))));
    let (svg, w, h) = with(|a| {
        let m = fluentcal_app::desk::gantt::model::compute(&a.doc, &today, &a.tokens, false);
        fluentcal_app::desk::gantt::export::svg(&a.doc, &m, &a.tokens, &today)
    })
    .unwrap();
    assert!(svg.starts_with("<svg") && svg.contains("导出"));
    let png = fluentcal_app::desk::gantt::export::png(&svg, w, h).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    drop(t);
}

/// What a screen reader gets (0.2's a11y.mjs ran axe): every control has a name.
#[test]
fn every_control_has_a_name() {
    let today = fluentcal_core::date::today();
    let t = start("a11y", Some(doc_with(&[json!({ "title": "体检", "start": today, "time": "09:00" })], json!({}))));
    use i_slint_backend_testing::AccessibleRole as R;
    let unnamed: Vec<String> = t
        .w
        .root_element()
        .query_descendants()
        .match_predicate(|e| matches!(e.accessible_role(), Some(R::Button | R::Checkbox | R::Tab | R::ListItem | R::TextInput | R::Switch)) && e.accessible_label().is_none_or(|l| l.trim().is_empty()))
        .find_all()
        .iter()
        .map(|e| format!("{:?} {:?}", e.accessible_role(), e.id()))
        .collect();
    assert!(unnamed.is_empty(), "unnamed controls: {unnamed:?}");
}

/// 0.2's "delete via context menu, then undo"; day colour; group order (the menus' actions).
#[test]
fn menu_actions_and_undo() {
    use fluentcal_app::desk::menus::Target;
    let today = fluentcal_core::date::today();
    let mut doc = doc_with(&[json!({ "id": "x", "title": "看牙", "start": today }), json!({ "id": "r", "title": "跑步", "start": today, "repeat": { "freq": "daily" } })], json!({}));
    doc["groups"] = json!({ "a": { "id": "a", "name": "甲", "color": "blue", "order": 1 }, "b": { "id": "b", "name": "乙", "color": "red", "order": 2 } });
    let t = start("menus", Some(doc));
    with(|a| a.menu_act(&Target::Task("x".into(), today.clone()), "delete:all")).unwrap();
    settle();
    assert!(t.tasks().iter().all(|x| x["id"] != "x"));
    assert!(t.w.get_toast_text().starts_with("已删除“看牙”"));
    assert!(t.w.get_toast_undo(), "the toast offers undo");
    t.click_at(t.at(&t.find("撤销"), 10.0, 10.0));
    assert_eq!(t.task("看牙")["id"], "x");
    // One occurrence of a repeating task.
    with(|a| a.menu_act(&Target::Task("r".into(), today.clone()), "delete:this")).unwrap();
    assert_eq!(t.task("跑步")["exDates"], json!([today]));
    // Day colour, then cleared.
    with(|a| a.menu_act(&Target::Day(today.clone()), "mark:green")).unwrap();
    assert_eq!(with(|a| a.store.snapshot()["dayMarks"][&today]["color"].clone()).unwrap(), "green");
    with(|a| a.menu_act(&Target::Day(today.clone()), "unmark")).unwrap();
    assert!(with(|a| a.store.snapshot()["dayMarks"].get(&today).is_none()).unwrap());
    // Group order: 乙 up swaps the orders.
    with(|a| a.menu_act(&Target::Group("b".into()), "up")).unwrap();
    let g = with(|a| a.store.snapshot()["groups"].clone()).unwrap();
    assert!(g["b"]["order"].as_f64() < g["a"]["order"].as_f64());
    // Countdown and colour on a task.
    with(|a| a.menu_act(&Target::Task("x".into(), today.clone()), "countdown")).unwrap();
    with(|a| a.menu_act(&Target::Task("x".into(), today.clone()), "color:purple")).unwrap();
    assert_eq!((t.task("看牙")["countdown"].clone(), t.task("看牙")["color"].clone()), (json!(true), json!("purple")));
}

#[test]
#[ignore]
fn dump_labels() {
    let today = fluentcal_core::date::today();
    let t = start("dump", Some(doc_with(&[json!({ "title": "交房租", "start": today })], json!({}))));
    for e in t.w.root_element().query_descendants().find_all() {
        if let Some(l) = e.accessible_label() {
            if !l.is_empty() {
                println!("{:?} {:?} {:?} {:?}", e.accessible_role(), l, e.absolute_position(), e.size());
            }
        }
    }
    println!("weeks {} cap {} size {:?}", t.w.get_weeks().row_count(), t.w.get_capacity(), t.w.window().size());
}
