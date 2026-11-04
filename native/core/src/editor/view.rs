//! What the editor window shows besides the fields themselves: hints, the repeat summary,
//! the choices of the selects (EditorApp, RepeatEditor, ReminderEditor, DepsEditor).

use crate::i18n::{tr, trf};

use super::Editor;
use crate::date::{days_in_month, diff_days, format_range, parts, weekday, WEEKDAY_SHORT};
use crate::lunar::display_info as day_info;
use crate::model::Freq;
use crate::recur::{describe_rule, task_end};
use crate::reminders::{offset_label, REMINDER_OFFSETS};
use crate::tasks::Doc;
use std::collections::BTreeSet;

/// A select choice.
#[derive(Debug, Clone, PartialEq)]
pub struct Option_ {
    pub value: String,
    pub label: String,
}

fn opt(value: impl Into<String>, label: impl Into<String>) -> Option_ {
    Option_ { value: value.into(), label: label.into() }
}

/// A predecessor shown as a tag; `title` None = deleted meanwhile.
#[derive(Debug, Clone, PartialEq)]
pub struct DepTag {
    pub id: String,
    pub title: Option<String>,
    pub color: Option<String>,
}

pub type DepOption = Option_;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    /// "共 5 天 · 农历九月初三" (EditorApp spanText).
    pub span: String,
    pub show_progress: bool,
    pub show_postpone: bool,
    pub postpone: bool,
    /// RepeatEditor: the frequency ("" = none), "每 N <unit>" ("" = no interval).
    pub freq: String,
    pub interval: u32,
    pub unit: &'static str,
    pub weekly: bool,
    pub weekdays: Vec<u32>,
    pub monthly: bool,
    pub month_mode: String,
    pub month_modes: Vec<Option_>,
    pub has_end: bool,
    pub end_mode: &'static str,
    pub count: u32,
    pub until: String,
    pub summary: String,
    pub reminders: Vec<i64>,
    pub reminder_options: Vec<Option_>,
    pub reminder_hint: String,
    pub deps: Vec<DepTag>,
    pub dep_options: Vec<Option_>,
    pub groups: Vec<Option_>,
}

impl Editor {
    /// ReminderEditor options: offsets that suit the task (all-day: on the day or days
    /// before), plus ones already in the list; 准时 reads "当天 09:00" for all-day tasks.
    pub fn reminder_options(&self, doc: &Doc) -> Vec<(i64, String)> {
        let Some(t) = &self.draft else { return Vec::new() };
        let all_day = t.time.is_none();
        let at = &doc.settings.reminders.all_day_time;
        let base: Vec<(i64, String)> = REMINDER_OFFSETS.iter().filter(|(v, _)| !all_day || *v == 0 || *v >= 1440).map(|(v, l)| (*v, tr(l).to_owned())).collect();
        let extra: Vec<(i64, String)> = t.reminders.iter().flatten().filter(|v| !base.iter().any(|(b, _)| b == *v)).map(|v| (*v, offset_label(*v))).collect();
        base.into_iter().chain(extra).map(|(v, l)| (v, if all_day && v == 0 { trf("当天 {0}", &[format!("{}", at)]) } else { l })).collect()
    }

    /// DepsEditor options: tasks that may become a predecessor (not repeating, not already
    /// one, not depending on this task: that would be a cycle), the same group first.
    pub fn dep_options(&self, doc: &Doc, today: &str) -> Vec<Option_> {
        let Some(me) = &self.draft else { return Vec::new() };
        let mut succ: BTreeSet<&str> = BTreeSet::from([me.id.as_str()]);
        loop {
            let more: Vec<&str> = doc.tasks.values().filter(|t| !succ.contains(t.id.as_str()) && t.deps().iter().any(|d| succ.contains(d.as_str()))).map(|t| t.id.as_str()).collect();
            if more.is_empty() {
                break;
            }
            succ.extend(more);
        }
        let mine = me.deps();
        let mut list: Vec<_> = doc.tasks.values().filter(|t| t.repeat.is_none() && !succ.contains(t.id.as_str()) && !mine.contains(&t.id)).collect();
        list.sort_by(|a, b| {
            let same = |t: &&crate::model::Task| t.group_id == me.group_id;
            same(b).cmp(&same(a)).then_with(|| a.start.cmp(&b.start)).then_with(|| a.id.cmp(&b.id))
        });
        list.into_iter().take(200).map(|t| opt(t.id.clone(), format!("{}（{}）", t.title, format_range(&t.start, task_end(t), today)))).collect()
    }

    /// Everything the window shows that isn't a plain field value.
    pub fn view(&self, doc: &Doc, today: &str) -> View {
        let Some(t) = &self.draft else { return View::default() };
        let rule = t.repeat.as_ref();
        let freq = rule.and_then(|r| r.freq);
        let n = diff_days(&t.start, &t.end) + 1;
        let lunar = day_info(&t.start).lunar_label;
        let (y, m, d) = parts(&t.start);
        let nth = d.div_ceil(7);
        let last = nth >= 5 || d + 7 > days_in_month(y, m);
        let wd = crate::i18n::tr(WEEKDAY_SHORT[weekday(&t.start) as usize]);
        let r = &doc.settings.reminders;
        let mut groups = vec![opt("", tr("未分组"))];
        let mut gs: Vec<_> = doc.groups.values().collect();
        gs.sort_by(|a, b| a.order.partial_cmp(&b.order).unwrap_or(std::cmp::Ordering::Equal));
        groups.extend(gs.into_iter().map(|g| opt(g.id.clone(), g.name.clone())));
        groups.push(opt("__new__", tr("新建分组…")));
        View {
            span: trf("{0}农历{1}", &[format!("{}", if t.is_milestone() || n == 1 { String::new() } else { trf("共 {0} 天 · ", &[n.to_string()]) }), format!("{}", lunar)]),
            show_progress: rule.is_none(),
            show_postpone: rule.is_none() && t.start == t.end,
            postpone: t.auto_postpone.unwrap_or(doc.settings.tasks.auto_postpone),
            freq: freq.and_then(|f| serde_json::to_value(f).ok()).and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default(),
            interval: rule.and_then(|r| r.interval).unwrap_or(1),
            unit: super::unit_of(freq),
            weekly: freq == Some(Freq::Weekly),
            weekdays: rule.and_then(|r| r.weekdays.clone()).unwrap_or_default(),
            monthly: freq == Some(Freq::Monthly),
            month_mode: rule.and_then(|r| r.month_mode.clone()).unwrap_or_else(|| "day".into()),
            month_modes: vec![
                opt("day", trf("每月 {0} 日", &[format!("{}", d)])),
                opt("nthWeekday", if last { trf("每月最后一个周{0}", &[format!("{}", wd)]) } else { trf("每月第 {0} 个周{1}", &[format!("{}", nth), format!("{}", wd)]) }),
            ],
            has_end: rule.is_some() && freq != Some(Freq::Ebbinghaus),
            end_mode: self.end_mode(),
            count: rule.and_then(|r| r.count).unwrap_or(10),
            until: rule.and_then(|r| r.until.clone()).unwrap_or_default(),
            summary: if rule.is_some() { describe_rule(rule, &t.start) } else { String::new() },
            reminders: t.reminders.clone().unwrap_or_default(),
            reminder_options: self.reminder_options(doc).into_iter().map(|(v, l)| opt(v.to_string(), l)).collect(),
            reminder_hint: if !r.enabled {
                tr("提醒已在设置中关闭").into()
            } else if t.time.is_none() && t.reminders.as_ref().is_some_and(|l| !l.is_empty()) {
                trf("全天任务在 {0} 提醒", &[format!("{}", r.all_day_time)])
            } else {
                String::new()
            },
            deps: t.deps().iter().map(|id| DepTag { id: id.clone(), title: doc.tasks.get(id).map(|d| d.title.clone()), color: doc.tasks.get(id).and_then(|d| d.color.clone()) }).collect(),
            dep_options: self.dep_options(doc, today),
            groups,
        }
    }
}
