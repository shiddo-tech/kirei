use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use tiny_skia::Pixmap;

use crate::model::{Art, ArtKind};

type V3 = [f64; 3];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn mix(a: V3, b: V3, t: f64) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Same integer hash as the Swift app (wrapping Int math truncated to UInt32), so a seed
/// produces the same shapes in both.
fn hash(x: i64, y: i64, s: i64) -> f64 {
    let v = x
        .wrapping_mul(374761393)
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(s.wrapping_mul(982451653));
    let mut h = v as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    h as f64 / u32::MAX as f64
}

fn noise(x: f64, y: f64, s: i64) -> f64 {
    let (fx, fy) = (x.floor(), y.floor());
    let (xi, yi) = (fx as i64, fy as i64);
    let (xf, yf) = (x - fx, y - fy);
    let u = xf * xf * xf * (xf * (xf * 6. - 15.) + 10.);
    let v = yf * yf * yf * (yf * (yf * 6. - 15.) + 10.);
    let a = hash(xi, yi, s);
    let b = hash(xi + 1, yi, s);
    let c = hash(xi, yi + 1, s);
    let d = hash(xi + 1, yi + 1, s);
    let top = a + (b - a) * u;
    top + (c + (d - c) * u - top) * v
}

fn fbm(x: f64, y: f64, s: i64, octaves: i64) -> f64 {
    let (mut t, mut amp, mut freq) = (0., 0.5, 1.);
    for i in 0..octaves {
        t += amp * noise(x * freq, y * freq, s + i * 17);
        freq *= 2.03;
        amp *= 0.5;
    }
    t / (1. - 0.5f64.powi(octaves as i32))
}

fn sstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.max(0.).powf(1. / 2.4) - 0.055
    }
}

fn oklab(c: V3) -> V3 {
    let (r, g, b) = (to_linear(c[0]), to_linear(c[1]), to_linear(c[2]));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

fn from_oklab(c: V3) -> V3 {
    let l = (c[0] + 0.3963377774 * c[1] + 0.2158037573 * c[2]).powi(3);
    let m = (c[0] - 0.1055613458 * c[1] - 0.0638541728 * c[2]).powi(3);
    let s = (c[0] - 0.0894841775 * c[1] - 1.2914855480 * c[2]).powi(3);
    [
        to_srgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        to_srgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        to_srgb(-0.0041960771 * l - 0.7034226162 * m + 1.7076147010 * s),
    ]
}

fn oklab_ramp(labs: &[V3], t: f64) -> V3 {
    if labs.len() < 2 {
        return from_oklab(labs[0]);
    }
    let t = t.clamp(0., 0.9999) * (labs.len() - 1) as f64;
    let i = t as usize;
    from_oklab(mix(labs[i], labs[i + 1], sstep(0., 1., t - i as f64)))
}

type Field = Box<dyn Fn(f64, f64, f64) -> V3 + Send + Sync>;

/// Multi-stop gradient blended in OKLab, bent slightly by low-frequency noise so the bands
/// don't read as a flat CSS ramp.
fn linear(labs: Vec<V3>, s: i64) -> Field {
    let ang = (0.15 + hash(s, 3, 21) * 0.5) * std::f64::consts::PI;
    let (dx, dy) = (ang.cos(), ang.sin());
    Box::new(move |u, v, a| {
        let (x, y) = ((u - 0.5) * a, v - 0.5);
        let span = (dx.abs() * a + dy.abs()) / 2.;
        let bend = (fbm(u * 0.9, v * 0.9, s + 60, 2) - 0.5) * 0.22;
        oklab_ramp(&labs, (x * dx + y * dy) / (2. * span) + 0.5 + bend)
    })
}

/// Light pooling from an off-canvas point: an OKLab ramp over distance, slightly elliptical.
fn radial(labs: Vec<V3>, s: i64) -> Field {
    let cx = if hash(s, 1, 31) < 0.5 {
        -0.1 + hash(s, 2, 31) * 0.4
    } else {
        0.7 + hash(s, 2, 31) * 0.4
    };
    let cy = -0.15 + hash(s, 4, 31) * 0.35;
    Box::new(move |u, v, a| {
        let (dx, dy) = ((u - cx) * a, (v - cy) * 1.25);
        let d = (dx * dx + dy * dy).sqrt() / (0.95 * a.max(1.));
        let bend = (fbm(u * 1.1, v * 1.1, s + 61, 2) - 0.5) * 0.12;
        oklab_ramp(&labs, d + bend)
    })
}

/// Soft color blobs over a base, blended in OKLab so mixes stay clean, with a gentle warp so
/// the blobs read as flowing light rather than circles.
fn mesh(labs: Vec<V3>, s: i64) -> Field {
    let base = labs[0];
    let blobs: Vec<V3> = if labs.len() > 1 {
        labs[1..].to_vec()
    } else {
        vec![labs[0]]
    };
    let centers: Vec<(f64, f64)> = (0..blobs.len() as i64)
        .map(|i| (0.05 + 0.9 * hash(s, i, 41), 0.05 + 0.9 * hash(s, i, 42)))
        .collect();
    let radii: Vec<f64> = (0..blobs.len() as i64)
        .map(|i| 0.38 + 0.22 * hash(s, i, 43))
        .collect();
    Box::new(move |u, v, a| {
        let wx = (fbm(u * 1.2, v * 1.2, s + 70, 3) - 0.5) * 0.4;
        let wy = (fbm(u * 1.2 + 3.7, v * 1.2 + 1.9, s + 71, 3) - 0.5) * 0.4;
        let (x, y) = (u + wx, v + wy);
        let mut sum = mul(base, 0.18);
        let mut total = 0.18;
        for i in 0..blobs.len() {
            let dx = (x - centers[i].0) * a;
            let dy = y - centers[i].1;
            let w = (-(dx * dx + dy * dy) / (radii[i] * radii[i]) * 1.6).exp();
            sum = add(sum, mul(blobs[i], w));
            total += w;
        }
        from_oklab(mul(sum, 1. / total))
    })
}

pub fn render_art(art: &Art, w: u32, h: u32) -> Option<Pixmap> {
    if art.palette.is_empty() {
        return None;
    }
    let labs: Vec<V3> = art.palette.iter().map(|c| oklab([c.r, c.g, c.b])).collect();
    let f = match art.kind {
        ArtKind::Mesh => mesh(labs, art.seed),
        ArtKind::Linear => linear(labs, art.seed),
        ArtKind::Radial => radial(labs, art.seed),
    };
    let mut pm = Pixmap::new(w, h)?;
    let aspect = w as f64 / h as f64;
    let q = |d: f64| (d * 255. + 0.5).clamp(0., 255.) as u8;
    pm.data_mut()
        .par_chunks_mut(w as usize * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let v = if h > 1 { y as f64 / (h - 1) as f64 } else { 0. };
            for x in 0..w as usize {
                let u = if w > 1 { x as f64 / (w - 1) as f64 } else { 0. };
                let c = f(u, v, aspect);
                row[x * 4] = q(c[0]);
                row[x * 4 + 1] = q(c[1]);
                row[x * 4 + 2] = q(c[2]);
                row[x * 4 + 3] = 255;
            }
        });
    Some(pm)
}

/// Renders art at a capped size and caches it. Fields are smooth, so upscaling loses nothing.
pub struct ArtCache {
    limit: usize,
    items: Mutex<Vec<((Art, u32, u32), Arc<Pixmap>)>>,
}

impl ArtCache {
    pub const fn new(limit: usize) -> Self {
        Self {
            limit,
            items: Mutex::new(Vec::new()),
        }
    }

    pub fn image(&self, art: &Art, width: f64, height: f64) -> Option<Arc<Pixmap>> {
        if width < 1. || height < 1. {
            return None;
        }
        // Aspect snapped to 0.05 steps so dragging padding reuses the same render.
        let aspect = ((width / height * 20.).round() / 20.).clamp(0.3, 4.);
        let needed = width.max(height);
        let long = [256., 512., 1024.]
            .into_iter()
            .find(|s| *s >= needed)
            .unwrap_or(1600.);
        let (w, h) = if aspect >= 1. {
            (long, long / aspect)
        } else {
            (long * aspect, long)
        };
        let key = (art.clone(), (w as u32).max(1), (h as u32).max(1));
        if let Some((_, img)) = self.items.lock().ok()?.iter().find(|(k, _)| *k == key) {
            return Some(img.clone());
        }
        let img = Arc::new(render_art(art, key.1, key.2)?);
        let mut items = self.items.lock().ok()?;
        items.push((key, img.clone()));
        if items.len() > self.limit {
            items.remove(0);
        }
        Some(img)
    }
}

pub static ART_CACHE: ArtCache = ArtCache::new(8);
pub static THUMB_CACHE: ArtCache = ArtCache::new(96);
