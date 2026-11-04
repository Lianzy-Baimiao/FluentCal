//! What the settings pages show (0.2's computed values in the pages' scripts): from the
//! settings, the document and `Info` (the app's state, or a parity scene's stand-in).

use fluentcal_core::i18n::{tr, trf};

use crate::{SAbout, SAddress, SAppearance, SBackup, SCalendar, SData, SGantt, SGeneral, SGroup, SMsg, SReminders, SSync, SUpcoming, SWeb, SelectOption, SetState, SettingsWindow, Severity, Swatch};
use fluentcal_core::cloud::service::Status;
use fluentcal_core::datadir::DataDir;
use fluentcal_core::settings_page as sp;
use fluentcal_core::store::BackupInfo;
use fluentcal_core::tasks::{self, Doc};
use fluentcal_core::theme::{best_text_on, Rgba, TASK_COLORS, WIN_ACCENTS};
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, VecModel};

/// The web view's state for the 网页查看 page.
#[derive(Clone, Default)]
pub struct WebView {
    pub enabled: bool,
    pub running: bool,
    pub port: u16,
    pub error: Option<String>,
    /// (ip, adapter, virtual, link)
    pub addresses: Vec<(String, String, bool, String)>,
    /// The QR code of the first link, and that link.
    pub qr: Option<(String, Image)>,
}

/// The window's own state (0.2's refs in the pages): the page, its message, what is busy,
/// and slow lookups made when a page opens.
#[derive(Default)]
pub struct Local {
    pub page: String,
    /// (severity, title, text)
    pub msg: Option<(Severity, String, String)>,
    pub checking: bool,
    pub busy: bool,
    pub syncing: bool,
    pub backups: Vec<BackupInfo>,
    /// Saved secrets ("webdav", "github", "gitee").
    pub secrets: Vec<String>,
    pub web: WebView,
    /// Tests: file/folder picked instead of showing a dialog (export, import, data folder).
    pub test_pick: Option<String>,
    /// Data folder changed: a restart was asked for (tests check it; the app restarts).
    pub restart_asked: bool,
    pub web_timer: slint::Timer,
    /// 删除分组 is open: the ticked groups (crate::group_delete).
    pub group_delete: Option<Vec<String>>,
}

/// The app's state the pages show.
pub struct Info {
    pub version: String,
    pub build: u32,
    pub debug: bool,
    /// Windows' "透明效果" (None: unknown).
    pub transparency: Option<bool>,
    pub system_accent: Option<String>,
    pub shortcut_errors: Vec<String>,
    pub data_dir: DataDir,
    pub sync: Status,
    /// (index as text, title, current)
    pub screens: Vec<(String, String, bool)>,
    pub holiday_years: Vec<String>,
    pub now_ms: i64,
    pub offset: i32,
}

fn swatch(key: &str, hex: &str, title: &str) -> Swatch {
    let c = Rgba::hex(hex);
    Swatch { key: key.into(), color: crate::theme::color(c), fg: crate::theme::color(best_text_on(c)), title: title.into(), selected: false, none: false }
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

/// Replaces a list in place, row by row (rows that didn't change keep their elements, so a
/// name being typed in a group's box survives a refresh).
fn update<T: Clone + PartialEq + 'static>(m: ModelRc<T>, rows: Vec<T>, set: impl FnOnce(ModelRc<T>)) {
    let Some(vm) = m.as_any().downcast_ref::<VecModel<T>>() else {
        set(model(rows));
        return;
    };
    for (i, r) in rows.iter().enumerate() {
        if i < vm.row_count() {
            if vm.row_data(i).as_ref() != Some(r) {
                vm.set_row_data(i, r.clone());
            }
        } else {
            vm.push(r.clone());
        }
    }
    while vm.row_count() > rows.len() {
        vm.remove(vm.row_count() - 1);
    }
}

/// The app icon (About page).
fn icon() -> Image {
    let bytes: &[u8] = include_bytes!("../../../res/icon-128.png");
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let Ok(mut reader) = dec.read_info() else { return Image::default() };
    let mut data = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let Ok(info) = reader.next_frame(&mut data) else { return Image::default() };
    if info.color_type != png::ColorType::Rgba {
        return Image::default();
    }
    let buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&data[..info.buffer_size()], info.width, info.height);
    Image::from_rgba8(buf)
}

/// A QR code (0.2's QrCode.vue: uqr, ECC M, 2-module border, dark on white) as an image of
/// `px` × `px` device pixels.
pub fn qr_image(text: &str, px: u32) -> Image {
    use qrcodegen::{QrCode, QrCodeEcc, QrSegment, Version};
    let segs = QrSegment::make_segments(text);
    let Ok(code) = QrCode::encode_segments_advanced(&segs, QrCodeEcc::Medium, Version::MIN, Version::MAX, None, false) else {
        return Image::default();
    };
    let n = code.size() + 4;
    let px = px.max(n as u32);
    let mut buf = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(px, px);
    let data = buf.make_mut_slice();
    for y in 0..px {
        let my = ((y as f64 + 0.5) * n as f64 / px as f64) as i32 - 2;
        for x in 0..px {
            let mx = ((x as f64 + 0.5) * n as f64 / px as f64) as i32 - 2;
            let v = if code.get_module(mx, my) { 0 } else { 255 };
            data[(y * px + x) as usize] = slint::Rgb8Pixel { r: v, g: v, b: v };
        }
    }
    Image::from_rgb8(buf)
}

/// A CSS linear-gradient(`deg`, stops…) as a `w`×`h` image with `radius` px rounded corners
/// (the software renderer draws hard stops and some 3-stop gradients wrongly). Stops are
/// (position 0..1, "#rrggbb").
pub fn gradient_image(w: u32, h: u32, radius: f64, deg: f64, stops: &[(f64, &str)]) -> Image {
    let (w, h) = (w.max(1), h.max(1));
    let a = deg.to_radians();
    let (dx, dy) = (a.sin(), -a.cos());
    let (fw, fh) = (f64::from(w), f64::from(h));
    let len = (fw * dx).abs() + (fh * dy).abs();
    let cols: Vec<(f64, Rgba)> = stops.iter().map(|(p, c)| (*p, Rgba::hex(c))).collect();
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
    let data = buf.make_mut_slice();
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let t = (((px - fw / 2.0) * dx + (py - fh / 2.0) * dy) / len + 0.5).clamp(0.0, 1.0);
            let i = cols.iter().position(|(p, _)| *p > t).unwrap_or(cols.len());
            let c = if i == 0 {
                cols[0].1
            } else if i == cols.len() {
                cols[cols.len() - 1].1
            } else {
                let ((p0, c0), (p1, c1)) = (cols[i - 1], cols[i]);
                c0.lerp(c1, if p1 > p0 { (t - p0) / (p1 - p0) } else { 1.0 })
            };
            // rounded corners: coverage of the pixel by the corner's circle
            let cx = px.clamp(radius, fw - radius);
            let cy = py.clamp(radius, fh - radius);
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let cover = if radius > 0.0 { (radius - d + 0.5).clamp(0.0, 1.0) } else { 1.0 };
            let u = |v: f64| v.round().clamp(0.0, 255.0) as u8;
            data[(y * w + x) as usize] = slint::Rgba8Pixel { r: u(c.r), g: u(c.g), b: u(c.b), a: u(cover * 255.0) };
        }
    }
    Image::from_rgba8(buf)
}

/// What doesn't change while the window is open: swatches, the icon; the thumbnails' shadow
/// (0 2px 6px rgba(0,0,0,.25)) for the window's scale.
pub fn setup_with(w: &SettingsWindow, scale: f32) {
    let g = w.global::<SetState>();
    g.set_accent_swatches(model(WIN_ACCENTS.iter().map(|h| swatch(h, h, h)).collect()));
    let bg = ["#FFFFFF", "#F3F3F3", "#E8EEF7", "#FFF4E5", "#202020", "#1C2433", "#2B1F2F", "#10241B"];
    g.set_bg_swatches(model(bg.iter().map(|h| swatch(h, h, h)).collect()));
    let detail = ["#C42B1C", "#E74856", "#FF8C00", "#107C10", "#0078D4", "#8764B8", "#E3008C", "#767676"];
    g.set_detail_swatches(model(detail.iter().map(|h| swatch(h, h, h)).collect()));
    refresh_labels(w);
    let sh = [fluentcal_core::theme::Shadow { x: 0.0, y: 2.0, blur: 6.0, color: Rgba::new(0.0, 0.0, 0.0, 0.25) }];
    g.set_thumb_shadow(crate::shadow::render(&sh, 4.0, scale));
    g.set_icon(icon());
    // ThemeCards' thumbnails (aspect 132:80, drawn at twice that) and the split mini window
    let (tw, th) = ((264.0 * scale / 1.25) as u32, (160.0 * scale / 1.25) as u32);
    let r = 4.0 * f64::from(tw) / 132.0;
    g.set_thumb_light(gradient_image(tw, th, r, 160.0, &[(0.0, "#cfe2fb"), (1.0, "#5d93e0")]));
    g.set_thumb_dark(gradient_image(tw, th, r, 160.0, &[(0.0, "#0b2a5b"), (1.0, "#1b4f9c")]));
    g.set_thumb_system(gradient_image(tw, th, r, 115.0, &[(0.0, "#cfe2fb"), (0.5, "#cfe2fb"), (0.5, "#0b2a5b"), (1.0, "#0b2a5b")]));
    g.set_thumb_wall(gradient_image(tw, th, r, 160.0, &[(0.0, "#0b2a5b"), (0.55, "#1b4f9c"), (1.0, "#3a7bd5")]));
    let (ww, wh) = ((92.0 * scale).round() as u32, (56.0 * scale).round() as u32);
    g.set_thumb_split_win(gradient_image(ww, wh, 4.0 * f64::from(scale), 115.0, &[(0.0, "#f3f3f3"), (0.5, "#f3f3f3"), (0.5, "#202020"), (1.0, "#202020")]));
    g.set_groups(model(Vec::new()));
    g.set_backups(model(Vec::new()));
    g.set_upcoming(model(Vec::new()));
    g.set_addresses(model(Vec::new()));
}

pub fn setup(w: &SettingsWindow, app: &crate::app::App) {
    setup_with(w, scale_of(w, app));
}

/// Refresh translated labels without rebuilding editable groups or thumbnail images.
pub fn refresh_labels(w: &SettingsWindow) {
    let g = w.global::<SetState>();
    update(g.get_task_swatches(), TASK_COLORS.iter().map(|c| swatch(c.key, c.bar, tr(c.label))).collect(), |m| g.set_task_swatches(m));
}

/// The window's scale; before Windows has created it (just shown), the screen it opens on.
pub fn scale_of(w: &SettingsWindow, app: &crate::app::App) -> f32 {
    if crate::platform::window::hwnd(w.window()).is_some() {
        w.window().scale_factor()
    } else {
        app.aux_screen().map_or(1.0, |m| m.scale) as f32
    }
}

fn s(v: &str) -> SharedString {
    v.into()
}

/// Sets everything the pages show.
pub fn push(w: &SettingsWindow, doc: &Doc, info: &Info, local: &Local) {
    let g = w.global::<SetState>();
    let st = &doc.settings;
    let (d, off, now) = (&st.desk, info.offset, info.now_ms);

    let cur_screen = info.screens.iter().find(|x| x.2).map_or(String::new(), |x| x.0.clone());
    g.set_screens(model(info.screens.iter().map(|(v, l, _)| SelectOption { value: s(v), label: s(l), icon: s(""), disabled: false }).collect()));
    g.set_general(SGeneral {
        language: s(&st.general.language),
        autostart: st.general.autostart,
        autostart_desc: s(if info.debug { tr("开发版本不会写入开机启动项") } else { tr("登录 Windows 后自动显示桌面日历") }),
        pin_mode: s(&d.pin_mode),
        pin_hint: s(sp::pin_hint(&d.pin_mode)),
        locked: d.locked,
        click_through: d.click_through,
        no_activate: d.no_activate,
        toggle_key: s(&st.shortcuts.toggle),
        new_key: s(&st.shortcuts.new_task),
        shortcut_errors: s(&info.shortcut_errors.join("；")),
        holiday_auto: st.holidays.auto_update,
        holiday_desc: s(&sp::holiday_desc(st.holidays.last_check, &info.holiday_years, off)),
        checking: local.checking,
        screen: s(&cur_screen),
    });

    let auto = |v: &str, a: &str| if v == a { s("") } else { s(v) };
    g.set_appearance(SAppearance {
        mode: s(&st.theme.mode),
        follow_accent: st.theme.accent == "system",
        accent_hex: auto(&st.theme.accent, "system"),
        preset: s(sp::match_preset(st)),
        bg_color: auto(&d.bg_color, "auto"),
        bg_opacity: d.bg_opacity as f32,
        cell_opacity: d.cell_opacity as f32,
        acrylic: d.acrylic,
        no_transparency: info.transparency == Some(false),
        radius: d.radius as f32,
        shadow: d.shadow,
        text_shadow: s(&d.text_shadow),
        font_scale: d.font_scale as f32,
        cell_gap: d.cell_gap as f32,
        line_spacing: d.line_spacing as f32,
        today_color: auto(&d.today_color, "accent"),
        rest_color: auto(&d.rest_bg_color, "auto"),
        holiday_color: auto(&d.holiday_text_color, "auto"),
        term_color: auto(&d.term_text_color, "auto"),
        reduce_motion: st.theme.reduce_motion,
    });

    let c = &st.calendar;
    g.set_calendar(SCalendar {
        view: s(&c.view),
        week_start: s(&c.week_start.to_string()),
        month_rows: s(&c.month_rows),
        weeks_rows: c.weeks_rows as f32,
        weeks_past: c.weeks_past as f32,
        show_lunar: c.show_lunar,
        show_holidays: c.show_holidays,
        show_week_numbers: c.show_week_numbers,
        show_almanac: c.show_almanac,
        holiday_countdown: c.holiday_countdown,
        rest_days: model((0..7).map(|i| c.rest_days.contains(&i)).collect()),
        show_completed: st.tasks.show_completed,
        completed_last: st.tasks.completed_last,
        auto_postpone: st.tasks.auto_postpone,
        default_color: s(&st.tasks.default_color),
    });
    let gt = &st.gantt;
    g.set_gantt(SGantt { scale: s(&gt.scale), show_table: gt.show_table, show_deps: gt.show_deps, hide_done: gt.hide_done, hide_single_day: gt.hide_single_day });

    let r = &st.reminders;
    g.set_reminders(SReminders {
        enabled: r.enabled,
        style: s(&r.style),
        all_day_time: s(&r.all_day_time),
        offsets: model(r.default_offsets.iter().map(|v| s(&v.to_string())).collect()),
        can_add: r.default_offsets.len() < 5,
    });
    let opts = sp::offset_options(&r.default_offsets);
    g.set_offset_options(model(opts.into_iter().map(|(v, l)| SelectOption { value: s(&v.to_string()), label: s(&l), icon: s(""), disabled: false }).collect()));
    let list: Vec<_> = doc.tasks.values().cloned().collect();
    let due = fluentcal_core::reminders::compute_reminders(&list, now, &r.all_day_time, fluentcal_core::reminders::HORIZON_MS, fluentcal_core::reminders::GRACE_MS, off);
    let upcoming: Vec<SUpcoming> =
        due.iter().take(5).map(|x| SUpcoming { key: s(&x.key), title: s(&x.title), desc: s(&format!("{} · {}", sp::zh_reminder_time(x.at_ms, off), x.body)) }).collect();
    update(g.get_upcoming(), upcoming, |m| g.set_upcoming(m));

    let samples = tasks::sample_count(doc);
    let dd = &info.data_dir;
    g.set_data(SData {
        samples: samples as i32,
        samples_desc: s(&trf("第一次启动时生成的示例任务和示例项目，还剩 {0} 项。改过名称的不算示例，会保留", &[format!("{}", samples)])),
        backups_desc: s(&trf("每天自动备份一次，保留最近 14 天。共 {0} 个备份", &[format!("{}", local.backups.len())])),
        dir_desc: s(&format!("{}{}", dd.path, if dd.custom { tr("（自定义，可放在同步盘多机共用）") } else { "" })),
        custom: dd.custom,
        missing: s(dd.missing.as_deref().unwrap_or("")),
    });
    let backups: Vec<SBackup> =
        local.backups.iter().take(30).map(|b| SBackup { name: s(&b.name), desc: s(&format!("{} · {}", sp::zh_datetime(b.modified as i64, off), sp::kb(b.size))) }).collect();
    update(g.get_backups(), backups, |m| g.set_backups(m));

    let sy = &st.sync;
    let cloud = matches!(sy.provider.as_str(), "webdav" | "github" | "gitee");
    let git = if sy.provider == "gitee" { &sy.gitee } else { &sy.github };
    let key = match sy.provider.as_str() {
        "webdav" => "webdav",
        "gitee" => "gitee",
        _ => "github",
    };
    let saved = local.secrets.iter().any(|k| k == key);
    let status = &info.sync;
    g.set_sync(SSync {
        status: s(&fluentcal_core::sync_text::status_line(status, now, off)),
        enabled: status.enabled,
        syncing: status.syncing || local.syncing,
        error: s(status.error.as_deref().unwrap_or("")),
        halted: status.halted,
        provider: s(&sy.provider),
        locked: sy.enabled && cloud,
        cloud,
        dav_url: s(&sy.webdav.url),
        dav_user: s(&sy.webdav.user),
        dav_folder: s(&sy.webdav.folder),
        git_name: s(if sy.provider == "github" { "GitHub" } else { "Gitee" }),
        repo: s(&git.repo),
        branch: s(&git.branch),
        path: s(&git.path),
        secret_key: s(key),
        secret_saved: saved,
        secret_hint: s(if saved { tr("已保存（输入新值可替换）") } else { "" }),
        interval: s(&sy.interval.to_string()),
        proxy: s(&sy.proxy),
        proxy_mode: s(sp::proxy_mode(&sy.proxy)),
        busy: local.busy,
    });

    let wv = &local.web;
    g.set_web(SWeb {
        enabled: st.web.enabled,
        running: wv.running,
        port: f32::from(st.web.port),
        error: s(wv.error.as_deref().unwrap_or("")),
        has_addresses: !wv.addresses.is_empty(),
        qr: wv.qr.as_ref().map_or(Image::default(), |q| q.1.clone()),
    });
    let addrs: Vec<SAddress> = wv
        .addresses
        .iter()
        .map(|(ip, label, virt, url)| {
            let note = format!("{label}{}", if *virt { tr("（虚拟网卡，手机一般连不上）") } else { "" });
            SAddress { ip: s(ip), text: s(&format!("http://{ip}:{}", wv.port)), note: s(&note), url: s(url) }
        })
        .collect();
    update(g.get_addresses(), addrs, |m| g.set_addresses(m));
    g.set_about(SAbout { version: s(&info.version), build: s(&info.build.to_string()) });

    let mut groups: Vec<&fluentcal_core::model::Group> = doc.groups.values().collect();
    groups.sort_by(|a, b| a.order.total_cmp(&b.order));
    let groups: Vec<SGroup> = groups
        .iter()
        .map(|gr| {
            let n = doc.tasks.values().filter(|t| t.group_id.as_deref() == Some(gr.id.as_str())).count();
            SGroup { id: s(&gr.id), name: s(&gr.name), color: s(&gr.color), desc: s(&trf("{0} 个任务", &[format!("{}", n)])) }
        })
        .collect();
    update(g.get_groups(), groups, |m| g.set_groups(m));

    g.set_msg(match &local.msg {
        Some((sev, title, text)) => SMsg { shown: true, severity: *sev, title: s(title), text: s(text) },
        None => SMsg { shown: false, severity: Severity::Info, title: s(""), text: s("") },
    });
}
