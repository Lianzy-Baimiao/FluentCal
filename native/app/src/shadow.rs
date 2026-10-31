//! CSS box-shadows for the software renderer, which has no drop shadow of its own. A shadow is
//! rendered once per look and scale for a small "virtual" box: corners as they are, one column
//! and one row of plain edge in the middle, and nothing under the box (CSS clips a box-shadow
//! to the outside of the box). theme.slint's BoxShadow stretches the middle over a box of any
//! size.

use crate::ShadowImage;
use fluentcal_core::theme::Shadow;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// Gaussian blur of CSS: `blur` px is twice the standard deviation.
fn sigma(blur: f64) -> f64 {
    blur / 2.0
}

pub fn render(layers: &[Shadow], radius: f64, scale: f32) -> ShadowImage {
    let layers: Vec<&Shadow> = layers.iter().filter(|l| l.color.a > 0.0).collect();
    if layers.is_empty() {
        return ShadowImage::default();
    }
    let scale = f64::from(scale.max(0.5));
    // Large blurs are soft enough to render at a lower resolution.
    let max_blur = layers.iter().map(|l| l.blur * scale).fold(0.0, f64::max);
    let k = ((max_blur / 40.0).ceil() as i32).clamp(1, 8);
    let s = scale / f64::from(k);
    let reach = |l: &Shadow| (3.0 * sigma(l.blur) * s).ceil() as i32;
    let side = |f: &dyn Fn(&Shadow) -> f64| layers.iter().map(|l| (reach(l) as f64 + f(l) * s).ceil() as i32).max().unwrap_or(0).max(1);
    let left = side(&|l| -l.x);
    let right = side(&|l| l.x);
    let top = side(&|l| -l.y);
    let bottom = side(&|l| l.y);
    let r = radius * s;
    let corner = layers
        .iter()
        .map(|l| (r + f64::from(reach(l)) + l.x.abs().max(l.y.abs()) * s).ceil() as i32)
        .max()
        .unwrap_or(1)
        + 1;
    let inner = 2 * corner + 1;
    let (w, h) = ((left + inner + right) as usize, (top + inner + bottom) as usize);
    let (bx, by) = (f64::from(left), f64::from(top));
    let mut acc = vec![0f64; w * h];
    let mut buf = vec![0f64; w * h];
    for l in &layers {
        cover(&mut buf, w, h, bx + l.x * s, by + l.y * s, f64::from(inner), f64::from(inner), r);
        blur(&mut buf, w, h, sigma(l.blur) * s);
        for (a, v) in acc.iter_mut().zip(&buf) {
            let src = v * l.color.a;
            *a = src + *a * (1.0 - src);
        }
    }
    cover(&mut buf, w, h, bx, by, f64::from(inner), f64::from(inner), r);
    let c = layers[0].color;
    let mut px = SharedPixelBuffer::<Rgba8Pixel>::new(w as u32, h as u32);
    for ((p, a), m) in px.make_mut_slice().iter_mut().zip(&acc).zip(&buf) {
        let alpha = (a * (1.0 - m)).clamp(0.0, 1.0);
        *p = Rgba8Pixel { r: c.r as u8, g: c.g as u8, b: c.b as u8, a: (alpha * 255.0).round() as u8 };
    }
    ShadowImage { image: Image::from_rgba8(px), k, left, top, right, bottom, corner }
}

/// Anti-aliased coverage of a rounded rectangle, one value per pixel.
#[allow(clippy::too_many_arguments)]
fn cover(out: &mut [f64], w: usize, h: usize, x: f64, y: f64, bw: f64, bh: f64, r: f64) {
    let (cx, cy) = (x + bw / 2.0, y + bh / 2.0);
    let (hx, hy) = (bw / 2.0 - r, bh / 2.0 - r);
    for j in 0..h {
        for i in 0..w {
            let (px, py) = (i as f64 + 0.5 - cx, j as f64 + 0.5 - cy);
            let (qx, qy) = (px.abs() - hx, py.abs() - hy);
            let outside = qx.max(0.0).hypot(qy.max(0.0));
            let d = outside + qx.max(qy).min(0.0) - r;
            out[j * w + i] = (0.5 - d).clamp(0.0, 1.0);
        }
    }
}

/// Separable Gaussian blur; outside the image counts as empty.
fn blur(v: &mut [f64], w: usize, h: usize, sigma: f64) {
    if sigma < 0.3 {
        return;
    }
    let n = (3.0 * sigma).ceil() as isize;
    let mut kernel: Vec<f64> = (-n..=n).map(|i| (-((i * i) as f64) / (2.0 * sigma * sigma)).exp()).collect();
    let sum: f64 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);
    let mut tmp = vec![0f64; w * h];
    let pass = |src: &[f64], dst: &mut [f64], len: usize, lines: usize, at: &dyn Fn(usize, usize) -> usize| {
        for line in 0..lines {
            for i in 0..len as isize {
                let mut acc = 0.0;
                for (ki, kv) in kernel.iter().enumerate() {
                    let p = i + ki as isize - n;
                    if p >= 0 && (p as usize) < len {
                        acc += src[at(line, p as usize)] * kv;
                    }
                }
                dst[at(line, i as usize)] = acc;
            }
        }
    };
    pass(v, &mut tmp, w, h, &|line, i| line * w + i);
    pass(&tmp, v, h, w, &|line, i| i * w + line);
}
