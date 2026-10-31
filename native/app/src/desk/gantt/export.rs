//! Exporting the Gantt chart (0.2's gantt/exportImage.ts): all rows, the span of the tasks,
//! as a standalone SVG; PNG is that SVG rasterised (as 0.2 drew it on a canvas).

use super::model::{char_w, table_width, Model, RH};
use fluentcal_core::date::{add_days, diff_days, parts};
use fluentcal_core::tasks::Doc;
use fluentcal_core::theme::{Rgba, Tokens};
use std::fmt::Write;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A colour as SVG understands it everywhere (8-digit hex is not universal).
fn css(c: Rgba) -> String {
    let c = c.rounded();
    if c.a >= 0.999 { c.to_hex() } else { format!("rgba({},{},{},{:.3})", c.r, c.g, c.b, c.a) }
}

fn slint_css(c: slint::Color) -> String {
    css(Rgba::new(f64::from(c.red()), f64::from(c.green()), f64::from(c.blue()), f64::from(c.alpha()) / 255.0))
}

/// The chart as SVG: (svg, width, height).
pub fn svg(doc: &Doc, m: &Model, t: &Tokens, today: &str) -> (String, f64, f64) {
    let pad = if doc.settings.gantt.scale == "day" { 3 } else { 10 };
    let (mut lo, mut hi) = ("9999-12-31".to_owned(), "0000-01-01".to_owned());
    for b in &m.bars {
        lo = lo.min(b.start.clone());
        hi = hi.max(b.end.clone());
    }
    if lo > hi {
        (lo, hi) = (today.to_owned(), today.to_owned());
    }
    let (lo, hi) = (add_days(&lo, -pad).max(m.range.0.clone()), add_days(&hi, pad).min(m.range.1.clone()));
    let x_of = |d: &str| diff_days(&m.range.0, d) as f64 * m.dw;
    let x0 = x_of(&lo);
    let cw = (diff_days(&lo, &hi) + 1) as f64 * m.dw;
    let tw = if doc.settings.gantt.show_table { table_width(doc, None).max(180.0) } else { 0.0 };
    let (w, h) = (tw + cw, 44.0 + m.rows.len() as f64 * RH);
    let (bg, text, sub, line, accent, group, crit) = (css(t.solid_base), css(t.text_primary), css(t.text_secondary), css(t.divider), css(t.accent_fill), css(t.subtle_fill_pressed), css(t.critical));
    let mut o = String::new();
    let _ = write!(o, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Segoe UI Variable Text, Segoe UI, Microsoft YaHei UI, sans-serif" font-size="12">"#);
    let _ = write!(o, r#"<rect width="{w}" height="{h}" fill="{bg}"/><clipPath id="c"><rect x="{tw}" y="0" width="{cw}" height="{h}"/></clipPath>"#);
    for (id, c) in [("a", &sub), ("l", &crit)] {
        let _ = write!(o, r#"<marker id="{id}" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0,0 L8,4 L0,8 z" fill="{c}"/></marker>"#);
    }
    for (i, r) in m.rows.iter().enumerate() {
        let y = 44.0 + i as f64 * RH;
        if r.kind == "group" {
            let _ = write!(o, r#"<rect x="0" y="{y}" width="{w}" height="{RH}" fill="{group}"/>"#);
        }
        let _ = write!(o, r#"<line x1="0" x2="{w}" y1="{}" y2="{}" stroke="{line}"/>"#, y + RH - 0.5, y + RH - 0.5);
        if tw == 0.0 || r.kind == "add" {
            continue;
        }
        let extra = r.pct.to_string();
        let (lx, weight) = if r.kind == "group" { (10, 600) } else { (24, 400) };
        let _ = write!(o, r#"<text x="{lx}" y="{}" fill="{text}" font-weight="{weight}">{}</text>"#, y + 20.0, esc(&r.name));
        if !extra.is_empty() {
            let _ = write!(o, r#"<text x="{}" y="{}" fill="{sub}" text-anchor="end">{extra}</text>"#, tw - 8.0, y + 20.0);
        }
    }
    if tw > 0.0 {
        let _ = write!(o, r#"<line x1="{tw}" x2="{tw}" y1="0" y2="{h}" stroke="{line}"/>"#);
    }
    let _ = write!(o, r#"<g clip-path="url(#c)"><g transform="translate({},0)">"#, tw - x0);
    for c in &m.top {
        let (cx, cwid) = (f64::from(c.x), f64::from(c.w));
        if cx + cwid <= x0 || cx >= x0 + cw {
            continue;
        }
        let _ = write!(o, r#"<text x="{}" y="16" fill="{text}" font-weight="600">{}</text><line x1="{cx}" x2="{cx}" y1="0" y2="{h}" stroke="{line}"/>"#, cx.max(x0) + 6.0, esc(&c.label));
    }
    for c in &m.bottom {
        let (cx, cwid) = (f64::from(c.x), f64::from(c.w));
        if cx + cwid < x0 || cx > x0 + cw {
            continue;
        }
        let _ = write!(o, r#"<text x="{}" y="37" fill="{}" text-anchor="middle" font-size="11">{}</text>"#, cx + cwid / 2.0, if c.today { &accent } else { &sub }, esc(&c.label));
    }
    let _ = write!(o, r#"<line x1="{x0}" x2="{}" y1="43.5" y2="43.5" stroke="{line}"/>"#, x0 + cw);
    let tx = x_of(today) + m.dw / 2.0;
    let _ = write!(o, r#"<line x1="{tx}" x2="{tx}" y1="44" y2="{h}" stroke="{accent}" stroke-width="2"/>"#);
    for s in &m.summaries {
        let (sx, sw, sy) = (f64::from(s.x), f64::from(s.w), 44.0 + f64::from(s.row) * RH + 11.0);
        let c = slint_css(s.color);
        let _ = write!(o, r#"<rect x="{sx}" y="{sy}" width="{sw}" height="10" rx="2" fill="{c}" opacity="0.45"/><rect x="{sx}" y="{sy}" width="{}" height="10" rx="2" fill="{c}"/>"#, sw * f64::from(s.progress) / 100.0);
    }
    links(&mut o, doc, m, &sub, &crit);
    bars(&mut o, m);
    o += "</g></g></svg>";
    (o, w, h)
}

/// Dependency arrows as 0.2 exports them: solid, with a marker, red when late.
fn links(o: &mut String, doc: &Doc, m: &Model, sub: &str, crit: &str) {
    if !doc.settings.gantt.show_deps {
        return;
    }
    let pos: std::collections::BTreeMap<&str, &super::model::Bar> = m.bars.iter().filter(|b| !b.g.rep).map(|b| (b.g.task_id.as_str(), b)).collect();
    for b in pos.values() {
        let Some(t) = doc.tasks.get(b.g.task_id.as_str()) else { continue };
        for dep in t.deps() {
            let Some(p) = pos.get(dep.as_str()) else { continue };
            let late = p.end >= b.start;
            let d = fluentcal_core::gantt::dep_path(p.x + p.w, f64::from(p.g.row) * RH + RH / 2.0, b.x, f64::from(b.g.row) * RH + RH / 2.0);
            let _ = write!(o, r#"<path d="{d}" transform="translate(0,44)" fill="none" stroke="{}" stroke-width="1.25" marker-end="url(#{})"/>"#, if late { crit } else { sub }, if late { "l" } else { "a" });
        }
    }
}

fn bars(o: &mut String, m: &Model) {
    for b in &m.bars {
        let y = 44.0 + f64::from(b.g.row) * RH;
        let op = if b.g.done { r#" opacity="0.55""# } else { "" };
        let (bar, on, text) = (slint_css(b.g.bar), slint_css(b.g.on_bar), slint_css(b.g.text));
        if b.g.milestone {
            let cx = b.x + m.dw / 2.0;
            let _ = write!(o, r#"<rect x="{}" y="{}" width="14" height="14" rx="2" fill="{bar}" transform="rotate(45 {cx} {})"{op}/>"#, cx - 7.0, y + 9.0, y + 16.0);
            let _ = write!(o, r#"<text x="{}" y="{}" fill="{text}">{}</text>"#, cx + 14.0, y + 20.0, esc(&b.g.title));
            continue;
        }
        let _ = write!(o, r#"<g{op}><rect x="{}" y="{}" width="{}" height="20" rx="4" fill="{bar}"/>"#, b.x + 1.0, y + 6.0, (b.w - 2.0).max(4.0));
        if b.g.progress > 0.0 {
            let _ = write!(o, r##"<rect x="{}" y="{}" width="{}" height="20" rx="4" fill="#000" opacity="0.22"/>"##, b.x + 1.0, y + 6.0, ((b.w - 2.0) * f64::from(b.g.progress) / 100.0).max(0.0));
        }
        let inside = char_w(&b.g.title) + 14.0 <= b.w;
        let _ = write!(o, r#"<text x="{}" y="{}" fill="{}">{}</text></g>"#, if inside { b.x + 8.0 } else { b.x + b.w + 6.0 }, y + 20.0, if inside { &on } else { &text }, esc(&b.g.title));
    }
}

/// "甘特图-20261014.png"
pub fn file_name(today: &str, ext: &str) -> String {
    let (y, m, d) = parts(today);
    format!("甘特图-{y}{m:02}{d:02}.{ext}")
}

/// The SVG as PNG: 2× when it fits (at most 16000 px a side), on its own background.
pub fn png(svg: &str, w: f64, h: f64) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    let db = opt.fontdb_mut();
    // Only the fonts the SVG names (scanning every system font takes a second).
    let dir = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| r"C:\Windows".into())).join("Fonts");
    for f in ["SegUIVar.ttf", "segoeui.ttf", "segoeuib.ttf", "seguisb.ttf", "msyh.ttc", "msyhbd.ttc"] {
        let _ = db.load_font_file(dir.join(f));
    }
    if db.is_empty() {
        db.load_system_fonts();
    }
    db.set_sans_serif_family("Microsoft YaHei UI");
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let k = 2f64.min(16000.0 / w).min(16000.0 / h);
    let (pw, ph) = ((w * k).round() as u32, (h * k).round() as u32);
    let mut pix = resvg::tiny_skia::Pixmap::new(pw.max(1), ph.max(1)).ok_or("图片太大")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k as f32, k as f32), &mut pix.as_mut());
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, pw, ph);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    // The background is opaque, so premultiplied = straight alpha.
    wr.write_image_data(pix.data()).map_err(|e| e.to_string())?;
    drop(wr);
    Ok(out)
}

impl crate::app::App {
    /// "导出图片 › PNG/SVG…": the chart of all rows over the tasks' span.
    pub fn gantt_export(&mut self, kind: &'static str) {
        let today = self.today();
        let m = super::model::compute(&self.doc, &today, &self.tokens, self.desk.gantt.ungrouped_collapsed);
        let (svg, w, h) = svg(&self.doc, &m, &self.tokens, &today);
        let (filter, ext) = if kind == "svg" { ("SVG 图片", "svg") } else { ("PNG 图片", "png") };
        self.save_file_later(&file_name(&today, ext), filter, &[ext], move || if kind == "svg" { Ok(svg.into_bytes()) } else { png(&svg, w, h) });
    }
}
