//! The menus' contents (0.2's chipMenu, cellMenu, moreMenu, groupMenu, the Gantt options).

use fluentcal_core::i18n::{tr};

use super::menus::{check, item, radio, sep, shortcut, sub, swatches, Entry};
use crate::app::App;
use crate::platform::screens;
use fluentcal_core::gantt::{in_gantt, RowOptions, UNGROUPED};
use fluentcal_core::recur::occurrence_on;

pub fn task(app: &App, id: &str, date: &str) -> Vec<Entry> {
    let Some(t) = app.doc.tasks.get(id) else { return Vec::new() };
    let s = &app.doc.settings;
    let done = occurrence_on(t, date).map_or(t.done, |o| o.done);
    let mut v = vec![
        shortcut(item("done", if done { tr("标记为未完成") } else { tr("标记为完成") }, "completed"), "Space"),
        swatches("color:", t.color.as_deref(), true),
        sep(),
        shortcut(item("edit", tr("编辑…"), "edit"), "Enter"),
        item("duplicate", tr("复制一份"), "copy"),
    ];
    if t.repeat.is_none() {
        v.push(item("postpone", tr("推迟一天"), "forward"));
        if t.start == t.end {
            v.push(check("autoPostpone", tr("未完成时自动顺延"), t.auto_postpone.unwrap_or(s.tasks.auto_postpone)));
        }
    }
    v.push(Entry { icon: "flag".into(), ..check("countdown", tr("倒数日"), t.countdown == Some(true)) });
    let o = RowOptions { hide_done: s.gantt.hide_done, hide_single_day: s.gantt.hide_single_day, group_filter: s.gantt.group_filter.clone() };
    if s.calendar.view != "gantt" && in_gantt(t, &o) {
        v.push(item("gantt", tr("在甘特图中查看"), "gantt"));
    }
    v.push(sep());
    if t.repeat.is_some() {
        let all = super::menus::Entry { danger: true, ..item("delete:all", tr("删除全部重复"), "") };
        v.push(super::menus::Entry { danger: true, ..sub(tr("删除"), "delete", vec![item("delete:this", tr("仅删除这一次"), ""), item("delete:future", tr("删除这一次及以后"), ""), all]) });
    } else {
        v.push(super::menus::Entry { danger: true, ..shortcut(item("delete:all", tr("删除"), "delete"), "Del") });
    }
    v
}

pub fn day(app: &App, date: &str) -> Vec<Entry> {
    let mark = app.doc.day_marks.get(date);
    let mut v = vec![
        shortcut(item("quickAdd", tr("快速添加"), "add"), tr("双击")),
        item("new", tr("新建任务…"), "edit"),
        sep(),
        Entry { disabled: true, ..item("", tr("日期颜色"), "color") },
        swatches("mark:", mark.map(String::as_str), false),
    ];
    if mark.is_some() {
        v.push(item("unmark", tr("清除日期颜色"), "close"));
    }
    v.extend([sep(), shortcut(item("today", tr("回到今天"), "today"), "Home"), item("browser", tr("在浏览器中查看"), "globe")]);
    v
}

pub fn more(app: &App) -> Vec<Entry> {
    let s = &app.doc.settings;
    let mut v = vec![shortcut(item("new", tr("新建任务…"), "add"), "Ctrl+N"), shortcut(item("search", tr("搜索"), "search"), "Ctrl+F")];
    if app.sync.enabled {
        v.push(item("syncNow", tr("立即同步"), "sync"));
    }
    v.push(item("browser", tr("在浏览器中查看"), "globe"));
    v.push(sep());
    v.push(check("showCompleted", tr("显示已完成任务"), s.tasks.show_completed));
    let themes = [("system", tr("跟随系统")), ("light", tr("浅色")), ("dark", tr("深色"))];
    v.push(sub(tr("主题"), "brightness", themes.iter().map(|(k, l)| radio(&format!("theme:{k}"), l, s.theme.mode == *k)).collect()));
    let pins = [("desktop", tr("嵌入桌面")), ("top", tr("始终置顶")), ("normal", tr("普通窗口"))];
    v.push(sub(tr("窗口层级"), "window", pins.iter().map(|(k, l)| radio(&format!("pin:{k}"), l, s.desk.pin_mode == *k)).collect()));
    let mons = screens::monitors();
    if mons.len() > 1 {
        let cur = crate::shell::current_screen(app);
        v.push(sub(tr("显示在"), "desktop", mons.iter().enumerate().map(|(i, m)| radio(&format!("screen:{i}"), &screens::title(m, i), cur == Some(i))).collect()));
    }
    v.push(if s.desk.locked { item("lock", tr("调整位置和大小"), "unlock") } else { item("lock", tr("完成调整"), "lock") });
    v.extend([item("appearance", tr("外观…"), "palette"), item("settings", tr("设置…"), "settings"), sep(), item("hide", tr("隐藏日历"), "eyeOff"), item("quit", tr("退出"), "power")]);
    v
}

pub fn group(app: &App, id: &str) -> Vec<Entry> {
    let mut v = vec![item("addTask", tr("添加任务"), "add"), item("addMilestone", tr("添加里程碑"), "flag")];
    if id != UNGROUPED {
        let color = app.doc.groups.get(id).map(|g| g.color.clone());
        v.extend([
            sep(),
            shortcut(item("rename", tr("重命名"), "edit"), "F2"),
            swatches("groupColor:", color.as_deref(), false),
            item("up", tr("上移"), "chevronUp"),
            item("down", tr("下移"), "chevronDown"),
            sep(),
            Entry { danger: true, ..item("deleteGroup", tr("删除分组"), "delete") },
        ]);
    }
    v
}

pub fn gantt_options(app: &App) -> Vec<Entry> {
    let g = &app.doc.settings.gantt;
    let mut groups: Vec<_> = app.doc.groups.values().collect();
    groups.sort_by(|a, b| a.order.total_cmp(&b.order));
    let mut filter = vec![radio("filter:", tr("全部分组"), g.group_filter.is_empty())];
    filter.extend(groups.iter().map(|x| radio(&format!("filter:{}", x.id), &x.name, g.group_filter.len() == 1 && g.group_filter[0] == x.id)));
    vec![
        check("showTable", tr("显示任务表"), g.show_table),
        check("showDeps", tr("显示依赖连线"), g.show_deps),
        check("hideDone", tr("隐藏已完成任务"), g.hide_done),
        check("hideSingleDay", tr("隐藏未分组的单日任务"), g.hide_single_day),
        sep(),
        sub(tr("导出图片"), "download", vec![item("export:png", tr("PNG 图片…"), ""), item("export:svg", tr("SVG 矢量图…"), "")]),
        sub(tr("只显示分组"), "filter", filter),
    ]
}
