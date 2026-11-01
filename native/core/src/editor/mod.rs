//! The task editor's logic without its window (0.2's src/windows/editor): the draft being
//! edited, its fields (fields.rs), what the window shows (view.rs), and saving, deleting and
//! the questions on the way (flow.rs). The window glue only maps these to controls.

mod fields;
mod flow;
mod view;

pub use fields::{freq_of, unit_of, FREQS};
pub use flow::{Answer, Ask, Effect};
pub use view::{DepOption, DepTag, Option_, View};

use crate::model::Task;
use crate::tasks::{make_task, Doc};
use serde::Deserialize;

/// What to open (0.2's EditorPayload).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Payload {
    /// `date`: the occurrence opened (repeating tasks); the start when absent.
    Edit { task_id: String, date: Option<String> },
    New { start: Option<String>, end: Option<String>, group_id: Option<String>, title: Option<String>, milestone: Option<bool> },
}

impl Payload {
    pub fn new_on(start: Option<String>) -> Payload {
        Payload::New { start, end: None, group_id: None, title: None, milestone: None }
    }

    pub fn edit(task_id: &str, date: Option<String>) -> Payload {
        Payload::Edit { task_id: task_id.into(), date }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Editor {
    /// The task as edited; None when nothing is loaded or the task is gone (`missing`).
    pub draft: Option<Task>,
    /// The task as it was loaded, for the three-way merge on save.
    pub base: Option<Task>,
    baseline: String,
    pub is_new: bool,
    /// The occurrence opened (repeating tasks: "仅这一次" deletes this one).
    pub occ_date: String,
    /// Opened on a task that no longer exists.
    pub missing: bool,
    /// Save was pressed with an empty title.
    pub title_error: bool,
    /// The question being shown, if any.
    pub asking: Option<Ask>,
    /// Opened on another task while this one had changes: loaded after "放弃".
    pending: Option<Payload>,
}

fn json(t: &Option<Task>) -> String {
    t.as_ref().map(|t| t.to_value().to_string()).unwrap_or_default()
}

impl Editor {
    /// Loads `p` (0.2's load): a copy of the task, or a new one with defaults.
    pub fn load(&mut self, doc: &Doc, p: &Payload, today: &str, now_ms: i64) {
        self.missing = false;
        self.title_error = false;
        match p {
            Payload::Edit { task_id, date } => {
                let t = doc.tasks.get(task_id).cloned();
                self.missing = t.is_none();
                self.is_new = false;
                self.occ_date = date.clone().or_else(|| t.as_ref().map(|t| t.start.clone())).unwrap_or_default();
                self.draft = t;
            }
            Payload::New { start, end, group_id, title, milestone } => {
                let start = start.clone().unwrap_or_else(|| today.to_owned());
                let p = Task {
                    title: title.clone().unwrap_or_default(),
                    end: end.clone().unwrap_or_else(|| start.clone()),
                    start: start.clone(),
                    group_id: group_id.clone(),
                    milestone: milestone.filter(|m| *m),
                    ..Default::default()
                };
                self.draft = Some(make_task(doc, p, None, now_ms));
                self.is_new = true;
                self.occ_date = start;
            }
        }
        self.baseline = json(&self.draft);
        self.base = self.draft.clone();
    }

    /// The draft differs from what was loaded.
    pub fn dirty(&self) -> bool {
        self.draft.is_some() && json(&self.draft) != self.baseline
    }

    /// The window title (0.2: 新建任务 / 编辑任务).
    pub fn window_title(&self) -> &'static str {
        if self.is_new { "新建任务" } else { "编辑任务" }
    }
}
