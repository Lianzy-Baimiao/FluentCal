use fluentcal_app::{app::{with, uninstall}, SetState};
use i_slint_backend_testing::{ElementHandle, TestingBackend, TestingBackendOptions};
use serde_json::{json, Value};
use slint::{ComponentHandle, Model};

fn settle() {
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        slint::platform::update_timers_and_animations();
    }
}

#[test]
fn switches_open_windows_preserves_input_and_restores_the_saved_language() {
    slint::platform::set_platform(Box::new(TestingBackend::new(TestingBackendOptions { mock_time: false, threading: false, ..Default::default() }))).unwrap();
    let dir = std::env::temp_dir().join(format!("fluentcal-i18n-{}-{}", std::process::id(), fluentcal_core::date::now_ms()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("data.json"), json!({"version":1,"tasks":{},"groups":{},"settings":{"general":{"language":"en-US","seeded":true,"onboarded":true},"holidays":{"autoUpdate":false}}}).to_string()).unwrap();
    let desk = fluentcal_app::startup::test_app(dir.clone(), 900.0, 640.0).unwrap();
    with(|a| { a.open_settings(Some("general")); a.open_editor_new(None); }).unwrap();
    settle();
    let settings = with(|a| a.settings_win.as_ref().unwrap().window.clone_strong()).unwrap();
    let editor = with(|a| a.editor_window().unwrap()).unwrap();
    assert!(ElementHandle::find_by_accessible_label(&settings, "Display language").next().is_some());
    assert_eq!(desk.get_heads().row_data(0).unwrap().label, "Mon");
    editor.set_title_text("任务 {1} / Weekly plan".into());
    editor.invoke_edit("title".into(), "任务 {1} / Weekly plan".into());
    editor.set_notes_text("保留原文 🌏".into());
    editor.invoke_edit("notes".into(), "保留原文 🌏".into());
    desk.set_quick_add_text("正在输入".into());
    settings.global::<SetState>().invoke_set_str("general.language".into(), "zh-TW".into());
    settle();
    assert_eq!(settings.global::<SetState>().get_general().language, "zh-TW");
    assert!(ElementHandle::find_by_accessible_label(&settings, "顯示語言").next().is_some());
    assert_eq!(editor.get_title_text(), "任务 {1} / Weekly plan");
    assert_eq!(editor.get_notes_text(), "保留原文 🌏");
    assert_eq!(desk.get_quick_add_text(), "正在输入");
    assert!(editor.get_freqs().iter().any(|f| f.label == "每週"));
    settings.global::<SetState>().invoke_set_str("general.language".into(), "en-US".into());
    settle();
    assert!(editor.get_freqs().iter().any(|f| f.label == "Weekly"));
    assert_eq!(editor.get_title_text(), "任务 {1} / Weekly plan");
    with(|a| a.store.flush()).unwrap();
    let persisted: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("data.json")).unwrap()).unwrap();
    assert_eq!(persisted["settings"]["general"]["language"], "en-US");
    drop(editor); drop(settings); drop(desk);
    let old = uninstall().unwrap(); old.store.flush(); drop(old);
    let next = fluentcal_app::startup::test_app(dir.clone(), 900.0, 640.0).unwrap();
    settle();
    assert_eq!(next.get_heads().row_data(0).unwrap().label, "Mon");
    assert_eq!(with(|a| a.settings().general.language.clone()).unwrap(), "en-US");
    with(|a| a.patch_settings(json!({"general":{"language":"zh-CN"}}))).unwrap();
    settle();
    assert_eq!(next.get_heads().row_data(0).unwrap().label, "一");
    with(|a| a.patch_settings(json!({"general":{"language":"system"}}))).unwrap();
    settle();
    assert_eq!(fluentcal_core::i18n::current(), fluentcal_core::i18n::resolve("system", &fluentcal_app::i18n::system_language()));
    drop(next); let old = uninstall().unwrap(); old.store.flush(); drop(old);
    std::fs::remove_dir_all(dir).unwrap();
}
