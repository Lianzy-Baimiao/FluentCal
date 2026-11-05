//! FluentCal core: the data document (store, merge, external changes, data folder), cloud
//! sync, the local web view server, window layout geometry, and the calendar logic. No UI and
//! no Tauri, so all of it is tested with plain `cargo test`.

pub mod cloud;
pub mod color;
pub mod countdown;
pub mod datadir;
pub mod date;
pub mod editor;
pub mod export;
pub mod gantt;
pub mod history;
pub mod i18n;
pub mod holidays;
pub mod layout;
pub mod lunar;
pub mod merge;
pub mod model;
pub mod pairing;
pub mod recur;
pub mod reminders;
pub mod search;
pub mod seed;
pub mod settings;
pub mod settings_page;
pub mod store;
pub mod sync;
pub mod sync_text;
pub mod tasks;
pub mod theme;
pub mod web;
pub mod week;
