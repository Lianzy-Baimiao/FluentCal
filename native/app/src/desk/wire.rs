//! Connects the desk window's callbacks to the app.

use crate::app::{run, with};
use crate::DeskWindow;

pub fn wire(w: &DeskWindow) {
    super::gantt::keys::wire(w);
    w.on_capacity_changed(|_| run(|a| a.desk_sync_capacity()));
    w.on_page(|d| run(move |a| a.desk_page(d)));
    w.on_go_today(|| run(|a| a.desk_go_today()));
    w.on_set_view(|v| run(move |a| a.desk_set_view(&v)));
    w.on_select(|d| run(move |a| a.desk_select(&d)));
    w.on_quick_add(|d| run(move |a| a.desk_quick_add(&d)));
    w.on_quick_add_submit(|t| run(move |a| a.desk_quick_add_submit(&t)));
    w.on_quick_add_close(|| run(|a| a.desk_quick_add_close()));
    w.on_quick_add_focus(|| run(|a| a.desk_wants_keyboard()));
    w.on_jump(|d| run(move |a| a.desk_jump(&d)));
    w.on_chip_toggle(|c| run(move |a| a.desk_chip_toggle(&c.task_id, &c.date, c.done)));
    w.on_chip_open(|c| run(move |a| a.open_editor_task(&c.task_id, Some(c.date.to_string()))));
    w.on_chip_menu(|c, x, y| run(move |a| a.desk_chip_menu(&c, x, y)));
    w.on_chip_press(|c, x, y| run(move |a| a.desk_chip_press(&c.key, &c.task_id, &c.date, x, y)));
    w.on_chip_move(|x, y| run(move |a| a.desk_chip_move(x, y)));
    w.on_chip_release(|x, y| run(move |a| a.desk_chip_release(x, y)));
    w.on_cell_menu(|d, x, y| run(move |a| a.desk_cell_menu(&d, x, y)));
    w.on_more(|d, x, y, bw, bh| run(move |a| a.desk_day_list(&d, (x, y, bw, bh))));
    w.on_wheel(|dy, ctrl| run(move |a| a.desk_wheel(dy, ctrl)));
    w.on_search(|| run(|a| a.desk_search()));
    w.on_sync_now(|| run(|a| a.cloud.request()));
    w.on_toggle_lock(|| run(|a| a.desk_toggle_lock()));
    w.on_settings(|| run(|a| a.open_settings(None)));
    w.on_more_menu(|x, y, bw, bh| run(move |a| a.desk_more_menu((x, y, bw, bh))));
    w.on_pick_month(|x, y, bw, bh| run(move |a| a.desk_pick_month((x, y, bw, bh))));
    w.on_start_move(|| run(|a| a.desk_start_move()));
    w.on_start_resize(|e| run(move |a| a.desk_start_resize(&e)));
    w.on_toast_undo_clicked(|| run(|a| a.desk_undo(true)));
    w.on_toast_close(|| run(|a| a.desk_close_toast()));
    w.on_key(|text, ctrl, shift, _alt| with(|a| a.desk_key(&text, ctrl, shift)).unwrap_or(false));
}
