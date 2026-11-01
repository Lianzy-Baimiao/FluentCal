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
    w.on_menu_activated(|k| run(move |a| a.desk_menu_activated(&k)));
    w.on_menu_swatch(|row, sw| run(move |a| a.desk_menu_swatch(&row, &sw)));
    w.on_day_list_toggle(|c| run(move |a| a.desk_chip_toggle(&c.task_id, &c.date, c.done)));
    w.on_day_list_open(|c| run(move |a| a.open_editor_task(&c.task_id, Some(c.date.to_string()))));
    w.on_day_list_menu(|c, x, y| run(move |a| a.desk_chip_menu(&c, x, y)));
    w.on_day_list_add(|t| run(move |a| a.desk_day_list_add(&t)));
    w.on_day_list_closed(|| run(|a| a.desk_day_list_closed()));
    w.on_ym_pick(|y, m| run(move |a| a.desk_ym_pick(y, m)));
    w.on_search_changed(|| run(|a| a.desk_search_refresh()));
    w.on_search_go(|h| run(move |a| a.desk_search_go(&h.date)));
    w.on_search_open_task(|h| run(move |a| a.open_editor_task(&h.task_id, Some(h.date.to_string()))));
    w.on_search_close(|| run(|a| a.desk_search_close()));
    w.on_wants_keyboard(|| run(|a| a.desk_wants_keyboard()));
    w.on_confirm_result(|r| run(move |a| a.desk_confirm_result(&r)));
    w.on_lock(|| run(|a| a.patch_settings(serde_json::json!({ "desk": { "locked": true } }))));
    w.on_clear_samples(|| run(|a| a.desk_clear_samples()));
    w.on_onboarded(|| run(|a| a.patch_settings(serde_json::json!({ "general": { "onboarded": true } }))));
    w.on_notice_closed(|| run(|a| a.desk_set_notice("")));
    let keys = slint::ComponentHandle::global::<crate::ChipKeys>(w);
    keys.on_remove(|c| run(move |a| a.desk_chip_remove(&c.task_id, &c.date)));
    keys.on_menu(|c, x, y, bw, bh| run(move |a| a.show_menu(super::menus::Target::Task(c.task_id.to_string(), c.date.to_string()), super::popups::At::Below(x, y, bw, bh))));
    keys.on_focused(|| run(|a| a.desk.ui.grid_focus = true));
    w.on_popups_changed(|on| {
        let close: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(|| run(|a| a.desk_close_popups()));
        crate::platform::outside::watch("desk", on.then(|| (Box::new(crate::platform::desk::desk_hwnd) as Box<dyn Fn() -> _>, close)));
    });
}
