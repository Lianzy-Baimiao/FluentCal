//! Gantt geometry that becomes paths and cells: dependency arrows (each in its own box,
//! dashed when late), the header tiers and the rest-day stripes.

use super::model::{Bar, RH};
use crate::{GCell, GLink, GStripe};
use fluentcal_core::date::{add_days, diff_days};
use fluentcal_core::gantt::{dep_points, header_tiers};
use fluentcal_core::lunar::{is_rest_day, legal_holiday};
use fluentcal_core::tasks::Doc;
use std::collections::BTreeMap;

const PAD: f64 = 6.0;

/// A polyline (and an arrow head at its end, pointing right) as a GLink in its own box.
pub fn link_of(key: &str, pts: &[(f64, f64)], late: bool, arrow: bool) -> GLink {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in pts {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + if arrow { 2.0 } else { 0.0 });
        y1 = y1.max(y);
    }
    let (bx, by) = (x0 - PAD, y0 - PAD);
    let rel: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| (x - bx, y - by)).collect();
    let path = if late { dashed(&rel, 3.0, 2.0) } else { solid(&rel) };
    // SVG marker of 0.2: 8×8 triangle, tip at refX 7, scaled by the 1.25 px stroke.
    let arrow_cmd = arrow.then(|| {
        let (tx, ty) = *rel.last().unwrap();
        let k = 1.25 * 7.0 / 8.0;
        let tip = tx + k;
        format!("M{:.2} {:.2} L{:.2} {:.2} L{:.2} {:.2} Z", tip - 8.0 * k, ty - 4.0 * k, tip, ty, tip - 8.0 * k, ty + 4.0 * k)
    });
    GLink {
        key: key.into(),
        x: bx as f32,
        y: by as f32,
        w: (x1 - x0 + 2.0 * PAD) as f32,
        h: (y1 - y0 + 2.0 * PAD) as f32,
        path: path.into(),
        arrow: arrow_cmd.unwrap_or_default().into(),
        late,
    }
}

fn solid(p: &[(f64, f64)]) -> String {
    let mut s = format!("M{:.2} {:.2}", p[0].0, p[0].1);
    for q in &p[1..] {
        s += &format!(" L{:.2} {:.2}", q.0, q.1);
    }
    s
}

/// Dashes of `on` px with `off` px gaps along the polyline (SVG stroke-dasharray).
fn dashed(p: &[(f64, f64)], on: f64, off: f64) -> String {
    let mut out = String::new();
    let (mut phase, mut drawing) = (0.0, true);
    for w in p.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        let len = (bx - ax).hypot(by - ay);
        let mut t = 0.0;
        while t < len {
            let seg = (if drawing { on } else { off }) - phase;
            let end = (t + seg).min(len);
            if drawing {
                let (sx, sy) = (ax + (bx - ax) * t / len, ay + (by - ay) * t / len);
                let (ex, ey) = (ax + (bx - ax) * end / len, ay + (by - ay) * end / len);
                out += &format!("M{sx:.2} {sy:.2} L{ex:.2} {ey:.2} ");
            }
            if end - t >= seg {
                phase = 0.0;
                drawing = !drawing;
            } else {
                phase += end - t;
            }
            t = end;
        }
    }
    out.trim_end().to_owned()
}

/// Finish-to-start arrows between the bars of non-repeating tasks; late when the successor
/// starts before its predecessor ends.
pub fn links(doc: &Doc, bars: &[Bar]) -> Vec<GLink> {
    if !doc.settings.gantt.show_deps {
        return Vec::new();
    }
    let pos: BTreeMap<&str, &Bar> = bars.iter().filter(|b| !b.g.rep).map(|b| (b.g.task_id.as_str(), b)).collect();
    let mut out = Vec::new();
    for b in pos.values() {
        let Some(t) = doc.tasks.get(b.g.task_id.as_str()) else { continue };
        for dep in t.deps() {
            let Some(p) = pos.get(dep.as_str()) else { continue };
            let (fy, ty) = (f64::from(p.g.row) * RH + RH / 2.0, f64::from(b.g.row) * RH + RH / 2.0);
            let pts = dep_points(p.x + p.w, fy, b.x, ty);
            out.push(link_of(&format!("{dep}>{}", t.id), &pts, p.end >= b.start, true));
        }
    }
    out
}

pub fn tiers(doc: &Doc, range: &(String, String), today: &str) -> (Vec<GCell>, Vec<GCell>) {
    let s = &doc.settings;
    let (top, bottom) = header_tiers((&range.0, &range.1), &s.gantt.scale, today, s.calendar.week_start);
    let rest = &s.calendar.rest_days;
    let cell = |c: fluentcal_core::gantt::HeaderCell| GCell {
        rest: s.gantt.scale != "month" && c.date.as_deref().is_some_and(|d| is_rest_day(d, rest)),
        key: c.key.into(),
        label: c.label.into(),
        x: c.x as f32,
        w: c.w as f32,
        today: c.today,
    };
    (top.into_iter().map(cell).collect(), bottom.into_iter().map(cell).collect())
}

/// Runs of consecutive rest days (weekends, holidays) for the column shading.
pub fn stripes(doc: &Doc, range: &(String, String), dw: f64) -> Vec<GStripe> {
    if doc.settings.gantt.scale == "month" {
        return Vec::new();
    }
    let rest = &doc.settings.calendar.rest_days;
    let days = diff_days(&range.0, &range.1) + 1;
    let mut out = Vec::new();
    let mut run: Option<(i32, i32, bool)> = None;
    let flush = |run: Option<(i32, i32, bool)>, out: &mut Vec<GStripe>| {
        if let Some((i, n, holiday)) = run {
            out.push(GStripe { x: (f64::from(i) * dw) as f32, w: (f64::from(n) * dw) as f32, holiday });
        }
    };
    for i in 0..days {
        let d = add_days(&range.0, i);
        if !is_rest_day(&d, rest) {
            flush(run.take(), &mut out);
            continue;
        }
        let holiday = legal_holiday(&d).is_some_and(|l| !l.work);
        match &mut run {
            Some((s, n, h)) if *h == holiday && *s + *n == i => *n += 1,
            _ => {
                flush(run.take(), &mut out);
                run = Some((i, 1, holiday));
            }
        }
    }
    flush(run, &mut out);
    out
}
