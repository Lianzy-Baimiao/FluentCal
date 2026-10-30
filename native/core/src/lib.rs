//! FluentCal core: the data document (store, merge, external changes, data folder), cloud
//! sync, the local web view server, window layout geometry, and the calendar logic. No UI and
//! no Tauri, so all of it is tested with plain `cargo test`.

pub mod cloud;
pub mod datadir;
pub mod layout;
pub mod merge;
pub mod store;
pub mod sync;
pub mod web;
