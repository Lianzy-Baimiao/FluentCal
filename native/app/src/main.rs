// No console window in release builds (debug builds keep it for the log).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    fluentcal_app::startup::main();
}
