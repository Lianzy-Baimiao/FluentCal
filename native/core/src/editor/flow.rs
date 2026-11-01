//! Saving, deleting, closing and switching tasks, with 0.2's questions on the way
//! (EditorApp save / remove / cancel / takePending / createGroup).

use super::{Editor, Payload};
use crate::tasks::{delete_occurrence, delete_task, make_group, merge_edit, save_group, save_task, DeleteMode, Doc, Edit};

/// A question the window shows (0.2's confirm dialogs).
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Unsaved changes: 放弃 / 继续编辑.
    Discard,
    /// Saving a task deleted elsewhere meanwhile: 重新创建 / 放弃.
    Recreate,
    /// Deleting a repeating task: 全部删除 / 仅这一次 / 取消.
    DeleteRepeat { title: String },
    /// Deleting a task: 删除 / 取消.
    Delete { title: String },
}

/// The button chosen: the primary one, the secondary one, or close (取消 / Esc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Primary,
    Secondary,
    Close,
}

impl Ask {
    /// Title, text, primary, secondary and close button labels, and whether it is dangerous.
    pub fn texts(&self) -> (String, String, &'static str, &'static str, &'static str, bool) {
        match self {
            Ask::Discard => ("放弃未保存的修改？".into(), String::new(), "放弃", "", "继续编辑", false),
            Ask::Recreate => ("任务已在其他窗口删除".into(), "要用当前内容重新创建它吗？".into(), "重新创建", "", "放弃", false),
            Ask::DeleteRepeat { title } => ("删除重复任务".into(), format!("“{title}”是重复任务，要删除哪些？"), "全部删除", "仅这一次", "取消", true),
            Ask::Delete { title } => ("删除任务？".into(), format!("“{title}”将被永久删除。"), "删除", "", "取消", true),
        }
    }
}

/// What the window does after an action: write `edit` (not undoable from the calendar, like
/// 0.2's separate editor window), then close, or show `ask`, or show the reloaded draft.
#[derive(Debug, Default)]
pub struct Effect {
    pub edit: Edit,
    pub close: bool,
    pub ask: Option<Ask>,
    /// The draft was replaced (another task loaded): show it again, focus the title.
    pub reloaded: bool,
    /// Save with an empty title: focus the title box.
    pub focus_title: bool,
}

impl Effect {
    fn close(edit: Edit) -> Effect {
        Effect { edit, close: true, ..Default::default() }
    }

    fn ask(a: Ask) -> Effect {
        Effect { ask: Some(a), ..Default::default() }
    }
}

impl Editor {
    fn asked(&mut self, a: Ask) -> Effect {
        self.asking = Some(a.clone());
        Effect::ask(a)
    }

    /// 保存 / 创建, Ctrl+Enter, Ctrl+S, Enter in the title.
    pub fn save(&mut self, doc: &Doc, now_ms: i64) -> Effect {
        if self.asking.is_some() {
            return Effect::default();
        }
        let Some(t) = self.draft.as_mut() else { return Effect::default() };
        t.title = t.title.trim().to_owned();
        if t.title.is_empty() {
            self.title_error = true;
            return Effect { focus_title: true, ..Default::default() };
        }
        if t.is_milestone() || t.end < t.start {
            t.end = t.start.clone();
        }
        let t = t.clone();
        if self.is_new {
            return Effect::close(save_task(doc, t, now_ms));
        }
        let Some(current) = doc.tasks.get(&t.id) else { return self.asked(Ask::Recreate) };
        // Only fields changed in this window overwrite the stored task.
        let merged = match &self.base {
            Some(base) => merge_edit(base, &t, current),
            None => t,
        };
        Effect::close(save_task(doc, merged, now_ms))
    }

    /// 删除: the stored task (not the draft: "仅这一次" must not write unsaved edits).
    pub fn remove(&mut self, doc: &Doc) -> Effect {
        if self.asking.is_some() {
            return Effect::default();
        }
        let stored = self.draft.as_ref().and_then(|d| doc.tasks.get(&d.id));
        match stored {
            Some(t) if !self.is_new => {
                let title = t.title.clone();
                self.asked(if t.repeat.is_some() { Ask::DeleteRepeat { title } } else { Ask::Delete { title } })
            }
            _ => Effect::close(Edit::default()),
        }
    }

    /// 取消, Esc, the window's close button.
    pub fn cancel(&mut self) -> Effect {
        if self.asking.is_some() {
            return Effect::default();
        }
        if self.dirty() {
            return self.asked(Ask::Discard);
        }
        Effect::close(Edit::default())
    }

    /// Opened again on `p` while open (0.2's pending://editor): switch, after "放弃" when the
    /// draft has changes.
    pub fn take(&mut self, doc: &Doc, p: Payload, today: &str, now_ms: i64) -> Effect {
        if self.dirty() || self.asking.is_some() {
            self.pending = Some(p);
            if self.asking.is_some() {
                return Effect::default();
            }
            return self.asked(Ask::Discard);
        }
        self.load(doc, &p, today, now_ms);
        Effect { reloaded: true, ..Default::default() }
    }

    /// The answer to the question being shown.
    pub fn answer(&mut self, doc: &Doc, a: Answer, today: &str, now_ms: i64) -> Effect {
        let Some(ask) = self.asking.take() else { return Effect::default() };
        match ask {
            Ask::Discard => {
                let pending = self.pending.take();
                match (a, pending) {
                    (Answer::Primary, Some(p)) => {
                        self.load(doc, &p, today, now_ms);
                        Effect { reloaded: true, ..Default::default() }
                    }
                    (Answer::Primary, None) => Effect::close(Edit::default()),
                    _ => Effect::default(),
                }
            }
            Ask::Recreate => {
                let edit = match (a, &self.draft) {
                    (Answer::Primary, Some(t)) => save_task(doc, t.clone(), now_ms),
                    _ => Edit::default(),
                };
                Effect::close(edit)
            }
            Ask::DeleteRepeat { .. } | Ask::Delete { .. } => {
                if a == Answer::Close {
                    return self.after_question(doc, today, now_ms);
                }
                let Some(t) = self.draft.as_ref().and_then(|d| doc.tasks.get(&d.id)) else { return Effect::close(Edit::default()) };
                let edit = if t.repeat.is_some() {
                    let mode = if a == Answer::Primary { DeleteMode::All } else { DeleteMode::This };
                    delete_occurrence(doc, t, &self.occ_date, mode, now_ms)
                } else {
                    delete_task(doc, &t.id, now_ms)
                };
                Effect::close(edit)
            }
        }
    }

    /// A question was dismissed: a task opened meanwhile is taken now.
    fn after_question(&mut self, doc: &Doc, today: &str, now_ms: i64) -> Effect {
        match self.pending.take() {
            Some(p) => self.take(doc, p, today, now_ms),
            None => Effect::default(),
        }
    }

    /// The new-group box's 创建: the group is saved at once and picked for the draft.
    pub fn create_group(&mut self, doc: &Doc, name: &str) -> Option<Edit> {
        let name = name.trim();
        if name.is_empty() || self.draft.is_none() {
            return None;
        }
        let g = make_group(doc, name, "blue");
        if let Some(t) = self.draft.as_mut() {
            t.group_id = Some(g.id.clone());
        }
        Some(save_group(doc, g))
    }
}
