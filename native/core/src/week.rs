//! Week-row layout of the month and multi-week views (0.2's src/lib/layout.ts): which lane each
//! task chip or multi-day bar takes, and what is hidden behind "+N".

use crate::date::{add_days, diff_days};
use crate::recur::{is_day_series, occurrence_on, Occurrence};
use std::cmp::Ordering;

/// A task occurrence clipped to one week row.
#[derive(Debug, Clone)]
pub struct Chip<'a> {
    pub key: String,
    pub occ: Occurrence<'a>,
    /// 0–6
    pub col: usize,
    pub span: usize,
    pub lane: usize,
    /// Continues from the previous / into the next week.
    pub cont_left: bool,
    pub cont_right: bool,
    pub multi_day: bool,
    pub visible: bool,
    /// A day of a series with an end (每天 from … to …, GitHub #12) whose day before / after
    /// is one too: the days share a lane and are drawn joined.
    pub linked: bool,
    pub join_left: bool,
    pub join_right: bool,
    /// The first day this chip covers (the occurrence's start, or the week's).
    pub seg_start: String,
    /// The days it stands for: its columns in a week row, the whole task elsewhere.
    pub seg_days: usize,
}

impl<'a> Chip<'a> {
    /// A chip outside the week rows (the list, the "+N" day list): the whole occurrence.
    pub fn alone(occ: Occurrence<'a>) -> Chip<'a> {
        Chip {
            key: format!("{}@{}", occ.task.id, occ.date),
            col: 0,
            span: 1,
            lane: 0,
            cont_left: false,
            cont_right: false,
            multi_day: occ.start != occ.end,
            visible: true,
            linked: false,
            join_left: false,
            join_right: false,
            seg_start: occ.start.clone(),
            seg_days: (diff_days(&occ.start, &occ.end) + 1).max(1) as usize,
            occ,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WeekLayout<'a> {
    pub chips: Vec<Chip<'a>>,
    /// Hidden chip count per column ("+N").
    pub overflow: [usize; 7],
    /// Lane index where the "+N" button sits, per column.
    pub overflow_lane: [usize; 7],
}

fn sort_key(a: &Occurrence, b: &Occurrence, completed_last: bool) -> Ordering {
    let am = if a.start != a.end { 0 } else { 1 };
    let bm = if b.start != b.end { 0 } else { 1 };
    if am != bm {
        return am.cmp(&bm); // multi-day bars first, so they get the top lanes
    }
    if a.start != b.start {
        return a.start.cmp(&b.start);
    }
    let (al, bl) = (diff_days(&a.start, &a.end), diff_days(&b.start, &b.end));
    if al != bl {
        return bl.cmp(&al);
    }
    if completed_last && a.done != b.done {
        return if a.done { Ordering::Greater } else { Ordering::Less };
    }
    let (at, bt) = (a.task.time.as_deref().unwrap_or(""), b.task.time.as_deref().unwrap_or(""));
    if at != bt {
        // all-day first, then by time
        return if at.is_empty() { Ordering::Less } else if bt.is_empty() { Ordering::Greater } else { at.cmp(bt) };
    }
    a.task.order.partial_cmp(&b.task.order).unwrap_or(Ordering::Equal)
}

/// Lays out occurrences in a week row starting at `week_start`: each chip takes the lowest lane
/// free across all its columns, then what doesn't fit (`capacity` lanes; None = unlimited) is
/// hidden. A column that overflows gives its last visible lane to a "+N" button.
pub fn layout_week<'a>(week_start: &str, occs: Vec<Occurrence<'a>>, capacity: Option<usize>, completed_last: bool) -> WeekLayout<'a> {
    let week_end = add_days(week_start, 6);
    let mut sorted: Vec<Occurrence> = occs.into_iter().filter(|o| o.end.as_str() >= week_start && o.start <= week_end).collect();
    sorted.sort_by(|a, b| sort_key(a, b, completed_last));

    // Days of a series with an end that follow each other take one lane together, like a bar
    // (GitHub #12): the stretch is placed as a whole, where its first day comes in the order.
    let mut runs: Vec<Vec<Occurrence>> = Vec::new();
    let mut units: Vec<(Occurrence, Option<usize>)> = Vec::with_capacity(sorted.len());
    {
        let mut series: Vec<Occurrence> = Vec::new();
        for occ in sorted {
            if occ.start == occ.end && is_day_series(occ.task) {
                series.push(occ);
            } else {
                units.push((occ, None));
            }
        }
        series.sort_by(|a, b| a.task.id.cmp(&b.task.id).then_with(|| a.start.cmp(&b.start)));
        for occ in series {
            match runs.last_mut().filter(|r| r.last().is_some_and(|l| l.task.id == occ.task.id && add_days(&l.start, 1) == occ.start)) {
                Some(r) => r.push(occ),
                None => runs.push(vec![occ]),
            }
        }
        for (i, r) in runs.iter().enumerate() {
            let (first, last) = (&r[0], &r[r.len() - 1]);
            let whole = Occurrence { task: first.task, date: first.date.clone(), start: first.start.clone(), end: last.start.clone(), done: r.iter().all(|o| o.done) };
            units.push((whole, Some(i)));
        }
        units.sort_by(|a, b| sort_key(&a.0, &b.0, completed_last));
    }

    let mut used: [Vec<bool>; 7] = Default::default();
    let mut chips = Vec::with_capacity(units.len());
    for (occ, run) in units {
        let s = if occ.start.as_str() < week_start { week_start.to_owned() } else { occ.start.clone() };
        let e = if occ.end > week_end { week_end.clone() } else { occ.end.clone() };
        let col = diff_days(week_start, &s) as usize;
        let span = (diff_days(&s, &e) + 1) as usize;
        let mut lane = 0;
        while (col..col + span).any(|i| used[i].get(lane).copied().unwrap_or(false)) {
            lane += 1;
        }
        for u in &mut used[col..col + span] {
            if u.len() <= lane {
                u.resize(lane + 1, false);
            }
            u[lane] = true;
        }
        let Some(run) = run else {
            chips.push(Chip {
                key: format!("{}@{}", occ.task.id, occ.date),
                cont_left: occ.start.as_str() < week_start,
                cont_right: occ.end > week_end,
                multi_day: occ.start != occ.end,
                occ,
                col,
                span,
                lane,
                visible: true,
                linked: false,
                join_left: false,
                join_right: false,
                seg_start: s,
                seg_days: span,
            });
            continue;
        };
        let days = std::mem::take(&mut runs[run]);
        let n = days.len();
        for (k, day) in days.into_iter().enumerate() {
            // at the week's edges: the series may go on in the next row
            let before = k > 0 || occurrence_on(day.task, &add_days(&day.start, -1)).is_some();
            let after = k + 1 < n || occurrence_on(day.task, &add_days(&day.start, 1)).is_some();
            chips.push(Chip {
                key: format!("{}@{}", day.task.id, day.date),
                cont_left: false,
                cont_right: false,
                multi_day: false,
                col: col + k,
                span: 1,
                lane,
                visible: true,
                linked: true,
                join_left: before,
                join_right: after,
                seg_start: day.start.clone(),
                seg_days: 1,
                occ: day,
            });
        }
    }

    let mut out = WeekLayout { chips, ..Default::default() };
    let Some(cap) = capacity else { return out };
    // Lanes each column may show: all when they fit, else one fewer (room for "+N").
    let mut max_lane = [-1i64; 7];
    for c in &out.chips {
        for m in &mut max_lane[c.col..c.col + c.span] {
            *m = (*m).max(c.lane as i64);
        }
    }
    let limit = max_lane.map(|m| if (m + 1) as usize <= cap { cap } else { cap.saturating_sub(1) });
    for c in &mut out.chips {
        if (c.col..c.col + c.span).any(|i| c.lane >= limit[i]) {
            c.visible = false;
        }
    }
    for c in out.chips.iter().filter(|c| !c.visible) {
        for o in &mut out.overflow[c.col..c.col + c.span] {
            *o += 1;
        }
    }
    for i in 0..7 {
        if out.overflow[i] == 0 {
            continue;
        }
        // Put the button in the first lane of this column not used by a visible chip.
        let busy: Vec<usize> = out.chips.iter().filter(|c| c.visible && c.col <= i && i < c.col + c.span).map(|c| c.lane).collect();
        let mut lane = 0;
        while busy.contains(&lane) {
            lane += 1;
        }
        out.overflow_lane[i] = lane.min(cap.saturating_sub(1));
    }
    out
}

/// Lanes a week row needs to show all of its chips (0 when it has none).
pub fn lanes_needed(layout: &WeekLayout) -> usize {
    layout.chips.iter().map(|c| c.lane + 1).max().unwrap_or(0)
}

/// 行高随任务多少调整 (GitHub #6): splits `room` (the grid's height without the gaps between
/// rows) between week rows needing `lanes[i]` task lanes, at least one each. A row with k lanes
/// is `top + k * lane` high. When everything fits, what is left is shared equally; when not,
/// rows needing less than an equal share keep what they need and the others share the rest
/// (and show "+N"). With no room for one lane each, the rows come out equal.
pub fn fit_rows(lanes: &[usize], room: f32, top: f32, lane: f32) -> Vec<f32> {
    let n = lanes.len();
    if n == 0 {
        return Vec::new();
    }
    let want: Vec<f32> = lanes.iter().map(|&k| top + k.max(1) as f32 * lane).collect();
    let total: f32 = want.iter().sum();
    if total <= room {
        let extra = (room - total) / n as f32;
        return want.iter().map(|w| w + extra).collect();
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| want[a].total_cmp(&want[b]));
    let (mut left, mut out) = (room, vec![0.0; n]);
    for (k, &i) in order.iter().enumerate() {
        let h = want[i].min(left / (n - k) as f32);
        out[i] = h;
        left -= h;
    }
    out
}

/// Lanes that fit in a row `h` high (the inverse of `fit_rows`'s `top + k * lane`).
pub fn lanes_in(h: f32, top: f32, lane: f32) -> usize {
    if lane <= 0.0 {
        return 0;
    }
    ((h - top) / lane + 1e-3).floor().max(0.0) as usize
}
