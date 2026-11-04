//! The desk window's Gantt view (0.2's gantt/): its state, what the window gets (only the
//! items near the visible part: the chart can be years wide) and scrolling.

use fluentcal_core::i18n::{trf};

pub mod actions;
pub mod drag;
pub mod export;
pub mod geom;
pub mod keys;
pub mod model;

use crate::app::App;
use crate::{GBar, GCell, GLink, GStripe, GanttData};
use fluentcal_core::date::{add_days, diff_days, parts};
use model::{char_w, table_width, Bar, Model, RH};
use slint::{ComponentHandle, ModelRc, VecModel};

pub struct BarDrag {
    pub bar: Bar,
    /// "move" | "start" | "end" | "progress"
    pub mode: String,
    pub x0: f64,
    pub moved: bool,
    /// (start, end, progress) while dragging.
    pub preview: Option<(String, String, f64)>,
}

#[derive(Default)]
pub struct GanttUi {
    pub selected: String,
    /// 未分组 collapsed (this window only, as in 0.2).
    pub ungrouped_collapsed: bool,
    pub bar_drag: Option<BarDrag>,
    /// Drag-creating on an add row: (group, row, first day, start, end).
    pub zone_drag: Option<(String, i32, String, String, String)>,
    /// Link rubber band: (from task, x, y).
    pub link_drag: Option<(String, f64, f64)>,
    /// Table splitter: (pointer x at the press, width then); the width while dragging.
    pub split: Option<(f64, f64)>,
    pub live_width: Option<f64>,
    pub model: Option<Model>,
    pub dirty: bool,
    /// Chart px range of the items the window has.
    pub window: (f64, f64),
    /// Scroll so this date is at this fraction of the chart, once the view has a size.
    pub center: Option<(String, f64)>,
    pub label: String,
    /// A smooth scroll under way (0.2: scrollTo behavior 'smooth'): its steps.
    pub glide: slint::Timer,
}

fn rc<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

/// What the Gantt functions work on: the window and the state, plus the data they show.
pub struct View<'a> {
    pub window: &'a crate::DeskWindow,
    pub doc: &'a fluentcal_core::tasks::Doc,
    pub tokens: &'a fluentcal_core::theme::Tokens,
    pub today: &'a str,
    pub g: &'a mut GanttUi,
}

impl View<'_> {
    /// Recomputes (when the data changed) and hands the window what it shows.
    pub fn push(&mut self) {
        if self.g.dirty || self.g.model.is_none() {
            let old = self.g.model.as_ref().map(|m| (m.dw, m.range.0.clone()));
            let keep = self.date_at(0.5);
            let m = model::compute(self.doc, self.today, self.tokens, self.g.ungrouped_collapsed);
            // Keep the date at the centre when zooming or when the range grows at the start.
            let moved = old.is_some_and(|(dw, rs)| dw != m.dw || rs != m.range.0);
            if moved && self.g.center.is_none() {
                self.g.center = Some((keep, 0.5));
            }
            self.g.model = Some(m);
            self.g.dirty = false;
            self.g.window = (f64::NAN, f64::NAN);
            self.push_static();
        }
        self.scrolled();
    }

    /// What doesn't depend on the scroll position.
    fn push_static(&mut self) {
        let (Some(m), gd) = (&self.g.model, self.window.global::<GanttData>()) else { return };
        let tw = table_width(self.doc, self.g.live_width);
        gd.set_rows(rc(m.rows.clone()));
        gd.set_summaries(rc(m.summaries.clone()));
        gd.set_top(rc(m.top.clone()));
        gd.set_month_lines(rc(m.month_lines.iter().map(|x| *x as f32).collect()));
        gd.set_scale(self.doc.settings.gantt.scale.clone().into());
        gd.set_dw(m.dw as f32);
        gd.set_table_w(tw as f32);
        gd.set_show_dates(tw >= 230.0);
        gd.set_show_pct(tw >= 180.0);
        gd.set_chart_w(m.chart_w as f32);
        gd.set_body_h((m.rows.len().max(1) as f64 * RH) as f32);
        gd.set_today_x(m.today_x as f32);
        gd.set_selected(self.g.selected.clone().into());
        gd.set_read_only(self.window.get_read_only());
    }

    /// Date at a fraction of the visible chart width (0.5 = centre).
    pub fn date_at(&self, frac: f64) -> String {
        let Some(m) = &self.g.model else { return self.today.to_owned() };
        let w = f64::from(self.window.get_gantt_view_w());
        let x = -f64::from(self.window.global::<GanttData>().get_scroll_x()) + w * frac;
        add_days(&m.range.0, (x / m.dw).floor() as i32)
    }

    /// The chart scrolled or changed size: pending scroll, items near the view, toolbar label.
    pub fn scrolled(&mut self) {
        let w = f64::from(self.window.get_gantt_view_w());
        let gd = self.window.global::<GanttData>();
        if w > 0.0 {
            if let (Some((d, frac)), Some(m)) = (self.g.center.take(), &self.g.model) {
                let x = (diff_days(&m.range.0, &d) as f64 * m.dw - w * frac).min(m.chart_w - w).max(0.0);
                gd.set_scroll_x(-x as f32);
            }
        }
        self.virtualize(false);
        let (y, mo, _) = parts(&self.date_at(0.3));
        let label = trf("{0}年{1}月", &[format!("{}", y), format!("{}", mo)]);
        if label != self.g.label {
            gd.set_label(label.clone().into());
            self.g.label = label;
        }
    }

    /// Hands the window the items within one and a half views of the visible part.
    pub fn virtualize(&mut self, force: bool) {
        let g = &mut *self.g;
        let Some(m) = &g.model else { return };
        if g.bar_drag.is_some() || g.zone_drag.is_some() || g.link_drag.is_some() {
            return; // keep the item under the pointer alive
        }
        let gd = self.window.global::<GanttData>();
        let w = f64::from(self.window.get_gantt_view_w()).max(600.0);
        let sx = -f64::from(gd.get_scroll_x());
        let (a, b) = g.window;
        if !force && !a.is_nan() && sx - a > w * 0.5 && b - (sx + w) > w * 0.5 {
            return;
        }
        let (a, b) = ((sx - w * 1.5).max(0.0), sx + w * 2.5);
        let near = |x: f64, len: f64| x + len >= a && x <= b;
        let label = |b: &Bar| if b.g.inside { 0.0 } else { char_w(&b.g.title) + 60.0 };
        let bars: Vec<GBar> = m.bars.iter().filter(|x| !x.g.milestone && near(x.x, x.w + label(x))).map(|x| x.g.clone()).collect();
        let ms: Vec<GBar> = m.bars.iter().filter(|x| x.g.milestone && near(x.x - 20.0, label(x) + 40.0)).map(|x| x.g.clone()).collect();
        let links: Vec<GLink> = m.links.iter().filter(|l| near(f64::from(l.x), f64::from(l.w))).cloned().collect();
        let cells: Vec<GCell> = m.bottom.iter().filter(|c| near(f64::from(c.x), f64::from(c.w))).cloned().collect();
        let stripes: Vec<GStripe> = m.stripes.iter().filter(|s| near(f64::from(s.x), f64::from(s.w))).cloned().collect();
        let weeks: Vec<f32> = m.week.map_or(Vec::new(), |(off, p)| {
            let first = ((a - off) / p).floor().max(0.0) as i64;
            (first..).map(|k| off + k as f64 * p).take_while(|x| *x <= b.min(m.chart_w)).map(|x| x as f32).collect()
        });
        gd.set_bars(rc(bars));
        gd.set_milestones(rc(ms));
        gd.set_links(rc(links));
        gd.set_bottom(rc(cells));
        gd.set_stripes(rc(stripes));
        gd.set_week_lines(rc(weeks));
        g.window = (a, b);
    }
}

impl App {
    /// The document, settings or theme changed: recompute when the view is shown.
    pub fn gantt_refresh(&mut self) {
        self.desk.gantt.dirty = true;
        if self.doc.settings.calendar.view == "gantt" {
            self.gantt_push();
        }
    }

    pub fn gantt_push(&mut self) {
        let today = self.today();
        View { window: &self.desk.window, doc: &self.doc, tokens: &self.tokens, today: &today, g: &mut self.desk.gantt }.push();
    }

    pub fn gantt_date_at(&mut self, frac: f64) -> String {
        let today = self.today();
        View { window: &self.desk.window, doc: &self.doc, tokens: &self.tokens, today: &today, g: &mut self.desk.gantt }.date_at(frac)
    }

    /// Scrolls so `date` is at `frac` of the chart (0.2's scrollToDate).
    pub fn gantt_scroll_to(&mut self, date: &str, frac: f64) {
        self.desk.gantt.glide.stop();
        self.desk.gantt.center = Some((date.to_owned(), frac));
        self.gantt_scrolled();
    }

    /// The same, gliding there (0.2's smooth scroll for 今天 and revealing a task): about 300 ms,
    /// ease in-out; a scroll by the user meanwhile ends it. At once when motion is reduced.
    pub fn gantt_glide_to(&mut self, date: &str, frac: f64) {
        let w = f64::from(self.desk.window.get_gantt_view_w());
        let Some(m) = self.desk.gantt.model.as_ref().filter(|_| w > 0.0 && !self.doc.settings.theme.reduce_motion) else {
            return self.gantt_scroll_to(date, frac);
        };
        let to = -((fluentcal_core::date::diff_days(&m.range.0, date) as f64 * m.dw - w * frac).min(m.chart_w - w).max(0.0)) as f32;
        let gd = slint::ComponentHandle::global::<crate::GanttData>(&self.desk.window);
        let from = gd.get_scroll_x();
        if (to - from).abs() < 1.0 {
            return;
        }
        let start = std::time::Instant::now();
        let last = std::rc::Rc::new(std::cell::Cell::new(from));
        self.desk.gantt.glide.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(16), move || {
            let last = last.clone();
            crate::app::run(move |app| {
                let gd = slint::ComponentHandle::global::<crate::GanttData>(&app.desk.window);
                if (gd.get_scroll_x() - last.get()).abs() > 0.5 {
                    return app.desk.gantt.glide.stop(); // the user scrolled
                }
                let t = (start.elapsed().as_secs_f32() / 0.3).min(1.0);
                let e = if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 };
                let x = from + (to - from) * e;
                gd.set_scroll_x(x);
                last.set(x);
                app.gantt_scrolled();
                if t >= 1.0 {
                    app.desk.gantt.glide.stop();
                }
            });
        });
    }

    pub fn gantt_scrolled(&mut self) {
        let today = self.today();
        View { window: &self.desk.window, doc: &self.doc, tokens: &self.tokens, today: &today, g: &mut self.desk.gantt }.scrolled();
    }

    pub fn gantt_virtualize(&mut self, force: bool) {
        let today = self.today();
        View { window: &self.desk.window, doc: &self.doc, tokens: &self.tokens, today: &today, g: &mut self.desk.gantt }.virtualize(force);
    }
}
