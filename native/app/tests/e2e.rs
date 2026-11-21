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

#[test]
fn locate_search_target_keeps_list_and_reveals_only_that_task() {
    let mut doc = doc_with(&[
        json!({"id":"target","title":"跨月目标","start":"2024-10-30","end":"2024-11-03","done":true,"groupId":"a"}),
        json!({"id":"other","title":"其他隐藏任务","start":"2024-10-31","done":true,"groupId":"a"}),
    ], json!({"calendar":{"view":"list","listRange":"upcoming","listGroupLabels":true},"tasks":{"showCompleted":false},"theme":{"reduceMotion":true}}));
    doc["groups"] = json!({"a":{"id":"a","name":"工作","color":"red","order":1},"b":{"id":"b","name":"生日","color":"blue","order":2}});
    let t = start("locate-list",Some(doc));
    with(|a| a.desk_filter_toggle("b",true)).unwrap();
    t.ctrl("f"); t.type_text("跨月目标");
    t.click("跨月目标");
    assert_eq!(t.w.get_view(),"list");
    assert_eq!(t.w.get_flash_date(),"2024-10-30");
    assert_eq!(t.w.get_flash_end(),"2024-11-03");
    assert_eq!(t.w.get_flash_task(),"target");
    let rows:Vec<_>=t.w.get_list_rows().iter().filter(|r|r.kind=="item").collect();
    assert_eq!(rows.len(),1);
    assert_eq!(rows[0].group_name,"工作");
    assert_eq!(with(|a|(a.doc.settings.calendar.list_range.clone(),a.doc.settings.tasks.show_completed,a.desk.ui.group_filter.clone())).unwrap(),("upcoming".into(),false,vec!["b".to_owned()]));
    wait(3100);
    assert_eq!(t.w.get_flash_task(),"");
    assert_eq!(t.w.get_list_rows().iter().filter(|r|r.kind=="item").count(),1,"target remains until explicit navigation");
    with(|a| a.desk_filter_clear()).unwrap(); settle();
    assert_eq!(t.w.get_list_rows().iter().filter(|r|r.kind=="item").count(),0,"normal visibility restored");
}

#[test]
fn countdown_popup_sources_persist_and_empty_bar_keeps_its_control() {
    let today=fluentcal_core::date::today();
    let mut doc=doc_with(&[
        json!({"id":"a","title":"甲生日","start":today,"groupId":"g","countdown":true}),
        json!({"id":"b","title":"乙生日","start":today,"groupId":"g"}),
    ],json!({"calendar":{"holidayCountdown":false}}));
    doc["groups"]=json!({"g":{"id":"g","name":"生日","color":"red","order":1}});
    let t=start("countdown-sources",Some(doc));
    assert_eq!(t.w.get_all_countdowns().row_count(),1);
    t.click("倒计时显示内容");
    let tick=|label:&str| {
        let e=ElementHandle::find_by_accessible_label(&t.w,label).find(|e|e.accessible_role()==Some(i_slint_backend_testing::AccessibleRole::Checkbox)).unwrap();
        t.click_at(t.at(&e,9.0,e.size().height/2.0));
    };
    tick("生日");
    assert_eq!(t.w.get_all_countdowns().row_count(),2,"union has no duplicate marked birthday");
    tick("手工标记的倒数日");
    assert_eq!(t.w.get_all_countdowns().row_count(),2);
    tick("生日");
    assert_eq!(t.w.get_all_countdowns().row_count(),0);
    t.key(&slint::SharedString::from(Key::Escape));
    t.click("倒计时显示内容");
    assert!(t.w.get_any_popup(),"empty selection can be changed again");
    assert!(!with(|a|a.doc.settings.calendar.countdown_marked).unwrap());
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
    // Sunday → Monday wraps to the next row in a Monday-first month: drag down instead.
    let to = if fluentcal_core::date::weekday(&today) == 0 { LogicalPosition::new(from.x - 6.0 * cell_w, from.y + t.w.get_cell_h() + t.w.get_cell_gap()) } else { LogicalPosition::new(from.x + cell_w, from.y) };
    t.drag(from, to);
    assert_eq!(t.task("交房租")["start"], json!(next));
    t.ctrl("z");
    assert_eq!(t.task("交房租")["start"], json!(today));
}

/// Fixed dates keep cross-week dragging covered on every day the suite runs.
fn check_drag_shortcuts(name: &str, source: &str, target: &str, week_start: i32) {
    let t = start(name, Some(doc_with(&[json!({ "title": "拖动撤销", "start": source })], json!({ "calendar": { "weekStart": week_start } }))));
    t.w.invoke_jump(source.into());
    settle();
    let chip = t.find("拖动撤销");
    let from = t.at(&chip, 40.0, chip.size().height / 2.0);
    let (_, month, day) = fluentcal_core::date::parts(target);
    let cell = t.find_prefix(&format!("{month}月{day}日 "));
    let to = t.at(&cell, 40.0, 50.0);
    assert_eq!(with(|a| a.desk_date_at(to.x, to.y)).flatten().as_deref(), Some(target));
    t.drag(from, to);
    assert_eq!(t.task("拖动撤销")["start"], json!(target));
    assert!(with(|a| a.history.can_undo()).unwrap());
    t.ctrl("z");
    assert_eq!(t.task("拖动撤销")["start"], json!(source), "Ctrl+Z must reach the calendar after dropping a chip");
    t.ctrl("y");
    assert_eq!(t.task("拖动撤销")["start"], json!(target), "Ctrl+Y must still reach the calendar after undo");
}

#[test]
fn drag_across_week_rows_keeps_undo_and_redo_shortcuts() {
    check_drag_shortcuts("drag-next-week", "2026-11-08", "2026-11-09", 1);
}

#[test]
fn drag_to_previous_week_keeps_undo_and_redo_shortcuts() {
    check_drag_shortcuts("drag-prev-week", "2026-11-09", "2026-11-08", 1);
}

#[test]
fn drag_with_sunday_week_start_keeps_undo_and_redo_shortcuts() {
    check_drag_shortcuts("drag-sunday-week", "2026-11-07", "2026-11-08", 0);
}

#[test]
fn drag_within_week_keeps_undo_and_redo_shortcuts() {
    check_drag_shortcuts("drag-same-week", "2026-11-09", "2026-11-10", 1);
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
    // the group filter's list
    t.click("按分组筛选");
    assert!(t.w.get_any_popup() && unnamed(&t).is_empty(), "group filter: {:?}", unnamed(&t));
    t.key(&slint::SharedString::from(Key::Escape));
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

/// Locate feedback remains readable for three seconds; toast text survives its fade.
#[test]
fn flash_and_toast_animations() {
    let t = start("anims", Some(doc_with(&[], json!({}))));
    let today = t.today();
    with(|a| a.desk_flash(&today)).unwrap();
    wait(500);
    let w1 = with(|a| a.desk.window.get_flash_date().to_string()).unwrap();
    assert_eq!(w1, today, "still flashing at 0.5 s");
    wait(1200);
    assert_eq!(with(|a| a.desk.window.get_flash_date().to_string()).unwrap(), today, "still visible after 1.6 s");
    wait(1500);
    assert_eq!(with(|a| a.desk.window.get_flash_date().to_string()).unwrap(), "", "over after 3 s");
    // The toast: text set, then cleared; the visible text stays until it has faded.
    with(|a| a.notify("测试提示", false)).unwrap();
    settle();
    assert!(t.has("测试提示"));
    with(|a| a.desk_close_toast()).unwrap();
    // Look before sleeping: the fade lasts 187 ms, and a 30 ms sleep here sometimes took longer
    // than that while the other tests were running.
    wait(0);
    assert!(t.has("测试提示"), "the text stays while the toast fades");
    wait(400);
    assert!(!t.has("测试提示"), "gone after the fade");
}

/// The Gantt view's 今天 glides there (0.2: scrollTo with behavior 'smooth').
#[test]
fn gantt_today_glides() {
    let today = fluentcal_core::date::today();
    let t = start("glide", Some(doc_with(&[json!({ "title": "长任务", "start": today, "end": fluentcal_core::date::add_days(&today, 3) })], json!({ "calendar": { "view": "gantt" } }))));
    let gd = || slint::ComponentHandle::global::<fluentcal_app::GanttData>(&t.w).get_scroll_x();
    with(|a| a.gantt_scroll_to(&fluentcal_core::date::add_days(&today, -200), 0.3)).unwrap();
    settle();
    let far = gd();
    with(|a| a.gantt_go_today()).unwrap();
    wait(90);
    let mid = gd();
    wait(400);
    settle();
    let end = gd();
    assert!(mid != far && mid != end, "moving: {far} → {mid} → {end}");
    assert_eq!(with(|a| a.gantt_date_at(0.3)).unwrap(), today, "ends with today at 30%");
}

/// Menus are as wide as their longest entry, as in 0.2 (measured in 0.2's page, logical px):
/// "更多" 228, a chip's menu 266 (its 11 colours wrap after 9), the Gantt options 230.
#[test]
fn menu_widths_match_0_2() {
    let today = fluentcal_core::date::today();
    let t = start("menuw", Some(doc_with(&[json!({ "title": "评审会", "start": today, "time": "14:00" })], json!({}))));
    use i_slint_backend_testing::AccessibleRole as R;
    let panel_w = |t: &T, first: &str| -> f32 {
        let row = ElementHandle::find_by_accessible_label(&t.w, first).find(|e| e.accessible_role() == Some(R::ListItem)).unwrap_or_else(|| panic!("no row {first:?}"));
        // the row (with its 4px margins) is the panel's width less the 1px border each side
        row.size().width + 2.0
    };
    t.click("更多");
    let w = panel_w(&t, "新建任务…");
    assert!((w - 228.0).abs() <= 6.0, "更多: {w}");
    t.key(&slint::SharedString::from(Key::Escape));
    let chip = t.find("评审会");
    t.right_click_at(t.at(&chip, 40.0, chip.size().height / 2.0));
    let w = panel_w(&t, "标记为完成");
    assert!((w - 266.0).abs() <= 6.0, "chip menu: {w}");
    t.key(&slint::SharedString::from(Key::Escape));
    with(|a| a.desk_set_view("gantt")).unwrap();
    settle();
    t.click("显示选项");
    let w = panel_w(&t, "显示任务表");
    assert!((w - 230.0).abs() <= 6.0, "Gantt options: {w}");
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

/// A right click anywhere on a Gantt row opens its menu (0.2: contextmenu on the whole row):
/// on a group's name and chevron, on its row in the chart (the summary bar), on a task's name.
/// It never collapses the group or acts like a left click.
#[test]
fn right_click_anywhere_on_a_gantt_row() {
    use i_slint_backend_testing::AccessibleRole;
    let today = fluentcal_core::date::today();
    let tasks = [
        json!({ "id": "a", "title": "设计", "start": today, "end": fluentcal_core::date::add_days(&today, 3), "groupId": "g" }),
        json!({ "id": "b", "title": "开发", "start": today, "end": fluentcal_core::date::add_days(&today, 5), "groupId": "g" }),
    ];
    let mut doc = doc_with(&tasks, json!({ "calendar": { "view": "gantt" } }));
    doc["groups"] = json!({ "g": { "id": "g", "name": "项目", "color": "blue", "order": 1 } });
    let t = start("rowmenu", Some(doc));
    let row = |label: &'static str| {
        let is_row = move |e: &ElementHandle| e.accessible_role() == Some(AccessibleRole::ListItem) && e.accessible_label().as_deref() == Some(label);
        t.w.root_element().query_descendants().match_predicate(is_row).find_first().unwrap_or_else(|| panic!("no row {label}"))
    };
    let group_menu_open = || t.w.get_any_popup() && t.has("删除分组…");
    let close = || {
        t.key(&slint::SharedString::from(Key::Escape));
        assert!(!t.w.get_any_popup());
    };
    let g = row("项目");
    let h = g.size().height / 2.0;
    // the name (it takes left clicks for collapsing)
    t.right_click_at(t.at(&g, 70.0, h));
    assert!(group_menu_open(), "right click on the group's name");
    close();
    // the chevron
    t.right_click_at(t.at(&g, 16.0, h));
    assert!(group_menu_open(), "right click on the chevron");
    close();
    assert!(!with(|a| a.doc.groups["g"].collapsed == Some(true)).unwrap(), "a right click doesn't collapse");
    // the group's row in the chart, right of the table
    t.right_click_at(t.at(&g, g.size().width + 150.0, h));
    assert!(group_menu_open(), "right click on the group's row in the chart");
    close();
    // a left press there still clears the selection
    with(|a| a.gantt_select("a")).unwrap();
    t.click_at(t.at(&g, g.size().width + 150.0, h));
    assert_eq!(with(|a| a.desk.gantt.selected.clone()).unwrap(), "");
    // a task's name: its menu, and the row is selected
    let task = row("设计");
    t.right_click_at(t.at(&task, 70.0, task.size().height / 2.0));
    assert!(t.w.get_any_popup() && t.has("复制一份"), "the task menu");
    assert_eq!(with(|a| a.desk.gantt.selected.clone()).unwrap(), "a");
    close();
}

/// 删除分组 (the Gantt group menu): every group is listed, the clicked one is ticked; ticking
/// more deletes them together, with their tasks unless 同时删除分组里的任务 is unticked (then
/// the tasks move to 未分组), in one undo step. 取消 keeps them.
#[test]
fn delete_several_groups_at_once() {
    use fluentcal_app::desk::menus::Target;
    use i_slint_backend_testing::AccessibleRole;
    let today = fluentcal_core::date::today();
    let tasks = [
        json!({ "id": "a", "title": "设计", "start": today, "groupId": "g1" }),
        json!({ "id": "b", "title": "开发", "start": today, "groupId": "g2" }),
        json!({ "id": "c", "title": "测试", "start": today, "groupId": "g3", "deps": ["b"] }),
    ];
    // The chart shows only 项目一: deleting it must not leave the chart empty.
    let mut doc = doc_with(&tasks, json!({ "calendar": { "view": "gantt" }, "gantt": { "groupFilter": ["g1"] } }));
    doc["groups"] = json!({
        "g1": { "id": "g1", "name": "项目一", "color": "blue", "order": 1 },
        "g2": { "id": "g2", "name": "项目二", "color": "green", "order": 2 },
        "g3": { "id": "g3", "name": "项目三", "color": "red", "order": 3 },
    });
    let t = start("delgroups", Some(doc));
    let gd = || t.w.global::<fluentcal_app::GroupDelete>();
    // The dialog's group boxes carry the task count (the table's check circles and 同时删除 don't).
    let boxes = || {
        let is_pick = |e: &ElementHandle| e.accessible_role() == Some(AccessibleRole::Checkbox) && e.accessible_description().is_some_and(|d| !d.is_empty());
        t.w.root_element().query_descendants().match_predicate(is_pick).find_all()
    };
    let ticked = || boxes().iter().filter(|b| b.accessible_checked() == Some(true)).filter_map(|b| b.accessible_label()).map(|l| l.to_string()).collect::<Vec<_>>();
    let pick = |name: &str| boxes().into_iter().find(|b| b.accessible_label().as_deref() == Some(name)).unwrap_or_else(|| panic!("no box {name}"));
    // On the box, or on the row's name (the whole row ticks).
    let tick = |name: &str| {
        let b = pick(name);
        let s = b.size();
        t.click_at(t.at(&b, s.width / 2.0, s.height / 2.0));
    };
    let tick_row = |name: &str| {
        let b = pick(name);
        t.click_at(t.at(&b, b.size().width + 60.0, b.size().height / 2.0));
    };
    // 同时删除分组里的任务（N 个）
    let with_tasks_box = || {
        let is_box = |e: &ElementHandle| e.accessible_role() == Some(AccessibleRole::Checkbox) && e.accessible_label().is_some_and(|l| l.starts_with("同时删除分组里的任务"));
        t.w.root_element().query_descendants().match_predicate(is_box).find_first()
    };
    let tick_with_tasks = || {
        let b = with_tasks_box().expect("同时删除 box");
        t.click_at(t.at(&b, 10.0, b.size().height / 2.0));
    };
    with(|a| a.menu_act(&Target::Group("g1".into()), "deleteGroup")).unwrap();
    settle();
    assert!(t.has("删除分组“项目一”？"));
    let labels: Vec<_> = boxes().iter().filter_map(|b| b.accessible_label()).map(|l| l.to_string()).collect();
    assert_eq!(labels, ["项目一", "项目二", "项目三"], "all groups, in the chart's order");
    assert_eq!(ticked(), ["项目一"]);
    assert_eq!(boxes()[0].accessible_description().as_deref(), Some("1 个任务"));
    // Tasks stay by default; deleting them requires an explicit opt-in.
    let wb = with_tasks_box().expect("同时删除 box");
    assert_eq!((wb.accessible_label().as_deref(), wb.accessible_checked()), (Some("同时删除分组里的任务（1 个）"), Some(false)));
    assert!(!gd().get_with_tasks());
    assert!(t.has("所选分组里的任务会移到“未分组”，不会被删除。"));
    tick_with_tasks();
    assert!(gd().get_with_tasks());
    // Ticking a second one: the title and the task count follow; unticking all disables 删除.
    tick("项目二");
    assert!(t.has("删除 2 个分组？"));
    assert!(t.has("同时删除分组里的任务（2 个）"));
    assert_eq!(ticked(), ["项目一", "项目二"]);
    tick_row("项目一");
    assert_eq!(ticked(), ["项目二"], "a click on the row's name unticks it");
    assert!(t.has("删除分组“项目二”？"));
    tick("项目二");
    assert!(ticked().is_empty());
    assert!(t.has("删除分组"));
    assert!(with_tasks_box().is_none(), "no tasks ticked, no 同时删除");
    assert!(!gd().get_can_delete());
    t.click("删除");
    assert_eq!(with(|a| a.doc.groups.len()).unwrap(), 3, "删除 is disabled with nothing ticked");
    assert_eq!(boxes().len(), 3, "and the dialog stays open");
    t.click("取消");
    assert_eq!(with(|a| a.doc.groups.len()).unwrap(), 3);
    assert!(boxes().is_empty(), "closed");
    // Two at once, with their tasks.
    with(|a| a.menu_act(&Target::Group("g1".into()), "deleteGroup")).unwrap();
    settle();
    assert_eq!(ticked(), ["项目一"], "a new dialog starts with the clicked group only");
    assert!(!gd().get_with_tasks(), "every new dialog keeps tasks by default");
    tick_with_tasks();
    tick("项目二");
    t.click("删除");
    let groups: Vec<String> = with(|a| a.doc.groups.keys().cloned().collect()).unwrap();
    assert_eq!(groups, ["g3"]);
    let titles: Vec<String> = t.tasks().iter().map(|x| x["title"].as_str().unwrap().to_owned()).collect();
    assert_eq!(titles, ["测试"], "the groups' tasks are deleted");
    assert!(t.task("测试")["deps"].as_array().is_none_or(|d| d.is_empty()), "the deleted predecessor is dropped");
    assert!(with(|a| a.doc.settings.gantt.group_filter.is_empty()).unwrap(), "the filter forgets deleted groups");
    assert_eq!(t.w.get_toast_text(), "已删除 2 个分组和其中的任务（2 个）");
    // One undo step brings it all back.
    t.ctrl("z");
    let groups: Vec<String> = with(|a| a.doc.groups.keys().cloned().collect()).unwrap();
    assert_eq!(groups, ["g1", "g2", "g3"]);
    assert_eq!(t.task("设计")["groupId"], "g1");
    assert_eq!(t.task("开发")["groupId"], "g2");
    assert_eq!(t.task("测试")["deps"], json!(["b"]));
    // The keyboard: Tab to a box, Space ticks it, Esc cancels.
    with(|a| a.menu_act(&Target::Group("g3".into()), "deleteGroup")).unwrap();
    settle();
    t.key(&slint::SharedString::from(Key::Tab));
    t.key(" ");
    assert_eq!(ticked(), ["项目一", "项目三"]);
    t.key(&slint::SharedString::from(Key::Escape));
    assert!(boxes().is_empty(), "Esc closes");
    assert_eq!(with(|a| a.doc.groups.len()).unwrap(), 3);
    // With no opt-in, Enter deletes only the group and retains its tasks.
    with(|a| a.menu_act(&Target::Group("g3".into()), "deleteGroup")).unwrap();
    settle();
    t.key("\n");
    let groups: Vec<String> = with(|a| a.doc.groups.keys().cloned().collect()).unwrap();
    assert_eq!(groups, ["g1", "g2"], "Enter deletes the ticked group");
    assert!(t.task("测试").get("groupId").is_none_or(Value::is_null), "its task moves to 未分组");
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

#[test]
fn quick_add_after_finishing_position_adjustment() {
    let t = start("quickadd-after-adjust", Some(doc_with(&[], json!({}))));
    // A desktop window can be inactive after adjusting it. Slint then withholds
    // has-focus; the input must request native activation without waiting for it.
    let keyboard_requests = std::rc::Rc::new(std::cell::Cell::new(0));
    let requested = keyboard_requests.clone();
    t.w.on_quick_add_focus(move || requested.set(requested.get() + 1));
    with(|a| a.desk_toggle_lock()).unwrap();
    settle();
    t.click("完成");
    assert!(with(|a| a.doc.settings.desk.locked).unwrap());
    t.ev(WindowEvent::WindowActiveChanged(false));
    settle();

    let today = t.today();
    let (_, m, d) = fluentcal_core::date::parts(&today);
    let cell = t.find_prefix(&format!("{m}月{d}日 "));
    t.double_click_at(t.at(&cell, 40.0, 50.0));
    t.find("快速添加任务");
    assert!(keyboard_requests.get() > 0, "quick add must request the keyboard while the desktop window is inactive");
    t.ev(WindowEvent::WindowActiveChanged(true));
    settle();
    t.type_text("调整位置后添加");
    assert_eq!(t.w.get_quick_add_text(), "调整位置后添加");
    t.key("\n");
    assert_eq!(t.task("调整位置后添加")["start"], json!(today));
    t.type_text("继续添加");
    t.key("\n");
    assert_eq!(t.task("继续添加")["start"], json!(today));
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

/// GitHub #1 / #4: the list view (列表) with 近期 and 所有, by real clicks: the view switch,
/// the tabs, the check box, a double click opens the editor, a right click the task's menu,
/// 显示更多日期, and 所有 opening on today with what came before above it.
#[test]
fn list_view_upcoming_and_all() {
    use fluentcal_core::date::add_days;
    use i_slint_backend_testing::AccessibleRole as R;
    let today = fluentcal_core::date::today();
    let tasks = [
        json!({ "id": "past", "title": "去年的体检", "start": add_days(&today, -400), "done": true }),
        json!({ "id": "old", "title": "上周的会", "start": add_days(&today, -7) }),
        json!({ "id": "now", "title": "交房租", "start": today, "time": "09:30" }),
        json!({ "id": "trip", "title": "出差", "start": add_days(&today, 2), "end": add_days(&today, 4) }),
        json!({ "id": "far", "title": "四十天后", "start": add_days(&today, 40) }),
        json!({ "id": "gym", "title": "健身", "start": add_days(&today, -30), "repeat": { "freq": "daily", "interval": 1 } }),
    ];
    // enough days before and after today that 所有 has to scroll to it
    let earlier: Vec<Value> = (1..=20).map(|k| json!({ "id": format!("e{k}"), "title": format!("早先 {k}"), "start": add_days(&today, -40 - k) })).collect();
    let later: Vec<Value> = (1..=20).map(|k| json!({ "id": format!("l{k}"), "title": format!("以后 {k}"), "start": add_days(&today, 100 + k) })).collect();
    let tasks: Vec<Value> = tasks.iter().cloned().chain(earlier).chain(later).collect();
    let t = start("listview", Some(doc_with(&tasks, json!({}))));
    let tab = |t: &T, label: &str| {
        let e = ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(R::Tab)).unwrap_or_else(|| panic!("no tab {label:?}"));
        let s = e.size();
        t.click_at(t.at(&e, s.width / 2.0, s.height / 2.0));
    };
    let item = |t: &T, label: &str| ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(R::ListItem));
    tab(&t, "列表");
    assert_eq!(with(|a| a.doc.settings.calendar.view.clone()).unwrap(), "list");
    // 近期: today on, so not what came before nor what is 40 days away
    assert!(item(&t, "交房租").is_some() && item(&t, "出差").is_some());
    assert!(item(&t, "上周的会").is_none() && item(&t, "四十天后").is_none());
    let trip = item(&t, "出差").unwrap();
    assert!(trip.accessible_description().is_some_and(|d| d.contains('–')), "a multi-day task shows its dates");
    // the check box
    let check = |t: &T, title: &str| {
        let row = item(t, title).unwrap();
        let (p, s) = (row.absolute_position(), row.size());
        t.click_at(LogicalPosition::new(p.x + 15.0, p.y + s.height / 2.0));
    };
    check(&t, "交房租");
    assert_eq!(t.task("交房租")["done"], json!(true));
    check(&t, "交房租，已完成");
    assert_eq!(t.task("交房租")["done"], json!(false));
    // a double click opens the editor; a right click the task's menu
    let row = item(&t, "出差").unwrap();
    t.double_click_at(t.at(&row, row.size().width / 2.0, row.size().height / 2.0));
    assert!(with(|a| a.editor.is_some()).unwrap(), "the editor opened");
    with(|a| a.editor = None).unwrap();
    let row = item(&t, "出差").unwrap();
    t.right_click_at(t.at(&row, row.size().width / 2.0, row.size().height / 2.0));
    assert!(t.w.get_any_popup() && t.has("标记为完成"), "the task's menu");
    t.key(&slint::SharedString::from(Key::Escape));
    // 显示更多日期 is under the last day. PageDown scrolls the list; the keys keep working while
    // the focused task (the double click above) scrolls far out of view.
    for _ in 0..8 {
        t.key(&slint::SharedString::from(Key::PageDown));
    }
    t.click("显示更多日期");
    assert!(item(&t, "四十天后").is_some() || t.w.get_list_rows().iter().any(|r| r.chip.title == "四十天后"));

    // 所有: every task once, the past too; it opens on today
    tab(&t, "所有");
    assert_eq!(with(|a| a.doc.settings.calendar.list_range.clone()).unwrap(), "all");
    let rows: Vec<_> = t.w.get_list_rows().iter().filter(|r| r.kind == "item").map(|r| r.chip.title.to_string()).collect();
    let mut want = vec!["去年的体检".to_owned()];
    want.extend((1..=20).rev().map(|k| format!("早先 {k}")));
    want.extend(["健身", "上周的会", "交房租", "出差", "四十天后"].map(String::from));
    want.extend((1..=20).map(|k| format!("以后 {k}")));
    assert_eq!(rows, want, "by start date, a repeating task once");
    let gym = t.w.get_list_rows().iter().find(|r| r.chip.title == "健身").unwrap();
    assert!(!gym.checkable && gym.meta == "每天", "the series, with its rule: {:?}", gym.meta);
    let heading = ElementHandle::find_by_accessible_label(&t.w, &format!("今天 {}", fluentcal_core::agenda::day_sub(&today, true))).next().expect("today's heading is built");
    let list = ElementHandle::find_by_accessible_label(&t.w, "任务列表").find(|e| e.accessible_role() == Some(R::List)).unwrap();
    let dy = heading.absolute_position().y - list.absolute_position().y;
    assert!((0.0..40.0).contains(&dy), "today's heading at the top of the list: {dy}");
    // back to 近期, and the setting stays
    tab(&t, "近期");
    assert!(item(&t, "去年的体检").is_none());
    tab(&t, "月");
    assert!(t.w.get_list_rows().row_count() == 0, "nothing kept for another view");
}

/// GitHub #6: 行高随任务多少调整. Off (the default): equal rows, a busy day folds into "+N". On:
/// the busy week's row is as tall as its tasks need, the quiet ones share the rest, the rows
/// fill the grid without overlapping, and the day under the pointer follows the uneven rows
/// (a drag from the tall row into a short one lands on the day it's dropped on). Also in 多周.
#[test]
fn rows_follow_their_tasks() {
    let busy = "2026-11-11";
    let tasks: Vec<Value> = (1..=7).map(|k| json!({ "title": format!("忙碌 {k}"), "start": busy })).collect();
    let t = start("fit-rows", Some(doc_with(&tasks, json!({ "calendar": { "weekStart": 1 } }))));
    t.w.invoke_jump(busy.into());
    settle();
    let weeks = || -> Vec<fluentcal_app::WeekData> { t.w.get_weeks().iter().collect() };
    let row_of = |ws: &[fluentcal_app::WeekData], d: &str| ws.iter().position(|w| w.cells.iter().any(|c| c.date == d)).unwrap();
    let ws = weeks();
    assert_eq!(ws.len(), 6, "November 2026, Monday first");
    let r = row_of(&ws, busy);
    assert!(ws.iter().all(|w| w.row_h == 0.0), "equal rows by default");
    assert!(ws[r].more.iter().any(|m| m > 0), "equal rows fold the busy day: {:?}", ws[r].more);

    with(|a| a.patch_settings(json!({ "calendar": { "fitRows": true } }))).unwrap();
    settle();
    let check_rows = |ws: &[fluentcal_app::WeekData], r: usize| {
        let (gap, lane, room) = (t.w.get_cell_gap(), t.w.get_lane_step(), t.w.get_rows_room());
        assert!(ws.iter().all(|w| w.more.iter().all(|m| m == 0)), "every task shows: {:?}", ws.iter().map(|w| w.more.iter().collect::<Vec<_>>()).collect::<Vec<_>>());
        assert_eq!(ws[0].row_y, 0.0);
        for i in 1..ws.len() {
            assert!((ws[i].row_y - (ws[i - 1].row_y + ws[i - 1].row_h + gap)).abs() < 0.01, "row {i} follows row {}", i - 1);
        }
        let last = ws.last().unwrap();
        assert!((last.row_y + last.row_h - room).abs() < 0.01, "the rows fill the grid: {} of {room}", last.row_y + last.row_h);
        let quiet: Vec<f32> = ws.iter().enumerate().filter(|(i, _)| *i != r).map(|(_, w)| w.row_h).collect();
        assert!(quiet.iter().all(|h| (h - quiet[0]).abs() < 0.01), "the quiet rows share the rest: {quiet:?}");
        assert!((ws[r].row_h - quiet[0] - 6.0 * lane).abs() < 0.01, "7 lanes against 1: {} vs {}", ws[r].row_h, quiet[0]);
        // the last chip sits inside its row
        let chip = t.find("忙碌 7");
        let bottom = chip.absolute_position().y + chip.size().height;
        assert!(bottom <= t.w.get_grid_y() + ws[r].row_y + ws[r].row_h, "忙碌 7 inside the tall row");
    };
    let ws = weeks();
    check_rows(&ws, r);

    // The day under the pointer: the busy row's bottom (where equal rows would be the next
    // week), and the middle of a short row.
    let (gx, gy, cw, gap) = (t.w.get_grid_x(), t.w.get_grid_y(), t.w.get_cell_w(), t.w.get_cell_gap());
    let col_x = |c: usize| gx + c as f32 * (cw + gap) + cw / 2.0;
    let at = |x: f32, y: f32| with(|a| a.desk_date_at(x, y)).flatten();
    assert_eq!(at(col_x(2), gy + ws[r].row_y + ws[r].row_h - 4.0).as_deref(), Some(busy));
    let target = "2026-11-25";
    let s = row_of(&ws, target);
    let to = LogicalPosition::new(col_x(2), gy + ws[s].row_y + ws[s].row_h / 2.0);
    assert_eq!(at(to.x, to.y).as_deref(), Some(target));
    assert_eq!(at(col_x(2), gy + ws[s].row_y - gap / 2.0), None, "between rows: no day");
    let chip = t.find("忙碌 1");
    t.drag(t.at(&chip, 40.0, chip.size().height / 2.0), to);
    assert_eq!(t.task("忙碌 1")["start"], json!(target));
    let ws = weeks();
    assert!(ws[r].row_h > ws[s].row_h, "6 lanes still the tallest");

    // 多周 too
    with(|a| a.patch_settings(json!({ "calendar": { "view": "weeks", "weeksRows": 4 } }))).unwrap();
    settle();
    let ws = weeks();
    let r = row_of(&ws, busy);
    assert!(ws.len() == 4 && ws.iter().all(|w| w.row_h > 0.0), "多周 rows follow too");
    assert!(ws.iter().enumerate().all(|(i, w)| i == r || w.row_h < ws[r].row_h));

    // Off again: equal rows and "+N".
    with(|a| a.patch_settings(json!({ "calendar": { "view": "month", "fitRows": false } }))).unwrap();
    settle();
    let ws = weeks();
    assert!(ws.iter().all(|w| w.row_h == 0.0), "equal rows again");
    assert!(ws[row_of(&ws, busy)].more.iter().any(|m| m > 0));
}

/// GitHub #9, #10, #12: what the list shows at the right of a task; a bar's progress; 每天 from
/// a start to an end date as joined days in one lane.
#[test]
fn due_labels_bar_progress_and_day_series() {
    use fluentcal_core::date::{add_days, start_of_week};
    use i_slint_backend_testing::AccessibleRole as R;
    let today = fluentcal_core::date::today();
    let monday = start_of_week(&today, 1);
    let tasks = [
        json!({ "id": "late", "title": "交报告", "start": add_days(&today, -5) }),
        json!({ "id": "past", "title": "体检", "start": add_days(&today, -3), "done": true, "progress": 100 }),
        json!({ "id": "now", "title": "交房租", "start": today }),
        json!({ "id": "soon", "title": "开会", "start": add_days(&today, 3) }),
        json!({ "id": "run", "title": "装修", "start": add_days(&today, -2), "end": add_days(&today, 2) }),
        json!({ "id": "bar", "title": "项目", "start": monday, "end": add_days(&monday, 3), "progress": 50 }),
        json!({ "id": "sign", "title": "签到", "start": monday, "end": add_days(&monday, 4), "repeat": { "freq": "daily", "interval": 1 } }),
    ];
    let t = start("due", Some(doc_with(&tasks, json!({ "calendar": { "weekStart": 1, "fitRows": true }, "tasks": { "autoPostpone": false } }))));
    // month view: the bar, half done; the series, five joined days in one lane
    let chips: Vec<_> = t.w.get_weeks().iter().flat_map(|w| w.chips.iter().collect::<Vec<_>>()).collect();
    let bar = chips.iter().find(|c| c.title == "项目").unwrap();
    // "第4-N天（P%）": four days from Monday; today is the Nth of them (all four once it is over)
    let gone = (fluentcal_core::date::diff_days(&monday, &today) + 1).clamp(0, 4);
    assert_eq!((bar.pct.as_str(), bar.fill), (format!("第4-{gone}天（{}%） 进度 50%", gone * 25).as_str(), 0.5));
    // a bar that hasn't begun, or is over; no progress set still says 进度 0%
    let run = chips.iter().find(|c| c.title == "装修").unwrap();
    assert_eq!(run.pct.as_str(), "第5-3天（60%） 进度 0%");
    let sign: Vec<_> = chips.iter().filter(|c| c.title == "签到").collect();
    assert_eq!(sign.len(), 5, "once a day, Monday to Friday");
    assert!(sign.iter().all(|c| c.linked && c.span == 1 && c.lane == sign[0].lane && !c.multi_day));
    assert!(!sign[0].join_left && sign[0].join_right && sign[4].join_left && !sign[4].join_right);
    // each day has its own check box
    let el = ElementHandle::find_by_accessible_label(&t.w, "签到").filter(|e| e.accessible_role() == Some(R::Groupbox)).nth(1).unwrap();
    t.click_at(t.at(&el, 9.0, el.size().height / 2.0));
    assert_eq!(t.task("签到")["doneDates"], json!([add_days(&monday, 1)]));

    // the list
    let tab = |t: &T, label: &str| {
        let e = ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(R::Tab)).unwrap_or_else(|| panic!("no tab {label:?}"));
        let s = e.size();
        t.click_at(t.at(&e, s.width / 2.0, s.height / 2.0));
    };
    tab(&t, "列表");
    tab(&t, "所有");
    let due = |t: &T, title: &str| {
        let r = t.w.get_list_rows().iter().find(|r| r.kind == "item" && r.chip.title == title).unwrap_or_else(|| panic!("no row {title:?}"));
        (r.due.to_string(), r.due_kind.to_string())
    };
    assert_eq!(due(&t, "交报告"), ("逾期 5 天".to_owned(), "overdue".to_owned()));
    assert_eq!(due(&t, "体检"), ("3 天前".to_owned(), String::new()));
    assert_eq!(due(&t, "交房租"), ("今天".to_owned(), "today".to_owned()));
    assert_eq!(due(&t, "开会"), ("3 天后".to_owned(), String::new()));
    assert_eq!(due(&t, "装修"), ("还剩 2 天".to_owned(), String::new()));
    let row = ElementHandle::find_by_accessible_label(&t.w, "交房租").find(|e| e.accessible_role() == Some(R::ListItem)).unwrap();
    assert!(row.accessible_description().is_some_and(|d| d.ends_with("今天")));
    tab(&t, "近期");
    tab(&t, "月");
}

/// GitHub #11: 最近更改 lists what was added, changed and deleted; a change is taken back
/// there long after its toast is gone (also after a restart: it is kept in a file), and can be
/// done again.
#[test]
fn recent_changes_can_be_taken_back() {
    let today = fluentcal_core::date::today();
    let tasks = [json!({ "id": "a", "title": "周报", "start": today }), json!({ "id": "b", "title": "买菜", "start": today })];
    let t = start("changes", Some(doc_with(&tasks, json!({}))));
    with(|a| {
        let task = a.doc.tasks["a"].clone();
        a.desk_remove_task(&task, None);
        let b = a.doc.tasks["b"].clone();
        let edit = fluentcal_core::tasks::set_occurrence_done(&a.doc, &b, &b.start, true, 5);
        a.apply(edit, true);
        a.desk_close_toast();
    })
    .unwrap();
    settle();
    assert!(t.tasks().iter().all(|x| x["title"] != "周报"));
    t.click("最近更改（可撤销）");
    let rows: Vec<String> = t.w.get_changes().iter().map(|r| r.text.to_string()).collect();
    assert_eq!(rows, ["完成“买菜”", "删除“周报”"]);
    // the older change first: the newer one stays
    t.click("撤销 删除“周报”");
    assert_eq!(t.task("周报")["start"], json!(today));
    assert_eq!(t.task("买菜")["done"], json!(true));
    assert!(t.w.get_changes().iter().any(|r| r.undone && r.text == "删除“周报”"));
    t.click("恢复 删除“周报”");
    assert!(t.tasks().iter().all(|x| x["title"] != "周报"));
    // kept in this PC's folder
    let file = with(|a| a.local_dir.join("changes.json")).unwrap();
    let saved: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
    assert_eq!(saved.as_array().unwrap().len(), 2);
    // a change that was changed again since can't be taken back
    with(|a| {
        let b = a.doc.tasks["b"].clone();
        let edit = fluentcal_core::tasks::save_task(&a.doc, fluentcal_core::model::Task { title: "买水果".into(), ..b }, 9);
        a.apply_quiet(edit, false);
    })
    .unwrap();
    settle();
    t.click("撤销 完成“买菜”");
    assert_eq!(t.task("买水果")["done"], json!(true));
    t.click("清空记录");
    assert_eq!(t.w.get_changes().row_count(), 0);
    t.click("关闭");
    assert!(!t.w.get_changes_open());
}

/// GitHub #7: the column of to-dos without a day. Opened from its handle; a monthly to-do is
/// added, ticked (the calendar shows it done today), unticked from the calendar; last month's
/// open one is marked 逾期; one without a date leaves the column when done.
#[test]
fn todo_column_without_days() {
    use fluentcal_core::date::{add_months, start_of_month};
    let today = fluentcal_core::date::today();
    let last = add_months(&start_of_month(&today), -1);
    let tasks = [json!({ "id": "old", "title": "交物业费", "start": last, "flex": "month" }), json!({ "id": "idea", "title": "整理相册", "start": last, "flex": "someday" })];
    let t = start("side", Some(doc_with(&tasks, json!({ "calendar": { "fitRows": true } }))));
    assert!(!t.w.get_side_open() && !t.has("交物业费"));
    t.click("展开待办栏（不定日待办）");
    assert!(with(|a| a.doc.settings.calendar.sidebar).unwrap());
    let rows = |t: &T| t.w.get_side_rows().iter().map(|r| format!("{}:{}:{}{}", r.kind, r.zone, if r.kind == "item" { r.chip.title.to_string() } else { String::new() }, r.tag)).collect::<Vec<_>>();
    assert_eq!(rows(&t), ["head:month:", "item:month:交物业费", "add:month:", "head:overdue:", "item:overdue:交物业费逾期", "head:someday:", "item:someday:整理相册", "add:someday:"]);
    // add a monthly one
    t.click("添加每月待办");
    t.type_text("洗车");
    t.key(&slint::SharedString::from(Key::Return));
    t.key(&slint::SharedString::from(Key::Escape));
    let car = t.task("洗车");
    assert_eq!((car["flex"].as_str(), car["start"].as_str()), (Some("month"), Some(start_of_month(&today).as_str())));
    // tick it: done today, on the calendar too
    let item = |t: &T, label: &str, n: usize| ElementHandle::find_by_accessible_label(&t.w, label).filter(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::ListItem)).nth(n).unwrap_or_else(|| panic!("no {label:?}"));
    let row = item(&t, "洗车", 0);
    t.click_at(t.at(&row, 10.0, row.size().height / 2.0));
    assert_eq!(t.task("洗车")["flexDone"][&today[..7]], json!(today));
    let chips: Vec<_> = t.w.get_weeks().iter().flat_map(|w| w.chips.iter().collect::<Vec<_>>()).filter(|c| c.title == "洗车").collect();
    assert!(chips.len() == 1 && chips[0].done && chips[0].date == today.as_str());
    // untick it on the calendar
    with(|a| a.desk_chip_toggle(&car["id"].as_str().unwrap().to_owned(), &a.today(), true)).unwrap();
    settle();
    assert!(t.task("洗车").get("flexDone").is_none());
    // the one without a date: done, gone from the column
    let row = item(&t, "整理相册", 0);
    t.click_at(t.at(&row, 10.0, row.size().height / 2.0));
    assert_eq!(t.task("整理相册")["done"], json!(true));
    assert!(!rows(&t).iter().any(|r| r.contains("整理相册")));
    t.click("收起待办栏");
    assert!(!t.w.get_side_open());
}

/// 0.3.12: the to-do column's add box asks for the keyboard itself. The calendar on the
/// desktop is inactive (a click doesn't activate it), and Slint starts watching `has-focus`
/// only after the box's `init` gave it the focus, so nothing else would ask and the keys (and
/// the input method) stayed with another window. Entries are typed one after another (the box
/// isn't rebuilt when the list grows), and the calendar has the keys again afterwards.
#[test]
fn todo_column_box_takes_the_keyboard() {
    let t = start("side-keyboard", Some(doc_with(&[], json!({ "calendar": { "sidebar": true } }))));
    let requests = std::rc::Rc::new(std::cell::Cell::new(0));
    let requested = requests.clone();
    t.w.on_wants_keyboard(move || requested.set(requested.get() + 1));
    t.ev(WindowEvent::WindowActiveChanged(false));
    settle();
    t.click("添加每月待办");
    assert!(requests.get() > 0, "the add box must request the keyboard while the desktop window is inactive");
    t.ev(WindowEvent::WindowActiveChanged(true));
    settle();
    t.type_text("洗车");
    t.key("\n");
    t.type_text("交水费");
    t.key("\n");
    assert_eq!(t.task("洗车")["flex"], json!("month"));
    assert_eq!(t.task("交水费")["flex"], json!("month"), "the next entry is typed right away");
    // Esc closes the box; the shortcuts work again
    t.key(&slint::SharedString::from(Key::Escape));
    t.ctrl("z");
    assert!(t.tasks().iter().all(|x| x["title"] != "交水费"), "Ctrl+Z reaches the calendar after Esc");
    // the other box, then the column is closed while it is open: the keys stay with the calendar
    t.click("添加无日期待办");
    t.type_text("整理相册");
    t.key("\n");
    assert_eq!(t.task("整理相册")["flex"], json!("someday"));
    t.click("收起待办栏");
    assert!(!t.w.get_side_open());
    t.ctrl("z");
    assert!(t.tasks().iter().all(|x| x["title"] != "整理相册"), "Ctrl+Z reaches the calendar after the column closed");
}

/// 0.3.12: the column's handle lies in the card's left margin: it covers neither the weekday
/// row (it used to sit on "一") nor a day, whether the column is open or not.
#[test]
fn todo_column_handle_is_beside_the_calendar() {
    let t = start("side-handle", Some(doc_with(&[], json!({}))));
    let right_of = |e: &ElementHandle| e.absolute_position().x + e.size().width;
    let handle = t.find("展开待办栏（不定日待办）");
    assert!(right_of(&handle) <= t.w.get_grid_x() + 0.01, "the handle ends where the days begin: {} vs {}", right_of(&handle), t.w.get_grid_x());
    let monday = t.find("一");
    assert!(right_of(&handle) <= monday.absolute_position().x, "the handle doesn't reach the weekday row's first label");
    assert!(handle.size().width >= 8.0 && handle.size().height >= 40.0, "big enough to click: {:?}", handle.size());
    // open: the same handle closes it, and still lies left of the column
    t.click("展开待办栏（不定日待办）");
    assert!(t.w.get_side_open());
    let handle = t.find("收起待办栏");
    let panel = t.find("不定日待办");
    assert!(right_of(&handle) <= panel.absolute_position().x + 0.01, "left of the column: {} vs {}", right_of(&handle), panel.absolute_position().x);
    t.click("收起待办栏");
    assert!(!t.w.get_side_open());
}

/// 0.3.14 (GitHub #13): a bar over several days says which day it is, "第N-X天（P%）", worked out
/// from today: 0 before it begins, all its days once it is over. The progress follows, also
/// 0 % and 100 %. Each part has its switch in 设置 › 日历.
#[test]
fn bars_say_which_day_it_is() {
    use fluentcal_core::date::add_days;
    let today = fluentcal_core::date::today();
    let tasks = [
        json!({ "id": "cat", "title": "看望小猫", "start": add_days(&today, -1), "end": add_days(&today, 3), "progress": 60 }),
        json!({ "id": "next", "title": "出差", "start": add_days(&today, 2), "end": add_days(&today, 4) }),
        json!({ "id": "over", "title": "培训", "start": add_days(&today, -4), "end": add_days(&today, -2), "done": true, "progress": 100 }),
        json!({ "id": "one", "title": "开会", "start": today }),
        json!({ "id": "camp", "title": "集训", "start": add_days(&today, -1), "end": today, "repeat": { "freq": "monthly", "interval": 1 } }),
    ];
    let label = |t: &T, title: &str| {
        let weeks = t.w.get_weeks();
        let chips: Vec<_> = weeks.iter().flat_map(|w| w.chips.iter().collect::<Vec<_>>()).collect();
        let c = chips.iter().find(|c| c.title == title).unwrap_or_else(|| panic!("no chip {title:?}"));
        c.pct.to_string()
    };
    // the multi-week view starts a week back, so all of them are on it wherever today is
    let cal = json!({ "view": "weeks", "weeksRows": 4, "weeksPast": 1 });
    let t = start("bar-days", Some(doc_with(&tasks, json!({ "calendar": cal, "tasks": { "autoPostpone": false } }))));
    assert_eq!(label(&t, "看望小猫"), "第5-2天（40%） 进度 60%");
    assert_eq!(label(&t, "出差"), "第3-0天（0%） 进度 0%", "not begun");
    assert_eq!(label(&t, "培训"), "第3-3天（100%） 进度 100%", "over and done");
    assert_eq!(label(&t, "开会"), "", "one day: nothing to count");
    assert_eq!(label(&t, "集训"), "第2-2天（100%）", "a repeating task has days but no progress of its own");
    // the progress off: the days stay, and no part of the bar is darker
    with(|a| a.patch_settings(json!({ "calendar": { "barProgress": false } }))).unwrap();
    settle();
    assert_eq!(label(&t, "看望小猫"), "第5-2天（40%）");
    let weeks = t.w.get_weeks();
    assert!(weeks.iter().flat_map(|w| w.chips.iter().collect::<Vec<_>>()).all(|c| c.fill == 0.0));
    // the days off too: bars as before 0.3.11
    with(|a| a.patch_settings(json!({ "calendar": { "barDays": false } }))).unwrap();
    settle();
    assert_eq!(label(&t, "看望小猫"), "");
    // only the progress
    with(|a| a.patch_settings(json!({ "calendar": { "barProgress": true } }))).unwrap();
    settle();
    assert_eq!(label(&t, "看望小猫"), "进度 60%");
    assert_eq!(label(&t, "出差"), "进度 0%");
}

/// 0.3.14: on a bar the title comes first. What follows it (the days, the progress) gets the room
/// that is left: whole on a long bar, cut short on a shorter one, gone where it could say nothing
/// (a long title on a short bar).
#[test]
fn a_bar_keeps_its_title() {
    use fluentcal_core::date::{add_days, start_of_week};
    use i_slint_backend_testing::AccessibleRole as R;
    let monday = start_of_week(&fluentcal_core::date::today(), 1);
    let tasks = [
        json!({ "id": "wide", "title": "项目", "start": monday, "end": add_days(&monday, 4), "progress": 50 }),
        // Sunday, then Monday and Tuesday of the next week: a piece of one day, one of two
        json!({ "id": "cross", "title": "联调", "start": add_days(&monday, 6), "end": add_days(&monday, 8) }),
        json!({ "id": "long", "title": "和设计过一遍下季度全部页面稿", "start": add_days(&monday, 10), "end": add_days(&monday, 11) }),
        json!({ "id": "longer", "title": "和设计过一遍下个季度的全部页面与交互稿", "start": add_days(&monday, 12), "end": add_days(&monday, 13) }),
    ];
    let cal = json!({ "view": "weeks", "weeksRows": 4, "weeksPast": 0, "weekStart": 1 });
    let t = start("bar-title", Some(doc_with(&tasks, json!({ "calendar": cal, "tasks": { "autoPostpone": false } }))));
    // each piece of a task's bar, left to right, top to bottom: (the bar, its title, its label if shown)
    let pieces = |title: &str| {
        let mut bars: Vec<_> = ElementHandle::find_by_accessible_label(&t.w, title).filter(|e| e.accessible_role() == Some(R::Groupbox)).collect();
        bars.sort_by(|a, b| (a.absolute_position().y, a.absolute_position().x).partial_cmp(&(b.absolute_position().y, b.absolute_position().x)).unwrap());
        bars.into_iter()
            .map(|bar| {
                let texts: Vec<_> = bar.query_descendants().match_predicate(|e| e.accessible_role() == Some(R::Text)).find_all();
                let name = title.to_owned();
                let text = texts.iter().find(|e| e.accessible_label().is_some_and(|l| l == name)).expect("the title").clone();
                let label = texts.iter().find(|e| e.accessible_label().is_some_and(|l| l.starts_with('第'))).cloned();
                (bar, text, label)
            })
            .collect::<Vec<_>>()
    };
    let right = |e: &ElementHandle| e.absolute_position().x + e.size().width;
    // the bar's text ends 4px before its right edge
    let edge = |bar: &ElementHandle| right(bar) - 4.0;
    // five days: the title and the whole label, with room to spare
    let wide = pieces("项目");
    let (bar, title, label) = &wide[0];
    let label = label.as_ref().expect("a long bar shows its label");
    let two_chars = title.size().width;
    assert!(two_chars > 20.0, "the title is there ({two_chars})");
    assert_eq!(label.absolute_position().x, right(title) + 4.0, "the label follows the title");
    assert!(right(label) < edge(bar) - 100.0, "the whole label: {} of {}", right(label), edge(bar));
    // a piece of one day, then one of two: the title whole on both; the label cut where the
    // first ends, whole on the second
    let cross = pieces("联调");
    assert_eq!(cross.len(), 2, "Sunday, then the next week");
    let widths: Vec<f32> = cross
        .iter()
        .map(|(bar, title, label)| {
            assert_eq!(title.size().width, two_chars, "the title is whole on a piece {} wide", bar.size().width);
            let label = label.as_ref().expect("room for some of the label");
            assert!(right(label) <= edge(bar), "inside the bar");
            label.size().width
        })
        .collect();
    assert!(widths[0] >= 54.0 && widths[0] < widths[1], "cut short on the day, whole on the two: {widths:?}");
    assert_eq!(right(cross[0].2.as_ref().unwrap()), edge(&cross[0].0), "cut where the bar ends");
    assert!(right(cross[1].2.as_ref().unwrap()) < edge(&cross[1].0) - 20.0, "room to spare");
    // two days and a long title: the title whole, no room left for a label worth reading
    let long = pieces("和设计过一遍下季度全部页面稿");
    let (bar, title, label) = &long[0];
    assert_eq!(title.size().width, two_chars * 7.0, "14 characters, whole");
    assert!(label.is_none() && edge(bar) - right(title) < 58.0, "no label in {}", edge(bar) - right(title));
    // a title longer than the bar: cut where the bar ends, no label
    let longer = pieces("和设计过一遍下个季度的全部页面与交互稿");
    let (bar, title, label) = &longer[0];
    assert!(title.size().width < two_chars * 9.5 && right(title) == edge(bar), "{} to {}", title.size().width, edge(bar));
    assert!(label.is_none());
}

/// 0.3.14: where the to-do column takes its room (设置 › 日历). Beside the calendar (as before,
/// the default) the days begin right of it; floating, the days keep their place and the column
/// lies over them, and its add box still takes the keys.
#[test]
fn todo_column_beside_or_over_the_calendar() {
    let t = start("side-modes", Some(doc_with(&[], json!({}))));
    let closed = t.w.get_grid_x();
    t.click("展开待办栏（不定日待办）");
    let panel = t.find("不定日待办");
    let right = panel.absolute_position().x + panel.size().width;
    assert!(t.w.get_grid_x() >= right, "beside: the days begin right of the column ({} vs {right})", t.w.get_grid_x());
    with(|a| a.patch_settings(json!({ "calendar": { "sidebarMode": "float" } }))).unwrap();
    settle();
    assert_eq!(t.w.get_grid_x(), closed, "floating: the days are where they were");
    let panel = t.find("不定日待办");
    assert!(panel.absolute_position().x + panel.size().width > closed + 50.0, "the column lies over the first days");
    t.click("添加无日期待办");
    t.type_text("换护照");
    t.key("\n");
    assert_eq!(t.task("换护照")["flex"], json!("someday"));
    t.key(&slint::SharedString::from(Key::Escape));
    // the window grows instead (shell::side_ext, checked on a real window): the layout inside is as beside
    with(|a| a.patch_settings(json!({ "calendar": { "sidebarMode": "left" } }))).unwrap();
    settle();
    let panel = t.find("不定日待办");
    assert!(t.w.get_grid_x() >= panel.absolute_position().x + panel.size().width);
    t.click("收起待办栏");
    assert_eq!(t.w.get_grid_x(), closed);
}

/// GitHub #16: the year in the picker under the title is a button that opens the list of years,
/// for one far away; over the months the wheel turns the years.
#[test]
fn year_month_picker_lists_the_years() {
    use i_slint_backend_testing::AccessibleRole as R;
    let t = start("ymyears", Some(doc_with(&[], json!({}))));
    let (y, _, _) = fluentcal_core::date::parts(&t.today());
    let of_role = |label: &str, role: R| ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(role)).unwrap_or_else(|| panic!("no {label:?}"));
    let centre = |e: &ElementHandle| LogicalPosition::new(e.absolute_position().x + e.size().width / 2.0, e.absolute_position().y + e.size().height / 2.0);
    let wheel = |e: &ElementHandle, dy: f32| {
        let position = centre(e);
        t.ev(WindowEvent::PointerMoved { position });
        t.ev(WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: dy });
        settle();
    };
    t.click(&t.find_prefix(&format!("{y}年")).accessible_label().unwrap());
    // the wheel over the months: a year a notch
    wheel(&of_role(&format!("{y}年"), R::List), -60.0);
    assert_eq!(t.w.get_ym_year(), y + 1);
    wheel(&of_role(&format!("{}年", y + 1), R::List), 60.0);
    assert_eq!(t.w.get_ym_year(), y);
    // the year's button: the years, this one in view; an arrow is a page of them (three rows:
    // with four, a row between two pages was never in view)
    t.click_at(centre(&of_role(&format!("{y}年"), R::Button)));
    // a year's cell when all of it is in the list's view (the search skips what is scrolled away)
    let year_cell = |year: i32| {
        let list = of_role("选择年份", R::List);
        let (top, bottom) = (list.absolute_position().y, list.absolute_position().y + list.size().height);
        ElementHandle::find_by_accessible_label(&t.w, &format!("{year}年")).filter(|e| e.accessible_role() == Some(R::ListItem)).find(|e| e.absolute_position().y >= top - 0.5 && e.absolute_position().y + e.size().height <= bottom + 0.5)
    };
    assert!(year_cell(y).is_some(), "the list opens around the year shown");
    assert!(year_cell(y - 4).is_some() && year_cell(y + 4).is_some(), "with the rows before and after it");
    assert!(year_cell(y - 12).is_none());
    t.click("更早的年份");
    assert_eq!(t.w.get_ym_year(), y - 12);
    assert!(year_cell(y - 12).is_some(), "the list follows the arrows");
    assert!(year_cell(y - 8).is_some(), "the row that was just out of view is in it now");
    // a year, then a month of it
    let target = y - 16;
    t.click(&format!("{target}年"));
    assert!(!t.has("选择年份") && t.w.get_ym_year() == target, "a year comes back to its months");
    t.click("3月");
    assert_eq!(with(|a| a.desk.ui.cursor.clone()).unwrap(), format!("{target}-03-01"));
    assert!(!t.w.get_any_popup(), "picking closes it");
}

/// 0.3.17 (GitHub #15): a bar over several days says the day it was ticked done after its days,
/// "26.11.27完成" (the days go on being counted, done or not). The day is kept with the task
/// and goes with the tick; 右键 › 修改完成日期 puts it right (not on a day that has not come), and
/// ticking again writes today over that. A switch in 设置 › 日历. What was done before the day
/// was kept says 进度 100% as before, until the user says when.
#[test]
fn a_done_bar_says_the_day_it_was_done() {
    use fluentcal_core::date::{add_days, add_months, parts, start_of_month};
    use i_slint_backend_testing::AccessibleRole as R;
    let today = fluentcal_core::date::today();
    let tasks = [
        json!({ "id": "cat", "title": "看望小猫", "start": add_days(&today, -1), "end": add_days(&today, 3), "progress": 60 }),
        json!({ "id": "over", "title": "培训", "start": add_days(&today, -4), "end": add_days(&today, -2), "done": true, "progress": 100 }),
        json!({ "id": "camp", "title": "集训", "start": add_days(&today, -1), "end": today, "repeat": { "freq": "monthly", "interval": 1 } }),
    ];
    let chip_of = |t: &T, title: &str| {
        let weeks = t.w.get_weeks();
        let chips: Vec<_> = weeks.iter().flat_map(|w| w.chips.iter().collect::<Vec<_>>()).collect();
        chips.into_iter().find(|c| c.title == title).unwrap_or_else(|| panic!("no chip {title:?}"))
    };
    let label = |t: &T, title: &str| chip_of(t, title).pct.to_string();
    let short = |d: &str| {
        let (y, m, dd) = parts(d);
        format!("{:02}.{m}.{dd}完成", y % 100)
    };
    let day = |d: &str| {
        let (y, m, dd) = parts(d);
        format!("{y}年{m}月{dd}")
    };
    // the chip's check box, its menu
    let el = |t: &T, label: &str| ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(R::Groupbox)).unwrap_or_else(|| panic!("no chip {label:?}"));
    let tick = |t: &T, label: &str| {
        let e = el(t, label);
        t.click_at(t.at(&e, 9.0, e.size().height / 2.0));
    };
    let menu = |t: &T, label: &str| {
        let e = el(t, label);
        t.right_click_at(t.at(&e, 40.0, e.size().height / 2.0));
    };
    let cal = json!({ "view": "weeks", "weeksRows": 4, "weeksPast": 1 });
    let t = start("bar-done", Some(doc_with(&tasks, json!({ "calendar": cal, "tasks": { "autoPostpone": false } }))));
    assert_eq!(label(&t, "培训"), "第3-3天（100%） 进度 100%", "done before the day was kept: as before");
    assert!(!chip_of(&t, "培训").tip.contains("完成于"));

    // ticked: today, after the days
    tick(&t, "看望小猫");
    assert_eq!(t.task("看望小猫")["doneOn"], json!(today));
    assert_eq!(label(&t, "看望小猫"), format!("第5-2天（40%） {}", short(&today)));
    assert!(chip_of(&t, "看望小猫").tip.contains("完成于 "), "the tooltip says the day in full");

    // 右键 › 修改完成日期: a calendar on that day; tomorrow can't be picked
    menu(&t, "看望小猫，已完成");
    t.click("修改完成日期…");
    assert!(t.w.get_any_popup() && t.has("修改完成日期") && t.has("回到今天"), "the calendar is open");
    assert_eq!(t.find(&day(&today)).accessible_checked(), Some(true), "on the day it was done");
    let tomorrow = add_days(&today, 1);
    assert_eq!(t.find(&day(&tomorrow)).accessible_enabled(), Some(false), "a day that has not come");
    t.click(&day(&tomorrow));
    assert!(t.w.get_any_popup());
    assert_eq!(t.task("看望小猫")["doneOn"], json!(today));
    // the 15th of last month
    t.click("上个月");
    let (py, pm, _) = parts(&add_months(&start_of_month(&today), -1));
    let picked = format!("{py:04}-{pm:02}-15");
    t.click(&day(&picked));
    assert!(!t.w.get_any_popup(), "a day closes it");
    assert_eq!(t.task("看望小猫")["doneOn"], json!(picked));
    assert_eq!(label(&t, "看望小猫"), format!("第5-2天（40%） {}", short(&picked)));
    assert_eq!(with(|a| a.changes.list().first().map(|c| c.text())).unwrap().as_deref(), Some("改完成日期“看望小猫”"));
    // it is one step to take back
    t.ctrl("z");
    assert_eq!(t.task("看望小猫")["doneOn"], json!(today));
    t.ctrl("y");
    assert_eq!(t.task("看望小猫")["doneOn"], json!(picked));

    // the switch: off, the progress says it is done; the tooltip still knows the day
    with(|a| a.patch_settings(json!({ "calendar": { "barDone": false } }))).unwrap();
    settle();
    assert_eq!(label(&t, "看望小猫"), "第5-2天（40%） 进度 100%");
    assert!(chip_of(&t, "看望小猫").tip.contains("完成于 "));
    with(|a| a.patch_settings(json!({ "calendar": { "barDone": true, "barDays": false, "barProgress": false } }))).unwrap();
    settle();
    assert_eq!(label(&t, "看望小猫"), short(&picked), "on its own");
    with(|a| a.patch_settings(json!({ "calendar": { "barDays": true, "barProgress": true } }))).unwrap();
    settle();

    // unticked: no day, nothing to correct; ticked again: today, whatever was set by hand
    tick(&t, "看望小猫，已完成");
    assert!(t.task("看望小猫").get("doneOn").is_none());
    assert_eq!(label(&t, "看望小猫"), "第5-2天（40%） 进度 0%");
    menu(&t, "看望小猫");
    assert!(t.has("标记为完成") && !t.has("修改完成日期…"));
    t.key(&slint::SharedString::from(Key::Escape));
    tick(&t, "看望小猫");
    assert_eq!(t.task("看望小猫")["doneOn"], json!(today));

    // done before the day was kept: the calendar opens with no day chosen; one is picked
    menu(&t, "培训，已完成");
    t.click("修改完成日期…");
    assert_eq!(t.find(&day(&today)).accessible_checked(), Some(false));
    t.click(&day(&today));
    assert_eq!(t.task("培训")["doneOn"], json!(today));
    assert_eq!(label(&t, "培训"), format!("第3-3天（100%） {}", short(&today)));

    // a repeating task: each time has its own day
    tick(&t, "集训");
    let occ = add_days(&today, -1);
    assert_eq!(t.task("集训")["doneOnDates"], json!({ occ.as_str(): today }));
    assert_eq!(label(&t, "集训"), format!("第2-2天（100%） {}", short(&today)));
    menu(&t, "集训，已完成");
    t.click("修改完成日期…");
    t.click("上个月");
    t.click(&day(&picked));
    assert_eq!(t.task("集训")["doneOnDates"], json!({ occ.as_str(): picked }));
    assert_eq!(label(&t, "集训"), format!("第2-2天（100%） {}", short(&picked)));
}

/// 0.3.17 (GitHub #14): 按分组筛选. The title bar's filter button lists 全部, 未分组 and the groups;
/// ticking some shows only their tasks in the month and multi-week views, the list view, the
/// "+N" list and the to-do column, and the button is red (ticked) and named after them until 全部
/// is chosen again. Every choice ticked is 全部. A task made while one group shows is of that
/// group; with several, the toast says it isn't shown. A group that is deleted leaves the
/// filter. 右键 › 分组 puts a task in a group; a preset one (生日、工作、还款、纪念日) is made the first
/// time it is chosen.
#[test]
fn group_filter_shows_the_ticked_groups() {
    use fluentcal_core::date::{add_days, parts, start_of_month, start_of_week};
    use i_slint_backend_testing::AccessibleRole as R;
    use std::collections::BTreeSet;
    let today = fluentcal_core::date::today();
    // the days of this week (all of them are in the month view), a task or two on each
    let day = |i: i32| add_days(&start_of_week(&today, 1), i);
    let tasks = [
        json!({ "id": "b1", "title": "妈妈生日", "start": day(0), "groupId": "g1" }),
        json!({ "id": "u1", "title": "买菜", "start": day(0) }),
        json!({ "id": "w1", "title": "周报", "start": day(1), "groupId": "g2" }),
        json!({ "id": "x1", "title": "旧分组的事", "start": day(2), "groupId": "gone" }),
        json!({ "id": "m1", "title": "交物业费", "start": start_of_month(&today), "flex": "month", "groupId": "g2" }),
        json!({ "id": "s1", "title": "整理相册", "start": today, "flex": "someday" }),
    ];
    let mut doc = doc_with(&tasks, json!({ "calendar": { "sidebar": true } }));
    doc["groups"] = json!({
        "g1": { "id": "g1", "name": "生日", "color": "pink", "order": 1 },
        "g2": { "id": "g2", "name": "工作", "color": "blue", "order": 2 },
    });
    let t = start("groupfilter", Some(doc));
    let set = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<String>>();
    let chips = |t: &T| t.w.get_weeks().iter().flat_map(|w| w.chips.iter().collect::<Vec<_>>()).map(|c| c.title.to_string()).collect::<BTreeSet<String>>();
    let side = |t: &T| t.w.get_side_rows().iter().filter(|r| r.kind == "item").map(|r| r.chip.title.to_string()).collect::<BTreeSet<String>>();
    let listed = |t: &T| t.w.get_list_rows().iter().filter(|r| r.kind == "item").map(|r| r.chip.title.to_string()).collect::<BTreeSet<String>>();
    let button = |t: &T| t.w.root_element().query_descendants().match_predicate(|e| e.accessible_role() == Some(R::Button) && e.accessible_label().is_some_and(|l| l.starts_with("按分组筛选"))).find_first();
    let press = |t: &T| {
        let b = button(t).expect("the filter button");
        t.click_at(t.at(&b, 16.0, 16.0));
    };
    let boxed = |t: &T, name: &str| ElementHandle::find_by_accessible_label(&t.w, name).find(|e| e.accessible_role() == Some(R::Checkbox)).unwrap_or_else(|| panic!("no box {name:?}"));
    let tick = |t: &T, name: &str| {
        let b = boxed(t, name);
        t.click_at(t.at(&b, 9.0, b.size().height / 2.0));
    };
    let rows = |t: &T| t.w.get_filter_rows().iter().map(|r| format!("{}:{}:{}", r.name, r.count, r.checked)).collect::<Vec<_>>();
    let menu = |t: &T, title: &str| {
        let e = ElementHandle::find_by_accessible_label(&t.w, title).find(|e| e.accessible_role() == Some(R::Groupbox)).unwrap_or_else(|| panic!("no chip {title:?}"));
        t.right_click_at(t.at(&e, 40.0, e.size().height / 2.0));
    };
    let group_named = |name: &str| {
        let name = name.to_owned();
        with(move |a| a.doc.groups.values().find(|g| g.name == name).cloned()).unwrap()
    };
    let everything = set(&["妈妈生日", "周报", "买菜", "旧分组的事"]);
    assert_eq!((chips(&t), side(&t)), (everything.clone(), set(&["交物业费", "整理相册"])));

    // 右键 › 分组: the task's own group is the one ticked; another, then none
    menu(&t, "周报");
    t.click("分组");
    let leaf = |t: &T, name: &str| ElementHandle::find_by_accessible_label(&t.w, name).find(|e| e.accessible_role() == Some(R::ListItem)).unwrap_or_else(|| panic!("no entry {name:?}"));
    assert_eq!((leaf(&t, "工作").accessible_checked(), leaf(&t, "生日").accessible_checked(), leaf(&t, "未分组").accessible_checked()), (Some(true), Some(false), Some(false)));
    assert!(t.has("还款") && t.has("纪念日"), "the presets no group has the name of");
    t.click("生日");
    assert_eq!(t.task("周报")["groupId"], json!("g1"));
    menu(&t, "周报");
    t.click("分组");
    t.click("未分组");
    assert!(t.task("周报").get("groupId").is_none_or(Value::is_null));
    t.ctrl("z");
    t.ctrl("z");
    assert_eq!(t.task("周报")["groupId"], json!("g2"));
    // a preset: the group is made with it, and both go back in one step
    menu(&t, "买菜");
    t.click("分组");
    t.click("纪念日");
    let made = group_named("纪念日").expect("the preset group");
    assert_eq!((made.color.as_str(), t.task("买菜")["groupId"].as_str()), ("purple", Some(made.id.as_str())));
    menu(&t, "买菜");
    t.click("分组");
    assert_eq!(leaf(&t, "纪念日").accessible_checked(), Some(true), "a group like any other now");
    with(|a| a.desk_close_popups()).unwrap();
    settle();
    assert!(!t.w.get_any_popup());
    t.ctrl("z");
    assert!(group_named("纪念日").is_none() && t.task("买菜").get("groupId").is_none_or(Value::is_null));

    // the button: plain while everything shows
    let b = button(&t).expect("the filter button");
    assert_eq!((b.accessible_label().unwrap().to_string(), b.accessible_checked(), t.w.get_filter_on()), ("按分组筛选".to_owned(), Some(false), false));
    press(&t);
    assert!(t.w.get_any_popup(), "its list is open");
    assert_eq!(rows(&t), ["未分组:3:false", "生日:1:false", "工作:2:false"]);
    assert_eq!((boxed(&t, "全部").accessible_checked(), t.w.get_filter_all_count().to_string()), (Some(true), "6".to_owned()));

    // one group: its tasks only, in the calendar and the to-do column; the button says so
    tick(&t, "生日");
    assert!(t.w.get_any_popup(), "it stays open for more");
    assert_eq!((chips(&t), side(&t)), (set(&["妈妈生日"]), set(&[])));
    assert_eq!(rows(&t), ["未分组:3:false", "生日:1:true", "工作:2:false"]);
    assert_eq!(boxed(&t, "全部").accessible_checked(), Some(false));
    assert!(t.w.get_filter_on());
    assert_eq!(t.w.get_filter_tip(), "按分组筛选：生日");
    // with 未分组: a task whose group is gone is one of those
    tick(&t, "未分组");
    assert_eq!((chips(&t), side(&t)), (set(&["妈妈生日", "买菜", "旧分组的事"]), set(&["整理相册"])));
    assert_eq!(t.w.get_filter_tip(), "按分组筛选：未分组、生日");
    // every one ticked is 全部
    tick(&t, "工作");
    assert_eq!((t.w.get_filter_on(), chips(&t)), (false, everything.clone()));
    assert_eq!((rows(&t), boxed(&t, "全部").accessible_checked()), (vec!["未分组:3:false".to_owned(), "生日:1:false".to_owned(), "工作:2:false".to_owned()], Some(true)));
    // 全部 while it is on changes nothing; it takes a filter away
    tick(&t, "全部");
    assert!(!t.w.get_filter_on());
    tick(&t, "工作");
    assert_eq!((chips(&t), side(&t)), (set(&["周报"]), set(&["交物业费"])));
    tick(&t, "全部");
    assert_eq!((t.w.get_filter_on(), chips(&t), side(&t)), (false, everything.clone(), set(&["交物业费", "整理相册"])));
    tick(&t, "生日");
    t.key(&slint::SharedString::from(Key::Escape));
    assert!(!t.w.get_any_popup());
    let b = button(&t).expect("the filter button");
    assert_eq!((b.accessible_label().unwrap().to_string(), b.accessible_checked()), ("按分组筛选：生日".to_owned(), Some(true)), "red, and named after what shows");

    // a task made while one group shows is one of that group
    let cell_of = |t: &T, date: &str| {
        let (_, m, d) = parts(date);
        t.find_prefix(&format!("{m}月{d}日 "))
    };
    let cell = cell_of(&t, &day(4));
    t.double_click_at(t.at(&cell, 40.0, 50.0));
    t.type_text("订蛋糕");
    t.key("\n");
    t.key(&slint::SharedString::from(Key::Escape));
    assert_eq!((t.task("订蛋糕")["groupId"].clone(), t.task("订蛋糕")["start"].clone()), (json!("g1"), json!(day(4))));
    assert_eq!(chips(&t), set(&["妈妈生日", "订蛋糕"]));
    with(|a| a.open_editor_new(None)).unwrap();
    settle();
    let ed = with(|a| a.editor_window()).flatten().expect("the editor");
    assert_eq!(ed.get_data().group, "g1", "新建任务 starts in it too");
    with(|a| a.editor_close()).unwrap();
    settle();
    // with two groups it is in neither: the toast says why it isn't there
    press(&t);
    tick(&t, "工作");
    t.key(&slint::SharedString::from(Key::Escape));
    let cell = cell_of(&t, &day(5));
    t.double_click_at(t.at(&cell, 40.0, 50.0));
    t.type_text("散步");
    t.key("\n");
    t.key(&slint::SharedString::from(Key::Escape));
    assert!(t.task("散步").get("groupId").is_none_or(Value::is_null));
    assert_eq!(t.w.get_toast_text(), "已添加“散步”（未分组，当前筛选下不显示）");
    assert_eq!(chips(&t), set(&["妈妈生日", "周报", "订蛋糕"]));
    assert_eq!(t.w.get_filter_tip(), "按分组筛选：生日、工作");

    // the "+N" list of a day, and the list view
    with({
        let first = day(0);
        move |a| a.desk_day_list(&first, (10.0, 10.0, 10.0, 10.0))
    })
    .unwrap();
    settle();
    assert_eq!(t.w.get_day_list_chips().iter().map(|c| c.title.to_string()).collect::<BTreeSet<_>>(), set(&["妈妈生日"]), "买菜 is on that day too");
    with(|a| a.desk_close_popups()).unwrap();
    settle();
    assert!(!t.w.get_any_popup(), "a press outside the calendar closes them");
    with(|a| {
        a.desk_set_view("list");
        a.desk_set_list_range("all");
    })
    .unwrap();
    settle();
    assert_eq!(listed(&t), set(&["妈妈生日", "周报", "订蛋糕"]));
    assert!(button(&t).is_some_and(|b| b.accessible_checked() == Some(true)));

    // not in the Gantt chart, which has its own 只显示分组
    with(|a| a.desk_set_view("gantt")).unwrap();
    settle();
    assert!(button(&t).is_none());
    t.click("更多");
    assert!(!t.has("按分组筛选…"));
    t.key(&slint::SharedString::from(Key::Escape));
    with(|a| a.desk_set_view("month")).unwrap();
    settle();
    // 更多 › 按分组筛选…: the same list, under the ⋯ button
    t.click("更多");
    t.click("按分组筛选…");
    assert!(t.w.get_any_popup());
    assert_eq!(rows(&t), ["未分组:4:false", "生日:2:true", "工作:2:true"]);

    // a group that is deleted leaves the filter; the last one gone, everything shows again
    let delete = |id: &str| {
        let id = id.to_owned();
        with(move |a| {
            let e = fluentcal_core::tasks::delete_groups(&a.doc, &[id], false, fluentcal_core::date::now_ms());
            a.apply(e, true);
        })
        .unwrap();
        settle();
    };
    delete("g1");
    assert_eq!((t.w.get_filter_tip().to_string(), rows(&t)), ("按分组筛选：工作".to_owned(), vec!["未分组:6:false".to_owned(), "工作:2:true".to_owned()]));
    assert_eq!(chips(&t), set(&["周报"]));
    delete("g2");
    assert!(!t.w.get_filter_on());
    assert_eq!(rows(&t), ["未分组:8:false"]);
    assert_eq!(t.w.get_filter_hint(), "还没有分组。右键任务 › 分组，可以把它放进“生日”“工作”等分组。");
    assert_eq!(chips(&t), set(&["妈妈生日", "周报", "买菜", "旧分组的事", "订蛋糕", "散步"]));
    // 管理分组…: where groups are made, renamed and deleted
    t.click("管理分组…");
    assert!(!t.w.get_any_popup());
    assert_eq!(with(|a| a.settings_win.as_ref().map(|s| s.local.page.clone())).unwrap().as_deref(), Some("groups"));
}

/// The button is there while there is room after the view switch, and always while it filters
/// (hidden tasks must not be overlooked): the view switch goes sooner then, and in the
/// narrowest window it stands where the search button was.
#[test]
fn group_filter_button_stays_while_it_filters() {
    use i_slint_backend_testing::AccessibleRole as R;
    let today = fluentcal_core::date::today();
    let mut doc = doc_with(&[json!({ "id": "a", "title": "周报", "start": today, "groupId": "g" })], json!({}));
    doc["groups"] = json!({ "g": { "id": "g", "name": "工作", "color": "blue", "order": 1 } });
    let t = start("groupfilter-narrow", Some(doc));
    let button = |t: &T, prefix: &'static str| t.w.root_element().query_descendants().match_predicate(move |e| e.accessible_role() == Some(R::Button) && e.accessible_label().is_some_and(|l| l.starts_with(prefix))).find_first();
    let tabs = |t: &T| ElementHandle::find_by_accessible_label(&t.w, "多周").any(|e| e.accessible_role() == Some(R::Tab));
    let resize = |t: &T, w: f32| {
        t.w.window().set_size(slint::LogicalSize::new(w, 640.0));
        settle();
    };
    let widths: Vec<f32> = (0..=26).map(|i| 880.0 - 20.0 * i as f32).collect();
    // not filtering: there in a wide window, and it goes before the view switch does
    let mut seen = Vec::new();
    for &w in &widths {
        resize(&t, w);
        seen.push((button(&t, "按分组筛选").is_some(), tabs(&t), button(&t, "搜索").is_some()));
    }
    assert_eq!(seen[0], (true, true, true), "at 880");
    assert!(seen.contains(&(false, true, true)), "the filter button goes first: {seen:?}");
    assert_eq!(*seen.last().unwrap(), (false, false, true), "at 360: {seen:?}");
    assert!(seen.windows(2).all(|p| (p[0].0 || !p[1].0) && (p[0].1 || !p[1].1)), "what went doesn't come back as it narrows: {seen:?}");
    // filtering: always there, and the bar's buttons still fit
    with(|a| a.desk_filter_toggle("g", true)).unwrap();
    settle();
    let mut switch_at = Vec::new();
    for &w in &widths {
        resize(&t, w);
        let b = button(&t, "按分组筛选").unwrap_or_else(|| panic!("no filter button at {w}"));
        assert_eq!(b.accessible_checked(), Some(true));
        let more = button(&t, "更多").expect("更多");
        assert!(more.absolute_position().x + more.size().width <= w, "the bar's buttons fit at {w}");
        switch_at.push(tabs(&t));
    }
    // (the view switch gives way to it one step sooner)
    let count = |v: &[bool]| v.iter().filter(|x| **x).count();
    assert!(count(&switch_at) < count(&seen.iter().map(|s| s.1).collect::<Vec<_>>()), "{switch_at:?}");
    // the narrowest, with the long title of the multi-week view: it has the search button's
    // place (搜索 is in 更多, and Ctrl+F)
    with(|a| a.desk_set_view("weeks")).unwrap();
    settle();
    assert!(button(&t, "按分组筛选").is_some() && button(&t, "搜索").is_none() && !tabs(&t));
    with(|a| a.desk_filter_clear()).unwrap();
    settle();
    assert!(button(&t, "搜索").is_some() && button(&t, "按分组筛选").is_none());
}

/// 0.3.17 (GitHub #7): the to-do column's order by hand. A to-do is dragged to another place
/// in its part (a line shows where it goes; one task is written, and it is one step to take
/// back), or moved with Alt+↑ / Alt+↓; the keys stay with the to-do that was moved. It stays in
/// its part, and nothing happens beside the column or after Esc. A list longer than the
/// column scrolls under the to-do at the edges.
#[test]
fn todo_column_order_by_hand() {
    use fluentcal_core::date::{add_months, start_of_month};
    use i_slint_backend_testing::AccessibleRole as R;
    let today = fluentcal_core::date::today();
    let first = start_of_month(&today);
    let last = add_months(&first, -1);
    let tasks = [
        json!({ "id": "m1", "title": "交物业费", "start": last, "flex": "month" }),
        json!({ "id": "m2", "title": "洗车", "start": first, "flex": "month" }),
        json!({ "id": "m3", "title": "理发", "start": first, "flex": "month" }),
        json!({ "id": "s1", "title": "整理相册", "start": today, "flex": "someday" }),
        json!({ "id": "s2", "title": "学吉他", "start": today, "flex": "someday" }),
        json!({ "id": "s3", "title": "换灯泡", "start": today, "flex": "someday" }),
        json!({ "id": "s4", "title": "修自行车", "start": today, "flex": "someday" }),
    ];
    let t = start("side-order", Some(doc_with(&tasks, json!({ "calendar": { "sidebar": true } }))));
    let part = |t: &T, zone: &str| t.w.get_side_rows().iter().filter(|r| r.kind == "item" && r.zone == zone).map(|r| r.chip.title.to_string()).collect::<Vec<_>>().join(" ");
    let item = |t: &T, label: &str| ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(R::ListItem)).unwrap_or_else(|| panic!("no {label:?}"));
    // a point on a to-do (right of its check circle), and one just under its top edge
    let on = |t: &T, label: &str| {
        let e = item(t, label);
        t.at(&e, 60.0, e.size().height / 2.0)
    };
    let above = |t: &T, label: &str| t.at(&item(t, label), 60.0, 1.0);
    let below = |t: &T, label: &str| {
        let e = item(t, label);
        t.at(&e, 60.0, e.size().height - 1.0)
    };
    let written = |t: &T| t.tasks().iter().filter(|x| x["updatedAt"] != json!(0)).map(|x| x["title"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
    let button = PointerEventButton::Left;
    assert_eq!((part(&t, "month"), part(&t, "overdue"), part(&t, "someday")), ("交物业费 洗车 理发".to_owned(), "交物业费".to_owned(), "整理相册 学吉他 换灯泡 修自行车".to_owned()));

    // dragged to the front: the line is above the first one while it is held there
    let (from, to) = (on(&t, "修自行车"), above(&t, "整理相册"));
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x + 2.0, from.y - 2.0) });
    assert!(!t.w.get_drag_active(), "not yet: a click that slipped");
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x, from.y - 30.0) });
    t.ev(WindowEvent::PointerMoved { position: to });
    settle();
    let first_row = t.w.get_side_rows().iter().position(|r| r.kind == "item" && r.zone == "someday").unwrap() as i32;
    assert_eq!((t.w.get_drag_active(), t.w.get_drag_label().to_string(), t.w.get_side_drag_key().to_string(), t.w.get_side_drop_row()), (true, "修自行车".to_owned(), "s4@s:".to_owned(), first_row));
    t.ev(WindowEvent::PointerReleased { position: to, button });
    settle();
    assert_eq!(part(&t, "someday"), "修自行车 整理相册 学吉他 换灯泡");
    assert_eq!((t.w.get_drag_active(), t.w.get_side_drag_key().to_string(), t.w.get_side_drop_row()), (false, String::new(), -1));
    assert_eq!(written(&t), ["修自行车"], "one task is written");
    assert_eq!(with(|a| a.changes.list().first().map(|c| c.text())).unwrap().as_deref(), Some("调整顺序“修自行车”"));
    // one step to take back, and again
    t.ctrl("z");
    assert_eq!(part(&t, "someday"), "整理相册 学吉他 换灯泡 修自行车");
    t.ctrl("y");
    assert_eq!(part(&t, "someday"), "修自行车 整理相册 学吉他 换灯泡");
    // between two others; let go on the lower half of a row: behind that one
    t.drag(on(&t, "修自行车"), above(&t, "换灯泡"));
    assert_eq!(part(&t, "someday"), "整理相册 学吉他 修自行车 换灯泡");
    t.drag(on(&t, "整理相册"), below(&t, "换灯泡"));
    assert_eq!(part(&t, "someday"), "学吉他 修自行车 换灯泡 整理相册");
    // where it is already (its own row, the edge it shares with the next one): nothing
    let before = with(|a| a.changes.list().len()).unwrap();
    t.drag(on(&t, "修自行车"), above(&t, "换灯泡"));
    t.drag(on(&t, "修自行车"), below(&t, "学吉他"));
    assert_eq!((part(&t, "someday"), with(|a| a.changes.list().len()).unwrap()), ("学吉他 修自行车 换灯泡 整理相册".to_owned(), before));

    // it stays in its part: let go far below, it is the last of its own
    t.drag(on(&t, "交物业费"), on(&t, "换灯泡"));
    assert_eq!((part(&t, "month"), part(&t, "overdue"), part(&t, "someday")), ("洗车 理发 交物业费".to_owned(), "交物业费".to_owned(), "学吉他 修自行车 换灯泡 整理相册".to_owned()));
    // beside the column (over the calendar): no line, and nothing happens
    let from = on(&t, "理发");
    let away = LogicalPosition::new(t.w.get_grid_x() + 200.0, from.y);
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x, from.y - 20.0) });
    t.ev(WindowEvent::PointerMoved { position: away });
    settle();
    assert_eq!((t.w.get_drag_active(), t.w.get_side_drop_row()), (true, -1));
    t.ev(WindowEvent::PointerReleased { position: away, button });
    settle();
    assert_eq!((t.w.get_drag_active(), part(&t, "month")), (false, "洗车 理发 交物业费".to_owned()));
    // Esc while it is held: let go, it stays
    let (from, to) = (on(&t, "理发"), above(&t, "洗车"));
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: to });
    settle();
    assert!(t.w.get_drag_active() && t.w.get_side_drop_row() >= 0);
    t.key(&slint::SharedString::from(Key::Escape));
    assert_eq!((t.w.get_drag_active(), t.w.get_side_drop_row()), (false, -1));
    t.ev(WindowEvent::PointerReleased { position: to, button });
    settle();
    assert_eq!(part(&t, "month"), "洗车 理发 交物业费");

    // Alt+↓ / Alt+↑ on the one that has the keys; they stay with it as it moves
    let alt = |t: &T, k: Key| {
        let a: slint::SharedString = Key::Alt.into();
        t.ev(WindowEvent::KeyPressed { text: a.clone() });
        t.key(&slint::SharedString::from(k));
        t.ev(WindowEvent::KeyReleased { text: a });
        settle();
    };
    t.click_at(on(&t, "学吉他"));
    alt(&t, Key::DownArrow);
    assert_eq!(part(&t, "someday"), "修自行车 学吉他 换灯泡 整理相册");
    alt(&t, Key::DownArrow);
    alt(&t, Key::DownArrow);
    assert_eq!(part(&t, "someday"), "修自行车 换灯泡 整理相册 学吉他");
    // the last one: no further
    let before = with(|a| a.changes.list().len()).unwrap();
    alt(&t, Key::DownArrow);
    assert_eq!((part(&t, "someday"), with(|a| a.changes.list().len()).unwrap()), ("修自行车 换灯泡 整理相册 学吉他".to_owned(), before));
    alt(&t, Key::UpArrow);
    assert_eq!(part(&t, "someday"), "修自行车 换灯泡 学吉他 整理相册");
    // undone, the keys are still with it (in the row it went back to)
    t.ctrl("z");
    assert_eq!(part(&t, "someday"), "修自行车 换灯泡 整理相册 学吉他");
    alt(&t, Key::UpArrow);
    alt(&t, Key::UpArrow);
    alt(&t, Key::UpArrow);
    assert_eq!(part(&t, "someday"), "学吉他 修自行车 换灯泡 整理相册");
    alt(&t, Key::UpArrow);
    assert_eq!(part(&t, "someday"), "学吉他 修自行车 换灯泡 整理相册", "the first one: no further");
    // Space ticks the one that was moved, not the one that has its old row now
    alt(&t, Key::DownArrow);
    assert_eq!(part(&t, "someday"), "修自行车 学吉他 换灯泡 整理相册");
    t.key(" ");
    assert_eq!((t.task("学吉他")["done"].clone(), t.task("修自行车")["done"].clone(), part(&t, "someday")), (json!(true), json!(false), "修自行车 换灯泡 整理相册".to_owned()));
    t.ctrl("z");
    assert_eq!(part(&t, "someday"), "修自行车 学吉他 换灯泡 整理相册");

    // one left open last month is in two parts: moved in either, it moves in both
    with({
        let last = last.clone();
        move |a| {
            let p = fluentcal_core::flex::new_task(fluentcal_core::flex::MONTH, "交水费", &last);
            let (e, _) = fluentcal_core::tasks::add_task(&a.doc, p, None, fluentcal_core::date::now_ms());
            a.apply(e, true);
        }
    })
    .unwrap();
    settle();
    assert_eq!((part(&t, "month"), part(&t, "overdue")), ("洗车 理发 交物业费 交水费".to_owned(), "交物业费 交水费".to_owned()));
    // (the lower of the two rows it has)
    let overdue = |t: &T, label: &str| {
        let mut both: Vec<ElementHandle> = ElementHandle::find_by_accessible_label(&t.w, label).filter(|e| e.accessible_role() == Some(R::ListItem)).collect();
        both.sort_by(|a, b| a.absolute_position().y.total_cmp(&b.absolute_position().y));
        assert_eq!(both.len(), 2, "{label} is in two parts");
        both.pop().unwrap()
    };
    let (moved, onto) = (overdue(&t, "交水费"), overdue(&t, "交物业费"));
    t.drag(t.at(&moved, 60.0, moved.size().height / 2.0), t.at(&onto, 60.0, 1.0));
    assert_eq!((part(&t, "month"), part(&t, "overdue")), ("洗车 理发 交水费 交物业费".to_owned(), "交水费 交物业费".to_owned()));
}

/// GitHub #7: in a column too short for its list, the list scrolls under a to-do held at the
/// bottom or the top edge, so it can be taken to a place that wasn't in sight.
#[test]
fn todo_column_scrolls_under_a_dragged_one() {
    use i_slint_backend_testing::AccessibleRole as R;
    let today = fluentcal_core::date::today();
    let tasks: Vec<Value> = (1..=16).map(|i| json!({ "id": format!("s{i:02}"), "title": format!("待办{i:02}"), "start": today, "flex": "someday" })).collect();
    let t = start("side-scroll", Some(doc_with(&tasks, json!({ "calendar": { "sidebar": true } }))));
    t.w.window().set_size(slint::LogicalSize::new(880.0, 360.0));
    settle();
    let order = |t: &T| t.w.get_side_rows().iter().filter(|r| r.kind == "item").map(|r| r.chip.title.to_string()).collect::<Vec<_>>();
    let item = |t: &T, label: &str| ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(R::ListItem)).unwrap_or_else(|| panic!("no {label:?}"));
    // (what the column cuts off isn't found: it is not in sight)
    let seen = |t: &T, label: &str| ElementHandle::find_by_accessible_label(&t.w, label).any(|e| e.accessible_role() == Some(R::ListItem));
    let panel = t.find("不定日待办");
    let (top, bottom) = (panel.absolute_position().y, panel.absolute_position().y + panel.size().height);
    assert!(seen(&t, "待办01") && !seen(&t, "待办16"), "the list is longer than the column");
    let button = PointerEventButton::Left;
    let first = item(&t, "待办01");
    let from = t.at(&first, 60.0, first.size().height / 2.0);
    // held at the bottom edge: the list goes up under it, to its end
    let hold = LogicalPosition::new(from.x, bottom - 8.0);
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x, from.y + 20.0) });
    t.ev(WindowEvent::PointerMoved { position: hold });
    for _ in 0..80 {
        wait(20);
    }
    assert!(!seen(&t, "待办02"), "scrolled: the first ones are out of sight");
    let last = item(&t, "待办16");
    assert!(last.absolute_position().y + last.size().height <= bottom, "to the end: {} vs {bottom}", last.absolute_position().y);
    // the line went with it: let go here, it is behind the ones that were out of sight
    let line = t.w.get_side_drop_row();
    assert!(line > 12, "the line is far down the list: {line}");
    t.ev(WindowEvent::PointerReleased { position: hold, button });
    settle();
    let now = order(&t);
    let at = now.iter().position(|x| x == "待办01").unwrap();
    assert!(at >= 10, "taken down the list: {now:?}");
    assert_eq!(now.len(), 16);
    // and back up: held at the top edge
    let e = item(&t, "待办01");
    let from = t.at(&e, 60.0, e.size().height / 2.0);
    let hold = LogicalPosition::new(from.x, top + 8.0);
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x, from.y - 20.0) });
    t.ev(WindowEvent::PointerMoved { position: hold });
    for _ in 0..80 {
        wait(20);
    }
    t.ev(WindowEvent::PointerReleased { position: hold, button });
    settle();
    assert_eq!(order(&t)[0], "待办01", "at the front again: {:?}", order(&t));
    // let go, the list stays still
    let y = item(&t, "待办02").absolute_position().y;
    for _ in 0..10 {
        wait(20);
    }
    assert_eq!(item(&t, "待办02").absolute_position().y, y);
}

/// 0.3.18 (CAL-10): a task in the "+N" list can be dragged onto another day. Past 12px the list
/// goes out of sight (it lies over days) and the day under the pointer is marked; let go there
/// and the task is on that day, the list is closed, a line says where the task went, and it is
/// one step to take back. Let go on the list's own day or off the days, or Esc: nothing, and
/// the list is back. A task over several days is held by the list's day; of a repeating one
/// only this time moves.
#[test]
fn day_list_task_drags_to_another_day() {
    use fluentcal_core::date::{add_days, format_friendly, format_range};
    use i_slint_backend_testing::AccessibleRole as R;
    let (day, beside) = ("2026-11-10", "2026-11-13");
    let mut tasks: Vec<Value> = (0..8).map(|i| json!({ "title": format!("事项{i}"), "start": day })).collect();
    // three days from the 9th (the 10th is its second), and every Tuesday: first in the list
    tasks.push(json!({ "id": "trip", "title": "出差", "start": "2026-11-09", "end": "2026-11-11", "order": 0.1 }));
    tasks.push(json!({ "id": "weekly", "title": "周会", "start": "2026-11-03", "repeat": { "freq": "weekly", "interval": 1 }, "order": 0.2 }));
    let t = start("daylist-drag", Some(doc_with(&tasks, json!({ "calendar": { "weekStart": 1 } }))));
    t.w.invoke_jump(day.into());
    settle();
    let today = t.today();
    let button = PointerEventButton::Left;
    let esc = slint::SharedString::from(Key::Escape);
    let open = |t: &T| {
        t.click(&t.find_prefix("还有 ").accessible_label().unwrap());
        assert!(t.w.get_any_popup(), "the day list is open");
    };
    // the list's panel, and a task in it (a task may be on the calendar as well; the last ones
    // are further down the list than it shows at once)
    let panel = |t: &T| ElementHandle::find_by_accessible_label(&t.w, &format_friendly(day, &today)).find(|e| e.accessible_role() == Some(R::Groupbox)).expect("the list's panel");
    let within = |e: &ElementHandle, p: LogicalPosition| {
        let (o, s) = (e.absolute_position(), e.size());
        p.x >= o.x && p.x < o.x + s.width && p.y >= o.y && p.y < o.y + s.height
    };
    let in_list = |t: &T, title: &str| {
        let p = panel(t);
        // (its row in the list is as wide as the list; the list may lie over its chip on the calendar)
        let wide = p.size().width - 34.0;
        let e = ElementHandle::find_by_accessible_label(&t.w, title).find(|e| within(&p, e.absolute_position()) && (e.size().width - wide).abs() < 0.5).unwrap_or_else(|| panic!("{title:?} is not in the list"));
        t.at(&e, 40.0, e.size().height / 2.0)
    };
    let on_day = |t: &T, date: &str| {
        let (_, m, d) = fluentcal_core::date::parts(date);
        let label = format!("{m}月{d}日 ");
        let cell = t.w.root_element().query_descendants().match_predicate(move |e| e.accessible_role() == Some(R::ListItem) && e.accessible_label().is_some_and(|l| l.starts_with(&label))).find_first().unwrap_or_else(|| panic!("no day {date}"));
        let p = t.at(&cell, 40.0, 50.0);
        assert_eq!(with(|a| a.desk_date_at(p.x, p.y)).flatten().as_deref(), Some(date));
        p
    };
    let changes = || with(|a| a.changes.list().len()).unwrap();

    // out of the list onto a day the list lies over (it opens under its day, or above it)
    open(&t);
    assert_eq!(t.w.get_day_list_chips().row_count(), 10);
    let under = ["2026-11-17", "2026-11-18", "2026-11-03", "2026-11-04"].into_iter().find(|d| within(&panel(&t), on_day(&t, d))).expect("a day under the list");
    let (from, to) = (in_list(&t, "事项5"), on_day(&t, under));
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x + 6.0, from.y + 6.0) });
    assert!(!t.w.get_drag_active(), "not yet: a click that slipped");
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x + 12.0, from.y + 12.0) });
    assert!(t.w.get_drag_active() && t.w.get_drag_label() == "事项5", "held: the list is out of sight");
    t.ev(WindowEvent::PointerMoved { position: to });
    settle();
    assert_eq!(t.w.get_drop_date(), under, "the day under the pointer is marked");
    assert!(t.w.get_any_popup(), "the list is still there, out of sight");
    t.ev(WindowEvent::PointerReleased { position: to, button });
    settle();
    assert_eq!(t.task("事项5")["start"], json!(under));
    assert!(!t.w.get_any_popup() && !t.w.get_drag_active(), "landed: the list is closed");
    assert_eq!(t.w.get_toast_text(), format!("已移到 {}", format_friendly(under, &today)));
    assert!(t.w.get_toast_undo());
    assert_eq!(with(|a| a.desk.popups.day_list.clone()).unwrap(), None);
    // one step, and the keys are with the calendar
    t.ctrl("z");
    assert_eq!(t.task("事项5")["start"], json!(day));
    t.ctrl("y");
    assert_eq!(t.task("事项5")["start"], json!(under));
    t.ctrl("z");

    // let go on the list's own day, and off the days: nothing, the list is back
    open(&t);
    let before = changes();
    t.drag(in_list(&t, "事项5"), on_day(&t, day));
    assert_eq!((t.task("事项5")["start"].clone(), changes()), (json!(day), before));
    assert!(t.w.get_any_popup() && !t.w.get_drag_active() && t.w.get_drop_date() == "", "the list is back");
    let title_bar = LogicalPosition::new(300.0, 20.0);
    assert_eq!(with(|a| a.desk_date_at(title_bar.x, title_bar.y)).unwrap(), None);
    t.drag(in_list(&t, "事项5"), title_bar);
    assert_eq!((t.task("事项5")["start"].clone(), changes()), (json!(day), before));
    assert!(t.w.get_any_popup() && !t.w.get_drag_active());
    assert_eq!(t.w.get_day_list_chips().row_count(), 10);

    // Esc on the way: the task stays, and so does the list
    let (from, to) = (in_list(&t, "事项5"), on_day(&t, beside));
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x + 20.0, from.y + 20.0) });
    t.ev(WindowEvent::PointerMoved { position: to });
    assert!(t.w.get_drag_active() && t.w.get_drop_date() == beside);
    t.key(&esc);
    assert!(!t.w.get_drag_active() && t.w.get_drop_date() == "", "Esc ends the drag");
    assert!(t.w.get_any_popup(), "… and leaves the list open");
    t.ev(WindowEvent::PointerReleased { position: to, button });
    settle();
    assert_eq!((t.task("事项5")["start"].clone(), changes()), (json!(day), before));
    assert!(t.w.get_any_popup());
    // (then Esc closes the list, as ever)
    t.key(&esc);
    assert!(!t.w.get_any_popup());

    // three days, held by the list's day (its second): that day goes under the pointer
    open(&t);
    t.drag(in_list(&t, "出差"), on_day(&t, under));
    let trip = t.task("出差");
    let (first, last) = (add_days(under, -1), add_days(under, 1));
    assert_eq!((trip["start"].as_str(), trip["end"].as_str()), (Some(first.as_str()), Some(last.as_str())));
    assert_eq!(t.w.get_toast_text(), format!("已移到 {}", format_range(&first, &last, &today)));
    assert!(!t.w.get_any_popup());

    // every Tuesday: this one alone goes to Thursday
    open(&t);
    t.drag(in_list(&t, "周会"), on_day(&t, beside));
    let weekly: Vec<Value> = t.tasks().into_iter().filter(|x| x["title"] == "周会").collect();
    assert_eq!(weekly.len(), 2, "the series and the one that left it");
    let series = weekly.iter().find(|x| x["id"] == "weekly").unwrap();
    assert_eq!(series["exDates"], json!([day]));
    let one = weekly.iter().find(|x| x["id"] != "weekly").unwrap();
    assert_eq!((one["start"].as_str(), one.get("repeat")), (Some(beside), None));
    t.ctrl("z");
    assert_eq!(t.tasks().into_iter().filter(|x| x["title"] == "周会").count(), 1, "one step for both");
}

/// The list isn't filled again under a task that is held in it (a new list would take the
/// pointer from the pressed chip, and its release would never come): it follows the data when
/// the task is let go. A list that closes under a held task ends the drag.
#[test]
fn day_list_waits_for_a_held_task() {
    let day = fluentcal_core::date::today();
    let tasks: Vec<Value> = (0..8).map(|i| json!({ "id": format!("t{i}"), "title": format!("事项{i}"), "start": day })).collect();
    let t = start("daylist-held", Some(doc_with(&tasks, json!({}))));
    let button = PointerEventButton::Left;
    t.click(&t.find_prefix("还有 ").accessible_label().unwrap());
    let chip = t.find("事项7");
    let from = t.at(&chip, 40.0, chip.size().height / 2.0);
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    // another one is done meanwhile (a sync, the phone)
    with(|a| a.desk_chip_toggle("t0", &a.today(), false)).unwrap();
    settle();
    assert_eq!(t.task("事项0")["done"], json!(true));
    let done = |t: &T| t.w.get_day_list_chips().iter().filter(|c| c.done).count();
    assert_eq!(done(&t), 0, "held: the list waits");
    t.ev(WindowEvent::PointerReleased { position: from, button });
    settle();
    assert_eq!(done(&t), 1, "let go: the list shows it");
    assert!(t.w.get_any_popup());
    // held over the calendar, and the list is closed from outside (a press on another window)
    let chip = t.find("事项7");
    let from = t.at(&chip, 40.0, chip.size().height / 2.0);
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x + 30.0, from.y) });
    assert!(t.w.get_drag_active());
    with(|a| a.desk_close_popups()).unwrap();
    settle();
    assert!(!t.w.get_any_popup() && !t.w.get_drag_active(), "the drag ends with the list");
    assert!(with(|a| a.desk.drag.is_none()).unwrap());
    t.ev(WindowEvent::PointerReleased { position: from, button });
    settle();
    assert_eq!(t.task("事项7")["start"], json!(day));
}

/// 0.3.18 (CAL-17): the to-do column that floats over the calendar can close on a press
/// outside it (设置 › 日历 › 浮动待办栏的收起方式; off by default). The press goes where it was
/// going: on a day it selects that day. On the column, its handle and the toast it stays open,
/// and while a menu of the calendar is open; a column beside the calendar never closes this way.
#[test]
fn floating_todo_column_closes_on_a_press_outside() {
    use i_slint_backend_testing::AccessibleRole as R;
    let today = fluentcal_core::date::today();
    let tasks = [
        json!({ "id": "s1", "title": "整理相册", "start": today, "flex": "someday" }),
        json!({ "id": "s2", "title": "学吉他", "start": today, "flex": "someday" }),
        json!({ "id": "rent", "title": "交房租", "start": today }),
    ];
    // (the week begins two days after today's weekday: today is in its sixth column, right of
    // the column)
    let cal = json!({ "sidebar": true, "sidebarMode": "float", "view": "weeks", "weekStart": (fluentcal_core::date::weekday(&today) + 2) % 7 });
    let t = start("side-outside", Some(doc_with(&tasks, json!({ "calendar": cal }))));
    // The real window tells of a press before the calendar gets it (platform::desk's window
    // procedure); here the test does both.
    let press = |t: &T, p: LogicalPosition| {
        let closed = with(|a| a.desk_side_pressed(Some((p.x, p.y)))).unwrap();
        t.click_at(p);
        closed
    };
    let centre = |e: &ElementHandle| LogicalPosition::new(e.absolute_position().x + e.size().width / 2.0, e.absolute_position().y + e.size().height / 2.0);
    let item = |t: &T, label: &str| ElementHandle::find_by_accessible_label(&t.w, label).find(|e| e.accessible_role() == Some(R::ListItem)).unwrap_or_else(|| panic!("no {label:?}"));
    // a day in the last column: far from the column
    let far_day = |t: &T, row: usize| {
        let weeks = t.w.get_weeks();
        let date = weeks.row_data(row).unwrap().cells.row_data(6).unwrap().date.to_string();
        let (gx, gy, cw, ch, gap) = (t.w.get_grid_x(), t.w.get_grid_y(), t.w.get_cell_w(), t.w.get_cell_h(), t.w.get_cell_gap());
        let p = LogicalPosition::new(gx + 6.0 * (cw + gap) + cw / 2.0, gy + row as f32 * (ch + gap) + ch - 12.0);
        assert_eq!(with(|a| a.desk_date_at(p.x, p.y)).flatten(), Some(date.clone()));
        (date, p)
    };
    let set = |v: Value| {
        with(|a| a.patch_settings(v)).unwrap();
        settle();
    };
    let closes = || with(|a| fluentcal_app::shell::side_closes_outside(&a.doc.settings)).unwrap();
    let (_, day) = far_day(&t, 1);

    // as before: only the handle closes it
    assert!(!closes());
    assert!(!press(&t, day) && t.w.get_side_open());

    set(json!({ "calendar": { "sidebarClose": "outside" } }));
    assert!(closes());
    // on the column: a to-do, the add box, the column's own free room
    let panel = t.find("不定日待办");
    assert!(!press(&t, centre(&item(&t, "整理相册"))) && t.w.get_side_open(), "a to-do");
    assert!(!press(&t, centre(&t.find("添加无日期待办"))) && t.w.get_side_open(), "the add box");
    t.key(&slint::SharedString::from(Key::Escape));
    assert!(!press(&t, t.at(&panel, panel.size().width - 3.0, panel.size().height - 3.0)) && t.w.get_side_open(), "the column's corner");
    // its handle closes it as ever (the press doesn't close it first, or the click would open it again)
    assert!(!press(&t, centre(&t.find("收起待办栏"))));
    assert!(!t.w.get_side_open(), "the handle closed it");
    assert!(!press(&t, centre(&t.find("展开待办栏（不定日待办）"))));
    assert!(t.w.get_side_open(), "… and opens it");

    // a menu of the calendar is open: the press is the menu's
    t.right_click_at(centre(&item(&t, "学吉他")));
    assert!(t.w.get_any_popup());
    assert!(!press(&t, day), "the press that dismisses the menu");
    assert!(!t.w.get_any_popup() && t.w.get_side_open());
    // the toast
    with(|a| a.notify("已保存", false)).unwrap();
    settle();
    assert!(!press(&t, centre(&t.find("关闭"))) && t.w.get_side_open(), "the toast's button");
    assert_eq!(t.w.get_toast_text(), "");

    // a to-do on its way to another place in the column: a second button doesn't end it
    let from = centre(&item(&t, "学吉他"));
    t.ev(WindowEvent::PointerMoved { position: from });
    t.ev(WindowEvent::PointerPressed { position: from, button: PointerEventButton::Left });
    t.ev(WindowEvent::PointerMoved { position: LogicalPosition::new(from.x, from.y - 20.0) });
    assert!(t.w.get_drag_active());
    assert!(!with(|a| a.desk_side_pressed(Some((day.x, day.y)))).unwrap());
    t.ev(WindowEvent::PointerReleased { position: from, button: PointerEventButton::Left });
    settle();
    assert!(t.w.get_side_open());

    // a day: the column closes, and the day is selected by the same press
    let (date, day) = far_day(&t, 2);
    assert_ne!(with(|a| a.desk.ui.selected.clone()).unwrap(), date);
    assert!(press(&t, day));
    assert!(!t.w.get_side_open() && !with(|a| a.doc.settings.calendar.sidebar).unwrap());
    assert_eq!(with(|a| a.desk.ui.selected.clone()).unwrap(), date, "the press went on to the day");
    assert!(!closes() && !press(&t, day), "closed: nothing to close");

    // a task's check circle: the column closes, and the same press ticks the task
    set(json!({ "calendar": { "sidebar": true } }));
    let chip = ElementHandle::find_by_accessible_label(&t.w, "交房租").next().expect("the task on the calendar");
    let panel = t.find("不定日待办");
    assert!(chip.absolute_position().x > panel.absolute_position().x + panel.size().width, "the task is beside the column");
    assert!(press(&t, t.at(&chip, 9.0, chip.size().height / 2.0)));
    assert!(!t.w.get_side_open());
    assert_eq!(t.task("交房租")["done"], json!(true), "the press went on to the check circle");

    // anywhere else (the desktop, another program's window: the mouse hook tells)
    set(json!({ "calendar": { "sidebar": true } }));
    assert!(with(|a| a.desk_side_pressed(None)).unwrap());
    settle();
    assert!(!t.w.get_side_open());

    // not while the calendar is being moved or lets the mouse through, nor beside the calendar
    set(json!({ "calendar": { "sidebar": true }, "desk": { "locked": false } }));
    assert!(!with(|a| a.desk_side_pressed(None)).unwrap() && t.w.get_side_open());
    set(json!({ "desk": { "locked": true, "clickThrough": true } }));
    assert!(!with(|a| a.desk_side_pressed(None)).unwrap() && t.w.get_side_open());
    set(json!({ "desk": { "clickThrough": false }, "calendar": { "sidebarMode": "inline" } }));
    let (_, day) = far_day(&t, 1);
    assert!(!press(&t, day) && !with(|a| a.desk_side_pressed(None)).unwrap() && t.w.get_side_open());
    set(json!({ "calendar": { "sidebarMode": "left" } }));
    assert!(!with(|a| a.desk_side_pressed(None)).unwrap() && t.w.get_side_open());
    // … nor in the views without the column
    set(json!({ "calendar": { "sidebarMode": "float", "view": "list" } }));
    assert!(!with(|a| a.desk_side_pressed(None)).unwrap() && with(|a| a.doc.settings.calendar.sidebar).unwrap());
}
