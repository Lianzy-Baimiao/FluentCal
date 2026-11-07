//! Reminder actions must remove the visible window, not only its entry in App.
//! Exercises real card hit-testing, callbacks, scheduling and task persistence.

use fluentcal_app::{app, reminders::{Reminder, Reminders, Style}, ReminderWindow};
use i_slint_backend_testing::{ElementHandle, TestingBackend, TestingBackendOptions};
use serde_json::json;
use slint::{platform::{PointerEventButton, WindowEvent}, ComponentHandle, LogicalPosition, Model};
use std::{path::PathBuf, time::{Duration, Instant}};

struct TestApp { dir: PathBuf }

impl TestApp {
    fn start(name: &str) -> Self {
        slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions {
            mock_time: false, threading: false, ..Default::default()
        }))).unwrap();
        let dir = std::env::temp_dir().join(format!("fluentcal-reminder-{name}-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let today = fluentcal_core::date::today();
        std::fs::write(dir.join("data.json"), json!({
            "version": 1, "tasks": {
                "task": {"id": "task", "title": "提醒回归测试", "start": today, "end": today,
                    "done": false, "progress": 0, "order": 1, "createdAt": 1, "updatedAt": 1}
            }, "groups": {}, "dayMarks": {}, "holidays": {},
            "settings": {"general": {"seeded": true, "onboarded": true, "language": "zh-CN"},
                "holidays": {"autoUpdate": false}}
        }).to_string()).unwrap();
        let mut a = fluentcal_app::startup::build(dir.clone()).unwrap();
        // Use the real scheduler without Windows sounds/toasts or a cross-thread UI post.
        a.reminders = Reminders::start(&dir, |_, _| {});
        app::install(a);
        Self { dir }
    }

    fn fire(&self, keys: &[&str]) -> ReminderWindow {
        app::with(|a| {
            a.reminders.set(keys.iter().map(|key| Reminder {
                key: (*key).into(), at_ms: fluentcal_core::date::now_ms(),
                title: "提醒回归测试".into(), body: "今天 02:15".into(),
                task_id: "task".into(), date: a.today(),
            }).collect(), Style::Popup);
        }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while app::with(|a| a.reminders.popup_list().len()).unwrap() != keys.len() {
            assert!(Instant::now() < deadline, "scheduler did not fire");
            std::thread::sleep(Duration::from_millis(5));
        }
        app::with(|a| a.reminder_cards_changed()).unwrap();
        settle();
        let w = app::with(|a| a.reminder_win.as_ref().unwrap().clone_strong()).unwrap();
        assert!(w.window().is_visible());
        w
    }

    fn done(&self) -> bool {
        app::with(|a| a.store.snapshot()["tasks"]["task"]["done"].as_bool().unwrap()).unwrap()
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(a) = app::uninstall() { a.store.flush(); }
        assert_eq!(self.dir.canonicalize().unwrap().parent(), Some(std::env::temp_dir().canonicalize().unwrap().as_path()));
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

fn settle() {
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(10));
        slint::platform::update_timers_and_animations();
    }
}

fn click(w: &ReminderWindow, label: &str) {
    let e = ElementHandle::find_by_accessible_label(w, label).next().expect(label);
    let p = e.absolute_position();
    let size = e.size();
    assert!(size.width > 0.0 && size.height > 0.0, "button must be laid out");
    let position = LogicalPosition::new(p.x + size.width / 2.0, p.y + size.height / 2.0);
    w.window().dispatch_event(WindowEvent::PointerMoved { position });
    w.window().dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    w.window().dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    settle();
}

fn assert_closed(w: &ReminderWindow) {
    assert!(app::with(|a| a.reminders.popup_list().is_empty()).unwrap(), "reminder state must be cleared");
    assert!(app::with(|a| a.reminder_win.is_none()).unwrap(), "App must release the window");
    assert!(!w.window().is_visible(), "reminder window is still visible after its last card was removed");
}

#[test]
fn close_button_hides_last_card_and_next_reminder_can_open() {
    let t = TestApp::start("close");
    let w = t.fire(&["first"]);
    click(&w, "关闭提醒");
    assert_closed(&w);
    assert!(!t.done(), "dismiss must not complete the task");
    let next = t.fire(&["next"]);
    assert!(!w.window().is_visible(), "old window must stay hidden");
    click(&next, "关闭提醒");
    assert_closed(&next);
}

#[test]
fn done_button_persists_completion_and_hides_last_card() {
    let t = TestApp::start("done");
    let w = t.fire(&["done"]);
    click(&w, "完成");
    assert!(t.done(), "completion must reach the store");
    assert_closed(&w);
}

#[test]
fn snooze_button_persists_schedule_and_hides_last_card() {
    let t = TestApp::start("snooze");
    let w = t.fire(&["snooze"]);
    let before = fluentcal_core::date::now_ms();
    click(&w, "10 分钟后提醒");
    let snoozed: Vec<Reminder> = serde_json::from_str(&std::fs::read_to_string(t.dir.join("snoozed-reminders.json")).unwrap()).unwrap();
    assert_eq!(snoozed.len(), 1);
    assert!(snoozed[0].at_ms >= before + 10 * 60_000);
    assert!(!t.done());
    assert_closed(&w);
}

#[test]
fn closing_one_card_keeps_others_until_last_is_closed() {
    let t = TestApp::start("stack");
    let w = t.fire(&["older", "newer"]);
    click(&w, "关闭提醒");
    assert_eq!(w.get_items().row_count(), 1);
    assert!(w.window().is_visible());
    assert_eq!(app::with(|a| a.reminders.popup_list().len()).unwrap(), 1);
    click(&w, "关闭提醒");
    assert_closed(&w);
    assert!(!t.done());
}
