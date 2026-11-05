//! The pages' buttons (SetState.action / action-arg): data, backups, sync, web, holidays.

use fluentcal_core::i18n::{tr, trf};

use super::actions::job;
use crate::app::{run, App, ORIGIN};
use crate::Severity;
use fluentcal_core::settings_page as sp;
use fluentcal_core::tasks::{self, Edit};
use serde_json::{json, Value};
use std::time::Duration;

const SUCCESS: Severity = Severity::Success;
const ERROR: Severity = Severity::Error;

impl App {
    pub fn settings_action(&mut self, key: &str, arg: &str) {
        match key {
            "pair-start" => self.settings_pair_start(),
            "pair-cancel" => self.settings_pair_cancel(),
            "pair-address" => self.settings_pair_address(arg),
            "mobile-download" => crate::webcmd::open_url(fluentcal_core::pairing::MOBILE_DOWNLOAD),
            "store-check-update" => self.settings_check_store_update(),
            "store-open" => crate::platform::package::open_store(),
            "msg-close" => {
                if let Some(s) = &mut self.settings_win {
                    s.local.msg = None;
                }
                self.settings_refresh();
            }
            "desk-hide" => {
                let _ = slint::ComponentHandle::hide(&self.desk.window);
                crate::shell::tray_refresh(self);
            }
            "move-to-screen" => {
                if let Ok(i) = arg.parse::<usize>() {
                    crate::shell::move_to_screen(self, i);
                    self.settings_refresh();
                }
            }
            "check-holidays" => self.settings_check_holidays(),
            "notify-test" => match crate::platform::toast::show(tr("FluentCal 提醒测试"), tr("如果你看到这条通知，任务提醒可以正常工作。")) {
                Ok(()) => self.settings_say(Severity::Info, tr("已发送测试通知。如果没有看到，请检查 Windows 设置 › 系统 › 通知。")),
                Err(e) => self.settings_say(ERROR, &trf("发送失败：{0}", &[format!("{}", e)])),
            },
            "add-offset" => {
                let next = sp::add_offset(&self.doc.settings.reminders.default_offsets);
                self.patch_settings(json!({ "reminders": { "defaultOffsets": next } }));
            }
            "preset" => {
                if let Some(p) = sp::preset_patch(arg) {
                    self.patch_settings(p);
                }
            }
            "delete-group" => self.open_group_delete(crate::group_delete::Host::Settings, arg),
            "clear-samples" => self.settings_clear_samples(),
            "export-json" => {
                let text = serde_json::to_string_pretty(&self.settings_export_doc()).unwrap_or_default();
                self.settings_save(tr("FluentCal-备份"), tr("FluentCal 备份"), "json", text.into_bytes());
            }
            "export-csv" => {
                let list: Vec<_> = self.doc.tasks.values().cloned().collect();
                let text = fluentcal_core::export::to_csv(&list, &self.doc.groups);
                // Excel needs the BOM to read UTF-8.
                self.settings_save(tr("FluentCal-任务"), tr("CSV（Excel 可打开）"), "csv", [b"\xEF\xBB\xBF".as_slice(), text.as_bytes()].concat());
            }
            "export-ics" => {
                let list: Vec<_> = self.doc.tasks.values().cloned().collect();
                let text = fluentcal_core::export::to_ics(&list, &self.doc.groups, &self.today(), fluentcal_core::date::now_ms());
                self.settings_save(tr("FluentCal-日历"), tr("iCalendar 日历"), "ics", text.into_bytes());
            }
            "import-json" => self.settings_import(),
            "backup-now" => {
                match self.store.backup_create() {
                    Ok(name) => {
                        self.settings_load_backups();
                        self.settings_say(SUCCESS, &trf("已创建备份 {0}", &[format!("{}", name)]));
                    }
                    Err(e) => self.settings_say(ERROR, &trf("备份失败：{0}", &[format!("{}", e)])),
                }
            }
            "restore" => self.settings_restore(arg),
            "open-data-dir" => crate::platform::dialog::open_folder(&self.store.dir.display().to_string()),
            "change-dir" => self.settings_change_dir(false),
            "reset-dir" => self.settings_change_dir(true),
            "sync-now" => self.settings_sync_now(),
            "sync-disable" => {
                self.settings_pair_cancel();
                self.patch_settings(json!({ "sync": { "enabled": false } }));
                self.settings_say(Severity::Info, tr("已关闭同步。本机数据保留，云端文件也不会删除。"));
            }
            "sync-test" => self.settings_probe(false),
            "sync-enable" => self.settings_probe(true),
            "proxy-mode" => self.patch_settings(json!({ "sync": { "proxy": sp::proxy_for_mode(arg) } })),
            "open-url" => crate::webcmd::open_url(arg),
            "web-open-local" => {
                self.open_in_browser();
                if let Some(e) = self.web.error.clone() {
                    self.settings_say(ERROR, &trf("无法在浏览器中查看：{0}", &[format!("{}", e)]));
                }
            }
            "copy" => match crate::platform::clipboard::set_text(arg) {
                Ok(()) => self.settings_say(SUCCESS, tr("已复制链接。链接里带访问码，只发给自己的设备。")),
                Err(_) => self.settings_say(ERROR, tr("无法复制，请手动选择链接复制")),
            },
            "web-reset-token" => {
                self.settings_confirm(tr("更换访问码？"), tr("已经发出的链接和已打开的网页会立即失效，需要重新扫码。"), tr("更换"), "", false, |app, r| {
                    if r != "primary" {
                        return;
                    }
                    match app.web_reset_token() {
                        Ok(()) => {
                            app.settings_load_web();
                            app.settings_say(Severity::Info, tr("已更换访问码，请用新的链接或二维码打开。"));
                        }
                        Err(e) => app.settings_say(ERROR, &e),
                    }
                });
            }
            _ => eprintln!("[settings] unknown action {key}"),
        }
    }

    fn settings_check_holidays(&mut self) {
        if let Some(s) = &mut self.settings_win {
            s.local.checking = true;
        }
        self.settings_refresh();
        let store = self.store.clone();
        let proxy = self.doc.settings.sync.proxy.clone();
        let year = fluentcal_core::date::parts(&self.today()).0;
        job(
            move || fluentcal_core::holidays::update(&store, year, true, &proxy),
            |app, (updated, failed)| {
                if let Some(s) = &mut app.settings_win {
                    s.local.checking = false;
                }
                app.settings_say(Severity::Info, &sp::holiday_result(&updated, failed));
            },
        );
    }

    fn settings_clear_samples(&mut self) {
        let text = tr("第一次启动时生成的示例任务和示例项目会被删除，你自己添加或改过名称的任务不受影响。");
        self.settings_confirm(tr("清除示例数据？"), text, tr("清除"), "", false, |app, r| {
            if r != "primary" {
                return;
            }
            let (edit, n) = tasks::clear_sample_data(&app.doc, fluentcal_core::date::now_ms());
            app.apply(edit, false);
            app.patch_settings(json!({ "general": { "onboarded": true } }));
            app.settings_say(SUCCESS, &trf("已删除 {0} 个示例任务。", &[format!("{}", n)]));
        });
    }

    /// The whole document as 0.2's exportDoc(): tasks, groups, day colours, holidays and the
    /// settings with their defaults filled in.
    pub fn settings_export_doc(&self) -> Value {
        let mut d = self.store.snapshot();
        if let Some(o) = d.as_object_mut() {
            o.insert("settings".into(), serde_json::to_value(&self.doc.settings).unwrap_or(Value::Null));
        }
        d
    }

    /// 0.2's file_save: asks where (next turn; the dialog is modal), writes, says where.
    fn settings_save(&mut self, name: &str, filter: &str, ext: &str, bytes: Vec<u8>) {
        let file = format!("{name}-{}.{ext}", self.today());
        if let Some(path) = self.local_test_pick() {
            let path = format!("{path}/{file}");
            let res = std::fs::write(&path, &bytes);
            return self.settings_saved(res.map(|()| path).map_err(|e| e.to_string()));
        }
        let owner = self.settings_win.as_ref().and_then(|s| crate::platform::window::hwnd(slint::ComponentHandle::window(&s.window)));
        let (filter, ext) = (filter.to_owned(), ext.to_owned());
        slint::Timer::single_shot(Duration::ZERO, move || {
            let Some(path) = crate::platform::dialog::save_file(owner, &file, &filter, &[ext.as_str()]) else { return };
            let res = std::fs::write(&path, &bytes).map(|()| path).map_err(|e| e.to_string());
            run(move |app| app.settings_saved(res));
        });
    }

    fn settings_saved(&mut self, res: Result<String, String>) {
        match res {
            Ok(p) => self.settings_say(SUCCESS, &trf("已导出到 {0}", &[format!("{}", p)])),
            Err(e) => self.settings_say(ERROR, &trf("导出失败：{0}", &[format!("{}", e)])),
        }
    }

    /// Tests: the folder (export, data folder) or file (import) "picked" without a dialog.
    fn local_test_pick(&self) -> Option<String> {
        self.settings_win.as_ref().and_then(|s| s.local.test_pick.clone())
    }

    fn settings_restore(&mut self, name: &str) {
        let name = name.to_owned();
        self.settings_confirm(tr("从备份恢复？"), &trf("将用 {0} 替换当前全部数据。当前数据会先自动备份。", &[format!("{}", name)]), tr("恢复"), "", true, move |app, r| {
            if r != "primary" {
                return;
            }
            match app.store.backup_restore(ORIGIN, &name) {
                Ok(()) => {
                    app.reload();
                    app.settings_load_backups();
                    app.settings_say(SUCCESS, tr("已恢复。"));
                }
                Err(e) => app.settings_say(ERROR, &trf("恢复失败：{0}", &[format!("{}", e)])),
            }
        });
    }
}

/// The edit that writes a merge plan's items (0.2: put('groups' / 'tasks' / 'dayMarks')).
pub fn plan_edit(plan: &fluentcal_core::export::MergePlan) -> Edit {
    let mut e = Edit::default();
    for (col, items) in [("groups", &plan.groups), ("tasks", &plan.tasks), ("dayMarks", &plan.day_marks)] {
        for (id, v) in items {
            e.put(col, id, v.clone());
        }
    }
    e
}
