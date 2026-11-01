//! The task editor window (0.2's src/windows/editor): one window, opened on a task or a new
//! one, switching (after asking) when opened on another task while open. Its saves aren't
//! undoable from the calendar: in 0.2 the editor was another window with its own history.

mod host;

pub use host::Msg;

use crate::app::{run, App};
use crate::windows::framed::{self, Spec};
use crate::EditorWindow;
use fluentcal_core::date::now_ms;
use fluentcal_core::editor::{Editor, Payload};
use fluentcal_core::store::StoreEvent;
use fluentcal_core::tasks::{Doc, Edit};
use fluentcal_core::theme::Tokens;
use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub struct EditorWin {
    pub window: EditorWindow,
    pub editor: Editor,
}

/// Lunar text and 休/班 for the date pickers (EditorApp lunarOf / markerOf).
fn day_labels(iso: &str) -> (String, String) {
    let i = fluentcal_core::lunar::day_info(iso);
    let marker = i.legal.map(|l| if l.work { "班" } else { "休" }).unwrap_or_default();
    (i.text, marker.into())
}

/// A new editor window with its helpers wired (`today`: the calendars' today, "" = the clock's).
pub fn make(today: &str) -> Result<EditorWindow, slint::PlatformError> {
    let w = EditorWindow::new()?;
    crate::fluent::install(&w, today, Some(Rc::new(day_labels)));
    host::setup(&w);
    Ok(w)
}

impl App {
    /// New task, optionally starting on `start`.
    pub fn open_editor_new(&mut self, start: Option<String>) {
        self.open_editor(Payload::new_on(start));
    }

    /// Edit a task; `date`: the occurrence clicked (repeating tasks).
    pub fn open_editor_task(&mut self, task_id: &str, date: Option<String>) {
        self.open_editor(Payload::edit(task_id, date));
    }

    /// Opens the editor on `p`; already open: switches to it (0.2's pending://editor).
    pub fn open_editor(&mut self, p: Payload) {
        let today = self.today();
        if let Some(ed) = self.editor.as_mut() {
            let fx = ed.editor.take(&self.doc, p, &today, now_ms());
            framed::raise(&ed.window);
            return self.editor_effect(fx);
        }
        let mut editor = Editor::default();
        editor.load(&self.doc, &p, &today, now_ms());
        let Some(window) = framed::open(self, &Spec { width: 560.0, height: 720.0 }, || make(""), |app| app.editor_close()) else { return };
        window.set_mica(self.has_mica());
        host::wire(&window, Rc::new(|m| run(move |app| app.editor_msg(m))));
        // The close button asks first when there are unsaved changes.
        window.window().on_close_requested(|| {
            run(|app| app.editor_msg(Msg::Cancel));
            slint::CloseRequestResponse::KeepWindowShown
        });
        host::push(&window, &editor, &self.doc, &today, &self.tokens, true);
        window.invoke_focus_title();
        self.editor = Some(EditorWin { window, editor });
    }

    pub fn editor_msg(&mut self, m: Msg) {
        let today = self.today();
        let Some(ed) = self.editor.as_mut() else { return };
        let fx = host::handle(&ed.window, &mut ed.editor, &self.doc, m, &today, now_ms());
        self.editor_effect(fx);
    }

    fn editor_effect(&mut self, mut fx: fluentcal_core::editor::Effect) {
        let edit = std::mem::take(&mut fx.edit);
        if !edit.is_empty() {
            self.apply(edit, false);
        }
        if fx.close {
            return self.editor_close();
        }
        let today = self.today();
        if let Some(ed) = &self.editor {
            host::push(&ed.window, &ed.editor, &self.doc, &today, &self.tokens, fx.reloaded);
            host::show(&ed.window, &fx);
        }
    }

    /// Closes the editor (the window is dropped on the next turn: this may run inside one of
    /// its callbacks).
    pub fn editor_close(&mut self) {
        if let Some(ed) = self.editor.take() {
            let _ = ed.window.hide();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || drop(ed));
        }
    }

    /// Tasks, groups or settings changed elsewhere: the choices and hints follow; the draft
    /// stays as it is (0.2 kept editing the copy).
    pub fn editor_refresh(&mut self) {
        let today = self.today();
        if let Some(ed) = &self.editor {
            host::push(&ed.window, &ed.editor, &self.doc, &today, &self.tokens, false);
        }
    }

    pub fn editor_retheme(&mut self) {
        if let Some(ed) = &self.editor {
            framed::retheme(self, &ed.window);
        }
        self.editor_refresh();
    }

    /// The open editor window (tests).
    pub fn editor_window(&self) -> Option<EditorWindow> {
        self.editor.as_ref().map(|e| e.window.clone_strong())
    }
}

/// The editor on a copy of the document with nothing written anywhere: the parity snapshot
/// (examples/snapshot.rs) opens a payload and plays clicks in it.
pub struct Offline {
    pub window: EditorWindow,
    _state: Rc<RefCell<(Editor, Doc)>>,
}

impl Offline {
    pub fn new(doc: Doc, p: &Payload, today: &str, tokens: Tokens) -> Result<Offline, slint::PlatformError> {
        let window = make(today)?;
        let mut editor = Editor::default();
        editor.load(&doc, p, today, now_ms());
        host::push(&window, &editor, &doc, today, &tokens, true);
        window.invoke_focus_title();
        let state = Rc::new(RefCell::new((editor, doc)));
        let (st, w, day) = (state.clone(), window.as_weak(), today.to_owned());
        host::wire(
            &window,
            Rc::new(move |m| {
                let (Some(w), Ok(mut s)) = (w.upgrade(), st.try_borrow_mut()) else { return };
                let (ed, doc) = &mut *s;
                let fx = host::handle(&w, ed, doc, m, &day, now_ms());
                apply_local(doc, &fx.edit);
                host::push(&w, ed, doc, &day, &tokens, fx.reloaded);
                host::show(&w, &fx);
            }),
        );
        Ok(Offline { window, _state: state })
    }
}

fn apply_local(doc: &mut Doc, edit: &Edit) {
    for (collection, id, v) in &edit.items {
        let items = serde_json::Map::from_iter([(id.clone(), v.clone().unwrap_or(serde_json::Value::Null))]);
        doc.apply(&StoreEvent::Changed { origin: "offline".into(), collection: collection.clone(), items });
    }
}
