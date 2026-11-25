use std::collections::HashMap;

use crate::model::{Rect, Rgba};
use crate::render::SourceImage;

/// Finds the region inside a uniform-colored border so the content can be re-centered.
pub fn balanced_rect(img: &SourceImage) -> Rect {
    let full = img.full_rect();
    let (w, h) = (img.width() as i64, img.height() as i64);
    if w <= 20 || h <= 20 {
        return full;
    }
    let data = img.pixmap.data();
    let at = |x: i64, y: i64| {
        let i = ((y * w + x) * 4) as usize;
        [
            data[i] as i32,
            data[i + 1] as i32,
            data[i + 2] as i32,
            data[i + 3] as i32,
        ]
    };
    let reference = at(0, 0);
    let same = |x: i64, y: i64| {
        let p = at(x, y);
        (0..4).all(|c| (p[c] - reference[c]).abs() <= 8)
    };
    let mut top = 0;
    while top < h - 1 && (0..w).all(|x| same(x, top)) {
        top += 1;
    }
    let mut bottom = h - 1;
    while bottom > top && (0..w).all(|x| same(x, bottom)) {
        bottom -= 1;
    }
    let mut left = 0;
    while left < w - 1 && (top..=bottom).all(|y| same(left, y)) {
        left += 1;
    }
    let mut right = w - 1;
    while right > left && (top..=bottom).all(|y| same(right, y)) {
        right -= 1;
    }
    let r = Rect::new(
        left as f64,
        top as f64,
        (right - left + 1) as f64,
        (bottom - top + 1) as f64,
    );
    // Ignore degenerate results (blank image or nothing to trim).
    if r.w < 16. || r.h < 16. { full } else { r }
}

/// Dominant opaque color along the crop's edges, used to fill the inset.
pub fn edge_color(img: &SourceImage, crop: Rect) -> Rgba {
    let c = crop.integral().intersection(&img.full_rect());
    if c.is_empty() {
        return Rgba::WHITE;
    }
    let mut counts: HashMap<u32, u32> = HashMap::new();
    let mut sample = |x: i64, y: i64| {
        let p = img.px(x as u32, y as u32);
        if p.3 <= 250 {
            return;
        }
        let key = ((p.0 as u32 >> 3) << 10) | ((p.1 as u32 >> 3) << 5) | (p.2 as u32 >> 3);
        *counts.entry(key).or_default() += 1;
    };
    let (x0, x1) = (c.x as i64, c.max_x() as i64 - 1);
    let (y0, y1) = (c.y as i64, c.max_y() as i64 - 1);
    let inset = 3.min((x1 - x0) / 4).min((y1 - y0) / 4);
    let step_x = ((x1 - x0) / 200).max(1) as usize;
    let step_y = ((y1 - y0) / 200).max(1) as usize;
    for x in (x0..=x1).step_by(step_x) {
        sample(x, y0 + inset);
        sample(x, y1 - inset);
    }
    for y in (y0..=y1).step_by(step_y) {
        sample(x0 + inset, y);
        sample(x1 - inset, y);
    }
    let Some((best, _)) = counts.into_iter().max_by_key(|(k, v)| (*v, *k)) else {
        return Rgba::WHITE;
    };
    Rgba::new(
        ((best >> 10) & 31) as f64 / 31.,
        ((best >> 5) & 31) as f64 / 31.,
        (best & 31) as f64 / 31.,
        1.,
    )
}
