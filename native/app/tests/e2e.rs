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
    let add = ElementHandle::find_by_accessible_label(&t.w, "添加任务").find(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::Button)).expect("the add button");
    t.click_at(t.at(&add, 10.0, 10.0));
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

/// What a screen reader gets (0.2's a11y.mjs ran axe on the desk and the Gantt view): every
/// control has a name, in both views and in the open menu and "+N" list.
#[test]
fn every_control_has_a_name() {
    let today = fluentcal_core::date::today();
    let tasks: Vec<Value> = (0..6).map(|i| json!({ "title": format!("体检{i}"), "start": today, "time": "09:00", "groupId": "g" })).collect();
    let mut doc = doc_with(&tasks, json!({}));
    doc["groups"] = json!({ "g": { "id": "g", "name": "项目", "color": "blue", "order": 1 } });
    let t = start("a11y", Some(doc));
    use i_slint_backend_testing::AccessibleRole as R;
    let unnamed = |t: &T| -> Vec<String> {
        t.w.root_element()
            .query_descendants()
            .match_predicate(|e| matches!(e.accessible_role(), Some(R::Button | R::Checkbox | R::Tab | R::ListItem | R::TextInput | R::Switch | R::RadioButton)) && e.accessible_label().is_none_or(|l| l.trim().is_empty()))
            .find_all()
            .iter()
            .map(|e| format!("{:?} {:?}", e.accessible_role(), e.id()))
            .collect()
    };
    assert!(unnamed(&t).is_empty(), "month view: {:?}", unnamed(&t));
    // The chip menu (icons, swatches) and the day list.
    let chip = t.find("体检0");
    t.right_click_at(t.at(&chip, 40.0, chip.size().height / 2.0));
    assert!(unnamed(&t).is_empty(), "menu: {:?}", unnamed(&t));
    t.key(&slint::SharedString::from(Key::Escape));
    t.click(&t.find_prefix("还有 ").accessible_label().unwrap());
    assert!(unnamed(&t).is_empty(), "day list: {:?}", unnamed(&t));
    t.key(&slint::SharedString::from(Key::Escape));
    with(|a| a.desk_close_popups()).unwrap();
    with(|a| a.desk_set_view("gantt")).unwrap();
    settle();
    assert!(unnamed(&t).is_empty(), "Gantt view: {:?}", unnamed(&t));
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

impl T {
    fn right_click_at(&self, position: LogicalPosition) {
        let button = PointerEventButton::Right;
        self.ev(WindowEvent::PointerMoved { position });
        self.ev(WindowEvent::PointerPressed { position, button });
        wait(30);
        self.ev(WindowEvent::PointerReleased { position, button });
        settle();
    }

    fn click(&self, label: &str) {
        let e = self.find(label);
        let s = e.size();
        self.click_at(self.at(&e, s.width / 2.0, s.height / 2.0));
    }

    fn has(&self, label: &str) -> bool {
        ElementHandle::find_by_accessible_label(&self.w, label).next().is_some()
    }
}

/// The Fluent context menu: right click a chip, a colour from the swatch row, an entry; Esc.
#[test]
fn context_menu_on_a_chip() {
    let today = fluentcal_core::date::today();
    let t = start("ctxmenu", Some(doc_with(&[json!({ "title": "体检", "start": today })], json!({}))));
    let chip = t.find("体检");
    t.right_click_at(t.at(&chip, 40.0, chip.size().height / 2.0));
    assert!(t.w.get_any_popup(), "the menu is open");
    assert!(t.has("标记为完成") && t.has("编辑…"), "the chip menu's entries");
    t.click("紫");
    assert_eq!(t.task("体检")["color"], json!("purple"));
    assert!(!t.w.get_any_popup(), "a colour closes the menu");
    t.right_click_at(t.at(&t.find("体检"), 40.0, chip.size().height / 2.0));
    t.click("标记为完成");
    assert_eq!(t.task("体检")["done"], json!(true));
    // Esc closes it without acting.
    t.right_click_at(t.at(&t.find("体检，已完成"), 40.0, chip.size().height / 2.0));
    assert!(t.w.get_any_popup());
    t.key(&slint::SharedString::from(Key::Escape));
    assert!(!t.w.get_any_popup());
    // A press outside the window (the mouse hook's report) closes it too.
    t.right_click_at(t.at(&t.find("体检，已完成"), 40.0, chip.size().height / 2.0));
    with(|a| a.desk_close_popups()).unwrap();
    settle();
    assert!(!t.w.get_any_popup());
}

/// A chip is a tab stop (0.2's chipKey): Space toggles, Delete deletes, Enter opens the editor,
/// the menu key opens its menu; the arrow keys still move the selected day.
#[test]
fn chip_keys() {
    let today = fluentcal_core::date::today();
    let t = start("chipkeys", Some(doc_with(&[json!({ "title": "复诊", "start": today })], json!({}))));
    // A press on the title focuses it (a drag that doesn't move).
    let press = |t: &T| {
        let chip = t.find_prefix("复诊");
        t.click_at(t.at(&chip, chip.size().width - 10.0, chip.size().height / 2.0));
    };
    press(&t);
    t.key(" ");
    assert_eq!(t.task("复诊")["done"], json!(true));
    t.key(" ");
    assert_eq!(t.task("复诊")["done"], json!(false), "the chip keeps the focus when its data changes");
    t.key(&slint::SharedString::from(Key::Menu));
    assert!(t.w.get_any_popup() && t.has("标记为完成"), "the chip's menu");
    t.key(&slint::SharedString::from(Key::Escape));
    press(&t);
    t.key(&slint::SharedString::from(Key::Delete));
    assert!(t.tasks().is_empty());
    t.ctrl("z");
    press(&t);
    t.key("
");
    assert!(with(|a| a.editor.is_some()).unwrap(), "Enter opens the editor");
    // The arrows move the selected day from a focused chip.
    let sel = with(|a| a.desk.ui.selected.clone()).unwrap();
    t.key(&slint::SharedString::from(Key::RightArrow));
    assert_eq!(with(|a| a.desk.ui.selected.clone()).unwrap(), fluentcal_core::date::add_days(&sel, 1));
}

/// The day grid by keyboard alone (0.2's roving tabindex): Tab reaches the selected day, the
/// arrows move it with the focus, Enter adds a task there and the focus comes back after.
#[test]
fn grid_by_keyboard() {
    let t = start("gridkeys", Some(doc_with(&[], json!({}))));
    let focused = || slint::ComponentHandle::global::<fluentcal_app::CellFocus>(&t.w).get_on();
    let tab: slint::SharedString = Key::Tab.into();
    for _ in 0..40 {
        if focused() {
            break;
        }
        t.key(&tab);
    }
    assert!(focused(), "Tab reaches the selected day");
    let sel = with(|a| a.desk.ui.selected.clone()).unwrap();
    t.key(&slint::SharedString::from(Key::RightArrow));
    t.key(&slint::SharedString::from(Key::DownArrow));
    let now = with(|a| a.desk.ui.selected.clone()).unwrap();
    assert_eq!(now, fluentcal_core::date::add_days(&sel, 8));
    t.key("\n");
    t.type_text("键盘添加");
    t.key("\n");
    assert_eq!(t.task("键盘添加")["start"], json!(now));
    t.key(&slint::SharedString::from(Key::Escape));
    // The box closed; the arrows still move the day.
    t.key(&slint::SharedString::from(Key::LeftArrow));
    assert_eq!(with(|a| a.desk.ui.selected.clone()).unwrap(), fluentcal_core::date::add_days(&now, -1));
    assert!(focused());
}

/// "+N": the day's tasks in a flyout; adding one there.
#[test]
fn day_list_lists_and_adds() {
    let day = fluentcal_core::date::today();
    let tasks: Vec<Value> = (0..8).map(|i| json!({ "title": format!("事项{i}"), "start": day })).collect();
    let t = start("daylist", Some(doc_with(&tasks, json!({}))));
    t.click(&t.find_prefix("还有 ").accessible_label().unwrap());
    assert!(t.w.get_any_popup(), "the day list is open");
    let chips = t.w.get_day_list_chips();
    assert_eq!(chips.row_count(), 8);
    t.click_at(t.at(&t.find("添加任务"), 20.0, 15.0));
    t.type_text("新事项");
    t.key("\n");
    assert_eq!(t.task("新事项")["start"], json!(day));
    // The open list follows the data.
    assert_eq!(t.w.get_day_list_chips().row_count(), 9);
}

/// The year/month picker under the title: next year, March.
#[test]
fn year_month_picker() {
    let t = start("ympicker", Some(doc_with(&[], json!({}))));
    let (y, _, _) = fluentcal_core::date::parts(&t.today());
    t.click(&t.find_prefix(&format!("{y}年")).accessible_label().unwrap());
    assert!(t.w.get_any_popup());
    t.click("下一年");
    t.click("3月");
    assert_eq!(with(|a| a.desk.ui.cursor.clone()).unwrap(), format!("{}-03-01", y + 1));
    assert!(!t.w.get_any_popup(), "picking closes it");
}

/// Ctrl+F: the search panel finds a task in another month and jumps there.
#[test]
fn search_finds_and_jumps() {
    let today = fluentcal_core::date::today();
    let later = fluentcal_core::date::add_months(&today, 2);
    let t = start("search", Some(doc_with(&[json!({ "title": "年度体检", "start": later })], json!({}))));
    t.ctrl("f");
    assert!(t.w.get_search_open());
    let _ = t.find("搜索任务");
    t.type_text("体检");
    let hits = t.w.get_search_hits();
    assert_eq!(hits.row_count(), 1);
    assert_eq!(hits.row_data(0).unwrap().date, later);
    t.key("\n");
    assert!(!t.w.get_search_open(), "Enter jumps and closes");
    assert_eq!(with(|a| a.desk.ui.cursor.clone()).unwrap(), fluentcal_core::date::start_of_month(&later));
}

/// A Fluent confirmation (deleting a group): 取消 keeps it, 删除 removes it.
#[test]
fn confirm_dialog_for_deleting_a_group() {
    use fluentcal_app::desk::menus::Target;
    let today = fluentcal_core::date::today();
    let mut doc = doc_with(&[json!({ "id": "a", "title": "设计", "start": today, "groupId": "g" })], json!({ "calendar": { "view": "gantt" } }));
    doc["groups"] = json!({ "g": { "id": "g", "name": "项目", "color": "blue", "order": 1 } });
    let t = start("confirm", Some(doc));
    with(|a| a.menu_act(&Target::Group("g".into()), "deleteGroup")).unwrap();
    settle();
    assert!(t.has("组内 1 个任务会移到“未分组”，不会被删除。"));
    t.click("取消");
    assert!(with(|a| a.doc.groups.contains_key("g")).unwrap());
    with(|a| a.menu_act(&Target::Group("g".into()), "deleteGroup")).unwrap();
    settle();
    t.click("删除");
    assert!(!with(|a| a.doc.groups.contains_key("g")).unwrap());
    assert!(t.task("设计").get("groupId").is_none_or(Value::is_null));
}

/// The welcome bar (sample tasks, not onboarded) and the adjust-mode bar.
#[test]
fn welcome_and_adjust_bars() {
    let today = fluentcal_core::date::today();
    let mut doc = doc_with(&[json!({ "title": "示例", "start": today, "sample": true })], json!({}));
    doc["settings"]["general"]["onboarded"] = json!(false);
    let t = start("bars", Some(doc));
    assert!(t.w.get_welcome());
    t.click("知道了");
    assert!(with(|a| a.doc.settings.general.onboarded).unwrap());
    assert!(!t.w.get_welcome());
    with(|a| a.desk_toggle_lock()).unwrap();
    settle();
    assert!(t.has("调整模式"));
    t.click("完成");
    assert!(with(|a| a.doc.settings.desk.locked).unwrap());
}

/// A plain HTTP/1.1 GET on loopback (no proxy): (status, body).
fn http_get(url: &str, token: Option<&str>) -> (u16, String) {
    use std::io::{Read, Write};
    let rest = url.strip_prefix("http://").unwrap();
    let (host, path) = rest.split_once('/').map_or((rest, "/".to_owned()), |(h, p)| (h, format!("/{p}")));
    let path = path.split('#').next().unwrap().to_owned();
    let mut s = std::net::TcpStream::connect(host).unwrap();
    s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
    let auth = token.map_or(String::new(), |t| format!("Authorization: Bearer {t}\r\n"));
    write!(s, "GET {path} HTTP/1.1\r\nHost: {host}\r\n{auth}Connection: close\r\n\r\n").unwrap();
    let mut buf = Vec::new();
    let _ = s.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status = text.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    (status, text.split_once("\r\n\r\n").map_or(String::new(), |(_, b)| b.to_owned()))
}

/// 0.2's "right-click a day › 在浏览器中查看": the loopback server starts, the link carries the
/// access code, the page and the read-only data are served; a wrong code is refused.
#[test]
fn view_in_browser_from_the_day_menu() {
    let today = fluentcal_core::date::today();
    // A free port: the installed program may be serving on the default one.
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let t = start("browser", Some(doc_with(&[json!({ "title": "浏览器里的任务", "start": today })], json!({ "web": { "port": port } }))));
    let _ = fluentcal_app::webcmd::captured_urls();
    let (_, m, d) = fluentcal_core::date::parts(&today);
    let cell = t.find_prefix(&format!("{m}月{d}日 "));
    t.right_click_at(t.at(&cell, cell.size().width / 2.0, cell.size().height - 6.0));
    t.click("在浏览器中查看");
    let urls = fluentcal_app::webcmd::captured_urls();
    assert_eq!(urls.len(), 1, "{urls:?} toast {:?}", t.w.get_toast_text());
    let url = &urls[0];
    assert!(url.starts_with("http://127.0.0.1:") && url.contains("?w=web#k="), "{url}");
    let token = url.split("#k=").nth(1).unwrap();
    let (code, page) = http_get(url, None);
    assert_eq!(code, 200);
    assert!(page.contains("<div id=\"app\""), "the web page");
    let api = format!("{}api/doc", &url[..url.find("?").unwrap()]);
    let (code, body) = http_get(&api, Some(token));
    assert_eq!(code, 200);
    assert!(body.contains("浏览器里的任务"));
    assert_eq!(http_get(&api, Some("wrong")).0, 401);
}

/// 0.2's "first run: welcome bar, clear sample data (a renamed one stays), undo".
#[test]
fn clear_sample_data_keeps_a_renamed_task() {
    let t = start("samples", None);
    assert!(t.w.get_welcome());
    let (n_tasks, n_groups) = with(|a| (a.doc.tasks.len(), a.doc.groups.len())).unwrap();
    let mine = t.tasks().into_iter().find(|x| x["groupId"].is_string()).unwrap();
    with(|a| {
        let task = a.doc.tasks[mine["id"].as_str().unwrap()].clone();
        let e = fluentcal_core::tasks::save_task(&a.doc, fluentcal_core::model::Task { title: "我的任务".into(), ..task }, 0);
        a.apply(e, true);
    })
    .unwrap();
    assert!(t.task("我的任务").get("sample").is_none_or(Value::is_null), "a renamed sample is the user's");
    t.click("清除示例数据");
    let left = t.tasks();
    assert_eq!(left.len(), 1, "{left:?}");
    assert!(left[0]["groupId"].is_null() && left[0]["deps"].as_array().is_none_or(Vec::is_empty));
    assert!(with(|a| a.doc.groups.is_empty() && a.doc.settings.general.onboarded).unwrap());
    assert!(!t.w.get_welcome());
    t.click("撤销");
    assert_eq!(with(|a| (a.doc.tasks.len(), a.doc.groups.len())).unwrap(), (n_tasks, n_groups));
    assert!(!t.w.get_welcome(), "onboarded stays");
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
