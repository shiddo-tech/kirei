use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use gpui::{Div, Hsla, ParentElement, RenderImage, Rgba, Styled, div, img, px};
use image::Frame;
use smallvec::smallvec;

const SCALE: f64 = 2.;

type Key = (String, u32, u32, [u8; 4]);

fn cache() -> &'static Mutex<HashMap<Key, Option<(Arc<RenderImage>, f32, f32)>>> {
    static CACHE: OnceLock<Mutex<HashMap<Key, Option<(Arc<RenderImage>, f32, f32)>>>> =
        OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// An SF Symbol tinted with `color`, sized in points.
pub fn symbol(
    name: &str,
    size: f32,
    weight: f32,
    color: Hsla,
) -> Option<(Arc<RenderImage>, f32, f32)> {
    let c: Rgba = color.into();
    let rgba = [
        (c.r * 255.) as u8,
        (c.g * 255.) as u8,
        (c.b * 255.) as u8,
        (c.a * 255.) as u8,
    ];
    let key = (
        name.to_string(),
        (size * 10.) as u32,
        (weight * 10.) as u32,
        rgba,
    );
    if let Some(hit) = cache().lock().ok()?.get(&key) {
        return hit.clone();
    }
    let made = crate::platform::render_symbol(name, size as f64, weight as f64, rgba, SCALE).map(
        |(w, h, bgra)| {
            let buffer = image::RgbaImage::from_raw(w, h, bgra).expect("symbol");
            let image = Arc::new(RenderImage::new(smallvec![Frame::new(buffer)]));
            (image, (w as f64 / SCALE) as f32, (h as f64 / SCALE) as f32)
        },
    );
    cache().lock().ok()?.insert(key, made.clone());
    made
}

pub fn icon(name: &str, size: f32, color: Hsla) -> Div {
    icon_weighted(name, size, 0.23, color)
}

/// `weight` uses NSFont.Weight values (0 regular, 0.23 medium, 0.3 semibold).
pub fn icon_weighted(name: &str, size: f32, weight: f32, color: Hsla) -> Div {
    let holder = div().flex().items_center().justify_center().flex_none();
    match symbol(name, size, weight, color) {
        Some((image, w, h)) => holder.child(img(image).w(px(w)).h(px(h))),
        None => holder.w(px(size)).h(px(size)),
    }
}
