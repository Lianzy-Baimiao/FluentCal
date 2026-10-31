//! The task editor window (0.2's src/windows/editor).

use crate::app::App;

impl App {
    /// New task, optionally starting on `start`.
    pub fn open_editor_new(&mut self, _start: Option<String>) {
        self.notify("任务编辑窗口还在移植中", false);
    }

    /// Edit a task; `date`: the occurrence clicked (repeating tasks).
    pub fn open_editor_task(&mut self, _task_id: &str, _date: Option<String>) {
        self.notify("任务编辑窗口还在移植中", false);
    }
}
