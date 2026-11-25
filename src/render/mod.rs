pub mod art;
pub mod blur;
pub mod text;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tiny_skia::{
    BlendMode, Color, FillRule, FilterQuality, GradientStop, LineCap, LineJoin, LinearGradient,
    Mask, Paint, Path, PathBuilder, Pattern, Pixmap, PixmapPaint, Point, RadialGradient,
    SpreadMode, Stroke, Transform,
};

use crate::model::{
    Annotation, AnnotationKind, Background, Pt, Rect, Rgba, Style, WindowFrame, counter_labels,
};

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn from_time() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(7);
        Self::new(t)
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Continuous (squircle) corners, the curve macOS uses for windows and icons.
pub fn squircle(r: Rect, radius: f64) -> Option<Path> {
    const K: f64 = 1.528665;
    let rad = radius.max(0.).min(r.w.min(r.h) / 2.);
    if rad <= 0. {
        return PathBuilder::from_rect(r.skia()?).into();
    }
    // A full continuous corner needs 1.53 r of straight edge.
    let c = rad.min(r.w.min(r.h) / 2. / K);
    let e = c * K;
    let (x0, y0, x1, y1) = (r.x, r.y, r.max_x(), r.max_y());
    let f = |v: f64| v as f32;
    let mut pb = PathBuilder::new();
    let corner = |pb: &mut PathBuilder, cx: f64, cy: f64, sx: f64, sy: f64, rot: bool| {
        // Points for the top-right corner in local (edge-distance) units, mirrored per corner.
        let pts: [(f64, f64); 9] = [
            (1.08849296, 0.),
            (0.86840694, 0.),
            (0.63149399, 0.07491100),
            (0.37282383, 0.16905956),
            (0.16905956, 0.37282383),
            (0.07491100, 0.63149399),
            (0., 0.86840694),
            (0., 1.08849296),
            (0., e / c),
        ];
        let map = |(a, b): (f64, f64)| {
            let (dx, dy) = if rot { (b, a) } else { (a, b) };
            (cx + sx * dx * c, cy + sy * dy * c)
        };
        let p: Vec<(f64, f64)> = pts.iter().map(|&q| map(q)).collect();
        pb.cubic_to(
            f(p[0].0),
            f(p[0].1),
            f(p[1].0),
            f(p[1].1),
            f(p[2].0),
            f(p[2].1),
        );
        pb.cubic_to(
            f(p[3].0),
            f(p[3].1),
            f(p[4].0),
            f(p[4].1),
            f(p[5].0),
            f(p[5].1),
        );
        pb.cubic_to(
            f(p[6].0),
            f(p[6].1),
            f(p[7].0),
            f(p[7].1),
            f(p[8].0),
            f(p[8].1),
        );
    };
    pb.move_to(f(x0 + e), f(y0));
    pb.line_to(f(x1 - e), f(y0));
    corner(&mut pb, x1, y0, -1., 1., false);
    pb.line_to(f(x1), f(y1 - e));
    corner(&mut pb, x1, y1, -1., -1., true);
    pb.line_to(f(x0 + e), f(y1));
    corner(&mut pb, x0, y1, 1., -1., false);
    pb.line_to(f(x0), f(y0 + e));
    corner(&mut pb, x0, y0, 1., 1., true);
    pb.close();
    pb.finish()
}

pub fn rounded_rect(r: Rect, radius: f64) -> Option<Path> {
    let rad = radius.max(0.).min(r.w.min(r.h) / 2.) as f32;
    let (x0, y0, x1, y1) = (r.x as f32, r.y as f32, r.max_x() as f32, r.max_y() as f32);
    if rad <= 0. {
        return PathBuilder::from_rect(r.skia()?).into();
    }
    let k = rad * 0.4477;
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + rad, y0);
    pb.line_to(x1 - rad, y0);
    pb.cubic_to(x1 - k, y0, x1, y0 + k, x1, y0 + rad);
    pb.line_to(x1, y1 - rad);
    pb.cubic_to(x1, y1 - k, x1 - k, y1, x1 - rad, y1);
    pb.line_to(x0 + rad, y1);
    pb.cubic_to(x0 + k, y1, x0, y1 - k, x0, y1 - rad);
    pb.line_to(x0, y0 + rad);
    pb.cubic_to(x0, y0 + k, x0 + k, y0, x0 + rad, y0);
    pb.close();
    pb.finish()
}

fn oval(r: Rect) -> Option<Path> {
    PathBuilder::from_oval(r.skia()?)
}

fn solid(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c.skia());
    p.anti_alias = true;
    p
}

fn gray(v: f32, a: f32) -> Color {
    Color::from_rgba(v, v, v, a).unwrap_or(Color::BLACK)
}

/// Geometry of the final composition in output pixels (scale 1 = export size).
#[derive(Clone, Copy, Debug)]
pub struct CanvasLayout {
    pub canvas_w: f64,
    pub canvas_h: f64,
    /// Where the (cropped) screenshot is drawn on the canvas.
    pub image_rect: Rect,
    /// The part of the source image being shown, in source pixels.
    pub crop: Rect,
    pub inset: f64,
    pub corner: f64,
    pub shadow_blur: f64,
    pub shadow_offset: f64,
    pub shadow_alpha: f64,
    pub window_frame: WindowFrame,
    /// Height of the mockup title/address bar above the image; 0 without a frame.
    pub chrome: f64,
}

impl CanvasLayout {
    pub fn new(style: &Style, crop: Rect) -> Self {
        let big = crop.w.max(crop.h);
        let inset = (style.inset * big).round();
        let corner = style.corner_radius * big;
        let pad = (style.padding * big).round();
        let s = style.shadow;
        let shadow_blur = s * 0.045 * big;
        let shadow_offset = s * 0.012 * big;
        let shadow_alpha = if s > 0. { 0.18 + 0.32 * s } else { 0. };
        let chrome = if style.frame == WindowFrame::None {
            0.
        } else {
            (big * if style.frame.is_browser() {
                0.042
            } else {
                0.032
            })
            .round()
            .max(22.)
        };
        let frame_w = crop.w + inset * 2.;
        let frame_h = crop.h + inset * 2. + chrome;
        let mut w = frame_w + pad * 2.;
        let mut h = frame_h + pad * 2.;
        if let Some(ratio) = style.aspect.value() {
            if w / h < ratio {
                w = h * ratio;
            } else {
                h = w / ratio;
            }
        }
        let (w, h) = (w.round(), h.round());
        // Position moves the frame through all the space around it (padding included), so it
        // works with any ratio. The side it moves toward keeps room for the shadow.
        let margin = pad.min((pad * 0.25).max(shadow_blur)).round();
        let col = (style.alignment.rem_euclid(9) % 3) as f64;
        let row = (style.alignment.rem_euclid(9) / 3) as f64;
        let fx = margin + (w - frame_w - margin * 2.) * col / 2.;
        let fy = margin + (h - frame_h - margin * 2.) * row / 2.;
        Self {
            canvas_w: w,
            canvas_h: h,
            image_rect: Rect::new(
                (fx + inset).round(),
                (fy + inset + chrome).round(),
                crop.w,
                crop.h,
            ),
            crop,
            inset,
            corner,
            shadow_blur,
            shadow_offset,
            shadow_alpha,
            window_frame: style.frame,
            chrome,
        }
    }

    pub fn frame_rect(&self) -> Rect {
        let r = self.image_rect;
        Rect::new(
            r.x - self.inset,
            r.y - self.inset - self.chrome,
            r.w + self.inset * 2.,
            r.h + self.inset * 2. + self.chrome,
        )
    }

    pub fn frame_corner(&self) -> f64 {
        if self.chrome > 0. {
            self.corner.max(self.chrome * 0.3)
        } else {
            self.corner
        }
    }

    pub fn image_corner(&self) -> f64 {
        if self.chrome > 0. {
            0.
        } else if self.inset > 0. {
            self.corner * 0.6
        } else {
            self.corner
        }
    }

    pub fn to_canvas(&self, p: Pt) -> Pt {
        Pt {
            x: self.image_rect.x + p.x - self.crop.x,
            y: self.image_rect.y + p.y - self.crop.y,
        }
    }

    pub fn to_source(&self, p: Pt) -> Pt {
        Pt {
            x: p.x - self.image_rect.x + self.crop.x,
            y: p.y - self.image_rect.y + self.crop.y,
        }
    }
}

/// A captured or loaded image as premultiplied RGBA.
pub struct SourceImage {
    pub pixmap: Pixmap,
}

impl SourceImage {
    pub fn from_bgra(width: u32, height: u32, bgra: &[u8]) -> Option<Self> {
        let mut pm = Pixmap::new(width, height)?;
        for (dst, src) in pm.data_mut().chunks_exact_mut(4).zip(bgra.chunks_exact(4)) {
            let a = src[3] as u32;
            let pre = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            dst[0] = pre(src[2]);
            dst[1] = pre(src[1]);
            dst[2] = pre(src[0]);
            dst[3] = src[3];
        }
        Some(Self { pixmap: pm })
    }

    pub fn from_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Self> {
        let mut pm = Pixmap::new(width, height)?;
        for (dst, src) in pm.data_mut().chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
            let a = src[3] as u32;
            let pre = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            dst[0] = pre(src[0]);
            dst[1] = pre(src[1]);
            dst[2] = pre(src[2]);
            dst[3] = src[3];
        }
        Some(Self { pixmap: pm })
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn full_rect(&self) -> Rect {
        Rect::new(0., 0., self.width() as f64, self.height() as f64)
    }

    pub fn crop(&self, r: Rect) -> Option<Pixmap> {
        let r = r.integral().intersection(&self.full_rect());
        let rect = tiny_skia::IntRect::from_xywh(r.x as i32, r.y as i32, r.w as u32, r.h as u32)?;
        self.pixmap.clone_rect(rect)
    }

    /// (r, g, b, a) straight alpha.
    pub fn px(&self, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let p = self.pixmap.pixel(x, y).map(|p| p.demultiply());
        p.map(|c| (c.red(), c.green(), c.blue(), c.alpha()))
            .unwrap_or((0, 0, 0, 0))
    }
}

/// Unpremultiplied RGBA bytes, for encoders and the clipboard.
pub fn straight_rgba(pm: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pm.data().len());
    for p in pm.pixels() {
        let c = p.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

/// Full-size images used by the pixelate and blur annotations.
pub struct EffectImages {
    pub pixelated: Pixmap,
    pub blurred: Pixmap,
}

impl EffectImages {
    pub fn new(src: &SourceImage) -> Self {
        let pm = &src.pixmap;
        let (w, h) = (pm.width(), pm.height());
        let big = w.max(h) as f64;
        let block = (big / 60.).max(14.);
        let mut pixelated = pm.clone();
        let data = pm.data();
        let out = pixelated.data_mut();
        let (bw, bh) = (
            (w as f64 / block).ceil() as u32,
            (h as f64 / block).ceil() as u32,
        );
        // CIPixellate's grid starts at the bottom-left corner (Core Image is y-up) and each
        // block takes the color at its center.
        for by in 0..bh {
            for bx in 0..bw {
                let x0 = (bx as f64 * block) as u32;
                let x1 = (((bx + 1) as f64 * block) as u32).min(w);
                let y1 = h as i64 - (by as f64 * block) as i64;
                let y0 = (h as i64 - ((by + 1) as f64 * block) as i64).max(0) as u32;
                let y1 = y1.max(0) as u32;
                if x0 >= x1 || y0 >= y1 {
                    continue;
                }
                let cx = (((bx as f64 + 0.5) * block) as u32).min(w - 1);
                let cy = (h as f64 - (by as f64 + 0.5) * block).clamp(0., (h - 1) as f64) as u32;
                let i = ((cy * w + cx) * 4) as usize;
                let c = [data[i], data[i + 1], data[i + 2], data[i + 3]];
                for y in y0..y1 {
                    for x in x0..x1 {
                        let j = ((y * w + x) * 4) as usize;
                        out[j..j + 4].copy_from_slice(&c);
                    }
                }
            }
        }
        let blurred = blur::blur_image(pm, (big / 70.).max(10.) as f32);
        Self { pixelated, blurred }
    }
}

struct ImageCache {
    plain: HashMap<String, Arc<Pixmap>>,
    blurred: HashMap<String, Arc<Pixmap>>,
}

fn image_cache() -> &'static Mutex<ImageCache> {
    static CACHE: OnceLock<Mutex<ImageCache>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Mutex::new(ImageCache {
            plain: HashMap::new(),
            blurred: HashMap::new(),
        })
    })
}

pub fn load_image_file(path: &str) -> Option<Pixmap> {
    let img = crate::platform::decode_image_file(std::path::Path::new(path))?;
    SourceImage::from_rgba(img.width(), img.height(), img.as_raw()).map(|s| s.pixmap)
}

fn background_image(path: &str) -> Option<Arc<Pixmap>> {
    let mut cache = image_cache().lock().ok()?;
    if let Some(p) = cache.plain.get(path) {
        return Some(p.clone());
    }
    let pm = Arc::new(load_image_file(path)?);
    cache.plain.insert(path.to_string(), pm.clone());
    Some(pm)
}

/// Blur radius is a fraction of the image size, so the preview and the export look the same.
/// The image is downscaled first: a blurred background loses nothing and the slider stays fast.
fn blurred_background(path: &str, amount: f64) -> Option<Arc<Pixmap>> {
    let q = (amount.clamp(0., 1.) * 50.).round() / 50.;
    if q <= 0. {
        return background_image(path);
    }
    let key = format!("{path}|{q}");
    if let Some(p) = image_cache().lock().ok()?.blurred.get(&key) {
        return Some(p.clone());
    }
    let src = background_image(path)?;
    let big = src.width().max(src.height()) as f32;
    let down = (1600. / big).min(1.);
    let (w, h) = (
        ((src.width() as f32 * down) as u32).max(1),
        ((src.height() as f32 * down) as u32).max(1),
    );
    let mut small = Pixmap::new(w, h)?;
    small.draw_pixmap(
        0,
        0,
        src.as_ref().as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        },
        Transform::from_scale(down, down),
        None,
    );
    let radius = q as f32 * 0.04 * w.max(h) as f32;
    let out = Arc::new(blur::blur_image(&small, radius));
    let mut cache = image_cache().lock().ok()?;
    if cache.blurred.len() > 8 {
        cache.blurred.clear();
    }
    cache.blurred.insert(key, out.clone());
    Some(out)
}

fn grain_tile() -> &'static Pixmap {
    static TILE: OnceLock<Pixmap> = OnceLock::new();
    TILE.get_or_init(|| {
        let n = 192;
        let mut pm = Pixmap::new(n, n).expect("grain tile");
        let mut rng = Rng::from_time();
        for px in pm.data_mut().chunks_exact_mut(4) {
            let v = (rng.next() & 0xff) as u32;
            let a = 60u32;
            let pv = (v * a / 255) as u8;
            px.copy_from_slice(&[pv, pv, pv, a as u8]);
        }
        pm
    })
}

/// Tiled in canvas pixels, so the preview and the export show the same grain.
fn draw_grain(pm: &mut Pixmap, amount: f64) {
    let tile = grain_tile();
    let paint = Paint {
        shader: Pattern::new(
            tile.as_ref(),
            SpreadMode::Repeat,
            FilterQuality::Nearest,
            amount.min(1.) as f32,
            Transform::identity(),
        ),
        blend_mode: BlendMode::Overlay,
        ..Default::default()
    };
    if let Some(r) = tiny_skia::Rect::from_xywh(0., 0., pm.width() as f32, pm.height() as f32) {
        pm.fill_rect(r, &paint, Transform::identity(), None);
    }
}

fn draw_aspect_fill(pm: &mut Pixmap, img: &Pixmap, rect: Rect, ts: Transform) {
    let (iw, ih) = (img.width() as f64, img.height() as f64);
    let s = (rect.w / iw).max(rect.h / ih);
    let (dw, dh) = (iw * s, ih * s);
    let t = ts
        .pre_translate(
            (rect.mid_x() - dw / 2.) as f32,
            (rect.mid_y() - dh / 2.) as f32,
        )
        .pre_scale(s as f32, s as f32);
    let paint = Paint {
        shader: Pattern::new(img.as_ref(), SpreadMode::Pad, FilterQuality::Bicubic, 1., t),
        ..Default::default()
    };
    if let Some(r) = rect.skia() {
        pm.fill_rect(r, &paint, ts, None);
    }
}

fn stops(colors: &[Rgba]) -> Vec<GradientStop> {
    let n = colors.len().max(2) - 1;
    colors
        .iter()
        .enumerate()
        .map(|(i, c)| GradientStop::new(i as f32 / n as f32, c.skia()))
        .collect()
}

fn draw_background(pm: &mut Pixmap, bg: &Background, rect: Rect, blur: f64, scale: f64) {
    let ts = Transform::from_scale(scale as f32, scale as f32);
    let Some(r) = rect.skia() else { return };
    match bg {
        Background::None {} => {}
        Background::Solid { color } => pm.fill_rect(r, &solid(*color), ts, None),
        Background::Gradient { colors, angle } => {
            if colors.is_empty() {
                return;
            }
            let colors = if colors.len() == 1 {
                vec![colors[0], colors[0]]
            } else {
                colors.clone()
            };
            // CSS-like angle: 90 = left→right, 180 = top→bottom.
            let rad = (angle - 90.).to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
            let half = ((rect.w * dx).abs() + (rect.h * dy).abs()) / 2.;
            let (cx, cy) = (rect.mid_x(), rect.mid_y());
            let shader = LinearGradient::new(
                Point::from_xy((cx - dx * half) as f32, (cy - dy * half) as f32),
                Point::from_xy((cx + dx * half) as f32, (cy + dy * half) as f32),
                stops(&colors),
                SpreadMode::Pad,
                Transform::identity(),
            );
            if let Some(shader) = shader {
                let paint = Paint {
                    shader,
                    anti_alias: true,
                    ..Default::default()
                };
                pm.fill_rect(r, &paint, ts, None);
            }
        }
        Background::Image { path } => {
            let img = if blur > 0. {
                blurred_background(path, blur)
            } else {
                background_image(path)
            };
            match img {
                Some(img) => draw_aspect_fill(pm, &img, rect, ts),
                None => pm.fill_rect(r, &solid(Rgba::hex("#8E8E93")), ts, None),
            }
        }
        Background::Mesh { colors } => draw_mesh(pm, colors, rect, ts),
        Background::Art { art } => {
            if let Some(img) = art::ART_CACHE.image(art, rect.w * scale, rect.h * scale) {
                draw_aspect_fill(pm, &img, rect, ts);
            }
        }
    }
}

/// Legacy mesh look built from soft radial blooms; kept so saved presets still load.
fn draw_mesh(pm: &mut Pixmap, colors: &[Rgba], rect: Rect, ts: Transform) {
    let Some(base) = colors.first() else { return };
    let Some(r) = rect.skia() else { return };
    pm.fill_rect(r, &solid(*base), ts, None);
    let anchors = [(0.12, 0.1, 0.85), (0.92, 0.3, 0.75), (0.4, 1.0, 0.9)];
    let big = rect.w.max(rect.h);
    for (i, c) in colors.iter().skip(1).take(3).enumerate() {
        let (ax, ay, ar) = anchors[i];
        let center = Point::from_xy((rect.x + rect.w * ax) as f32, (rect.y + rect.h * ay) as f32);
        let shader = RadialGradient::new(
            center,
            center,
            (big * ar) as f32,
            vec![
                GradientStop::new(0., c.with_alpha(0.95).skia()),
                GradientStop::new(0.45, c.with_alpha(0.45).skia()),
                GradientStop::new(1., c.with_alpha(0.).skia()),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        );
        if let Some(shader) = shader {
            let paint = Paint {
                shader,
                ..Default::default()
            };
            pm.fill_rect(r, &paint, ts, None);
        }
    }
}

/// An offscreen layer covering part of the canvas, in device pixels.
struct Layer {
    pm: Pixmap,
    x: i32,
    y: i32,
}

impl Layer {
    fn new(bounds: Rect, canvas: &Pixmap) -> Option<Self> {
        let full = Rect::new(0., 0., canvas.width() as f64, canvas.height() as f64);
        let b = bounds.integral().intersection(&full);
        if b.is_empty() {
            return None;
        }
        Some(Self {
            pm: Pixmap::new(b.w as u32, b.h as u32)?,
            x: b.x as i32,
            y: b.y as i32,
        })
    }

    /// Prepends the layer offset so drawing code can keep using canvas coordinates.
    fn ts(&self, ts: Transform) -> Transform {
        ts.post_translate(-self.x as f32, -self.y as f32)
    }

    fn composite(&self, target: &mut Pixmap) {
        target.draw_pixmap(
            self.x,
            self.y,
            self.pm.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    fn shadow(&self, target: &mut Pixmap, s: Shadow) {
        blur::draw_shadow(
            target,
            &self.pm,
            (self.x, self.y),
            s.blur / 2.,
            (0., s.dy),
            gray(0., s.alpha),
        );
    }
}

/// CGContext.setShadow equivalent in device pixels: `blur` is CG's blur value.
#[derive(Clone, Copy)]
struct Shadow {
    blur: f32,
    dy: f32,
    alpha: f32,
}

pub struct RenderInput<'a> {
    pub source: &'a SourceImage,
    pub style: &'a Style,
    pub crop: Rect,
    pub edge_color: Rgba,
    pub annotations: &'a [Annotation],
    pub effects: Option<&'a EffectImages>,
    pub hidden: Option<uuid::Uuid>,
    pub scale: f64,
}

/// Renders the full composition. `scale` < 1 is used for fast previews.
pub fn render(input: &RenderInput) -> Option<Pixmap> {
    let layout = CanvasLayout::new(input.style, input.crop);
    let s = input.scale;
    let w = ((layout.canvas_w * s).round() as u32).max(1);
    let h = ((layout.canvas_h * s).round() as u32).max(1);
    let mut pm = Pixmap::new(w, h)?;
    draw_composition(&mut pm, input, &layout);
    if !input.annotations.is_empty() {
        let ts = Transform::from_scale(s as f32, s as f32).pre_translate(
            (layout.image_rect.x - layout.crop.x) as f32,
            (layout.image_rect.y - layout.crop.y) as f32,
        );
        draw_annotations(
            &mut pm,
            input.annotations,
            input.effects,
            &layout,
            ts,
            s,
            input.hidden,
        );
    }
    Some(pm)
}

fn draw_composition(pm: &mut Pixmap, input: &RenderInput, layout: &CanvasLayout) {
    let style = input.style;
    let s = input.scale;
    let ts = Transform::from_scale(s as f32, s as f32);
    let canvas = Rect::new(0., 0., layout.canvas_w, layout.canvas_h);
    draw_background(pm, &style.background, canvas, style.background_blur, s);
    if style.grain > 0. && !style.background.is_none() {
        draw_grain(pm, style.grain);
    } else if style.background.is_smooth() {
        draw_grain(pm, 0.05);
    }

    // Two shadows, like real depth: a wide ambient one and a short contact one under the edge.
    // Nested layers draw the content once, so translucent pixels (window corners) don't darken
    // from being composited twice.
    let a = layout.shadow_alpha as f32;
    let k = s as f32;
    let frame = layout.frame_rect();
    let device = |r: Rect| Rect::new(r.x * s, r.y * s, r.w * s, r.h * s);
    let Some(mut inner) = Layer::new(device(frame).inset(-2., -2.), pm) else {
        return;
    };
    draw_frame_content(&mut inner, input, layout, ts);

    let contact = (layout.shadow_blur * 0.14).max(1.5) as f32;
    let ambient_blur = layout.shadow_blur as f32 * 1.25 * k;
    let ambient_dy = layout.shadow_offset as f32 * 1.6 * k;
    let outer_pad = if a > 0. {
        (ambient_blur * 1.5 + ambient_dy) as f64
    } else {
        0.
    };
    let Some(mut mid) = Layer::new(device(frame).inset(-outer_pad - 4., -outer_pad - 4.), pm)
    else {
        return;
    };
    if a > 0. {
        blur::draw_shadow(
            &mut mid.pm,
            &inner.pm,
            (inner.x - mid.x, inner.y - mid.y),
            contact * k / 2.,
            (0., contact * 0.35 * k),
            gray(0., (a * 0.55).min(0.5)),
        );
    }
    mid.pm.draw_pixmap(
        inner.x - mid.x,
        inner.y - mid.y,
        inner.pm.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    if a > 0. {
        mid.shadow(
            pm,
            Shadow {
                blur: ambient_blur,
                dy: ambient_dy,
                alpha: a * 0.72,
            },
        );
    }
    mid.composite(pm);

    if !style.watermark.trim().is_empty() {
        draw_watermark(pm, &style.watermark, layout, s);
    }
}

fn draw_frame_content(
    layer: &mut Layer,
    input: &RenderInput,
    layout: &CanvasLayout,
    ts: Transform,
) {
    let lts = layer.ts(ts);
    let edge = input.edge_color;
    let frame = layout.frame_rect();
    if layout.chrome > 0. {
        draw_window_frame(&mut layer.pm, layout, edge, lts);
    } else if layout.inset > 0.
        && let Some(p) = squircle(frame, layout.corner)
    {
        layer
            .pm
            .fill_path(&p, &solid(edge), FillRule::Winding, lts, None);
    }

    let clip = if layout.chrome > 0. {
        squircle(frame, layout.frame_corner()).and_then(|p| {
            let mut m = Mask::new(layer.pm.width(), layer.pm.height())?;
            m.fill_path(&p, FillRule::Winding, true, lts);
            Some(m)
        })
    } else {
        None
    };
    let ir = layout.image_rect;
    let r = layout.image_corner().min(ir.w.min(ir.h) / 2.);
    if let (Some(path), Some(cropped)) = (squircle(ir, r), input.source.crop(layout.crop)) {
        let quality = if (input.scale - 1.).abs() < 1e-6 {
            FilterQuality::Nearest
        } else {
            FilterQuality::Bicubic
        };
        let paint = Paint {
            shader: Pattern::new(
                cropped.as_ref(),
                SpreadMode::Pad,
                quality,
                1.,
                Transform::from_translate(ir.x as f32, ir.y as f32),
            ),
            anti_alias: true,
            ..Default::default()
        };
        layer
            .pm
            .fill_path(&path, &paint, FillRule::Winding, lts, clip.as_ref());
    }

    // A window capture brings its own rounded, transparent corners; an edge drawn on our clip
    // would show as a ghost arc outside them.
    let own_corners = layout.chrome == 0.
        && layout.inset == 0.
        && has_transparent_corner(input.source, layout.crop);
    let style = input.style;
    if style.hairline
        && !own_corners
        && (!style.background.is_none() || layout.chrome > 0. || layout.inset > 0.)
    {
        draw_hairline(&mut layer.pm, layout, edge, lts);
    }
}

pub fn has_transparent_corner(src: &SourceImage, crop: Rect) -> bool {
    let (x, y) = (crop.x.round().max(0.) as u32, crop.y.round().max(0.) as u32);
    if x >= src.width() || y >= src.height() {
        return false;
    }
    src.px(x, y).3 < 250
}

/// A half-point inner edge keeps the screenshot separate from a background of a similar color.
fn draw_hairline(pm: &mut Pixmap, layout: &CanvasLayout, edge: Rgba, ts: Transform) {
    let outer = layout.chrome > 0. || layout.inset > 0.;
    let rect = if outer {
        layout.frame_rect()
    } else {
        layout.image_rect
    };
    let radius = if outer {
        if layout.chrome > 0. {
            layout.frame_corner()
        } else {
            layout.corner
        }
    } else {
        layout.image_corner().min(rect.w.min(rect.h) / 2.)
    };
    let dark = if layout.chrome > 0. {
        layout.window_frame.is_dark()
    } else {
        edge.luminance() < 0.5
    };
    let w = (rect.w.max(rect.h) / 1400.).round().max(1.);
    let Some(path) = squircle(rect, radius) else {
        return;
    };
    let Some(mut mask) = Mask::new(pm.width(), pm.height()) else {
        return;
    };
    mask.fill_path(&path, FillRule::Winding, true, ts);
    let mut paint = Paint::default();
    paint.set_color(if dark { gray(1., 0.14) } else { gray(0., 0.1) });
    paint.anti_alias = true;
    let stroke = Stroke {
        width: (w * 2.) as f32,
        ..Default::default()
    };
    pm.stroke_path(&path, &paint, &stroke, ts, Some(&mask));
}

/// Mockup window: title bar with traffic lights (and an address field for the browser look).
fn draw_window_frame(pm: &mut Pixmap, layout: &CanvasLayout, edge: Rgba, ts: Transform) {
    let f = layout.frame_rect();
    let h = layout.chrome;
    let dark = layout.window_frame.is_dark();
    let Some(frame_path) = squircle(f, layout.frame_corner()) else {
        return;
    };
    let Some(mut clip) = Mask::new(pm.width(), pm.height()) else {
        return;
    };
    clip.fill_path(&frame_path, FillRule::Winding, true, ts);
    let clip = Some(&clip);
    let fill = |pm: &mut Pixmap, path: &Path, c: Rgba| {
        pm.fill_path(path, &solid(c), FillRule::Winding, ts, clip);
    };
    let body = if layout.inset > 0. {
        edge
    } else {
        Rgba::hex(if dark { "#1E1E20" } else { "#FFFFFF" })
    };
    fill(pm, &frame_path, body);

    let bar = Rect::new(f.x, f.y, f.w, h);
    let top = Rgba::hex(if dark { "#3A3A3D" } else { "#F6F6F6" });
    let bottom = Rgba::hex(if dark { "#2C2C2F" } else { "#E8E8E8" });
    if let (Some(shader), Some(r)) = (
        LinearGradient::new(
            Point::from_xy(0., bar.y as f32),
            Point::from_xy(0., bar.max_y() as f32),
            vec![
                GradientStop::new(0., top.skia()),
                GradientStop::new(1., bottom.skia()),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ),
        bar.skia(),
    ) {
        let paint = Paint {
            shader,
            anti_alias: true,
            ..Default::default()
        };
        pm.fill_rect(r, &paint, ts, clip);
    }
    let line = (h / 36.).max(1.);
    if let Some(r) = Rect::new(bar.x, bar.max_y() - line, bar.w, line).skia() {
        let c = if dark {
            Rgba::hex_alpha("#000000", 0.6)
        } else {
            Rgba::hex("#C8C8C8")
        };
        fill(pm, &PathBuilder::from_rect(r), c);
    }

    let d = h * 0.34;
    let gap = d * 0.62;
    let mut x = f.x + h * 0.42;
    for hex in ["#FF5F57", "#FEBC2E", "#28C840"] {
        let c = Rect::new(x, bar.mid_y() - d / 2., d, d);
        if let Some(p) = oval(c) {
            fill(pm, &p, Rgba::hex(hex));
        }
        if let Some(p) = oval(c.inset(d * 0.025, d * 0.025)) {
            let mut paint = Paint::default();
            paint.set_color(gray(0., 0.12));
            paint.anti_alias = true;
            let stroke = Stroke {
                width: (d * 0.05).max(0.5) as f32,
                ..Default::default()
            };
            pm.stroke_path(&p, &paint, &stroke, ts, clip);
        }
        x += d + gap;
    }

    if layout.window_frame.is_browser() {
        let w = (f.w * 0.46).min((f.w * 0.3).max(h * 9.));
        let field = Rect::new(f.mid_x() - w / 2., bar.mid_y() - h * 0.29, w, h * 0.58);
        if let Some(p) = squircle(field, field.h * 0.3) {
            let c = if dark {
                Rgba::hex_alpha("#1C1C1E", 0.9)
            } else {
                Rgba::hex_alpha("#FFFFFF", 0.95)
            };
            fill(pm, &p, c);
        }
        let dot = field.h * 0.22;
        if let Some(r) = Rect::new(
            field.mid_x() - w * 0.18,
            field.mid_y() - dot / 2.,
            w * 0.36,
            dot,
        )
        .skia()
        {
            let c = Rgba::hex(if dark { "#8E8E93" } else { "#AEAEB2" });
            pm.fill_rect(r, &solid(c), ts, clip);
        }
    }
}

fn draw_watermark(pm: &mut Pixmap, text: &str, layout: &CanvasLayout, s: f64) {
    let size = (layout.canvas_w.min(layout.canvas_h) * 0.022).max(11.);
    let (tw, th) = text::measure(text, size, text::SEMIBOLD);
    let margin = size * 1.1;
    let x = layout.canvas_w - tw - margin;
    let y = layout.canvas_h - th - margin * 0.7;
    let ts = Transform::from_scale(s as f32, s as f32);
    let Some(path) = text::path_at(text, size, text::SEMIBOLD, x, y) else {
        return;
    };
    let pad = size * 2.;
    let bounds = Rect::new(
        (x - pad) * s,
        (y - pad) * s,
        (tw + pad * 2.) * s,
        (th + pad * 2.) * s,
    );
    let Some(mut layer) = Layer::new(bounds, pm) else {
        return;
    };
    let lts = layer.ts(ts);
    layer.pm.fill_path(
        &path,
        &solid(Rgba::new(1., 1., 1., 0.85)),
        FillRule::Winding,
        lts,
        None,
    );
    layer.shadow(
        pm,
        Shadow {
            blur: (size * 0.5 * s) as f32,
            dy: 0.,
            alpha: 0.35,
        },
    );
    layer.composite(pm);
}

/// Draws annotations in source-pixel coordinates; `ts` maps source pixels to the target.
pub fn draw_annotations(
    pm: &mut Pixmap,
    list: &[Annotation],
    effects: Option<&EffectImages>,
    layout: &CanvasLayout,
    ts: Transform,
    k: f64,
    hidden: Option<uuid::Uuid>,
) {
    let labels = counter_labels(list);
    for a in list.iter().filter(|a| Some(a.id) != hidden) {
        let label = labels.get(&a.id).map(String::as_str).unwrap_or("");
        draw_annotation(pm, a, label, effects, layout, ts, k);
    }
}

fn map_rect(ts: Transform, r: Rect) -> Rect {
    let mut pts = [
        Point::from_xy(r.x as f32, r.y as f32),
        Point::from_xy(r.max_x() as f32, r.max_y() as f32),
    ];
    ts.map_points(&mut pts);
    Rect::from_points(
        Pt {
            x: pts[0].x as f64,
            y: pts[0].y as f64,
        },
        Pt {
            x: pts[1].x as f64,
            y: pts[1].y as f64,
        },
    )
}

fn soft_shadow(width: f64, k: f64) -> Shadow {
    Shadow {
        blur: (width * 1.5 * k) as f32,
        dy: (width * 0.4 * k) as f32,
        alpha: 0.3,
    }
}

/// Draws into a layer around `bounds` (source pixels) and composites it with its shadow.
fn shadowed(
    pm: &mut Pixmap,
    ts: Transform,
    bounds: Rect,
    shadow: Shadow,
    draw: impl FnOnce(&mut Pixmap, Transform),
) {
    let pad = (shadow.blur * 1.6 + shadow.dy) as f64 + 2.;
    let Some(mut layer) = Layer::new(map_rect(ts, bounds).inset(-pad, -pad), pm) else {
        return;
    };
    let lts = layer.ts(ts);
    draw(&mut layer.pm, lts);
    layer.shadow(pm, shadow);
    layer.composite(pm);
}

fn draw_annotation(
    pm: &mut Pixmap,
    a: &Annotation,
    label: &str,
    effects: Option<&EffectImages>,
    layout: &CanvasLayout,
    ts: Transform,
    k: f64,
) {
    let stroke = |w: f64| Stroke {
        width: w as f32,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let paint = solid(a.color);
    match a.kind {
        AnnotationKind::Pixelate | AnnotationKind::Blur => {
            let Some(fx) = effects else { return };
            let img = if a.kind == AnnotationKind::Pixelate {
                &fx.pixelated
            } else {
                &fx.blurred
            };
            let Some(mut mask) = Mask::new(pm.width(), pm.height()) else {
                return;
            };
            let (Some(clip), Some(r)) = (
                squircle(layout.crop, layout.image_corner()),
                a.rect().skia().map(PathBuilder::from_rect),
            ) else {
                return;
            };
            mask.fill_path(&clip, FillRule::Winding, true, ts);
            mask.intersect_path(&r, FillRule::Winding, false, ts);
            let paint = Paint {
                shader: Pattern::new(
                    img.as_ref(),
                    SpreadMode::Pad,
                    FilterQuality::Bilinear,
                    1.,
                    Transform::identity(),
                ),
                ..Default::default()
            };
            pm.fill_path(&r, &paint, FillRule::Winding, ts, Some(&mask));
        }
        AnnotationKind::Highlight => {
            let r = a.rect();
            if let Some(p) = rounded_rect(r, 4f64.min(r.w / 2.).min(r.h / 2.)) {
                pm.fill_path(
                    &p,
                    &solid(a.color.with_alpha(0.38)),
                    FillRule::Winding,
                    ts,
                    None,
                );
            }
        }
        AnnotationKind::Rect => {
            let r = a.rect();
            let cr = (a.width * 1.5).min(r.w.min(r.h) / 2.);
            if let Some(p) = rounded_rect(r, cr) {
                shadowed(
                    pm,
                    ts,
                    r.inset(-a.width, -a.width),
                    soft_shadow(a.width, k),
                    |pm, ts| {
                        pm.stroke_path(&p, &paint, &stroke(a.width), ts, None);
                    },
                );
            }
        }
        AnnotationKind::Ellipse => {
            let r = a.rect();
            if let Some(p) = oval(r) {
                shadowed(
                    pm,
                    ts,
                    r.inset(-a.width, -a.width),
                    soft_shadow(a.width, k),
                    |pm, ts| {
                        pm.stroke_path(&p, &paint, &stroke(a.width), ts, None);
                    },
                );
            }
        }
        AnnotationKind::Arrow => {
            shadowed(pm, ts, a.bounds(), soft_shadow(a.width, k), |pm, ts| {
                draw_arrow(pm, a.p1, a.p2, a.width, a.color, ts);
            });
        }
        AnnotationKind::Counter => {
            let r = a.counter_radius();
            let circle = Rect::new(a.p1.x - r, a.p1.y - r, r * 2., r * 2.);
            if let Some(p) = oval(circle) {
                shadowed(pm, ts, circle, soft_shadow(a.width, k), |pm, ts| {
                    pm.fill_path(&p, &paint, FillRule::Winding, ts, None);
                });
            }
            if let Some(p) = oval(circle.inset(r * 0.06, r * 0.06)) {
                let mut white = Paint::default();
                white.set_color(gray(1., 0.9));
                white.anti_alias = true;
                pm.stroke_path(&p, &white, &stroke((r * 0.12).max(1.5)), ts, None);
            }
            let dark_text = a.color.r + a.color.g + a.color.b > 2.4;
            let mut size = r * 1.15;
            let (w, _) = text::measure(label, size, text::HEAVY);
            if w > r * 1.5 {
                size = size * r * 1.5 / w;
            }
            let (tw, th) = text::measure(label, size, text::HEAVY);
            if let Some(p) = text::path_at(
                label,
                size,
                text::HEAVY,
                circle.mid_x() - tw / 2.,
                circle.mid_y() - th / 2.,
            ) {
                let c = if dark_text { Rgba::BLACK } else { Rgba::WHITE };
                pm.fill_path(&p, &solid(c), FillRule::Winding, ts, None);
            }
        }
        AnnotationKind::Text => {
            let blur = a.font_size * 0.25;
            if let Some(p) = text::path_at(&a.text, a.font_size, text::BOLD, a.p1.x, a.p1.y) {
                let shadow = Shadow {
                    blur: (blur * k) as f32,
                    dy: (blur * 0.2 * k) as f32,
                    alpha: 0.35,
                };
                shadowed(pm, ts, a.bounds(), shadow, |pm, ts| {
                    pm.fill_path(&p, &paint, FillRule::Winding, ts, None);
                });
            }
        }
    }
}

pub fn draw_arrow(pm: &mut Pixmap, p1: Pt, p2: Pt, width: f64, color: Rgba, ts: Transform) {
    let (dx, dy) = (p2.x - p1.x, p2.y - p1.y);
    let len = dx.hypot(dy);
    if len <= 0.5 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let head_len = (width * 4.5).max(14.).min(len * 0.9);
    let head_half = head_len * 0.55;
    let base = Pt {
        x: p2.x - ux * head_len,
        y: p2.y - uy * head_len,
    };
    // Tapered shaft: thin at the tail, full width at the head.
    let (tail_half, head_shaft_half) = (width * 0.25, width * 0.5);
    let (nx, ny) = (-uy, ux);
    let shaft_end = Pt {
        x: base.x + ux * head_len * 0.3,
        y: base.y + uy * head_len * 0.3,
    };
    let f = |v: f64| v as f32;
    let paint = solid(color);

    // Each piece is filled on its own: in a single path, opposite windings cancel out where they
    // overlap and leave a hole where the shaft meets the head.
    let mut pb = PathBuilder::new();
    pb.move_to(f(p1.x + nx * tail_half), f(p1.y + ny * tail_half));
    pb.line_to(
        f(shaft_end.x + nx * head_shaft_half),
        f(shaft_end.y + ny * head_shaft_half),
    );
    pb.line_to(
        f(shaft_end.x - nx * head_shaft_half),
        f(shaft_end.y - ny * head_shaft_half),
    );
    pb.line_to(f(p1.x - nx * tail_half), f(p1.y - ny * tail_half));
    pb.close();
    if let Some(p) = pb.finish() {
        pm.fill_path(&p, &paint, FillRule::Winding, ts, None);
    }
    if let Some(p) = oval(Rect::new(
        p1.x - tail_half,
        p1.y - tail_half,
        tail_half * 2.,
        tail_half * 2.,
    )) {
        pm.fill_path(&p, &paint, FillRule::Winding, ts, None);
    }
    let mut pb = PathBuilder::new();
    pb.move_to(f(p2.x), f(p2.y));
    pb.line_to(f(base.x + nx * head_half), f(base.y + ny * head_half));
    pb.line_to(f(base.x - nx * head_half), f(base.y - ny * head_half));
    pb.close();
    if let Some(p) = pb.finish() {
        pm.fill_path(&p, &paint, FillRule::Winding, ts, None);
        // Rounds the head's corners slightly.
        let stroke = Stroke {
            width: (width * 0.35).max(1.) as f32,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&p, &paint, &stroke, ts, None);
    }
}
