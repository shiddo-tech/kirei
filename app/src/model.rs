use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::l10n::l;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    #[serde(default = "one")]
    pub a: f64,
}

fn one() -> f64 {
    1.
}

impl Rgba {
    pub const WHITE: Rgba = Rgba::new(1., 1., 1., 1.);
    pub const BLACK: Rgba = Rgba::new(0., 0., 0., 1.);

    pub const fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    pub fn hex(hex: &str) -> Self {
        Self::hex_alpha(hex, 1.)
    }

    pub fn hex_alpha(hex: &str, a: f64) -> Self {
        let v = u32::from_str_radix(hex.trim().trim_start_matches('#'), 16).unwrap_or(0);
        Self {
            r: ((v >> 16) & 0xff) as f64 / 255.,
            g: ((v >> 8) & 0xff) as f64 / 255.,
            b: (v & 0xff) as f64 / 255.,
            a,
        }
    }

    pub fn with_alpha(self, a: f64) -> Self {
        Self { a, ..self }
    }

    pub fn to_hex(self) -> String {
        let q = |c: f64| (c.clamp(0., 1.) * 255.).round() as u8;
        format!("#{:02X}{:02X}{:02X}", q(self.r), q(self.g), q(self.b))
    }

    pub fn luminance(self) -> f64 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }

    pub fn skia(self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba(
            self.r.clamp(0., 1.) as f32,
            self.g.clamp(0., 1.) as f32,
            self.b.clamp(0., 1.) as f32,
            self.a.clamp(0., 1.) as f32,
        )
        .unwrap_or(tiny_skia::Color::BLACK)
    }

    pub fn gpui(self) -> gpui::Rgba {
        gpui::Rgba {
            r: self.r as f32,
            g: self.g as f32,
            b: self.b as f32,
            a: self.a as f32,
        }
    }
}

/// Mirrors Swift's synthesized Codable for enums with associated values, so presets and
/// style files are interchangeable with the Swift app.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Background {
    None {},
    Solid {
        #[serde(rename = "_0")]
        color: Rgba,
    },
    Gradient {
        #[serde(rename = "_0")]
        colors: Vec<Rgba>,
        angle: f64,
    },
    Image {
        path: String,
    },
    Mesh {
        #[serde(rename = "_0")]
        colors: Vec<Rgba>,
    },
    Art {
        #[serde(rename = "_0")]
        art: Art,
    },
}

impl Background {
    pub fn solid(color: Rgba) -> Self {
        Self::Solid { color }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None {})
    }

    /// Smooth color ramps band in 8-bit exports unless a little noise dithers them.
    pub fn is_smooth(&self) -> bool {
        matches!(
            self,
            Self::Gradient { .. } | Self::Mesh { .. } | Self::Art { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtKind {
    Mesh,
    Linear,
    Radial,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Art {
    pub kind: ArtKind,
    pub palette: Vec<Rgba>,
    pub seed: i64,
}

impl Art {
    fn new(kind: ArtKind, hexes: &[&str], seed: i64) -> Self {
        Self {
            kind,
            palette: hexes.iter().map(|h| Rgba::hex(h)).collect(),
            seed,
        }
    }

    pub fn reseeded(&self) -> Self {
        let mut a = self.clone();
        let mut rng = crate::render::Rng::from_time();
        while a.seed == self.seed {
            a.seed = 1 + (rng.next() % 9999) as i64;
        }
        a
    }

    pub fn is_variant_of(&self, other: &Art) -> bool {
        self.kind == other.kind && self.palette == other.palette
    }

    pub fn presets() -> Vec<Art> {
        use ArtKind::*;
        vec![
            Art::new(
                Mesh,
                &["#1A0B2E", "#FF6B8B", "#FFB36B", "#7C4DFF", "#FFD9C2"],
                3,
            ),
            Art::new(
                Mesh,
                &["#04122E", "#1F5BFF", "#00D1C1", "#5B3BFF", "#7FE7FF"],
                8,
            ),
            Art::new(
                Mesh,
                &["#E9E4FF", "#B9A6FF", "#FFB8D9", "#9CC8FF", "#FFE6D1"],
                5,
            ),
            Art::new(
                Mesh,
                &["#07060F", "#4B1FFF", "#E13CFF", "#0E5BFF", "#FF4D8D"],
                12,
            ),
            Art::new(
                Mesh,
                &["#062820", "#1DB98A", "#3AD1FF", "#C4F27A", "#0E7C6B"],
                4,
            ),
            Art::new(
                Mesh,
                &["#120606", "#FF4A1C", "#FF9A3C", "#B3123A", "#5A1150"],
                9,
            ),
            Art::new(
                Mesh,
                &["#1B1D24", "#4A4F63", "#8A90A8", "#D9D4C7", "#2E3242"],
                6,
            ),
            Art::new(
                Mesh,
                &["#DCEBFF", "#8FD3FF", "#B9A4FF", "#FFC0E0", "#FFF1C9"],
                2,
            ),
            Art::new(
                Mesh,
                &["#FFE9EC", "#FF9EB5", "#FFC9A8", "#E58CFF", "#FFF4E8"],
                7,
            ),
            Art::new(
                Mesh,
                &["#0B0A12", "#2A2160", "#C88A3B", "#6A3BB5", "#F2C46D"],
                11,
            ),
            Art::new(
                Mesh,
                &["#EFFFF7", "#7FE8C4", "#7CC8FF", "#D7F7A6", "#B6F0E6"],
                14,
            ),
            Art::new(
                Mesh,
                &["#F4EDE2", "#E2C8A8", "#C9A38A", "#F7D9C4", "#B8C2D6"],
                10,
            ),
            Art::new(Linear, &["#2B1B5A", "#A43E8C", "#FF7A6B", "#FFC38A"], 3),
            Art::new(Linear, &["#050816", "#102A6B", "#2F6BFF", "#8FD0FF"], 5),
            Art::new(Linear, &["#FFE3D1", "#FFB5A0", "#F08BA8"], 2),
            Art::new(Linear, &["#04161A", "#0E5C55", "#3FD1A0", "#D6FF9E"], 8),
            Art::new(Linear, &["#F3EEFF", "#C8B6FF", "#8F7BFF"], 4),
            Art::new(Linear, &["#0D0E12", "#23252E", "#4A4E5C"], 6),
            Art::new(Linear, &["#FFF6D6", "#FFD36E", "#FF8A4C"], 9),
            Art::new(Linear, &["#F2F8FF", "#BFD9FF", "#7FA6E8", "#3A5BA8"], 1),
            Art::new(Linear, &["#12040B", "#4A0E2C", "#A8325E", "#F08A7E"], 7),
            Art::new(Radial, &["#C9B8FF", "#6B4BFF", "#22145C", "#07060F"], 3),
            Art::new(Radial, &["#FFE2B0", "#FF9A4A", "#7A2A1A", "#120806"], 6),
            Art::new(Radial, &["#CDEBFF", "#3E8BFF", "#0C2766", "#03081A"], 8),
            Art::new(Radial, &["#FFFFFF", "#EEF0F6", "#CDD3E1"], 2),
            Art::new(Radial, &["#CFFFE6", "#2BD49A", "#0B4A3A", "#021512"], 5),
            Art::new(Radial, &["#FFE0EC", "#FF7FAE", "#7A1E5B", "#1A0614"], 9),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowFrame {
    #[default]
    None,
    MacLight,
    MacDark,
    BrowserLight,
    BrowserDark,
}

impl WindowFrame {
    pub const ALL: [WindowFrame; 5] = [
        Self::None,
        Self::MacLight,
        Self::MacDark,
        Self::BrowserLight,
        Self::BrowserDark,
    ];

    pub fn title(self) -> String {
        match self {
            Self::None => l("None"),
            Self::MacLight => l("macOS · Light"),
            Self::MacDark => l("macOS · Dark"),
            Self::BrowserLight => l("Browser · Light"),
            Self::BrowserDark => l("Browser · Dark"),
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, Self::MacDark | Self::BrowserDark)
    }

    pub fn is_browser(self) -> bool {
        matches!(self, Self::BrowserLight | Self::BrowserDark)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AspectRatio {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "square")]
    Square,
    #[serde(rename = "r4x3")]
    R4x3,
    #[serde(rename = "r3x2")]
    R3x2,
    #[serde(rename = "r16x9")]
    R16x9,
    #[serde(rename = "r16x10")]
    R16x10,
    #[serde(rename = "r21x9")]
    R21x9,
    #[serde(rename = "r9x16")]
    R9x16,
    #[serde(rename = "r4x5")]
    R4x5,
}

impl AspectRatio {
    pub const ALL: [AspectRatio; 9] = [
        Self::Auto,
        Self::Square,
        Self::R4x3,
        Self::R3x2,
        Self::R16x9,
        Self::R16x10,
        Self::R21x9,
        Self::R9x16,
        Self::R4x5,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Auto => l("Automatic"),
            Self::Square => l("1:1 · Square"),
            Self::R4x3 => "4:3".into(),
            Self::R3x2 => "3:2".into(),
            Self::R16x9 => "16:9 · X / YouTube".into(),
            Self::R16x10 => "16:10".into(),
            Self::R21x9 => "21:9 · Ultrawide".into(),
            Self::R9x16 => l("9:16 · Stories"),
            Self::R4x5 => "4:5 · Instagram".into(),
        }
    }

    pub fn value(self) -> Option<f64> {
        Some(match self {
            Self::Auto => return None,
            Self::Square => 1.,
            Self::R4x3 => 4. / 3.,
            Self::R3x2 => 3. / 2.,
            Self::R16x9 => 16. / 9.,
            Self::R16x10 => 16. / 10.,
            Self::R21x9 => 21. / 9.,
            Self::R9x16 => 9. / 16.,
            Self::R4x5 => 4. / 5.,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Style {
    pub background: Background,
    /// Fractions of the screenshot's larger side.
    pub padding: f64,
    pub inset: f64,
    pub corner_radius: f64,
    pub shadow: f64,
    pub aspect: AspectRatio,
    /// Row-major 3×3 grid; 4 is the center.
    pub alignment: i64,
    pub auto_balance: bool,
    pub frame: WindowFrame,
    pub grain: f64,
    pub watermark: String,
    pub background_blur: f64,
    pub hairline: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: Background::solid(Rgba::hex("#F2F2F7")),
            padding: 0.08,
            inset: 0.,
            corner_radius: 0.012,
            shadow: 0.5,
            aspect: AspectRatio::Auto,
            alignment: 4,
            auto_balance: true,
            frame: WindowFrame::None,
            grain: 0.,
            watermark: String::new(),
            background_blur: 0.,
            hairline: true,
        }
    }
}

pub fn gradient_presets() -> Vec<Vec<Rgba>> {
    [
        &["#FF9A8B", "#FF6A88", "#FF99AC"][..],
        &["#4158D0", "#C850C0", "#FFCC70"],
        &["#0093E9", "#80D0C7"],
        &["#8EC5FC", "#E0C3FC"],
        &["#C9D6FF", "#E2E2E2"],
        &["#FBAB7E", "#F7CE68"],
        &["#F6D5F7", "#FBE9D7"],
        &["#21D4FD", "#B721FF"],
        &["#08AEEA", "#2AF598"],
        &["#FEE140", "#FA709A"],
        &["#134E5E", "#71B280"],
        &["#FF3CAC", "#784BA0", "#2B86C5"],
        &["#667EEA", "#764BA2"],
        &["#F5F7FA", "#C3CFE2"],
        &["#0F2027", "#203A43", "#2C5364"],
        &["#232526", "#414345"],
    ]
    .iter()
    .map(|g| g.iter().map(|h| Rgba::hex(h)).collect())
    .collect()
}

pub fn solid_presets() -> Vec<Rgba> {
    [
        "#FFFFFF", "#F2F2F7", "#D1D1D6", "#1C1C1E", "#000000", "#FF3B30", "#FF9500", "#FFCC00",
        "#34C759", "#00C7BE", "#007AFF", "#5856D6", "#AF52DE", "#FF2D55",
    ]
    .iter()
    .map(|h| Rgba::hex(h))
    .collect()
}

pub fn annotation_palette() -> Vec<Rgba> {
    [
        "#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#007AFF", "#AF52DE", "#FFFFFF", "#000000",
    ]
    .iter()
    .map(|h| Rgba::hex(h))
    .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Arrow,
    Rect,
    Ellipse,
    Highlight,
    Text,
    Counter,
    Pixelate,
    Blur,
    Crop,
}

impl Tool {
    pub const ALL: [Tool; 10] = [
        Self::Select,
        Self::Arrow,
        Self::Rect,
        Self::Ellipse,
        Self::Highlight,
        Self::Text,
        Self::Counter,
        Self::Pixelate,
        Self::Blur,
        Self::Crop,
    ];

    pub fn title(self) -> String {
        l(match self {
            Self::Select => "Select (V)",
            Self::Arrow => "Arrow (A)",
            Self::Rect => "Rectangle (R)",
            Self::Ellipse => "Ellipse (O)",
            Self::Highlight => "Highlighter (H)",
            Self::Text => "Text (T)",
            Self::Counter => "Counter (N)",
            Self::Pixelate => "Pixelate (P)",
            Self::Blur => "Blur (B)",
            Self::Crop => "Crop (C)",
        })
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Select => "v",
            Self::Arrow => "a",
            Self::Rect => "r",
            Self::Ellipse => "o",
            Self::Highlight => "h",
            Self::Text => "t",
            Self::Counter => "n",
            Self::Pixelate => "p",
            Self::Blur => "b",
            Self::Crop => "c",
        }
    }

    pub fn annotation_kind(self) -> Option<AnnotationKind> {
        Some(match self {
            Self::Arrow => AnnotationKind::Arrow,
            Self::Rect => AnnotationKind::Rect,
            Self::Ellipse => AnnotationKind::Ellipse,
            Self::Highlight => AnnotationKind::Highlight,
            Self::Text => AnnotationKind::Text,
            Self::Counter => AnnotationKind::Counter,
            Self::Pixelate => AnnotationKind::Pixelate,
            Self::Blur => AnnotationKind::Blur,
            Self::Select | Self::Crop => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationKind {
    Arrow,
    Rect,
    Ellipse,
    Highlight,
    Text,
    Counter,
    Pixelate,
    Blur,
}

impl AnnotationKind {
    /// Kinds defined by a two-point box that can be resized by their corners.
    pub fn is_box(self) -> bool {
        matches!(
            self,
            Self::Rect | Self::Ellipse | Self::Highlight | Self::Pixelate | Self::Blur
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "[f64; 2]", into = "[f64; 2]")]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl From<[f64; 2]> for Pt {
    fn from(v: [f64; 2]) -> Self {
        Pt { x: v[0], y: v[1] }
    }
}

impl From<Pt> for [f64; 2] {
    fn from(p: Pt) -> Self {
        [p.x, p.y]
    }
}

pub fn pt(x: f64, y: f64) -> Pt {
    Pt { x, y }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn from_points(a: Pt, b: Pt) -> Self {
        Self::new(
            a.x.min(b.x),
            a.y.min(b.y),
            (a.x - b.x).abs(),
            (a.y - b.y).abs(),
        )
    }

    pub fn max_x(&self) -> f64 {
        self.x + self.w
    }

    pub fn max_y(&self) -> f64 {
        self.y + self.h
    }

    pub fn mid_x(&self) -> f64 {
        self.x + self.w / 2.
    }

    pub fn mid_y(&self) -> f64 {
        self.y + self.h / 2.
    }

    pub fn inset(&self, dx: f64, dy: f64) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w - dx * 2., self.h - dy * 2.)
    }

    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x && p.x <= self.max_x() && p.y >= self.y && p.y <= self.max_y()
    }

    pub fn intersects(&self, o: &Rect) -> bool {
        self.x < o.max_x() && o.x < self.max_x() && self.y < o.max_y() && o.y < self.max_y()
    }

    pub fn intersection(&self, o: &Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let w = (self.max_x().min(o.max_x()) - x).max(0.);
        let h = (self.max_y().min(o.max_y()) - y).max(0.);
        Rect::new(x, y, w, h)
    }

    pub fn union(&self, o: &Rect) -> Rect {
        let x = self.x.min(o.x);
        let y = self.y.min(o.y);
        Rect::new(
            x,
            y,
            self.max_x().max(o.max_x()) - x,
            self.max_y().max(o.max_y()) - y,
        )
    }

    pub fn integral(&self) -> Rect {
        let x = self.x.floor();
        let y = self.y.floor();
        Rect::new(x, y, self.max_x().ceil() - x, self.max_y().ceil() - y)
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0. || self.h <= 0.
    }

    pub fn skia(&self) -> Option<tiny_skia::Rect> {
        tiny_skia::Rect::from_xywh(self.x as f32, self.y as f32, self.w as f32, self.h as f32)
    }
}

/// All geometry is in source-image pixel coordinates (origin top-left, y down).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub kind: AnnotationKind,
    pub p1: Pt,
    pub p2: Pt,
    pub color: Rgba,
    pub width: f64,
    #[serde(default)]
    pub text: String,
    #[serde(default = "default_font_size")]
    pub font_size: f64,
}

fn default_font_size() -> f64 {
    32.
}

impl Annotation {
    pub fn new(kind: AnnotationKind, p1: Pt, p2: Pt, color: Rgba, width: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            p1,
            p2,
            color,
            width,
            text: String::new(),
            font_size: 32.,
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::from_points(self.p1, self.p2)
    }

    pub fn counter_radius(&self) -> f64 {
        (self.width * 4.2).max(14.)
    }

    pub fn text_size(&self) -> (f64, f64) {
        let s = if self.text.is_empty() {
            " "
        } else {
            &self.text
        };
        let (w, h) = crate::render::text::measure(s, self.font_size, crate::render::text::BOLD);
        (w.ceil(), h.ceil())
    }

    /// Bounding box used for hit testing and selection outlines.
    pub fn bounds(&self) -> Rect {
        match self.kind {
            AnnotationKind::Arrow => {
                let d = self.width * 3.;
                self.rect().inset(-d, -d)
            }
            AnnotationKind::Text => {
                let (w, h) = self.text_size();
                Rect::new(self.p1.x, self.p1.y, w, h)
            }
            AnnotationKind::Counter => {
                let r = self.counter_radius();
                Rect::new(self.p1.x - r, self.p1.y - r, r * 2., r * 2.)
            }
            _ => self.rect(),
        }
    }

    pub fn hit_test(&self, p: Pt, tolerance: f64) -> bool {
        match self.kind {
            AnnotationKind::Arrow => {
                distance_to_segment(p, self.p1, self.p2) <= (self.width * 2.).max(tolerance)
            }
            AnnotationKind::Rect | AnnotationKind::Ellipse => {
                self.rect().inset(-tolerance, -tolerance).contains(p)
            }
            _ => self.bounds().inset(-tolerance, -tolerance).contains(p),
        }
    }

    /// Stricter hit used while a drawing tool is active: only outlines and arrow lines respond,
    /// so a new shape can still start on top of a label, a counter or a filled area.
    pub fn grab_test(&self, p: Pt, tolerance: f64) -> bool {
        let r = self.rect();
        match self.kind {
            AnnotationKind::Ellipse => {
                let pad = self.width.max(tolerance);
                let (rx, ry) = (r.w / 2., r.h / 2.);
                if rx <= 0. || ry <= 0. {
                    return false;
                }
                let nx = (p.x - r.mid_x()) / rx;
                let ny = (p.y - r.mid_y()) / ry;
                ((nx * nx + ny * ny).sqrt() - 1.).abs() * rx.min(ry) <= pad
            }
            AnnotationKind::Rect => {
                let pad = self.width.max(tolerance);
                r.inset(-pad, -pad).contains(p) && !r.inset(pad, pad).contains(p)
            }
            AnnotationKind::Arrow => self.hit_test(p, tolerance),
            _ => false,
        }
    }

    pub fn offset(&mut self, dx: f64, dy: f64) {
        self.p1.x += dx;
        self.p1.y += dy;
        self.p2.x += dx;
        self.p2.y += dy;
    }
}

pub fn distance_to_segment(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0. {
        return (p.x - a.x).hypot(p.y - a.y);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0., 1.);
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

/// Counters number themselves in stacking order. A counter set to a number restarts the
/// sequence from it; one set to other text (a letter, say) shows it without taking a number.
pub fn counter_labels(list: &[Annotation]) -> std::collections::HashMap<Uuid, String> {
    let mut n: i64 = 0;
    let mut out = std::collections::HashMap::new();
    for a in list.iter().filter(|a| a.kind == AnnotationKind::Counter) {
        let custom = a.text.trim();
        if custom.is_empty() {
            n += 1;
        } else if let Ok(v) = custom.parse::<i64>() {
            n = v;
        }
        out.insert(
            a.id,
            if custom.is_empty() {
                n.to_string()
            } else {
                custom.to_string()
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_matches_swift_codable() {
        let json = r##"{"background":{"gradient":{"_0":[{"r":1,"g":0,"b":0,"a":1}],"angle":135}},
            "padding":0.1,"aspect":"r16x9","frame":"macDark","autoBalance":false}"##;
        let s: Style = serde_json::from_str(json).unwrap();
        assert_eq!(s.aspect, AspectRatio::R16x9);
        assert_eq!(s.frame, WindowFrame::MacDark);
        assert!(!s.auto_balance);
        assert_eq!(s.shadow, 0.5);
        let back = serde_json::to_string(&Style::default()).unwrap();
        assert!(back.contains(r#""background":{"solid":{"_0":"#));
        let none: Background = serde_json::from_str(r#"{"none":{}}"#).unwrap();
        assert!(none.is_none());
    }
}
