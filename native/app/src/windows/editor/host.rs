//! The editor window's glue, shared by the live window (mod.rs) and the offscreen parity
//! snapshot (Offline): window callbacks become `Msg`s, `handle` applies one to the draft,
//! `push` shows the draft, `show` does what an action's effect asks of the window.

use crate::desk::sync;
use crate::theme::color;
use crate::{DepTag, EditorData, EditorWindow, RadioOption, SelectOption, Swatch};
use fluentcal_core::editor::{Answer, Effect, Editor, Option_, FREQS};
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::{best_text_on, task_color, Rgba, Tokens, TASK_COLORS};
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;

const TITLE_MAX: usize = 200;
const NOTES_MAX: usize = 5000;

/// What the window asks for.
#[derive(Debug, Clone)]
pub enum Msg {
    Edit(String, String),
    ToggleDay(u32),
    ReminderSet(usize, i64),
    ReminderRemove(usize),
    ReminderAdd,
    DepAdd(String),
    DepRemove(String),
    CreateGroup(String),
    Save,
    Remove,
    Cancel,
    Answer(Answer),
}

/// Sends the window's callbacks to `to`.
pub fn wire(w: &EditorWindow, to: Rc<dyn Fn(Msg)>) {
    let t = to.clone();
    w.on_edit(move |k, v| t(Msg::Edit(k.into(), v.into())));
    let t = to.clone();
    w.on_toggle_day(move |d| t(Msg::ToggleDay(d.max(0) as u32)));
    let t = to.clone();
    w.on_reminder_set(move |i, v| t(Msg::ReminderSet(i.max(0) as usize, v.parse().unwrap_or(0))));
    let t = to.clone();
    w.on_reminder_remove(move |i| t(Msg::ReminderRemove(i.max(0) as usize)));
    let t = to.clone();
    w.on_reminder_add(move || t(Msg::ReminderAdd));
    let t = to.clone();
    w.on_dep_add(move |id| t(Msg::DepAdd(id.into())));
    let t = to.clone();
    w.on_dep_remove(move |id| t(Msg::DepRemove(id.into())));
    let t = to.clone();
    w.on_create_group(move |n| t(Msg::CreateGroup(n.into())));
    let t = to.clone();
    w.on_save(move || t(Msg::Save));
    let t = to.clone();
    w.on_remove(move || t(Msg::Remove));
    let t = to.clone();
    w.on_cancel(move || t(Msg::Cancel));
    w.on_ask_result(move |a| {
        to(Msg::Answer(match a.as_str() {
            "primary" => Answer::Primary,
            "secondary" => Answer::Secondary,
            _ => Answer::Close,
        }))
    });
}

fn options(list: &[Option_]) -> Vec<SelectOption> {
    list.iter().map(|o| SelectOption { value: o.value.clone().into(), label: o.label.clone().into(), ..Default::default() }).collect()
}

/// What doesn't change while the window is open: the repeat choices and the colour palette.
pub fn setup(w: &EditorWindow) {
    let freqs: Vec<SelectOption> = FREQS.iter().map(|(v, l)| SelectOption { value: (*v).into(), label: (*l).into(), ..Default::default() }).collect();
    w.set_freqs(ModelRc::new(VecModel::from(freqs)));
    let colors: Vec<Swatch> = TASK_COLORS
        .iter()
        .map(|c| {
            let bar = Rgba::hex(c.bar);
            Swatch { key: c.key.into(), color: color(bar), fg: color(best_text_on(bar)), title: c.label.into(), ..Default::default() }
        })
        .collect();
    w.set_colors(ModelRc::new(VecModel::from(colors)));
}

/// Shows the draft. `texts`: also the title and notes boxes (they keep their own text while
/// typing; set them when a task is loaded).
pub fn push(w: &EditorWindow, ed: &Editor, doc: &Doc, today: &str, t: &Tokens, texts: bool) {
    w.set_is_new(ed.is_new);
    w.set_missing(ed.missing);
    w.set_title_error(ed.title_error);
    w.set_dirty(ed.dirty());
    let Some(d) = &ed.draft else { return };
    if texts {
        w.set_title_text(d.title.clone().into());
        w.set_notes_text(d.notes.clone().unwrap_or_default().into());
    }
    let v = ed.view(doc, today);
    let mut days = vec![false; 7];
    for wd in &v.weekdays {
        if let Some(x) = days.get_mut(*wd as usize) {
            *x = true;
        }
    }
    w.set_data(EditorData {
        start: d.start.clone().into(),
        end: d.end.clone().into(),
        all_day: d.time.is_none(),
        time: d.time.clone().unwrap_or_default().into(),
        milestone: d.is_milestone(),
        span: v.span.into(),
        freq: v.freq.into(),
        interval: v.interval as f32,
        unit: v.unit.into(),
        weekly: v.weekly,
        days: ModelRc::new(VecModel::from(days)),
        monthly: v.monthly,
        month_mode: v.month_mode.into(),
        has_end: v.has_end,
        end_mode: v.end_mode.into(),
        count: v.count as f32,
        until: v.until.into(),
        summary: v.summary.into(),
        reminder_count: v.reminders.len() as i32,
        reminder_hint: v.reminder_hint.into(),
        group: d.group_id.clone().unwrap_or_default().into(),
        color: d.color.clone().unwrap_or_default().into(),
        show_progress: v.show_progress,
        progress: d.progress as f32,
        done: d.done,
        countdown: d.countdown == Some(true),
        show_postpone: v.show_postpone,
        postpone: v.postpone,
    });
    // Lists are updated in place: a replaced model recreates its rows (and an open select).
    let reminders: Vec<SharedString> = v.reminders.iter().map(|r| r.to_string().into()).collect();
    sync::rows(&w.get_reminders(), reminders, |m| w.set_reminders(m));
    let deps: Vec<DepTag> = v
        .deps
        .iter()
        .map(|g| DepTag { id: g.id.clone().into(), title: g.title.clone().unwrap_or_else(|| "（已删除）".into()).into(), dot: color(task_color(g.color.as_deref(), t).bar) })
        .collect();
    sync::rows(&w.get_deps(), deps, |m| w.set_deps(m));
    sync::rows(&w.get_reminder_options(), options(&v.reminder_options), |m| w.set_reminder_options(m));
    sync::rows(&w.get_groups(), options(&v.groups), |m| w.set_groups(m));
    sync::rows(&w.get_dep_options(), options(&v.dep_options), |m| w.set_dep_options(m));
    let modes: Vec<RadioOption> = v.month_modes.iter().map(|o| RadioOption { value: o.value.clone().into(), label: o.label.clone().into(), disabled: false }).collect();
    sync::rows(&w.get_month_modes(), modes, |m| w.set_month_modes(m));
}

/// Applies `m` to the draft; the returned effect has the edit to write and what to show.
pub fn handle(w: &EditorWindow, ed: &mut Editor, doc: &Doc, m: Msg, today: &str, now_ms: i64) -> Effect {
    match m {
        Msg::Edit(k, v) => {
            // 0.2's maxlength: 200 for the title, 5000 for the notes
            let max = match k.as_str() {
                "title" => TITLE_MAX,
                "notes" => NOTES_MAX,
                _ => usize::MAX,
            };
            if v.chars().count() > max {
                let cut: String = v.chars().take(max).collect();
                if k == "title" { w.set_title_text(cut.clone().into()) } else { w.set_notes_text(cut.clone().into()) }
                ed.edit(&k, &cut);
            } else {
                ed.edit(&k, &v);
            }
            Effect::default()
        }
        Msg::ToggleDay(d) => {
            ed.toggle_day(d);
            Effect::default()
        }
        Msg::ReminderSet(i, v) => {
            ed.set_reminder(i, v);
            Effect::default()
        }
        Msg::ReminderRemove(i) => {
            ed.remove_reminder(i);
            Effect::default()
        }
        Msg::ReminderAdd => {
            ed.add_reminder(doc);
            Effect::default()
        }
        Msg::DepAdd(id) => {
            ed.add_dep(&id);
            Effect::default()
        }
        Msg::DepRemove(id) => {
            ed.remove_dep(&id);
            Effect::default()
        }
        Msg::CreateGroup(name) => match ed.create_group(doc, &name) {
            Some(edit) => {
                w.set_new_group_open(false);
                w.set_new_group_name(SharedString::new());
                Effect { edit, ..Default::default() }
            }
            None => Effect::default(),
        },
        Msg::Save => ed.save(doc, now_ms),
        Msg::Remove => ed.remove(doc),
        Msg::Cancel => ed.cancel(),
        Msg::Answer(a) => ed.answer(doc, a, today, now_ms),
    }
}

/// The window side of an effect (after its edit is written): a question, the title's focus.
pub fn show(w: &EditorWindow, fx: &Effect) {
    if let Some(a) = &fx.ask {
        let (title, text, p, s, c, danger) = a.texts();
        w.invoke_ask(title.into(), text.into(), p.into(), s.into(), c.into(), danger);
    }
    if fx.focus_title || fx.reloaded {
        w.invoke_focus_title();
    }
}
