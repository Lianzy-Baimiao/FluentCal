//! FluentCal's native UI (Slint, software renderer): windows, view models and the platform
//! layer. The calendar logic and data are in fluentcal_core.

slint::include_modules!();

pub mod app;
pub mod desk;
pub mod fluent;
pub mod fonts;
pub mod gallery;
pub mod i18n;
pub mod platform;
pub mod reminders;
pub mod shadow;
pub mod shell;
pub mod spin;
pub mod startup;
pub mod theme;
pub mod webcmd;
pub mod windows;
