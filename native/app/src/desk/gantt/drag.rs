//! Gantt dragging (0.2's gantt/drag.ts): bars (move, resize, progress), drag-creating on an
//! add row, dependency links, the table splitter; and the view's keys. Chart-local px.

use super::{model::RH, BarDrag};
use crate::app::App;
use crate::{GBar, GLink, GanttData};
use fluentcal_core::date::{add_days, diff_days, now_ms};
use fluentcal_core::gantt::can_link;
use fluentcal_core::model::Task;
use fluentcal_core::tasks;
use slint::{ComponentHandle, Model};

fn span(a: &str, b: &str) -> String {
    format!("{} – {}（{} 天）", a[5..].replace('-', "/"), b[5..].replace('-', "/"), diff_days(a, b) + 1)
}

impl App {
    fn tip(&self, x: f64, y: f64, text: &str) {
        let gd = self.desk.window.global::<GanttData>();
        gd.set_tip_x(x as f32);
        gd.set_tip_y(y as f32);
        gd.set_tip_text(text.into());
        gd.set_tip_show(true);
    }

    fn day_at(&self, x: f64) -> String {
        let m = self.desk.gantt.model.as_ref().expect("model");
        add_days(&m.range.0, (x / m.dw).floor() as i32)
    }

    pub fn gantt_zone_down(&mut self, group: &str, row: i32, x: f64) {
        if self.desk.window.get_read_only() || self.desk.gantt.model.is_none() {
            return;
        }
        let d = self.day_at(x);
        self.desk.gantt.zone_drag = Some((group.into(), row, d.clone(), d.clone(), d.clone()));
        self.gantt_ghost(row, &d, &d);
    }

    fn gantt_ghost(&self, row: i32, a: &str, b: &str) {
        let Some(m) = &self.desk.gantt.model else { return };
        let gd = self.desk.window.global::<GanttData>();
        gd.set_ghost_row(row);
        gd.set_ghost_x((diff_days(&m.range.0, a) as f64 * m.dw) as f32);
        gd.set_ghost_w(((diff_days(a, b) + 1) as f64 * m.dw) as f32);
        gd.set_ghost_show(true);
    }

    pub fn gantt_zone_move(&mut self, x: f64) {
        let Some((_, row, d0, ..)) = self.desk.gantt.zone_drag.clone() else { return };
        let d = self.day_at(x);
        let (a, b) = if d < d0 { (d, d0.clone()) } else { (d0.clone(), d) };
        self.gantt_ghost(row, &a, &b);
        self.tip(x, f64::from(row) * RH + RH, &span(&a, &b));
        if let Some(z) = self.desk.gantt.zone_drag.as_mut() {
            (z.3, z.4) = (a, b);
        }
    }

    pub fn gantt_zone_up(&mut self) {
        let Some((group, _, _, a, b)) = self.desk.gantt.zone_drag.take() else { return };
        self.gantt_end_feedback();
        let end = if a == b { add_days(&a, if self.desk.gantt.model.as_ref().is_some_and(|m| m.dw >= 32.0) { 2 } else { 6 }) } else { b };
        self.gantt_create(&group, &a, end, false);
    }

    fn gantt_end_feedback(&self) {
        let gd = self.desk.window.global::<GanttData>();
        gd.set_ghost_show(false);
        gd.set_tip_show(false);
        gd.set_rubber_show(false);
    }

    pub fn gantt_bar_down(&mut self, b: &GBar, mode: &str, x: f64) {
        self.history.boundary();
        self.gantt_select(&b.task_id);
        if self.desk.window.get_read_only() {
            return self.open_editor_task(&b.task_id, Some(b.date.to_string()));
        }
        let Some(bar) = self.desk.gantt.model.as_ref().and_then(|m| m.bars.iter().find(|x| x.g.key == b.key).cloned()) else { return };
        if bar.g.rep {
            return; // repeating instances are edited as a series in the editor
        }
        let mode = if bar.g.milestone { "move" } else { mode };
        self.desk.gantt.bar_drag = Some(BarDrag { bar, mode: mode.into(), x0: x, moved: false, preview: None });
    }

    pub fn gantt_bar_move(&mut self, x: f64, y: f64) {
        let dw = self.desk.gantt.model.as_ref().map_or(32.0, |m| m.dw);
        let Some(s) = self.desk.gantt.bar_drag.as_mut() else { return };
        let dx = x - s.x0;
        if !s.moved && dx.abs() < 3.0 {
            return;
        }
        s.moved = true;
        let (bs, be, bp) = (s.bar.start.clone(), s.bar.end.clone(), f64::from(s.bar.g.progress));
        let (start, end, progress, text) = if s.mode == "progress" {
            let p = ((bp + dx / s.bar.w.max(1.0) * 100.0) / 5.0).round().clamp(0.0, 20.0) * 5.0;
            (bs, be, p, format!("进度 {p}%"))
        } else {
            let dd = (dx / dw).round() as i32;
            let len = diff_days(&bs, &be);
            let (a, b) = match s.mode.as_str() {
                "move" => (add_days(&bs, dd), add_days(&be, dd)),
                "start" => (add_days(&bs, dd.min(len)), be),
                _ => (bs, add_days(&be, dd.max(-len))),
            };
            let text = if s.bar.g.milestone { a.clone() } else { span(&a, &b) };
            (a, b, bp, text)
        };
        s.preview = Some((start.clone(), end.clone(), progress));
        let key = s.bar.g.key.clone();
        self.tip(x, y, &text);
        // Move the bar in place: replacing the model would end the drag (the item goes away).
        let Some(m) = &self.desk.gantt.model else { return };
        let gx = diff_days(&m.range.0, &start) as f64 * dw;
        let gw = (diff_days(&start, &end) + 1) as f64 * dw;
        let gd = self.desk.window.global::<GanttData>();
        for list in [gd.get_bars(), gd.get_milestones()] {
            if let Some(i) = list.iter().position(|b| b.key == key) {
                let mut b = list.row_data(i).unwrap();
                (b.x, b.w, b.progress) = (gx as f32, gw as f32, progress as f32);
                list.set_row_data(i, b);
            }
        }
    }

    pub fn gantt_bar_up(&mut self) {
        let Some(s) = self.desk.gantt.bar_drag.take() else { return };
        self.gantt_end_feedback();
        let (Some((start, end, p)), true) = (s.preview, s.moved) else { return };
        let Some(t) = self.doc.tasks.get(s.bar.g.task_id.as_str()).cloned() else { return };
        let edit = if s.mode == "progress" {
            (p != t.progress).then(|| tasks::save_task(&self.doc, Task { progress: p, done: p >= 100.0, ..t.clone() }, now_ms()))
        } else {
            (start != t.start || end != t.end).then(|| tasks::save_task(&self.doc, Task { start, end, ..t.clone() }, now_ms()))
        };
        match edit {
            Some(e) => self.apply(e, true),
            None => self.gantt_virtualize(true), // put the bar back
        }
    }

    pub fn gantt_link_down(&mut self, b: &GBar, x: f64, y: f64) {
        if self.desk.window.get_read_only() {
            return;
        }
        let (fx, fy) = (f64::from(b.x + b.w), f64::from(b.row) * RH + RH / 2.0);
        self.desk.gantt.link_drag = Some((b.task_id.to_string(), fx, fy));
        self.gantt_link_move(x, y);
    }

    pub fn gantt_link_move(&mut self, x: f64, y: f64) {
        let Some((from, fx, fy)) = self.desk.gantt.link_drag.clone() else { return };
        let to = self.gantt_task_at(x, y);
        let ok = to.as_ref().is_some_and(|t| can_link(&self.doc.tasks, &from, t));
        let gd = self.desk.window.global::<GanttData>();
        let l: GLink = super::geom::link_of("rubber", &[(fx, fy), (x, y)], !ok, false);
        gd.set_rubber(l);
        gd.set_rubber_show(true);
        let text = match &to {
            Some(t) if *t != from && ok => format!("设为“{}”的前置任务", self.doc.tasks.get(t).map_or("", |t| t.title.as_str())),
            Some(t) if *t != from => "不能连接（会形成循环或已连接）".into(),
            _ => "拖到另一个任务上".into(),
        };
        self.tip(x, y, &text);
    }

    pub fn gantt_link_up(&mut self, x: f64, y: f64) {
        let Some((from, ..)) = self.desk.gantt.link_drag.take() else { return };
        self.gantt_end_feedback();
        let Some(to) = self.gantt_task_at(x, y).filter(|t| can_link(&self.doc.tasks, &from, t)) else { return };
        let Some(t) = self.doc.tasks.get(&to).cloned() else { return };
        self.history.boundary();
        let mut deps = t.deps().to_vec();
        deps.push(from);
        let edit = tasks::save_task(&self.doc, Task { deps: Some(deps), ..t }, now_ms());
        self.apply(edit, true);
    }

    /// The task under a chart point: its bar, or its row in the table (left of the chart).
    fn gantt_task_at(&self, x: f64, y: f64) -> Option<String> {
        let m = self.desk.gantt.model.as_ref()?;
        let row = (y / RH).floor();
        if row < 0.0 {
            return None;
        }
        let sx = -f64::from(self.desk.window.global::<GanttData>().get_scroll_x());
        if x < sx {
            let r = m.rows.get(row as usize)?;
            return (r.kind == "task").then(|| r.id.to_string());
        }
        m.bars
            .iter()
            .find(|b| b.g.row == row as i32 && if b.g.milestone { (x - (b.x + m.dw / 2.0)).abs() <= 11.0 } else { x >= b.x && x <= b.x + b.w })
            .map(|b| b.g.task_id.to_string())
    }
}
