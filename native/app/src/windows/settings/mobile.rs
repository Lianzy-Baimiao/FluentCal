use crate::{app::{run, App}, Severity};
use fluentcal_core::{cloud::secrets, i18n::{tr, trf}, pairing};
use std::time::Duration;

impl App {
    pub fn settings_pair_cancel(&mut self) {
        if let Some(s) = &mut self.settings_win {
            s.local.pairing = None; s.local.pair_qr = None; s.local.pair_timer.stop();
        }
        self.settings_refresh();
    }
    pub fn settings_pair_start(&mut self) {
        self.settings_pair_cancel();
        let addresses: Vec<_> = crate::platform::net::local_addresses().into_iter().map(|(ip, label, _)| (ip, label)).collect();
        if addresses.is_empty() {
            self.settings_say(Severity::Error, tr("未找到局域网地址，请先连接 Wi-Fi 或有线网络")); return;
        }
        let snap = self.store.snapshot();
        let provider = snap["settings"]["sync"]["provider"].as_str().unwrap_or("");
        let secret = secrets::get(provider).unwrap_or_default();
        let result = pairing::payload(&snap["settings"]["sync"], &secret, &fluentcal_core::cloud::device_name())
            .and_then(|data| pairing::start(&data, "0.0.0.0:0", pairing::TTL));
        match result {
            Ok(session) => {
                if let Some(s) = &mut self.settings_win {
                    s.local.pair_ip = addresses[0].0.clone();
                    s.local.pair_addresses = addresses;
                    s.local.pairing = Some(session);
                    s.local.pair_timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), || run(|a| a.settings_refresh()));
                    let ip = s.local.pair_ip.clone();
                    self.settings_pair_address(&ip);
                }
            }
            Err(_) => self.settings_say(Severity::Error, tr("无法生成配对码，请确认已开启云同步、WebDAV 使用 HTTPS，并检查网络权限")),
        }
    }
    pub fn settings_pair_address(&mut self, ip: &str) {
        let scale = self.settings_win.as_ref().map_or(1.0, |s| super::view::scale_of(&s.window, self));
        if let Some(s) = &mut self.settings_win {
            if !s.local.pair_addresses.iter().any(|(addr, _)| addr == ip) { return; }
            if let Some(session) = &s.local.pairing {
                if let Ok(qr) = session.qr(ip) {
                    s.local.pair_ip = ip.into();
                    s.local.pair_qr = Some(super::view::qr_image(&qr, (240.0 * scale).round() as u32));
                }
            }
        }
        self.settings_refresh();
    }
    pub fn settings_check_store_update(&mut self) {
        let Some(s) = &mut self.settings_win else { return };
        if s.local.update_checking { return; }
        s.local.update_checking = true;
        s.local.update_message = tr("正在检查 Microsoft Store 更新…").into();
        let hwnd = crate::platform::window::hwnd(slint::ComponentHandle::window(&s.window)).map_or(0, |h| h.0 as isize);
        self.settings_refresh();
        super::actions::job(move || crate::platform::package::check_store_update(hwnd), |app, result| {
            if let Some(s) = &mut app.settings_win {
                s.local.update_checking = false;
                s.local.update_message = match result {
                    Ok(Some(0)) => tr("Microsoft Store 暂无可用更新").into(),
                    Ok(Some(n)) => trf("发现 {0} 项商店更新，请打开 Microsoft Store 安装", &[n.to_string()]),
                    Ok(None) => tr("当前不是 Microsoft Store 安装包，请打开商店安装或查看版本").into(),
                    Err(_) => tr("无法检查商店更新，请检查网络和 Microsoft Store 登录状态后重试").into(),
                };
            }
            app.settings_refresh();
        });
    }
}
