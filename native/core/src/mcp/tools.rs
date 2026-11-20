//! What an assistant can do with the calendar: six tools that look and five that change.
//!
//! The rules are the task editor's (editor/flow.rs, editor/fields.rs): a title, an end not
//! before the start, a milestone on one day, "every day from … to …" written as a daily rule
//! with an end, no predecessors for repeating tasks. Whatever is wrong with the arguments is
//! told in the result (Err here → `isError` in proto.rs), in words an assistant can act on.
//!
//! Tools that change something build an Edit inside `Host::change`, with the document as it
//! is at that moment; the host writes it (in the app: on the UI thread, recorded under
//! 最近更改).
//!
//! The schemas avoid what some clients cannot digest (union types, `format`): a field is
//! cleared with an empty string or an empty list, and `null` is accepted for the same.

use super::Host;
use crate::date::{add_days, diff_days, is_iso_date, now_ms, parts, today, weekday};
use crate::flex;
use crate::gantt::would_cycle;
use crate::i18n::tr;
use crate::lunar;
use crate::model::{Freq, Group, RepeatRule, Task};
use crate::recur::{describe_rule, normalize_daily_range, occurrence_on, occurrences, task_end};
use crate::search::{search_tasks as search, SearchOptions};
use crate::tasks::{delete_occurrence, delete_task as delete, make_group, make_task, save_group, save_task, set_occurrence_done, DeleteMode, Doc, Edit};
use crate::theme::TASK_COLORS;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

type Args = Map<String, Value>;
type Res = Result<Value, String>;

const NAMES: [&str; 11] =
    ["list_tasks", "search_tasks", "get_task", "list_groups", "list_todos", "get_days", "add_tasks", "update_task", "set_task_done", "delete_task", "add_group"];
const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const FREQS: &str = "daily, weekly, monthly, yearly, workday, legalWorkday, lunarYearly, monthLastDay, ebbinghaus";
/// At most this many tasks in one add_tasks call.
const MAX_ADD: usize = 50;

pub fn exists(name: &str) -> bool {
    NAMES.contains(&name)
}

/// Runs a tool. Ok: what to tell the assistant; Err: why it did not work.
pub fn call(host: &dyn Host, name: &str, a: &Args) -> Res {
    match name {
        "list_tasks" => list_tasks(&host.doc(), a),
        "search_tasks" => search_tasks(&host.doc(), a),
        "get_task" => get_task(&host.doc(), a),
        "list_groups" => list_groups(&host.doc(), a),
        "list_todos" => list_todos(&host.doc(), a),
        "get_days" => get_days(&host.doc(), a),
        "add_tasks" => may_write(host).and_then(|_| add_tasks(host, a)),
        "update_task" => may_write(host).and_then(|_| update_task(host, a)),
        "set_task_done" => may_write(host).and_then(|_| set_task_done(host, a)),
        "delete_task" => may_write(host).and_then(|_| delete_task(host, a)),
        "add_group" => may_write(host).and_then(|_| add_group(host, a)),
        _ => Err(format!("Unknown tool: {name}")),
    }
}

fn may_write(host: &dyn Host) -> Result<(), String> {
    if host.can_write() {
        return Ok(());
    }
    Err(format!(
        "FluentCal is set to let assistants look at the calendar but not change it. The user can allow changes in FluentCal: {} → {} → {}.",
        tr("设置"),
        tr("AI 助手"),
        tr("允许修改日历")
    ))
}

// ---------------------------------------------------------------------------------------
// The tools as the client sees them
// ---------------------------------------------------------------------------------------

fn tool(name: &str, title: &str, description: &str, schema: Value, annotations: Value) -> Value {
    json!({ "name": name, "title": title, "description": description, "inputSchema": schema, "annotations": annotations })
}

fn object(properties: Value, required: &[&str]) -> Value {
    let mut s = json!({ "type": "object", "properties": properties, "additionalProperties": false });
    if !required.is_empty() {
        s["required"] = json!(required);
    }
    s
}

fn looks() -> Value {
    json!({ "readOnlyHint": true, "openWorldHint": false })
}

fn changes(destructive: bool, idempotent: bool) -> Value {
    json!({ "readOnlyHint": false, "destructiveHint": destructive, "idempotentHint": idempotent, "openWorldHint": false })
}

/// The fields of a task, for add_tasks (`update` false) and update_task.
fn task_fields(update: bool) -> Map<String, Value> {
    let clear = |what: &str| if update { format!(" \"\" {what}.") } else { String::new() };
    let mut freqs: Vec<&str> = FREQS.split(", ").collect();
    if update {
        freqs.push("none");
    }
    let fields = json!({
        "title": { "type": "string", "description": "What to do, in one line." },
        "start": { "type": "string", "description": if update {
            "First day, YYYY-MM-DD. Without `end` the task is moved: it keeps its length."
        } else {
            "First day, YYYY-MM-DD. Required unless `todo` is set."
        } },
        "end": { "type": "string", "description": "Last day (inclusive), YYYY-MM-DD. Default: the same day as `start`." },
        "time": { "type": "string", "description": format!("Time of day, HH:MM (24-hour). Leave out for a task that lasts all day.{}", clear("makes it all day")) },
        "notes": { "type": "string", "description": format!("Free text.{}", clear("removes the notes")) },
        "color": { "type": "string", "description": format!(
            "One of {}, or #RRGGBB. Default: the user's default colour.{}",
            TASK_COLORS.iter().map(|c| c.key).collect::<Vec<_>>().join(", "),
            clear("removes the colour")
        ) },
        "group": { "type": "string", "description": format!("The group the task belongs to: its id or its exact name (see list_groups).{}", clear("takes it out of its group")) },
        "milestone": { "type": "boolean", "description": "A milestone: one day, drawn as a diamond in the Gantt chart." },
        "countdown": { "type": "boolean", "description": "Count the days down to this task in the calendar's countdown bar." },
        "repeat": {
            "type": "object",
            "description": if update {
                "Replaces the repeat rule; `start` is the first occurrence. {\"freq\": \"none\"} stops the repeating."
            } else {
                "Makes the task repeat; `start` is the first occurrence."
            },
            "properties": {
                "freq": { "type": "string", "enum": freqs, "description": "workday: Monday to Friday. legalWorkday: the working days of mainland China's official schedule (holidays off, make-up days on). lunarYearly: every year on the Chinese lunar date of `start`. monthLastDay: the last day of every month. ebbinghaus: seven review days (days 1, 2, 3, 5, 8, 16 and 31 counted from `start`)." },
                "interval": { "type": "integer", "minimum": 1, "maximum": 99, "description": "Every N days, weeks, months or years. Default 1." },
                "weekdays": { "type": "array", "items": { "type": "integer", "minimum": 0, "maximum": 6 }, "description": "weekly only: the days of the week, 0 = Sunday … 6 = Saturday. Default: the weekday of `start`." },
                "monthMode": { "type": "string", "enum": ["day", "nthWeekday"], "description": "monthly only: \"day\" = the same day of the month (default), \"nthWeekday\" = the same weekday of the month as `start` (the second Tuesday, the last Friday)." },
                "count": { "type": "integer", "minimum": 1, "maximum": 999, "description": "Ends after this many occurrences." },
                "until": { "type": "string", "description": "The last day an occurrence may start on, YYYY-MM-DD. Not together with `count`." }
            },
            "required": ["freq"],
            "additionalProperties": false
        },
        "reminders": { "type": "array", "maxItems": 5, "items": { "type": "integer", "minimum": 0 }, "description": "Minutes before the task at which to remind: 0 = at its time, 1440 = a day before (a task that lasts all day counts from the user's reminder time of day). [] = no reminders. Default: the user's default reminders." },
        "dependsOn": { "type": "array", "items": { "type": "string" }, "description": if update {
            "Predecessors in the Gantt chart: the ids of the tasks that come before this one. Replaces the list; [] removes them. Not for repeating tasks."
        } else {
            "Predecessors in the Gantt chart: the ids of the tasks that come before this one, or the `ref` of a task earlier in the same call. Not for repeating tasks."
        } },
        "done": { "type": "boolean", "description": "Done. Not for repeating tasks and to-dos without a day: tick those with set_task_done." },
        "progress": { "type": "integer", "minimum": 0, "maximum": 100, "description": "Percent done; 100 = done. Not for repeating tasks." }
    });
    match fields {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// The definitions for `tools/list`, always in this order.
pub fn list() -> Value {
    let mut add = task_fields(false);
    add.insert("todo".into(), json!({ "type": "string", "enum": ["monthly", "someday"], "description": "Makes this a to-do without a day instead of a task on the calendar: \"monthly\" = to do once every month, on whichever day; \"someday\" = to do once, no date. It takes title, notes, color, group and reminders only." }));
    add.insert("ref".into(), json!({ "type": "string", "description": "A label of your choice for this task, so that tasks after it in the same call can name it in `dependsOn`." }));
    let mut update = task_fields(true);
    update.insert("id".into(), json!({ "type": "string", "description": "The task's id." }));
    update.insert("occurrence".into(), json!({ "type": "string", "description": "Repeating tasks: change only the occurrence on this day (YYYY-MM-DD). It becomes a task of its own and the series skips that day. Without it the whole series changes." }));

    let status = json!({ "type": "string", "enum": ["all", "open", "done"], "description": "Default: all." });
    let group = json!({ "type": "string", "description": "Only tasks of this group: its id or its exact name." });
    json!([
        tool(
            "list_tasks",
            "List tasks",
            "What is on the calendar between two days (both included): every task and every occurrence of a repeating task, in date order. Without arguments: today and the six days after it. The answer begins with today's date and weekday.",
            object(json!({
                "from": { "type": "string", "description": "First day, YYYY-MM-DD. Default: today." },
                "to": { "type": "string", "description": "Last day, YYYY-MM-DD. Default: six days after `from`. At most 366 days after `from`." },
                "status": status,
                "group": group,
                "limit": { "type": "integer", "minimum": 1, "maximum": 500, "description": "Default 200." }
            }), &[]),
            looks()
        ),
        tool(
            "search_tasks",
            "Search tasks",
            "Finds tasks by words in the title, the notes or the group's name, whatever their date; the ones nearest to today come first. Each task is listed once (a repeating task with its occurrence nearest to today). To-dos without a day are found too.",
            object(json!({
                "query": { "type": "string", "description": "Words separated by spaces; all of them must be found. Leave out to list by the other filters only." },
                "status": status,
                "group": group,
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "Default 50." }
            }), &[]),
            looks()
        ),
        tool(
            "get_task",
            "Get task",
            "Everything about one task: the notes in full, reminders, the repeat rule, predecessors; for a repeating task also its next occurrences.",
            object(json!({ "id": { "type": "string", "description": "The task's id (from list_tasks or search_tasks)." } }), &["id"]),
            looks()
        ),
        tool("list_groups", "List groups", "The groups (projects) tasks can belong to, with how many tasks each has and how many of them are open.", object(json!({}), &[]), looks()),
        tool(
            "list_todos",
            "List to-dos",
            "The to-dos without a day, which are not on the calendar: the ones to do once every month (this month's, and last month's that were left open) and the ones to do someday.",
            object(json!({}), &[]),
            looks()
        ),
        tool(
            "get_days",
            "Calendar days",
            "Facts about days: the weekday, the Chinese lunar date, festivals, solar terms, the statutory holidays of mainland China, and whether the day is a working day (weekends moved to work count as working days). Without arguments: today.",
            object(json!({
                "from": { "type": "string", "description": "First day, YYYY-MM-DD. Default: today." },
                "to": { "type": "string", "description": "Last day, YYYY-MM-DD. Default: the same day as `from`. At most 400 days after `from`." },
                "only": { "type": "string", "enum": ["all", "special", "holidays"], "description": "all (default): every day. special: only days with a festival, a solar term or a statutory holiday. holidays: only statutory days off and make-up working days." }
            }), &[]),
            looks()
        ),
        tool(
            "add_tasks",
            "Add tasks",
            "Adds one task or several (50 at most) in one step: all of them, or none when one is not valid. The answer has the new tasks' ids.",
            object(json!({
                "tasks": { "type": "array", "minItems": 1, "maxItems": MAX_ADD, "description": "The tasks to add.", "items": { "type": "object", "properties": add, "required": ["title"], "additionalProperties": false } }
            }), &["tasks"]),
            changes(false, false)
        ),
        tool(
            "update_task",
            "Update task",
            "Changes a task: only the fields given, the others stay as they are. Moves it (`start`), renames it, changes its time, notes, colour, group, repeat rule, reminders or progress. To tick it done use set_task_done.",
            object(Value::Object(update), &["id"]),
            changes(true, true)
        ),
        tool(
            "set_task_done",
            "Mark task done",
            "Ticks a task as done, or unticks it. A repeating task is ticked one occurrence at a time: give that occurrence's `date`. A monthly to-do is ticked for the month of `date` (default: this month), a someday to-do once.",
            object(json!({
                "id": { "type": "string", "description": "The task's id." },
                "done": { "type": "boolean", "description": "Default true. false unticks." },
                "date": { "type": "string", "description": "YYYY-MM-DD. Repeating tasks: the occurrence (required). To-dos without a day: the day it was done (default today). Other tasks: not needed." }
            }), &["id"]),
            changes(false, true)
        ),
        tool(
            "delete_task",
            "Delete task",
            "Deletes a task. For a repeating task say how much with `scope`: one occurrence, that occurrence and all later ones, or the whole task. The user can take a deletion back in FluentCal.",
            object(json!({
                "id": { "type": "string", "description": "The task's id." },
                "scope": { "type": "string", "enum": ["this", "future", "all"], "description": "Repeating tasks only, and required for them. this: only the occurrence on `date`. future: the occurrences on and after `date`. all: the whole task." },
                "date": { "type": "string", "description": "YYYY-MM-DD, with scope this or future." }
            }), &["id"]),
            changes(true, true)
        ),
        tool(
            "add_group",
            "Add group",
            "Adds a group (a project) tasks can be put in. When a group of that name exists already, the answer is that group.",
            object(json!({
                "name": { "type": "string", "description": "The group's name." },
                "color": { "type": "string", "description": format!("One of {}, or #RRGGBB. Default: the next colour of the palette.", TASK_COLORS.iter().map(|c| c.key).collect::<Vec<_>>().join(", ")) }
            }), &["name"]),
            changes(false, true)
        )
    ])
}

// ---------------------------------------------------------------------------------------
// Reading arguments
// ---------------------------------------------------------------------------------------

/// A misspelt argument must not be ignored without a word.
fn only(a: &Args, known: &[&str]) -> Result<(), String> {
    match a.keys().find(|k| !known.contains(&k.as_str())) {
        Some(k) if known.is_empty() => Err(format!("unknown argument \"{k}\": this takes no arguments")),
        Some(k) => Err(format!("unknown argument \"{k}\". The arguments are: {}.", known.join(", "))),
        None => Ok(()),
    }
}

/// An argument that is there (`null` counts as left out).
fn given<'a>(a: &'a Args, k: &str) -> Option<&'a Value> {
    a.get(k).filter(|v| !v.is_null())
}

fn text<'a>(a: &'a Args, k: &str) -> Result<Option<&'a str>, String> {
    match given(a, k) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(v) => Err(format!("\"{k}\" must be a string, got {v}")),
    }
}

fn required<'a>(a: &'a Args, k: &str) -> Result<&'a str, String> {
    text(a, k)?.map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| format!("\"{k}\" is required"))
}

/// A whole number, also when it arrives as 5.0.
fn whole(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 9e15).map(|f| f as i64))
}

fn int(a: &Args, k: &str, min: i64, max: i64) -> Result<Option<i64>, String> {
    match given(a, k) {
        None => Ok(None),
        Some(v) => whole(v).filter(|n| (min..=max).contains(n)).map(Some).ok_or_else(|| format!("\"{k}\" must be a whole number from {min} to {max}, got {v}")),
    }
}

fn flag(a: &Args, k: &str) -> Result<Option<bool>, String> {
    match given(a, k) {
        None => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(v) => Err(format!("\"{k}\" must be true or false, got {v}")),
    }
}

fn choice<'a>(a: &Args, k: &str, options: &[&'a str]) -> Result<Option<&'a str>, String> {
    let Some(s) = text(a, k)? else { return Ok(None) };
    options.iter().find(|o| **o == s.trim()).copied().map(Some).ok_or_else(|| format!("\"{k}\" must be one of {}, got \"{s}\"", options.join(", ")))
}

/// A day. Years far from now are a slip more often than a plan, and the lunar calendar does
/// not reach everywhere.
fn date_of(k: &str, v: &Value) -> Result<String, String> {
    let s = v.as_str().map(str::trim).unwrap_or("");
    if is_iso_date(s) && (1900..2200).contains(&parts(s).0) {
        Ok(s.to_owned())
    } else {
        Err(format!("\"{k}\" must be a day written YYYY-MM-DD (1900 to 2199), got {v}"))
    }
}

fn date(a: &Args, k: &str) -> Result<Option<String>, String> {
    given(a, k).map(|v| date_of(k, v)).transpose()
}

/// "9:30", "09:30" and "09:30:00" are 09:30; "" is no time (all day).
fn time_of(v: &Value) -> Result<Option<String>, String> {
    let bad = || format!("\"time\" must be a time of day written HH:MM (24-hour), got {v}");
    let s = v.as_str().map(str::trim).ok_or_else(bad)?;
    if s.is_empty() {
        return Ok(None);
    }
    let p: Vec<&str> = s.split(':').collect();
    let digits = |x: &str, len: std::ops::RangeInclusive<usize>| len.contains(&x.len()) && x.bytes().all(|b| b.is_ascii_digit());
    if !(2..=3).contains(&p.len()) || !digits(p[0], 1..=2) || !p[1..].iter().all(|x| digits(*x, 2..=2)) {
        return Err(bad());
    }
    let (h, m): (u32, u32) = (p[0].parse().unwrap_or(99), p[1].parse().unwrap_or(99));
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(Some(format!("{h:02}:{m:02}")))
}

fn title_of(v: &Value) -> Result<String, String> {
    let s = v.as_str().ok_or_else(|| format!("\"title\" must be a string, got {v}"))?;
    let s = s.replace(['\r', '\n', '\t'], " ").trim().to_owned();
    if s.is_empty() {
        return Err("\"title\" cannot be empty".into());
    }
    if s.chars().count() > 500 {
        return Err("\"title\" is too long (500 characters at most); put the details in \"notes\"".into());
    }
    Ok(s)
}

/// A palette key or #RRGGBB; "" is none.
fn color_of(v: &Value) -> Result<Option<String>, String> {
    let s = v.as_str().map(str::trim).ok_or_else(|| format!("\"color\" must be a string, got {v}"))?;
    if s.is_empty() {
        return Ok(None);
    }
    let low = s.to_ascii_lowercase();
    let low = if low == "grey" { "gray" } else { low.as_str() };
    if let Some(c) = TASK_COLORS.iter().find(|c| c.key == low) {
        return Ok(Some(c.key.to_owned()));
    }
    match s.strip_prefix('#') {
        Some(hex) if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => Ok(Some(format!("#{}", hex.to_ascii_uppercase()))),
        _ => Err(format!("\"color\" must be one of {}, or #RRGGBB; got \"{s}\"", TASK_COLORS.iter().map(|c| c.key).collect::<Vec<_>>().join(", "))),
    }
}

fn by_order(a: &&Group, b: &&Group) -> std::cmp::Ordering {
    a.order.total_cmp(&b.order).then_with(|| a.id.cmp(&b.id))
}

/// A group by its id or its name.
fn find_group<'a>(doc: &'a Doc, s: &str) -> Result<&'a Group, String> {
    let s = s.trim();
    if let Some(g) = doc.groups.get(s) {
        return Ok(g);
    }
    let mut hits: Vec<&Group> = doc.groups.values().filter(|g| g.name == s).collect();
    if hits.is_empty() {
        hits = doc.groups.values().filter(|g| g.name.to_lowercase() == s.to_lowercase()).collect();
    }
    match hits.as_slice() {
        [g] => Ok(*g),
        [] => {
            let mut all: Vec<&Group> = doc.groups.values().collect();
            all.sort_by(by_order);
            let names: Vec<String> = all.iter().map(|g| format!("\"{}\"", g.name)).collect();
            Err(if names.is_empty() {
                format!("there is no group \"{s}\": there are no groups yet. add_group makes one.")
            } else {
                format!("there is no group \"{s}\". The groups are: {}. add_group makes a new one.", names.join(", "))
            })
        }
        many => Err(format!("{} groups are called \"{s}\": give the id instead ({})", many.len(), many.iter().map(|g| g.id.as_str()).collect::<Vec<_>>().join(", "))),
    }
}

/// The `group` filter of the tools that look: the group's id.
fn group_filter(doc: &Doc, a: &Args) -> Result<Option<String>, String> {
    match text(a, "group")?.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Ok(Some(find_group(doc, s)?.id.clone())),
        None => Ok(None),
    }
}

fn no_task(id: &str) -> String {
    format!("there is no task with the id \"{id}\" (deleted meanwhile, or not an id). list_tasks and search_tasks give the ids.")
}

// ---------------------------------------------------------------------------------------
// What a task looks like in an answer
// ---------------------------------------------------------------------------------------

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

fn todo_kind(t: &Task) -> Option<&'static str> {
    flex::kind(t).map(|k| if k == flex::MONTH { "monthly" } else { "someday" })
}

/// What every listing says of a task, after its id, title and dates.
fn common(doc: &Doc, t: &Task, m: &mut Map<String, Value>, notes: usize) {
    if let Some(g) = t.group_id.as_ref().and_then(|g| doc.groups.get(g)) {
        m.insert("group".into(), json!(g.name));
    }
    if let Some(c) = &t.color {
        m.insert("color".into(), json!(c));
    }
    if let Some(n) = t.notes.as_deref().filter(|n| !n.trim().is_empty()) {
        m.insert("notes".into(), json!(clip(n, notes)));
    }
}

/// A task (or one occurrence of it) in a list.
fn row(doc: &Doc, t: &Task, date: &str, end: &str, done: bool) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(t.id));
    m.insert("title".into(), json!(t.title));
    m.insert("date".into(), json!(date));
    if end > date {
        m.insert("end".into(), json!(end));
    }
    if let Some(time) = &t.time {
        m.insert("time".into(), json!(time));
    }
    m.insert("done".into(), json!(done));
    if t.repeat.is_some() {
        m.insert("repeats".into(), json!(describe_rule(t.repeat.as_ref(), &t.start)));
    }
    if let Some(k) = todo_kind(t) {
        m.insert("todo".into(), json!(k));
    }
    if t.is_milestone() {
        m.insert("milestone".into(), json!(true));
    }
    if t.repeat.is_none() && !flex::is_flex(t) && t.progress > 0.0 && t.progress < 100.0 {
        m.insert("progress".into(), json!(t.progress.round() as i64));
    }
    common(doc, t, &mut m, 200);
    Value::Object(m)
}

/// A to-do without a day in a list.
fn todo_row(doc: &Doc, t: &Task) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("id".into(), json!(t.id));
    m.insert("title".into(), json!(t.title));
    common(doc, t, &mut m, 200);
    m
}

fn rule_view(t: &Task) -> Option<Value> {
    let r = t.repeat.as_ref()?;
    let mut m = Map::new();
    m.insert("freq".into(), serde_json::to_value(r.freq?).unwrap_or(Value::Null));
    if let Some(i) = r.interval.filter(|i| *i > 1) {
        m.insert("interval".into(), json!(i));
    }
    if let Some(w) = r.weekdays.as_ref().filter(|w| !w.is_empty()) {
        m.insert("weekdays".into(), json!(w));
    }
    if let Some(mode) = &r.month_mode {
        m.insert("monthMode".into(), json!(mode));
    }
    if let Some(c) = r.count.filter(|c| *c > 0) {
        m.insert("count".into(), json!(c));
    } else if let Some(u) = &r.until {
        m.insert("until".into(), json!(u));
    }
    m.insert("inWords".into(), json!(describe_rule(Some(r), &t.start)));
    Some(Value::Object(m))
}

/// The next occurrences from `from` on: (date, done).
fn upcoming(t: &Task, from: &str, n: usize) -> Vec<(String, bool)> {
    occurrences(t, from, &add_days(from, 800), n + 1).into_iter().filter(|o| o.start.as_str() >= from).take(n).map(|o| (o.date, o.done)).collect()
}

/// Everything about a task (get_task, and what add and update answer).
fn full(doc: &Doc, t: &Task, today: &str) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(t.id));
    m.insert("title".into(), json!(t.title));
    if let Some(kind) = todo_kind(t) {
        m.insert("todo".into(), json!(kind));
        if kind == "monthly" {
            m.insert("since".into(), json!(flex::month_key(&t.start)));
            // month → the day it was done
            m.insert("doneMonths".into(), json!(t.flex_done.clone().unwrap_or_default()));
            m.insert("doneThisMonth".into(), json!(flex::slot_done(t, &flex::month_slot(today)).is_some()));
        } else {
            m.insert("done".into(), json!(t.done));
            if let Some(d) = flex::slot_done(t, flex::SOMEDAY_SLOT) {
                m.insert("doneOn".into(), json!(d));
            }
        }
    } else {
        m.insert("start".into(), json!(t.start));
        m.insert("end".into(), json!(task_end(t)));
        if let Some(time) = &t.time {
            m.insert("time".into(), json!(time));
        }
        match rule_view(t) {
            Some(rule) => {
                m.insert("repeat".into(), rule);
                let next: Vec<Value> = upcoming(t, today, 5).into_iter().map(|(date, done)| json!({ "date": date, "done": done })).collect();
                m.insert("nextOccurrences".into(), json!(next));
                if let Some(ex) = t.ex_dates.as_ref().filter(|e| !e.is_empty()) {
                    m.insert("skipped".into(), json!(ex));
                }
            }
            None => {
                m.insert("done".into(), json!(t.done));
                // the day it was ticked (kept since 0.3.17)
                if let Some(d) = crate::tasks::done_day(t, &t.start) {
                    m.insert("doneOn".into(), json!(d));
                }
                m.insert("progress".into(), json!(t.progress.round() as i64));
            }
        }
        if t.is_milestone() {
            m.insert("milestone".into(), json!(true));
        }
        if t.countdown == Some(true) {
            m.insert("countdown".into(), json!(true));
        }
        if !t.deps().is_empty() {
            let deps: Vec<Value> = t.deps().iter().map(|id| json!({ "id": id, "title": doc.tasks.get(id).map(|d| d.title.as_str()) })).collect();
            m.insert("dependsOn".into(), json!(deps));
        }
    }
    if let Some(g) = t.group_id.as_ref().and_then(|g| doc.groups.get(g)) {
        m.insert("group".into(), json!({ "id": g.id, "name": g.name }));
    }
    if let Some(c) = &t.color {
        m.insert("color".into(), json!(c));
    }
    if let Some(r) = t.reminders.as_ref().filter(|r| !r.is_empty()) {
        m.insert("reminders".into(), json!(r));
    }
    if let Some(n) = t.notes.as_deref().filter(|n| !n.is_empty()) {
        m.insert("notes".into(), json!(n));
    }
    Value::Object(m)
}

// ---------------------------------------------------------------------------------------
// Looking
// ---------------------------------------------------------------------------------------

fn list_tasks(doc: &Doc, a: &Args) -> Res {
    only(a, &["from", "to", "status", "group", "limit"])?;
    let today = today();
    let from = date(a, "from")?.unwrap_or_else(|| today.clone());
    let to = date(a, "to")?.unwrap_or_else(|| add_days(&from, 6));
    if to < from {
        return Err(format!("\"to\" ({to}) is before \"from\" ({from})"));
    }
    if diff_days(&from, &to) > 366 {
        return Err("at most 366 days at a time: ask for a shorter stretch, or use search_tasks to find a task whatever its date".into());
    }
    let status = choice(a, "status", &["all", "open", "done"])?.unwrap_or("all");
    let group = group_filter(doc, a)?;
    let limit = int(a, "limit", 1, 500)?.unwrap_or(200) as usize;

    let mut found = Vec::new();
    for t in doc.tasks.values() {
        if group.is_some() && t.group_id != group {
            continue;
        }
        for o in occurrences(t, &from, &to, 400) {
            if (status == "open" && o.done) || (status == "done" && !o.done) {
                continue;
            }
            found.push(o);
        }
    }
    found.sort_by(|x, y| {
        (x.start.as_str(), x.task.time.as_deref().unwrap_or(""))
            .cmp(&(y.start.as_str(), y.task.time.as_deref().unwrap_or("")))
            .then_with(|| x.task.order.total_cmp(&y.task.order))
            .then_with(|| x.task.title.cmp(&y.task.title))
            .then_with(|| x.task.id.cmp(&y.task.id))
    });
    let total = found.len();
    let tasks: Vec<Value> = found.iter().take(limit).map(|o| row(doc, o.task, &o.date, &o.end, o.done)).collect();
    let mut out = json!({ "today": today, "weekday": WEEKDAYS[weekday(&today) as usize], "from": from, "to": to, "count": total, "tasks": tasks });
    if total > limit {
        out["truncated"] = json!(true);
    }
    Ok(out)
}

fn search_tasks(doc: &Doc, a: &Args) -> Res {
    only(a, &["query", "status", "group", "limit"])?;
    let today = today();
    let status = choice(a, "status", &["all", "open", "done"])?.unwrap_or("all");
    let group = group_filter(doc, a)?;
    let limit = int(a, "limit", 1, 200)?.unwrap_or(50) as usize;
    let opts = SearchOptions {
        q: text(a, "query")?.unwrap_or("").to_owned(),
        range: "all".into(),
        status: match status {
            "open" => "undone",
            other => other,
        }
        .into(),
        color: String::new(),
    };
    let tasks: Vec<Task> = doc.tasks.values().filter(|t| group.is_none() || t.group_id == group).cloned().collect();
    let hits = search(&tasks, &doc.groups, &opts, &today);
    let total = hits.len();
    let rows: Vec<Value> = hits
        .iter()
        .take(limit)
        .map(|h| {
            let t = &h.task;
            if flex::is_flex(t) {
                let mut m = todo_row(doc, t);
                m.insert("todo".into(), json!(todo_kind(t)));
                m.insert("done".into(), json!(t.done));
                return Value::Object(m);
            }
            match occurrence_on(t, &h.date).filter(|_| t.repeat.is_some()) {
                Some(o) => row(doc, t, &o.date, &o.end, o.done),
                None => row(doc, t, &t.start, task_end(t), t.done),
            }
        })
        .collect();
    let mut out = json!({ "today": today, "count": total, "tasks": rows });
    if total > limit {
        out["truncated"] = json!(true);
    }
    Ok(out)
}

fn get_task(doc: &Doc, a: &Args) -> Res {
    only(a, &["id"])?;
    let id = required(a, "id")?;
    let t = doc.tasks.get(id).ok_or_else(|| no_task(id))?;
    Ok(full(doc, t, &today()))
}

fn list_groups(doc: &Doc, a: &Args) -> Res {
    only(a, &[])?;
    let count = |id: Option<&str>| {
        let of = doc.tasks.values().filter(|t| t.group_id.as_deref().filter(|g| doc.groups.contains_key(*g)) == id);
        let (mut all, mut open) = (0, 0);
        for t in of {
            all += 1;
            open += usize::from(!t.done);
        }
        (all, open)
    };
    let mut groups: Vec<&Group> = doc.groups.values().collect();
    groups.sort_by(by_order);
    let list: Vec<Value> = groups
        .iter()
        .map(|g| {
            let (all, open) = count(Some(g.id.as_str()));
            json!({ "id": g.id, "name": g.name, "color": g.color, "tasks": all, "open": open })
        })
        .collect();
    let (all, open) = count(None);
    Ok(json!({ "groups": list, "withoutGroup": { "tasks": all, "open": open } }))
}

fn list_todos(doc: &Doc, a: &Args) -> Res {
    only(a, &[])?;
    let today = today();
    let side = flex::side(doc.tasks.values(), &today);
    let monthly: Vec<Value> = side
        .month
        .iter()
        .map(|(t, done)| {
            let mut m = todo_row(doc, t);
            m.insert("done".into(), json!(done.is_some()));
            if let Some(d) = done {
                m.insert("doneOn".into(), json!(d));
            }
            Value::Object(m)
        })
        .collect();
    let plain = |list: &[&Task]| -> Vec<Value> { list.iter().map(|t| Value::Object(todo_row(doc, t))).collect() };
    Ok(json!({
        "today": today,
        "month": flex::month_key(&today),
        "monthly": monthly,
        "overdueMonth": side.overdue_slot.trim_start_matches("m:"),
        "overdue": plain(&side.overdue),
        "someday": plain(&side.someday),
    }))
}

fn get_days(doc: &Doc, a: &Args) -> Res {
    only(a, &["from", "to", "only"])?;
    let today = today();
    let from = date(a, "from")?.unwrap_or_else(|| today.clone());
    let to = date(a, "to")?.unwrap_or_else(|| from.clone());
    if to < from {
        return Err(format!("\"to\" ({to}) is before \"from\" ({from})"));
    }
    let span = diff_days(&from, &to);
    if span > 400 {
        return Err("at most 400 days at a time".into());
    }
    let which = choice(a, "only", &["all", "special", "holidays"])?.unwrap_or("all");
    const MOST: usize = 120;
    let mut days = Vec::new();
    let mut more = false;
    for i in 0..=span {
        let d = add_days(&from, i);
        let info = lunar::display_info(&d);
        let special = !info.festivals.is_empty() || info.term.is_some() || info.legal.is_some();
        if (which == "special" && !special) || (which == "holidays" && info.legal.is_none()) {
            continue;
        }
        if days.len() == MOST {
            more = true;
            break;
        }
        let mut m = Map::new();
        m.insert("date".into(), json!(d));
        m.insert("weekday".into(), json!(WEEKDAYS[weekday(&d) as usize]));
        m.insert("lunar".into(), json!(info.lunar_label));
        if !info.festivals.is_empty() {
            m.insert("festivals".into(), json!(info.festivals));
        }
        if let Some(term) = &info.term {
            m.insert("solarTerm".into(), json!(term));
        }
        if let Some(l) = &info.legal {
            m.insert("holiday".into(), json!(l.name));
            // a day off, or a weekend day moved to work
            m.insert("holidayKind".into(), json!(if l.work { "work" } else { "off" }));
        }
        m.insert("workday".into(), json!(!lunar::is_rest_day(&d, &doc.settings.calendar.rest_days)));
        days.push(Value::Object(m));
    }
    let mut out = json!({ "today": today, "from": from, "to": to, "days": days });
    if more {
        out["truncated"] = json!(true);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Changing
// ---------------------------------------------------------------------------------------

/// What a task's fields are checked against.
struct Ctx<'a> {
    doc: &'a Doc,
    /// add_tasks: the labels (`ref`) of the tasks before this one in the call → their ids …
    refs: &'a BTreeMap<String, String>,
    /// … and those tasks.
    made: &'a [Task],
    adding: bool,
}

const TASK_KEYS: [&str; 14] = ["title", "start", "end", "time", "notes", "color", "group", "milestone", "countdown", "repeat", "reminders", "dependsOn", "done", "progress"];
/// What a to-do without a day does not have.
const NOT_FOR_TODOS: [&str; 9] = ["start", "end", "time", "repeat", "milestone", "countdown", "dependsOn", "done", "progress"];

fn rule_of(v: &Value, start: &str) -> Result<Option<RepeatRule>, String> {
    let r = v.as_object().ok_or_else(|| format!("\"repeat\" must be an object such as {{\"freq\": \"weekly\"}}, got {v}"))?;
    let inner = |e: String| format!("repeat: {e}");
    only(r, &["freq", "interval", "weekdays", "monthMode", "count", "until"]).map_err(inner)?;
    let name = text(r, "freq").map_err(inner)?.map(str::trim).unwrap_or("");
    if name == "none" {
        return Ok(None);
    }
    let freq = crate::editor::freq_of(name).ok_or_else(|| format!("repeat: \"freq\" must be one of {FREQS}; got \"{name}\""))?;
    let interval = int(r, "interval", 1, 99).map_err(inner)?.unwrap_or(1) as u32;
    if interval > 1 && matches!(freq, Freq::Workday | Freq::LegalWorkday | Freq::Ebbinghaus) {
        return Err(format!("repeat: \"interval\" does not go with {name}"));
    }
    let weekdays = match given(r, "weekdays") {
        None => None,
        Some(Value::Array(list)) => {
            if freq != Freq::Weekly {
                return Err(format!("repeat: \"weekdays\" only goes with weekly, not {name}"));
            }
            let mut days = Vec::new();
            for x in list {
                days.push(whole(x).filter(|n| (0..=6).contains(n)).ok_or_else(|| format!("repeat: a weekday is a number from 0 (Sunday) to 6 (Saturday), got {x}"))? as u32);
            }
            days.sort_unstable();
            days.dedup();
            (!days.is_empty()).then_some(days)
        }
        Some(v) => return Err(format!("repeat: \"weekdays\" must be a list of numbers from 0 (Sunday) to 6 (Saturday), got {v}")),
    };
    // the editor's weekly rule always says its days
    let weekdays = (freq == Freq::Weekly).then(|| weekdays.unwrap_or_else(|| vec![weekday(start)]));
    let month_mode = choice(r, "monthMode", &["day", "nthWeekday"]).map_err(inner)?;
    if month_mode.is_some() && freq != Freq::Monthly {
        return Err(format!("repeat: \"monthMode\" only goes with monthly, not {name}"));
    }
    let count = int(r, "count", 1, 999).map_err(inner)?.map(|n| n as u32);
    let until = date(r, "until").map_err(inner)?;
    if count.is_some() && until.is_some() {
        return Err("repeat: give \"count\" or \"until\", not both".into());
    }
    if freq == Freq::Ebbinghaus && (count.is_some() || until.is_some()) {
        return Err("repeat: ebbinghaus always has its seven review days; \"count\" and \"until\" do not go with it".into());
    }
    if let Some(u) = until.as_deref().filter(|u| *u < start) {
        return Err(format!("repeat: \"until\" ({u}) is before \"start\" ({start})"));
    }
    Ok(Some(RepeatRule { freq: Some(freq), interval: Some(interval), weekdays, month_mode: month_mode.map(str::to_owned), count, until, ..Default::default() }))
}

fn reminders_of(v: &Value) -> Result<Option<Vec<i64>>, String> {
    let list = v.as_array().ok_or_else(|| format!("\"reminders\" must be a list of minutes, got {v}"))?;
    if list.len() > 5 {
        return Err("\"reminders\": five at most".into());
    }
    let mut out = Vec::new();
    for x in list {
        out.push(whole(x).filter(|n| (0..=525_600).contains(n)).ok_or_else(|| format!("\"reminders\": a reminder is a whole number of minutes before the task (0 to 525600), got {x}"))?);
    }
    out.sort_unstable();
    out.dedup();
    Ok((!out.is_empty()).then_some(out))
}

fn deps_of(c: &Ctx, t: &Task, v: &Value) -> Result<Option<Vec<String>>, String> {
    let list = v.as_array().ok_or_else(|| format!("\"dependsOn\" must be a list of task ids, got {v}"))?;
    let mut out: Vec<String> = Vec::new();
    for x in list {
        let s = x.as_str().map(str::trim).filter(|s| !s.is_empty()).ok_or_else(|| format!("\"dependsOn\" must be a list of task ids, got {v}"))?;
        let id = c.refs.get(s).map_or(s, String::as_str);
        let dep = c.made.iter().find(|m| m.id == id).or_else(|| c.doc.tasks.get(id)).ok_or_else(|| {
            let hint = if c.adding { " (an id, or the \"ref\" of a task earlier in this call)" } else { "" };
            format!("\"dependsOn\": there is no task \"{s}\"{hint}")
        })?;
        if id == t.id {
            return Err("\"dependsOn\": a task cannot come after itself".into());
        }
        if dep.repeat.is_some() || flex::is_flex(dep) {
            return Err(format!("\"dependsOn\": \"{}\" {}, so nothing can come after it", dep.title, if dep.repeat.is_some() { "repeats" } else { "is a to-do without a day" }));
        }
        if !c.adding && would_cycle(&c.doc.tasks, id, &t.id) {
            return Err(format!("\"dependsOn\": \"{}\" already comes after this task; that would go round in a circle", dep.title));
        }
        if !out.iter().any(|o| o == id) {
            out.push(id.to_owned());
        }
    }
    Ok((!out.is_empty()).then_some(out))
}

/// Applies the fields given to a task, then the editor's rules. `null`, like an empty string
/// or list, clears a field.
fn patch(c: &Ctx, t: &mut Task, f: &Args) -> Result<(), String> {
    if flex::is_flex(t) {
        if let Some(k) = NOT_FOR_TODOS.iter().find(|k| f.contains_key(**k)) {
            let tick = if *k == "done" { " (tick it with set_task_done)" } else { "" };
            return Err(format!("\"{k}\" does not go with a to-do without a day{tick}; it takes title, notes, color, group and reminders"));
        }
    }
    if let Some(v) = f.get("title") {
        t.title = title_of(v)?;
    }
    match f.get("start") {
        None => {}
        Some(Value::Null) => return Err("\"start\" cannot be removed: a task has a day. (A to-do without a day is added with \"todo\".)".into()),
        Some(v) => {
            let start = date_of("start", v)?;
            // moving keeps the length (the editor's start field)
            let days = diff_days(&t.start, task_end(t)).max(0);
            t.end = add_days(&start, days);
            t.start = start;
        }
    }
    match f.get("end") {
        None => {}
        Some(Value::Null) => t.end = t.start.clone(),
        Some(v) if v.as_str().is_some_and(|s| s.trim().is_empty()) => t.end = t.start.clone(),
        Some(v) => t.end = date_of("end", v)?,
    }
    if !flex::is_flex(t) && t.end < t.start {
        return Err(format!("\"end\" ({}) is before \"start\" ({})", t.end, t.start));
    }
    match f.get("time") {
        None => {}
        Some(Value::Null) => t.time = None,
        Some(v) => t.time = time_of(v)?,
    }
    match f.get("notes") {
        None => {}
        Some(Value::Null) => t.notes = None,
        Some(Value::String(s)) if s.chars().count() <= 20_000 => t.notes = (!s.trim().is_empty()).then(|| s.clone()),
        Some(Value::String(_)) => return Err("\"notes\" is too long (20000 characters at most)".into()),
        Some(v) => return Err(format!("\"notes\" must be a string, got {v}")),
    }
    match f.get("color") {
        None => {}
        Some(Value::Null) => t.color = None,
        Some(v) => t.color = color_of(v)?,
    }
    match f.get("group") {
        None => {}
        Some(Value::Null) => t.group_id = None,
        Some(Value::String(s)) if s.trim().is_empty() => t.group_id = None,
        Some(Value::String(s)) => t.group_id = Some(find_group(c.doc, s)?.id.clone()),
        Some(v) => return Err(format!("\"group\" must be a string, got {v}")),
    }
    if f.contains_key("milestone") {
        t.milestone = flag(f, "milestone")?.unwrap_or(false).then_some(true);
    }
    if f.contains_key("countdown") {
        t.countdown = flag(f, "countdown")?.unwrap_or(false).then_some(true);
    }
    match f.get("repeat") {
        None => {}
        Some(Value::Null) => t.repeat = None,
        Some(v) => t.repeat = rule_of(v, &t.start)?,
    }
    match f.get("reminders") {
        None => {}
        Some(Value::Null) => t.reminders = None,
        Some(v) => t.reminders = reminders_of(v)?,
    }
    match f.get("dependsOn") {
        None => {}
        Some(Value::Null) => t.deps = None,
        Some(v) => t.deps = deps_of(c, t, v)?,
    }
    let done = flag(f, "done")?;
    let progress = int(f, "progress", 0, 100)?;
    if t.repeat.is_some() && (done.is_some() || progress.is_some()) {
        return Err(format!(
            "a repeating task is ticked one occurrence at a time: {}set_task_done with the occurrence's date",
            if c.adding { "add it without \"done\" and \"progress\", then use " } else { "use " }
        ));
    }
    if let Some(p) = progress {
        t.progress = p as f64;
        t.done = p >= 100;
    }
    if let Some(d) = done {
        // the editor's 已完成 box
        t.done = d;
        t.progress = if d {
            100.0
        } else if t.progress == 100.0 {
            0.0
        } else {
            t.progress
        };
    }

    // what the editor does on 保存
    if t.is_milestone() || t.end < t.start {
        t.end = t.start.clone();
    }
    normalize_daily_range(t);
    if t.repeat.is_some() {
        if !t.deps().is_empty() {
            return Err("a repeating task cannot come after other tasks: leave out \"dependsOn\" (or empty it), or do not let it repeat".into());
        }
        if !c.adding && c.doc.tasks.values().any(|o| o.deps().iter().any(|d| *d == t.id)) {
            return Err("other tasks come after this one in the Gantt chart, so it cannot repeat".into());
        }
    }
    Ok(())
}

/// A new task from its fields. `make_task` gives it the user's default colour and reminders
/// where it has none; an emptied field ("", []) means none all the same.
fn build(c: &Ctx, f: &Args, today: &str, order: f64, now: i64) -> Result<Task, String> {
    let mut keys: Vec<&str> = TASK_KEYS.to_vec();
    keys.extend(["todo", "ref"]);
    only(f, &keys)?;
    let title = title_of(f.get("title").unwrap_or(&Value::Null)).map_err(|e| if f.contains_key("title") { e } else { "\"title\" is required".into() })?;
    let mut t = match choice(f, "todo", &["monthly", "someday"])? {
        Some(kind) => flex::new_task(if kind == "monthly" { flex::MONTH } else { flex::SOMEDAY }, &title, today),
        None => {
            let start = date(f, "start")?.ok_or("\"start\" (the day, YYYY-MM-DD) is required; for a to-do without a day set \"todo\"")?;
            Task { title, end: start.clone(), start, ..Default::default() }
        }
    };
    patch(c, &mut t, f)?;
    let (color, reminders) = (t.color.is_some() || !f.contains_key("color"), t.reminders.is_some() || !f.contains_key("reminders"));
    let mut t = make_task(c.doc, t, Some(order), now);
    if !color {
        t.color = None;
    }
    if !reminders {
        t.reminders = None;
    }
    Ok(t)
}

fn add_tasks(host: &dyn Host, a: &Args) -> Res {
    only(a, &["tasks"])?;
    let items = match a.get("tasks") {
        Some(Value::Array(list)) if list.is_empty() => return Err("\"tasks\" is empty: give at least one task".into()),
        Some(Value::Array(list)) if list.len() > MAX_ADD => return Err(format!("{} tasks at once is too many: {MAX_ADD} at most per call", list.len())),
        Some(Value::Array(list)) => list.clone(),
        _ => return Err("\"tasks\" (a list of the tasks to add) is required".into()),
    };
    host.change(Box::new(move |doc: &Doc| {
        let (today, now) = (today(), now_ms());
        let last = doc.tasks.values().map(|t| t.order).fold(0.0, f64::max);
        let mut refs: BTreeMap<String, String> = BTreeMap::new();
        let mut made: Vec<Task> = Vec::new();
        let mut rows = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let at = |e: String| if items.len() == 1 { e } else { format!("tasks[{i}]: {e}") };
            let f = item.as_object().ok_or_else(|| at(format!("a task must be an object, got {item}")))?;
            let label = text(f, "ref").map_err(at)?.map(str::trim).filter(|s| !s.is_empty());
            if label.is_some_and(|l| refs.contains_key(l)) {
                return Err(at(format!("the \"ref\" \"{}\" is used twice", label.unwrap_or_default())));
            }
            let c = Ctx { doc, refs: &refs, made: &made, adding: true };
            let t = build(&c, f, &today, last + 1.0 + i as f64, now).map_err(at)?;
            let mut view = if flex::is_flex(&t) {
                let mut m = todo_row(doc, &t);
                m.insert("todo".into(), json!(todo_kind(&t)));
                Value::Object(m)
            } else {
                row(doc, &t, &t.start, task_end(&t), t.done)
            };
            if let Some(l) = label {
                view["ref"] = json!(l);
                refs.insert(l.to_owned(), t.id.clone());
            }
            rows.push(view);
            made.push(t);
        }
        let mut edit = Edit::default();
        for t in &made {
            edit.put_task(t);
        }
        Ok((edit, json!({ "added": made.len(), "tasks": rows })))
    }))
}

fn update_task(host: &dyn Host, a: &Args) -> Res {
    let mut keys: Vec<&str> = TASK_KEYS.to_vec();
    keys.extend(["id", "occurrence"]);
    only(a, &keys)?;
    let id = required(a, "id")?.to_owned();
    if !a.keys().any(|k| k != "id" && k != "occurrence") {
        return Err("nothing to change: give at least one field besides \"id\"".into());
    }
    let occurrence = date(a, "occurrence")?;
    let f = a.clone();
    host.change(Box::new(move |doc: &Doc| {
        let (today, now) = (today(), now_ms());
        let stored = doc.tasks.get(&id).ok_or_else(|| no_task(&id))?;
        let none = BTreeMap::new();
        let c = Ctx { doc, refs: &none, made: &[], adding: false };
        let Some(day) = occurrence else {
            let mut t = stored.clone();
            patch(&c, &mut t, &f)?;
            if t == *stored {
                return Ok((Edit::default(), json!({ "unchanged": true, "task": full(doc, stored, &today) })));
            }
            let edit = save_task(doc, t.clone(), now);
            return Ok((edit, json!({ "task": full(doc, &t, &today) })));
        };
        // one occurrence of a repeating task: it leaves the series (like dragging it away in
        // the calendar, tasks::move_occurrence)
        if stored.repeat.is_none() {
            return Err("\"occurrence\" is for repeating tasks; this task does not repeat, leave it out".into());
        }
        let occ = occurrence_on(stored, &day).ok_or_else(|| no_occurrence(stored, &day, &today))?;
        if given(&f, "repeat").is_some() {
            return Err("\"repeat\" does not go with \"occurrence\": one occurrence does not repeat. Leave out \"occurrence\" to change the rule of the whole series.".into());
        }
        let mut copy = stored.clone();
        copy.repeat = None;
        copy.done_dates = None;
        copy.ex_dates = None;
        copy.start = occ.start.clone();
        copy.end = occ.end.clone();
        copy.done = occ.done;
        copy.progress = if occ.done { 100.0 } else { 0.0 };
        patch(&c, &mut copy, &f)?;
        // as it is, not with the defaults make_task fills in for a new task
        let (color, reminders) = (copy.color.clone(), copy.reminders.clone());
        let ticked_now = copy.done && !occ.done;
        let mut copy = make_task(doc, copy, Some(stored.order), now);
        copy.color = color;
        copy.reminders = reminders;
        // done on the day the occurrence was, unless this call ticks it
        if !ticked_now {
            copy.done_on = crate::tasks::done_day(stored, &day).map(str::to_owned).filter(|_| copy.done);
        }
        let mut series = stored.clone();
        let mut skipped: BTreeSet<String> = series.ex_dates.take().unwrap_or_default().into_iter().collect();
        skipped.insert(day.clone());
        series.ex_dates = Some(skipped.into_iter().collect());
        series.updated_at = now;
        let mut edit = Edit::task(&series);
        edit.put_task(&copy);
        Ok((edit, json!({ "task": full(doc, &copy, &today), "detachedFrom": stored.id, "note": format!("The occurrence of {day} is a task of its own now; the series skips that day.") })))
    }))
}

fn no_occurrence(t: &Task, day: &str, today: &str) -> String {
    let next: Vec<String> = upcoming(t, today, 3).into_iter().map(|(d, _)| d).collect();
    let rule = describe_rule(t.repeat.as_ref(), &t.start);
    if next.is_empty() {
        format!("\"{}\" ({rule}) has no occurrence on {day}, and none from today on", t.title)
    } else {
        format!("\"{}\" ({rule}) has no occurrence on {day}. The next ones: {}.", t.title, next.join(", "))
    }
}

fn set_task_done(host: &dyn Host, a: &Args) -> Res {
    only(a, &["id", "done", "date"])?;
    let id = required(a, "id")?.to_owned();
    let done = flag(a, "done")?.unwrap_or(true);
    let day = date(a, "date")?;
    host.change(Box::new(move |doc: &Doc| {
        let (today, now) = (today(), now_ms());
        let t = doc.tasks.get(&id).ok_or_else(|| no_task(&id))?;
        let mut out = Map::new();
        out.insert("id".into(), json!(t.id));
        out.insert("title".into(), json!(t.title));
        let edit = if let Some(kind) = flex::kind(t) {
            let day = day.unwrap_or_else(|| today.clone());
            if done && day > today {
                return Err(format!("a to-do cannot have been done on {day}: that day has not come (today is {today})"));
            }
            let slot = if kind == flex::MONTH {
                if flex::month_key(&day) < flex::month_key(&t.start) {
                    return Err(format!("this to-do only began in {}; there is nothing to tick for {}", flex::month_key(&t.start), flex::month_key(&day)));
                }
                out.insert("month".into(), json!(flex::month_key(&day)));
                flex::month_slot(&day)
            } else {
                flex::SOMEDAY_SLOT.to_owned()
            };
            out.insert("todo".into(), json!(todo_kind(t)));
            let mut changed = t.clone();
            flex::set_slot(&mut changed, &slot, done.then_some(day.as_str()));
            if done {
                out.insert("doneOn".into(), json!(day));
            }
            if changed == *t { Edit::default() } else { save_task(doc, changed, now) }
        } else if t.repeat.is_some() {
            let Some(day) = day else {
                let next: Vec<String> = upcoming(t, &today, 3).into_iter().map(|(d, _)| d).collect();
                return Err(format!(
                    "\"{}\" repeats ({}): say which occurrence with \"date\".{}",
                    t.title,
                    describe_rule(t.repeat.as_ref(), &t.start),
                    if next.is_empty() { String::new() } else { format!(" The next ones: {}.", next.join(", ")) }
                ));
            };
            let occ = occurrence_on(t, &day).ok_or_else(|| no_occurrence(t, &day, &today))?;
            out.insert("date".into(), json!(day));
            if occ.done == done { Edit::default() } else { set_occurrence_done(doc, t, &day, done, now) }
        } else {
            out.insert("date".into(), json!(t.start));
            if t.done == done { Edit::default() } else { set_occurrence_done(doc, t, &t.start, done, now) }
        };
        out.insert("done".into(), json!(done));
        if edit.is_empty() {
            out.insert("unchanged".into(), json!(true));
        }
        Ok((edit, Value::Object(out)))
    }))
}

fn delete_task(host: &dyn Host, a: &Args) -> Res {
    only(a, &["id", "scope", "date"])?;
    let id = required(a, "id")?.to_owned();
    let scope = choice(a, "scope", &["this", "future", "all"])?;
    let day = date(a, "date")?;
    host.change(Box::new(move |doc: &Doc| {
        let (today, now) = (today(), now_ms());
        let t = doc.tasks.get(&id).ok_or_else(|| no_task(&id))?;
        let mut out = Map::new();
        out.insert("id".into(), json!(t.id));
        out.insert("title".into(), json!(t.title));
        let edit = if t.repeat.is_none() {
            out.insert("deleted".into(), json!("task"));
            delete(doc, &id, now)
        } else {
            let rule = describe_rule(t.repeat.as_ref(), &t.start);
            let scope = scope.ok_or_else(|| {
                format!("\"{}\" repeats ({rule}): say how much to delete with \"scope\": \"this\" (the occurrence on \"date\"), \"future\" (the occurrences on and after \"date\") or \"all\" (the whole task)", t.title)
            })?;
            let mode = match scope {
                "this" => DeleteMode::This,
                "future" => DeleteMode::Future,
                _ => DeleteMode::All,
            };
            let day = match (mode, day) {
                (DeleteMode::All, _) => t.start.clone(),
                (_, Some(d)) => d,
                (_, None) => return Err(format!("scope \"{scope}\" needs \"date\": the occurrence")),
            };
            if mode == DeleteMode::This {
                occurrence_on(t, &day).ok_or_else(|| no_occurrence(t, &day, &today))?;
            }
            let edit = delete_occurrence(doc, t, &day, mode, now);
            // from the first occurrence on is the whole task
            let gone = edit.items.iter().any(|(c, i, v)| c == "tasks" && *i == id && v.is_none());
            out.insert("deleted".into(), json!(if gone { "task" } else if mode == DeleteMode::This { "occurrence" } else { "occurrencesFrom" }));
            if !gone {
                out.insert("date".into(), json!(day));
            }
            edit
        };
        Ok((edit, Value::Object(out)))
    }))
}

fn add_group(host: &dyn Host, a: &Args) -> Res {
    only(a, &["name", "color"])?;
    let name = required(a, "name")?.replace(['\r', '\n', '\t'], " ").trim().to_owned();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("\"name\" must have 1 to 100 characters".into());
    }
    let color = given(a, "color").map(color_of).transpose()?.flatten();
    host.change(Box::new(move |doc: &Doc| {
        if let Some(g) = doc.groups.values().find(|g| g.name == name) {
            return Ok((Edit::default(), json!({ "id": g.id, "name": g.name, "color": g.color, "existing": true })));
        }
        let color = color.unwrap_or_else(|| TASK_COLORS[doc.groups.len() % TASK_COLORS.len()].key.to_owned());
        let g = make_group(doc, &name, &color);
        let view = json!({ "id": g.id, "name": g.name, "color": g.color });
        Ok((save_group(doc, g), view))
    }))
}
