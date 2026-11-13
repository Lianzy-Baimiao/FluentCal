//! Updating the window's list models in place: replacing a model recreates every item drawn
//! from it, and an item the pointer is on loses its pressed or hover state (a quick second
//! click on a chip's check went nowhere). Only rows that changed are rewritten.

use crate::WeekData;
use slint::{Model, ModelRc, VecModel};

fn vec_model<T: Clone + 'static>(m: &ModelRc<T>) -> Option<&VecModel<T>> {
    m.as_any().downcast_ref::<VecModel<T>>()
}

/// Makes `m` hold `rows`, rewriting only the rows that differ (`same`).
fn reconcile<T: Clone + 'static>(m: &VecModel<T>, rows: Vec<T>, same: impl Fn(&T, &T) -> bool) {
    let n = rows.len();
    for (i, r) in rows.into_iter().enumerate() {
        match m.row_data(i) {
            Some(old) if same(&old, &r) => {}
            Some(_) => m.set_row_data(i, r),
            None => m.push(r),
        }
    }
    while m.row_count() > n {
        m.remove(m.row_count() - 1);
    }
}

/// Plain rows (cells, chips, countdowns): `==` compares them. `set` installs a new model when
/// the window doesn't have one yet.
pub fn rows<T: Clone + PartialEq + 'static>(current: &ModelRc<T>, rows: Vec<T>, set: impl FnOnce(ModelRc<T>)) {
    match vec_model(current) {
        Some(m) => reconcile(m, rows, |a, b| a == b),
        None => set(ModelRc::new(VecModel::from(rows))),
    }
}

/// Week rows: their cells and chips are updated inside the existing week models.
pub fn weeks(current: &ModelRc<WeekData>, new: Vec<WeekData>, set: impl FnOnce(ModelRc<WeekData>)) {
    let Some(m) = vec_model(current) else {
        return set(ModelRc::new(VecModel::from(new.into_iter().map(own).collect::<Vec<_>>())));
    };
    let n = new.len();
    for (i, w) in new.into_iter().enumerate() {
        let Some(old) = m.row_data(i) else {
            m.push(own(w));
            continue;
        };
        let (Some(cm), Some(hm)) = (vec_model(&old.cells), vec_model(&old.chips)) else {
            m.set_row_data(i, own(w));
            continue;
        };
        reconcile(cm, w.cells.iter().collect(), |a, b| a == b);
        reconcile(hm, w.chips.iter().collect(), |a, b| a == b);
        let same = old.more.iter().eq(w.more.iter()) && old.more_lane.iter().eq(w.more_lane.iter()) && old.week_no == w.week_no && old.row_y == w.row_y && old.row_h == w.row_h;
        if !same {
            m.set_row_data(i, WeekData { cells: old.cells.clone(), chips: old.chips.clone(), more: ints(&w.more), more_lane: ints(&w.more_lane), week_no: w.week_no, row_y: w.row_y, row_h: w.row_h });
        }
    }
    while m.row_count() > n {
        m.remove(m.row_count() - 1);
    }
}

/// A week whose inner models are VecModels (so they can be updated later).
fn own(w: WeekData) -> WeekData {
    WeekData {
        cells: ModelRc::new(VecModel::from(w.cells.iter().collect::<Vec<_>>())),
        chips: ModelRc::new(VecModel::from(w.chips.iter().collect::<Vec<_>>())),
        more: ints(&w.more),
        more_lane: ints(&w.more_lane),
        week_no: w.week_no,
        row_y: w.row_y,
        row_h: w.row_h,
    }
}

fn ints(m: &ModelRc<i32>) -> ModelRc<i32> {
    ModelRc::new(VecModel::from(m.iter().collect::<Vec<_>>()))
}
