//! Import, the data folder (with the restart), cloud sync's test/enable/now.

use fluentcal_core::i18n::{tr, trf};

use super::actions::job;
use super::commands::plan_edit;
use crate::app::{App, ORIGIN};
use crate::Severity;
use fluentcal_core::cloud::engine::Probe;
use fluentcal_core::sync_text::uploaded_by;
use serde_json::{json, Value};
use std::time::Duration;

impl App {
    /// 设置 › 数据 › 从备份文件导入: merge (new and newer tasks) or replace, backed up first.
    pub fn settings_import(&mut self) {
        if let Some(path) = self.settings_win.as_ref().and_then(|s| s.local.test_pick.clone()) {
            return self.settings_import_file(&path);
        }
        let owner = self.settings_win.as_ref().and_then(|s| crate::platform::window::hwnd(slint::ComponentHandle::window(&s.window)));
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(p) = crate::platform::dialog::open_file(owner, tr("FluentCal 备份"), &["json"]) {
                crate::app::run(move |app| app.settings_import_file(&p));
            }
        });
    }

    fn settings_import_file(&mut self, path: &str) {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => return self.settings_say(Severity::Error, &e.to_string()),
        };
        let incoming = match fluentcal_core::export::parse_backup(text.trim_start_matches('\u{FEFF}')) {
            Ok(d) => d,
            Err(e) => return self.settings_say(Severity::Error, &e),
        };
        let n = incoming.get("tasks").and_then(Value::as_object).map_or(0, |m| m.len());
        let plan = fluentcal_core::export::plan_merge(&self.store.snapshot(), &incoming);
        let content = trf("文件中有 {0} 个任务。合并：加入 {1} 个新任务，{2} 个在文件中更新过的任务用文件版本，其余 {3} 个保留本机版本。替换：用文件内容覆盖全部任务。导入前会自动备份当前数据。", &[format!("{}", n), format!("{}", plan.added), format!("{}", plan.updated), format!("{}", plan.kept)]);
        self.settings_confirm(tr("导入备份"), &content, tr("合并"), tr("替换"), false, move |app, r| {
            if r == "close" {
                return;
            }
            if r == "primary" {
                if let Err(e) = app.store.backup_create() {
                    return app.settings_say(Severity::Error, &trf("导入前备份失败，已取消导入：{0}", &[format!("{}", e)]));
                }
                app.apply(plan_edit(&plan), false);
                app.settings_say(Severity::Success, &trf("已合并：新增 {0} 个，更新 {1} 个，保留本机 {2} 个。", &[format!("{}", plan.added), format!("{}", plan.updated), format!("{}", plan.kept)]));
            } else {
                // The file's tasks, groups and day colours; this PC's settings and holidays.
                let cur = app.store.snapshot();
                let mut doc = incoming.clone();
                if let Some(o) = doc.as_object_mut() {
                    o.insert("settings".into(), cur.get("settings").cloned().unwrap_or(json!({})));
                    o.insert("holidays".into(), cur.get("holidays").cloned().unwrap_or(json!({})));
                }
                // store.import backs the current data up first.
                if let Err(e) = app.store.import(ORIGIN, doc) {
                    return app.settings_say(Severity::Error, &trf("导入失败：{0}", &[format!("{}", e)]));
                }
                app.reload();
                app.settings_say(Severity::Success, &trf("已用备份替换，共 {0} 个任务。", &[format!("{}", n)]));
            }
            app.settings_load_backups();
            app.settings_refresh();
        });
    }

    /// 更改… / 恢复默认: the data moves (merged with what is there), then the program restarts.
    pub fn settings_change_dir(&mut self, reset: bool) {
        let picked = self.settings_win.as_ref().and_then(|s| s.local.test_pick.clone());
        if reset || picked.is_some() {
            return self.settings_confirm_dir(if reset { None } else { picked });
        }
        let owner = self.settings_win.as_ref().and_then(|s| crate::platform::window::hwnd(slint::ComponentHandle::window(&s.window)));
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(p) = crate::platform::dialog::pick_folder(owner) {
                crate::app::run(move |app| app.settings_confirm_dir(Some(p)));
            }
        });
    }

    fn settings_confirm_dir(&mut self, path: Option<String>) {
        let (title, content, primary) = match &path {
            None => (tr("恢复默认数据文件夹？"), tr("数据会合并回本机的默认文件夹，然后程序重新启动。").to_owned(), tr("恢复并重启")),
            Some(p) => (
                tr("更改数据文件夹？"),
                trf("数据将保存到：{0}\n\n放在 OneDrive 等同步盘里可以多台电脑共用；文件夹里已有的 FluentCal 数据会与本机数据合并（按每个任务的最后修改时间），不会被覆盖。设置（外观、位置等）仍保存在每台电脑上。程序会重新启动。", &[format!("{}", p)]),
                tr("更改并重启"),
            ),
        };
        self.settings_confirm(title, &content, primary, "", false, move |app, r| {
            if r != "primary" {
                return;
            }
            match fluentcal_core::datadir::set(&app.local_dir, &app.store, path) {
                Ok(()) => app.restart(),
                Err(e) => app.settings_say(Severity::Error, &trf("更改失败：{0}", &[format!("{}", e)])),
            }
        });
    }

    /// Starts a new copy (it waits for this one to exit: `--restart`) and quits.
    pub fn restart(&mut self) {
        if let Some(s) = &mut self.settings_win {
            s.local.restart_asked = true;
        }
        if crate::platform::msgwin::hwnd().is_none() {
            return; // tests: no program to restart
        }
        let Ok(exe) = std::env::current_exe() else { return };
        if let Err(e) = std::process::Command::new(exe).arg("--restart").spawn() {
            self.settings_say(Severity::Error, &trf("无法重新启动：{0}", &[format!("{}", e)]));
            return;
        }
        crate::startup::quit(self);
    }

    pub fn settings_sync_now(&mut self) {
        if let Some(s) = &mut self.settings_win {
            s.local.syncing = true;
        }
        self.settings_refresh();
        let cloud = self.cloud.clone();
        job(
            move || {
                let r = cloud.sync_now();
                (r.err().map(|e| e.message), cloud.status())
            },
            |app, (_, status)| {
                app.sync = status;
                if let Some(s) = &mut app.settings_win {
                    s.local.syncing = false;
                }
                app.desk_refresh_status();
                app.settings_refresh();
            },
        );
    }

    /// 测试连接 / 开启同步: probes the cloud with the current settings first.
    pub fn settings_probe(&mut self, then_enable: bool) {
        self.settings_busy(true);
        let cloud = self.cloud.clone();
        job(
            move || cloud.probe().map_err(|e| e.message),
            move |app, res: Result<Probe, String>| {
                app.settings_busy(false);
                let p = match res {
                    Ok(p) => p,
                    Err(e) => return app.settings_say(Severity::Error, &trf("连接失败：{0}", &[format!("{}", e)])),
                };
                if !then_enable {
                    let text = app.settings_describe(&p);
                    return app.settings_say(Severity::Success, &text);
                }
                let local = app.doc.tasks.len();
                if p.exists && p.tasks > 0 && local > 0 {
                    let (now, off) = (fluentcal_core::date::now_ms(), fluentcal_core::date::utc_offset_minutes());
                    let by = uploaded_by(p.device.as_deref(), p.saved_at, now, off);
                    let by = if by.is_empty() { String::new() } else { format!("（{by}）") };
                    let content = trf("云端：{0} 个任务{1}\n本机：{2} 个任务\n\n“合并”保留两边的任务，同一个任务取最后修改的一方。\n“用云端数据”会替换本机的任务（适合刚装好的新电脑）。\n两种方式都会先备份本机数据。", &[format!("{}", p.tasks), format!("{}", by), format!("{}", local)]);
                    app.settings_confirm(tr("本机和云端都有数据"), &content, tr("合并"), tr("用云端数据"), false, |app, r| match r {
                        "primary" => app.settings_enable(false),
                        "secondary" => app.settings_enable(true),
                        _ => {}
                    });
                } else {
                    app.settings_enable(false);
                }
            },
        );
    }

    fn settings_describe(&self, p: &Probe) -> String {
        if !p.exists {
            return trf("连接成功。云端还没有数据，开启后会上传本机的 {0} 个任务。", &[format!("{}", self.doc.tasks.len())]);
        }
        let by = uploaded_by(p.device.as_deref(), p.saved_at, fluentcal_core::date::now_ms(), fluentcal_core::date::utc_offset_minutes());
        trf("连接成功。云端已有 {0} 个任务、{1} 个分组{2}。", &[format!("{}", p.tasks), format!("{}", p.groups), format!("{}", if by.is_empty() { String::new() } else { format!("，{by}") })])
    }

    fn settings_busy(&mut self, busy: bool) {
        if let Some(s) = &mut self.settings_win {
            s.local.busy = busy;
        }
        self.settings_refresh();
    }

    fn settings_enable(&mut self, replace: bool) {
        self.settings_busy(true);
        let cloud = self.cloud.clone();
        job(
            move || {
                let r = cloud.enable(replace).map_err(|e| e.message);
                (r.map(|_| ()), cloud.status())
            },
            move |app, (res, status)| {
                app.settings_busy(false);
                match res {
                    Ok(()) => {
                        app.sync = status;
                        app.patch_settings(json!({ "sync": { "enabled": true } }));
                        if replace {
                            app.reload();
                        }
                        app.settings_say(Severity::Success, if replace { tr("已开启同步，本机任务已换成云端数据。") } else { tr("已开启同步。") });
                    }
                    Err(e) => app.settings_say(Severity::Error, &trf("开启失败：{0}", &[format!("{}", e)])),
                }
            },
        );
    }
}
