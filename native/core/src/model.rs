//! The data model of 0.2's src/types.ts and src/defaults.ts. data.json is unchanged: tasks
//! and groups keep any field this version doesn't know (`extra`), so writing one back never
//! drops data from 0.2 or another PC. Settings are read with defaults filled in and written as
//! patches (Store::update_settings), never as a whole.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

/// JS writes 3 for 3.0; so do we, so the JSON (and cloud sync signatures) stay the same.
fn int_if_whole<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    if v.fract() == 0.0 && v.abs() < 9e15 {
        s.serialize_i64(*v as i64)
    } else {
        s.serialize_f64(*v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
    /// Monday–Friday
    Workday,
    /// Working days per the official holiday schedule (incl. 调休)
    LegalWorkday,
    LunarYearly,
    MonthLastDay,
    Ebbinghaus,
}

/// An unknown frequency (written by a newer version) reads as None instead of failing the task.
fn lenient<'de, D: Deserializer<'de>, T: for<'a> Deserialize<'a>>(d: D) -> Result<Option<T>, D::Error> {
    Ok(Option::<Value>::deserialize(d)?.and_then(|v| serde_json::from_value(v).ok()))
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepeatRule {
    #[serde(default, deserialize_with = "lenient")]
    pub freq: Option<Freq>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u32>,
    /// weekly: 0 = Sunday … 6 = Saturday. Defaults to the start date's weekday.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekdays: Option<Vec<u32>>,
    /// monthly: "day" (same date) or "nthWeekday" (e.g. second Tuesday).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Last allowed occurrence start (inclusive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub start: String,
    /// Inclusive end date.
    #[serde(default)]
    pub end: String,
    /// HH:mm; None = all day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// TASK_COLORS key or #RRGGBB; None = default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// 0–100
    #[serde(default, serialize_with = "int_if_whole")]
    pub progress: f64,
    #[serde(default)]
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<bool>,
    /// Predecessor task ids (finish-to-start).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deps: Option<Vec<String>>,
    /// Minutes before the occurrence (0 = on time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reminders: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<RepeatRule>,
    /// Repeating tasks: occurrence start dates marked done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_dates: Option<Vec<String>>,
    /// Repeating tasks: occurrence start dates deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ex_dates: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub countdown: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_postpone: Option<bool>,
    /// First-run sample data; dropped once the user renames it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<bool>,
    #[serde(default, serialize_with = "int_if_whole")]
    pub order: f64,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Task {
    pub fn is_milestone(&self) -> bool {
        self.milestone == Some(true)
    }
    pub fn is_sample(&self) -> bool {
        self.sample == Some(true)
    }
    pub fn deps(&self) -> &[String] {
        self.deps.as_deref().unwrap_or(&[])
    }
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collapsed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<bool>,
    #[serde(default, serialize_with = "int_if_whole")]
    pub order: f64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Group {
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HolidayDay {
    pub date: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_off_day: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HolidayYear {
    #[serde(default)]
    pub year: i32,
    #[serde(default)]
    pub fetched_at: i64,
    #[serde(default)]
    pub days: Vec<HolidayDay>,
}

/// Tasks of the document; entries that don't parse are left out (and left alone in the data).
pub fn tasks(doc: &Value) -> Vec<Task> {
    collection(doc, "tasks")
}

pub fn groups(doc: &Value) -> Vec<Group> {
    collection(doc, "groups")
}

pub fn holidays(doc: &Value) -> std::collections::BTreeMap<String, HolidayYear> {
    doc.get("holidays")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), serde_json::from_value(v.clone()).ok()?))).collect())
        .unwrap_or_default()
}

/// Day colours: date → colour key.
pub fn day_marks(doc: &Value) -> std::collections::BTreeMap<String, String> {
    doc.get("dayMarks")
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.get("color")?.as_str()?.to_owned()))).collect())
        .unwrap_or_default()
}

fn collection<T: for<'de> Deserialize<'de>>(doc: &Value, name: &str) -> Vec<T> {
    doc.get(name)
        .and_then(Value::as_object)
        .map(|m| m.values().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect())
        .unwrap_or_default()
}
