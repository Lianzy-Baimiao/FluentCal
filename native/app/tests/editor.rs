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

#[test]
fn a_task_deleted_elsewhere() {
    let t = start("gone", &[json!({ "id": "a", "title": "周报", "start": "2026-10-14" })]);
    // opened on a task that no longer exists: the message, no buttons
    let w = t.open(|a| a.open_editor_task("nope", None));
    assert!(w.get_missing());
    assert!(t.find(&w, "这个任务已被删除。").accessible_label().is_some());
    t.key(&w, &slint::SharedString::from(Key::Escape));
    assert!(t.editor().is_none());
    // deleted while being edited: saving asks to create it again
    let w = t.open(|a| a.open_editor_task("a", None));
    t.type_text(&w, "初稿");
    run(|a| {
        let edit = fluentcal_core::tasks::delete_task(&a.doc, "a", 5);
        a.apply(edit, true);
    });
    settle();
    t.ctrl(&w, "s");
    t.click(&w, &t.find(&w, "重新创建"));
    assert!(t.editor().is_none());
    assert_eq!(t.task("a").unwrap()["title"], "周报初稿");
}

/// GitHub #10: the progress slider's thumb follows the mouse from the first press on (the
/// title has the focus then), also when the hand drifts up or down on the way (the form
/// scrolls with the wheel; a drag must not pan it).
#[test]
fn progress_slider_follows_a_drag() {
    let t = start("slider", &[json!({ "id": "a", "title": "长任务", "start": "2026-11-10", "end": "2026-11-20" })]);
    let w = t.open(|a| a.open_editor_task("a", None));
    let s = ElementHandle::find_by_element_type_name(&w, "Slider").next().expect("the progress slider");
    let (p, size) = (s.absolute_position(), s.size());
    let at = |dx: f32, dy: f32| LogicalPosition::new(p.x + 1.0 + dx, p.y + size.height / 2.0 + dy);
    let button = PointerEventButton::Left;
    w.window().dispatch_event(WindowEvent::PointerMoved { position: at(0.0, 0.0) });
    w.window().dispatch_event(WindowEvent::PointerPressed { position: at(0.0, 0.0), button });
    for i in 1..=9 {
        wait(10);
        w.window().dispatch_event(WindowEvent::PointerMoved { position: at(i as f32 * 10.0, i as f32 * 1.5) });
    }
    w.window().dispatch_event(WindowEvent::PointerReleased { position: at(90.0, 13.5), button });
    settle();
    t.ctrl(&w, "s");
    assert_eq!(t.task("a").unwrap()["progress"], 50);
}

/// GitHub #16: a year far away without paging there month by month. The calendar's title goes up
/// a level (the days, the months, the years); a year and then a month come back down. The years
/// are a list that the wheel scrolls; over the days the wheel turns the months, over the months
/// the years. 今天 comes back to this month from wherever the view is, without choosing a day.
#[test]
fn date_picker_reaches_a_far_year() {
    use i_slint_backend_testing::AccessibleRole as R;
    let t = start("years", &[json!({ "id": "a", "title": "外婆生日", "start": "2026-11-19" })]);
    let today = t.today();
    let (ty, tm, td) = fluentcal_core::date::parts(&today);
    let w = t.open(|a| a.open_editor_task("a", None));
    let has = |label: &str| ElementHandle::find_by_accessible_label(&w, label).next().is_some();
    let of_role = |label: &str, role: R| ElementHandle::find_by_accessible_label(&w, label).find(|e| e.accessible_role() == Some(role)).unwrap_or_else(|| panic!("no {label:?}"));
    let wheel = |e: &ElementHandle, dy: f32| {
        let (p, s) = (e.absolute_position(), e.size());
        let position = LogicalPosition::new(p.x + s.width / 2.0, p.y + s.height / 2.0);
        w.window().dispatch_event(WindowEvent::PointerMoved { position });
        w.window().dispatch_event(WindowEvent::PointerScrolled { position, delta_x: 0.0, delta_y: dy });
        settle();
    };
    let open = ElementHandle::find_by_accessible_label(&w, "打开日历").next().expect("the start date's calendar button");
    t.click(&w, &open);
    // up to the months, up to the years
    t.click(&w, &of_role("2026年11月", R::Button));
    assert!(has("5月") && !has("2026年11月"), "the months of 2026");
    t.click(&w, &of_role("2026年", R::Button));
    let list = of_role("选择年份", R::List);
    // a year's cell when all of it is in the list's view (the search skips what is scrolled away)
    let year_cell = |label: &str| {
        let list = of_role("选择年份", R::List);
        let (top, bottom) = (list.absolute_position().y, list.absolute_position().y + list.size().height);
        ElementHandle::find_by_accessible_label(&w, label).filter(|e| e.accessible_role() == Some(R::ListItem)).find(|e| e.absolute_position().y >= top - 0.5 && e.absolute_position().y + e.size().height <= bottom + 0.5)
    };
    assert!(year_cell("2026年").is_some(), "the list opens around the year shown");
    assert!(year_cell("1960年").is_none(), "1960 is far above");
    // the wheel scrolls the years: a notch is 60px, more than a row
    let mut turns = 0;
    while year_cell("1960年").is_none() {
        turns += 1;
        assert!(turns <= 20, "the wheel reaches 1960 from 2026 in a few turns");
        wheel(&list, 60.0);
    }
    assert!(turns >= 8, "66 years are not on one page ({turns} notches)");
    t.click(&w, &t.find(&w, "1960年"));
    assert!(of_role("1960年", R::Button).accessible_label().is_some() && has("10月"), "the months of 1960");
    t.click(&w, &t.find(&w, "10月"));
    let days = of_role("1960年10月", R::Table);
    // over the days the wheel turns the months, a notch a month; over the months the years
    wheel(&days, -60.0);
    assert!(has("1960年11月"), "wheel down: the next month");
    wheel(&days, 60.0);
    wheel(&days, 60.0);
    assert!(has("1960年9月"), "wheel up twice: back two months");
    // small steps (a touchpad) add up to one
    for _ in 0..4 {
        wheel(&days, -12.0);
    }
    assert!(has("1960年10月") && !has("1960年11月"), "four small steps make one month");
    t.click(&w, &of_role("1960年10月", R::Button));
    wheel(&t.find(&w, "6月"), 60.0);
    assert!(has("1959年"), "over the months: the year before");
    wheel(&t.find(&w, "6月"), -60.0);
    assert!(has("1960年"));
    t.click(&w, &t.find(&w, "10月"));
    // 今天: back to this month, nothing chosen; then 1960 again by the keys' way of a year
    t.click(&w, &t.find(&w, "回到今天"));
    assert!(has(&format!("{ty}年{tm}月")) && has(&format!("{ty}年{tm}月{td}")), "今天 shows this month");
    assert_eq!(w.get_data().start, "2026-11-19", "今天 doesn't choose a day");
    t.key(&w, &slint::SharedString::from(Key::PageDown));
    let next = fluentcal_core::date::add_months(&fluentcal_core::date::start_of_month(&today), 1);
    let (ny, nm, _) = fluentcal_core::date::parts(&next);
    assert!(has(&format!("{ny}年{nm}月")), "PageDown: the next month");
    // choose the day in 1960
    t.click(&w, &of_role(&format!("{ny}年{nm}月"), R::Button));
    t.click(&w, &of_role(&format!("{ny}年"), R::Button));
    // the arrows: four rows of years a click
    t.click(&w, &t.find(&w, "更早的年份"));
    assert!(year_cell(&format!("{}年", ny - 16)).is_some(), "an arrow: 16 years");
    let list = of_role("选择年份", R::List);
    for _ in 0..40 {
        if year_cell("1960年").is_some() {
            break;
        }
        wheel(&list, 60.0);
    }
    t.click(&w, &t.find(&w, "1960年"));
    t.click(&w, &t.find(&w, "10月"));
    t.click(&w, &t.find(&w, "1960年10月1"));
    assert_eq!(w.get_data().start, "1960-10-01", "the day is chosen and the calendar closes");
    assert!(!has("回到今天"), "the calendar closed");
    t.ctrl(&w, "s");
    assert_eq!(t.task("a").unwrap()["start"], "1960-10-01");
}

/// GitHub #15: ticking 已完成 shows 完成日期 with today in it; another day can be put there (not
/// one that has not come), and unticking takes the row away again.
#[test]
fn done_day_in_the_editor() {
    use fluentcal_core::date::{add_days, add_months, parts, start_of_month};
    let t = start("done-on", &[json!({ "id": "a", "title": "看望小猫", "start": "2026-11-10", "end": "2026-11-12" })]);
    let today = t.today();
    let w = t.open(|a| a.open_editor_task("a", None));
    let has = |label: &str| ElementHandle::find_by_accessible_label(&w, label).next().is_some();
    let calendars = || ElementHandle::find_by_accessible_label(&w, "打开日历").count();
    let day = |d: &str| {
        let (y, m, dd) = parts(d);
        format!("{y}年{m}月{dd}")
    };
    assert_eq!(calendars(), 2, "from and to");
    t.click(&w, &t.find(&w, "已完成"));
    assert_eq!((w.get_data().done, w.get_data().done_on.to_string(), calendars()), (true, today.clone(), 3), "完成日期 comes with today in it");
    // its calendar: tomorrow can't be chosen, a day last month can
    t.click(&w, &ElementHandle::find_by_accessible_label(&w, "打开日历").last().unwrap());
    let tomorrow = t.find(&w, &day(&add_days(&today, 1)));
    assert_eq!(tomorrow.accessible_enabled(), Some(false));
    t.click(&w, &tomorrow);
    assert!(has("回到今天"), "still open");
    assert_eq!(w.get_data().done_on, today.as_str());
    t.click(&w, &t.find(&w, "上个月"));
    let (py, pm, _) = parts(&add_months(&start_of_month(&today), -1));
    let picked = format!("{py:04}-{pm:02}-15");
    t.click(&w, &t.find(&w, &day(&picked)));
    assert!(!has("回到今天"), "a day closes the calendar");
    assert_eq!(w.get_data().done_on, picked.as_str());
    // unticked: the row is gone; ticked again: today
    t.click(&w, &t.find(&w, "已完成"));
    assert_eq!((w.get_data().done_on.to_string(), calendars()), (String::new(), 2));
    t.click(&w, &t.find(&w, "已完成"));
    assert_eq!(w.get_data().done_on, today.as_str());
    t.click(&w, &ElementHandle::find_by_accessible_label(&w, "打开日历").last().unwrap());
    t.click(&w, &t.find(&w, "上个月"));
    t.click(&w, &t.find(&w, &day(&picked)));
    t.ctrl(&w, "s");
    let saved = t.task("a").unwrap();
    assert_eq!((saved["done"].clone(), saved["doneOn"].clone()), (json!(true), json!(picked)));
}

/// GitHub #14: 分组 offers the preset groups (生日、工作、还款、纪念日) that aren't there yet; one is
/// made when it is chosen, and the task is in it.
#[test]
fn preset_group_from_the_editor() {
    let t = start("preset-group", &[json!({ "id": "a", "title": "妈妈生日", "start": "2026-11-10" })]);
    let w = t.open(|a| a.open_editor_task("a", None));
    let groups = || with(|a| a.doc.groups.values().map(|g| (g.name.clone(), g.color.clone(), g.id.clone())).collect::<Vec<_>>()).unwrap();
    assert!(groups().is_empty());
    use slint::Model;
    let choices: Vec<String> = w.get_groups().iter().map(|o| o.label.to_string()).collect();
    assert_eq!(choices, ["未分组", "生日", "工作", "还款", "纪念日", "新建分组…"]);
    // chosen from the list (the box shows the group now chosen: 未分组)
    t.click(&w, &t.find(&w, "未分组"));
    let option = ElementHandle::find_by_accessible_label(&w, "生日").find(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::ListItem)).expect("the choice");
    t.click(&w, &option);
    let made = groups();
    assert_eq!(made.iter().map(|(n, c, _)| (n.as_str(), c.as_str())).collect::<Vec<_>>(), [("生日", "pink")]);
    assert_eq!(w.get_data().group, made[0].2.as_str(), "the task is in it");
    let choices: Vec<String> = w.get_groups().iter().map(|o| o.label.to_string()).collect();
    assert_eq!(choices, ["未分组", "生日", "工作", "还款", "纪念日", "新建分组…"], "生日 is a group now, once");
    t.ctrl(&w, "s");
    assert_eq!(t.task("a").unwrap()["groupId"], json!(made[0].2));
}
