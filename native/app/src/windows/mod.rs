//! The other windows: settings, task editor, reminder cards. Each module owns its window.

pub mod editor;
pub mod framed;
pub mod reminder;
pub mod settings;

use crate::app::App;

/// The theme changed: every open settings/editor window follows (each adds its line here).
pub fn retheme(app: &mut App) {
    app.editor_retheme();
}
