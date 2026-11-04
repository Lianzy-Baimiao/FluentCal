//! View models of the desk calendar (0.2's MonthView/WeekRow/TaskChip/CountdownBar/TitleBar
//! computeds): plain data for the Slint structs.

use fluentcal_core::i18n::{tr, trf};

use crate::theme::color;
use crate::{CellData, ChipData, CountdownData, WeekData, WeekdayHead};
use fluentcal_core::countdown::countdowns;
use fluentcal_core::date::{add_days, add_months, format_friendly, format_range, format_ymd, iso_week, month_weeks, parts, start_of_month, start_of_week, weekday, WEEKDAY_SHORT};
use fluentcal_core::lunar::{almanac, display_info as day_info, Kind};
use fluentcal_core::model::Task;
use fluentcal_core::recur::{describe_rule, occurrences, Occurrence};
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::{mark_color, task_color, Tokens, TRANSPARENT};
use fluentcal_core::week::{layout_week, Chip};
use slint::{ModelRc, SharedString, VecModel};

/// The desk window's own state (0.2's `ui` in desk/state.ts).
#[derive(Debug, Clone, Default)]
pub struct DeskUi {
    /// Month view: the first day of the shown month.
    pub cursor: String,
    /// Weeks view: the first shown week.
    pub weeks_start: String,
    /// Keyboard-focused day.
    pub selected: String,
    /// Day highlighted briefly after a jump.
    pub flash: String,
    /// Arrow keys move the selected day (a day was clicked; 0.2: a focused cell).
    pub grid_focus: bool,
    /// Day with the inline quick-add box.
    pub quick_add: Option<String>,
    /// Wheel paging: accumulated delta and the time of the last page (ms).
    pub wheel_acc: f32,
    pub wheel_at: i64,
}

impl DeskUi {
    pub fn new(doc: &Doc, today: &str) -> DeskUi {
        DeskUi {
            cursor: start_of_month(today),
            weeks_start: default_weeks_start(doc, today),
            selected: today.to_owned(),
            ..Default::default()
        }
    }
}

/// First and last day shown (month view: whole weeks around the month).
pub fn shown_range(doc: &Doc, ui: &DeskUi) -> (String, String) {
    let c = &doc.settings.calendar;
    if c.view == "weeks" {
        let n = c.weeks_rows.clamp(2, 10) as i32;
        (ui.weeks_start.clone(), add_days(&ui.weeks_start, n * 7 - 1))
    } else {
        let (y, m, _) = parts(&ui.cursor);
        let w = month_weeks(y, m, c.week_start, c.month_rows == "six");
        (w[0].clone(), add_days(w.last().unwrap(), 6))
    }
}

/// First week of the rolling multi-week view.
pub fn default_weeks_start(doc: &Doc, day: &str) -> String {
    let c = &doc.settings.calendar;
    add_days(&start_of_week(day, c.week_start), -7 * c.weeks_past as i32)
}

pub fn page(doc: &Doc, ui: &mut DeskUi, delta: i32) {
    if doc.settings.calendar.view == "weeks" {
        ui.weeks_start = add_days(&ui.weeks_start, delta * 7);
    } else {
        ui.cursor = add_months(&start_of_month(&ui.cursor), delta);
    }
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

/// "2026年10月", the weeks view's range, or "甘特图".
pub fn view_title(doc: &Doc, ui: &DeskUi, today: &str) -> String {
    let c = &doc.settings.calendar;
    match c.view.as_str() {
        "gantt" => tr("甘特图").into(),
        "weeks" => {
            let s = &ui.weeks_start;
            let e = add_days(s, c.weeks_rows as i32 * 7 - 1);
            let ((sy, sm, sd), (ey, em, ed)) = (parts(s), parts(&e));
            let yr = if sy == parts(today).0 { String::new() } else { trf("{0}年", &[format!("{}", sy)]) };
            if sy == ey {
                trf("{0}{1}月{2}日 – {3}月{4}日", &[format!("{}", yr), format!("{}", sm), format!("{}", sd), format!("{}", em), format!("{}", ed)])
            } else {
                trf("{0}年{1}月{2}日 – {3}年{4}月{5}日", &[format!("{}", sy), format!("{}", sm), format!("{}", sd), format!("{}", ey), format!("{}", em), format!("{}", ed)])
            }
        }
        _ => {
            let (y, m, _) = parts(&ui.cursor);
            trf("{0}年{1}月", &[format!("{}", y), format!("{}", m)])
        }
    }
}

pub fn today_tip(today: &str) -> String {
    let i = day_info(today);
    let fest = if i.festivals.is_empty() { String::new() } else { format!("\n{}", i.festivals.join("、")) };
    trf("{0} 周{1}\n农历{2} · {3}{4}", &[format!("{}", format_ymd(today)), format!("{}", fluentcal_core::i18n::tr(WEEKDAY_SHORT[weekday(today) as usize])), format!("{}", i.lunar_label), format!("{}", i.ganzhi_year), format!("{}", fest)])
}

pub fn weekday_heads(doc: &Doc) -> ModelRc<WeekdayHead> {
    let c = &doc.settings.calendar;
    model(
        (0..7)
            .map(|i| {
                let wd = (c.week_start + i) % 7;
                WeekdayHead { label: fluentcal_core::i18n::tr(WEEKDAY_SHORT[wd as usize]).into(), rest: c.rest_days.contains(&wd) }
            })
            .collect(),
    )
}

pub fn countdown_items(doc: &Doc, today: &str) -> ModelRc<CountdownData> {
    let tasks: Vec<Task> = doc.tasks.values().cloned().collect();
    model(
        countdowns(&tasks, today, doc.settings.calendar.holiday_countdown)
            .into_iter()
            .take(6)
            .map(|c| CountdownData {
                key: c.key.into(),
                label: if c.task_id.is_none() { tr(&c.label).into() } else { c.label.into() },
                tip: format_friendly(&c.date, today).into(),
                date: c.date.into(),
                days: c.days,
                task: c.task_id.is_some(),
            })
            .collect(),
    )
}

/// Lane capacity: None lays out everything (before the first measurement).
pub fn weeks(doc: &Doc, ui: &DeskUi, today: &str, capacity: Option<usize>, t: &Tokens) -> ModelRc<WeekData> {
    let c = &doc.settings.calendar;
    let month_mode = c.view != "weeks";
    let starts: Vec<String> = if month_mode {
        let (y, m, _) = parts(&ui.cursor);
        month_weeks(y, m, c.week_start, c.month_rows == "six")
    } else {
        let n = c.weeks_rows.clamp(2, 10) as i32;
        (0..n).map(|i| add_days(&ui.weeks_start, i * 7)).collect()
    };
    let (first, last) = (starts[0].clone(), add_days(starts.last().unwrap(), 6));
    let focus = if month_mode { ui.cursor[..7].to_owned() } else { String::new() };
    let show_done = doc.settings.tasks.show_completed;
    let occs: Vec<Occurrence> = doc
        .tasks
        .values()
        .flat_map(|task| occurrences(task, &first, &last, 400))
        .filter(|o| show_done || !o.done)
        .collect();
    let rows = starts
        .iter()
        .map(|w| {
            let cells: Vec<CellData> = (0..7).map(|i| cell(doc, &add_days(w, i), &focus, month_mode, today, &occs)).collect();
            let layout = layout_week(w, occs.clone(), capacity, doc.settings.tasks.completed_last);
            let mut chips: Vec<&Chip> = layout.chips.iter().filter(|c| c.visible).collect();
            chips.sort_by_key(|c| c.col);
            WeekData {
                cells: model(cells),
                chips: model(chips.into_iter().map(|ch| chip_data(doc, ch, today, t)).collect()),
                more: model(layout.overflow.iter().map(|&n| n as i32).collect()),
                more_lane: model(layout.overflow_lane.iter().map(|&n| n as i32).collect()),
                week_no: if c.show_week_numbers { iso_week(&add_days(w, if c.week_start == 0 { 1 } else { 0 })) as i32 } else { 0 },
            }
        })
        .collect();
    model(rows)
}

fn cell(doc: &Doc, date: &str, focus: &str, month_mode: bool, today: &str, occs: &[Occurrence]) -> CellData {
    let c = &doc.settings.calendar;
    let info = day_info(date);
    let (_, m, d) = parts(date);
    let legal = if c.show_holidays { info.legal.clone() } else { None };
    let rest = match &legal {
        Some(l) => !l.work,
        None => c.rest_days.contains(&weekday(date)),
    };
    let lunar = if c.show_lunar || info.kind == Kind::Festival { info.text.clone() } else { String::new() };
    let wd = trf("周{0}", &[format!("{}", fluentcal_core::i18n::tr(WEEKDAY_SHORT[weekday(date) as usize]))]);
    let mut tip = vec![format!("{} {wd}", format_ymd(date)), trf("农历{0} · {1}", &[format!("{}", info.lunar_label), format!("{}", info.ganzhi_year)])];
    if !info.festivals.is_empty() {
        tip.push(info.festivals.join("、"));
    }
    if let Some(term) = &info.term {
        tip.push(trf("节气：{0}", &[format!("{}", term)]));
    }
    if let Some(l) = &legal {
        tip.push(format!("{} · {}", l.name, if l.work { tr("调休上班") } else { tr("放假") }));
    }
    if c.show_almanac {
        let (rec, avoid) = almanac(date);
        let join = |v: &[String]| if v.is_empty() { "—".to_owned() } else { v.iter().take(8).map(|s| tr(s)).collect::<Vec<_>>().join(" · ") };
        tip.push(trf("宜：{0}", &[format!("{}", join(&rec))]));
        tip.push(trf("忌：{0}", &[format!("{}", join(&avoid))]));
    }
    let count = occs.iter().filter(|o| o.start.as_str() <= date && o.end.as_str() >= date).count();
    let in_focus = &date[..7] == focus;
    let mark = doc.day_marks.get(date).and_then(|k| mark_color(k));
    let legal_aria = legal.as_ref().map_or(String::new(), |l| format!("，{}", if l.work { tr("调休上班") } else { tr("放假") }));
    CellData {
        date: date.into(),
        label: if d == 1 && !in_focus { trf("{0}月{1}日", &[m.to_string(), d.to_string()]) } else { d.to_string() }.into(),
        lunar: lunar.into(),
        lunar_kind: match info.kind {
            Kind::Lunar => "lunar",
            Kind::Month => "month",
            Kind::Term => "term",
            Kind::Festival => "festival",
        }
        .into(),
        badge: legal.as_ref().map_or("", |l| if l.work { tr("班") } else { tr("休") }).into(),
        rest,
        today: date == today,
        dim: month_mode && !in_focus,
        marked: mark.is_some(),
        mark: color(mark.unwrap_or(TRANSPARENT)),
        tip: tip.join("\n").into(),
        aria: trf("{0}月{1}日 {2}，农历{3}{4}，{5}", &[format!("{}", m), format!("{}", d), format!("{}", wd), format!("{}", info.lunar_label), format!("{}", legal_aria), format!("{}", if count > 0 { trf("{0} 项任务", &[count.to_string()]) } else { tr("无任务").into() })])
        .into(),
    }
}

pub fn chip_data(doc: &Doc, ch: &Chip, today: &str, t: &Tokens) -> ChipData {
    let task = ch.occ.task;
    let col = task_color(task.color.as_deref(), t);
    let mut tip = vec![task.title.clone(), format_range(&ch.occ.start, &ch.occ.end, today) + &task.time.as_deref().map_or(String::new(), |tm| format!(" {tm}"))];
    if let Some(g) = task.group_id.as_deref().and_then(|id| doc.groups.get(id)) {
        tip.push(trf("分组：{0}", &[format!("{}", g.name)]));
    }
    if task.repeat.is_some() {
        tip.push(trf("重复：{0}", &[format!("{}", describe_rule(task.repeat.as_ref(), &task.start))]));
    }
    if task.repeat.is_none() && task.progress > 0.0 && !task.done {
        tip.push(trf("进度：{0}%", &[format!("{}", task.progress)]));
    }
    if let Some(n) = task.notes.as_deref().filter(|n| !n.is_empty()) {
        let short: String = n.chars().take(120).collect();
        tip.push(if n.chars().count() > 120 { format!("{short}…") } else { n.to_owned() });
    }
    ChipData {
        key: ch.key.clone().into(),
        task_id: task.id.clone().into(),
        date: ch.occ.date.clone().into(),
        title: task.title.clone().into(),
        time: task.time.clone().unwrap_or_default().into(),
        col: ch.col as i32,
        span: ch.span as i32,
        lane: ch.lane as i32,
        cont_left: ch.cont_left,
        cont_right: ch.cont_right,
        multi_day: ch.multi_day,
        done: ch.occ.done,
        milestone: task.is_milestone(),
        repeat: task.repeat.is_some(),
        bar: color(col.bar),
        text: color(col.text),
        on_bar: color(col.on_bar),
        bar_hover: color(col.bar.mix(0.88, t.text_primary)),
        check_ring: color(col.bar.mix(0.7, t.text_secondary)),
        tip: SharedString::from(tip.join("\n")),
    }
}
