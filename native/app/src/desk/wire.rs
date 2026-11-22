//! Connects the desk window's callbacks to the app.

use crate::app::{run, with};
use crate::DeskWindow;

pub fn wire(w: &DeskWindow) {
    w.on_custom_color_commit(|default_accent, value|run(move|a|a.desk_color_commit(default_accent, &value)));
    // the calendar of 修改完成日期 (its date helpers, lunar days and 休/班)
    crate::fluent::install(w, "", Some(std::rc::Rc::new(crate::windows::editor::day_labels)));
    super::gantt::keys::wire(w);
    crate::group_delete::wire(w, crate::group_delete::Host::Desk);
    w.on_capacity_changed(|_| run(|a| a.desk_sync_capacity()));
    w.on_page(|d| run(move |a| a.desk_page(d)));
    w.on_go_today(|| run(|a| a.desk_go_today()));
    w.on_set_view(|v| run(move |a| a.desk_set_view(&v)));
    w.on_set_list_range(|r| run(move |a| a.desk_set_list_range(&r)));
    w.on_list_more(|| run(|a| a.desk_list_more()));
    w.on_list_toggle(|c| run(move |a| a.desk_chip_toggle(&c.task_id, &c.date, c.done)));
    w.on_list_open(|c| run(move |a| a.open_editor_task(&c.task_id, Some(c.date.to_string()))));
    w.on_list_menu(|c, x, y| run(move |a| a.desk_chip_menu(&c, x, y)));
    w.on_list_jump_section(|section| run(move |a| a.desk_list_jump_section(&section)));
    w.on_select(|d| run(move |a| a.desk_select(&d)));
    w.on_quick_add(|d| run(move |a| a.desk_quick_add(&d)));
    w.on_quick_add_submit(|t| run(move |a| a.desk_quick_add_submit(&t)));
    w.on_quick_add_close(|| run(|a| a.desk_quick_add_close()));
    w.on_quick_add_focus(|| run(|a| a.desk_wants_keyboard()));
    w.on_jump(|d| run(move |a| a.desk_jump(&d)));
    w.on_locate(|id, d| run(move |a| a.desk_locate(&id, &d)));
    w.on_countdown_filter(|x, y, bw, bh| run(move |a| a.desk_countdown_filter_open(super::popups::At::Below(x, y, bw, bh))));
    w.on_countdown_source(|key, value| run(move |a| {
        if key == "countdownMarked" {
            a.patch_settings(serde_json::json!({ "calendar": { "countdownMarked": !a.doc.settings.calendar.countdown_marked } }));
        } else {
            a.patch_settings(serde_json::json!({ "calendar": { key.as_str(): value } }));
        }
    }));
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
    w.on_side_toggle(|c| run(move |a| a.desk_chip_toggle(&c.task_id, &c.date, c.done)));
    w.on_side_open_task(|c| run(move |a| a.open_editor_task(&c.task_id, Some(c.date.to_string()))));
    w.on_side_menu(|c, x, y| run(move |a| a.desk_chip_menu(&c, x, y)));
    w.on_side_add(|z, t| run(move |a| a.desk_side_add(&z, &t)));
    w.on_side_set(|on| run(move |a| a.desk_side_set(on)));
    w.on_side_press(|c, i, x, y| run(move |a| a.desk_side_press(&c, i, x, y)));
    w.on_side_move(|x, y, i| run(move |a| a.desk_side_move(x, y, i)));
    w.on_side_release(|i| run(move |a| a.desk_side_release(i)));
    w.on_side_cancel(|| run(|a| drop(a.desk_side_drag_end())));
    w.on_side_nudge(|i, d| run(move |a| a.desk_side_nudge(i, d)));
    w.on_side_refocus(|k| run(move |a| a.desk_side_refocus(&k)));
    w.on_changes_toggle_open(|| run(|a| a.desk_changes()));
    w.on_changes_close(|| run(|a| a.desk_changes_close()));
    w.on_changes_revert(|id| run(move |a| a.desk_changes_toggle(id)));
    w.on_changes_clear(|| run(|a| a.desk_changes_clear()));
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
    w.on_day_list_press(|c, x, y| run(move |a| a.desk_day_list_press(&c.key, &c.task_id, &c.date, x, y)));
    w.on_day_list_add(|t| run(move |a| a.desk_day_list_add(&t)));
    w.on_day_list_closed(|| run(|a| a.desk_day_list_closed()));
    w.on_ym_pick(|y, m| run(move |a| a.desk_ym_pick(y, m)));
    w.on_done_day_picked(|d| run(move |a| a.desk_done_day_picked(&d)));
    w.on_filter_open(|x, y, bw, bh| run(move |a| a.desk_filter_open(super::popups::At::Below(x, y, bw, bh))));
    w.on_filter_toggle(|id, on| run(move |a| a.desk_filter_toggle(&id, on)));
    w.on_filter_clear(|| run(|a| a.desk_filter_clear()));
    w.on_filter_manage(|| run(|a| a.desk_filter_manage()));
    w.on_filter_closed(|| run(|a| a.desk_filter_closed()));
    w.on_search_changed(|| run(|a| a.desk_search_refresh()));
    w.on_search_go(|h| run(move |a| { a.desk_search_close(); a.desk_locate(&h.task_id, &h.date); }));
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
    keys.on_focused(|| run(|a| a.desk_set_grid_focus(true)));
    slint::ComponentHandle::global::<crate::CellFocus>(w).on_focused(|| run(|a| a.desk.ui.grid_focus = true));
    w.on_popups_changed(|on| {
        let close: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(|| run(|a| a.desk_close_popups()));
        crate::platform::outside::watch("desk", on.then(|| (Box::new(crate::platform::desk::desk_hwnd) as Box<dyn Fn() -> _>, close)));
    });
}
