use std::sync::OnceLock;

use tiny_skia::{Path, PathBuilder, Transform};
use ttf_parser::{Face, OutlineBuilder, Tag};

pub const SEMIBOLD: f32 = 600.;
pub const BOLD: f32 = 700.;
pub const HEAVY: f32 = 800.;

struct FontFile {
    data: Vec<u8>,
    index: u32,
}

static FONTS: OnceLock<Vec<FontFile>> = OnceLock::new();

/// The system UI font first (SF on macOS), then faces that cover other scripts.
fn fonts() -> &'static [FontFile] {
    FONTS.get_or_init(|| {
        let mut out = Vec::new();
        let candidates: &[(&str, u32)] = &[
            ("/System/Library/Fonts/SFNS.ttf", 0),
            ("/System/Library/Fonts/HelveticaNeue.ttc", 1),
            ("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
            ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
            ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
        ];
        for (path, index) in candidates {
            if let Ok(data) = std::fs::read(path)
                && Face::parse(&data, *index).is_ok()
            {
                out.push(FontFile {
                    data,
                    index: *index,
                });
            }
        }
        if out.is_empty() {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            let query = fontdb::Query {
                families: &[fontdb::Family::SansSerif],
                weight: fontdb::Weight::BOLD,
                ..Default::default()
            };
            if let Some(id) = db.query(&query) {
                db.with_face_data(id, |data, index| {
                    out.push(FontFile {
                        data: data.to_vec(),
                        index,
                    })
                });
            }
        }
        out
    })
}

fn face(file: &FontFile, weight: f32) -> Option<Face<'_>> {
    let mut face = Face::parse(&file.data, file.index).ok()?;
    if face.is_variable() {
        face.set_variation(Tag::from_bytes(b"wght"), weight);
    }
    Some(face)
}

struct Metrics {
    ascent: f32,
    line_height: f32,
}

fn metrics(size: f32, weight: f32) -> Metrics {
    let Some(face) = fonts().first().and_then(|f| face(f, weight)) else {
        return Metrics {
            ascent: size * 0.95,
            line_height: size * 1.2,
        };
    };
    let k = size / face.units_per_em() as f32;
    let ascent = face.ascender() as f32 * k;
    let descent = -(face.descender() as f32) * k;
    Metrics {
        ascent,
        line_height: ascent + descent + face.line_gap() as f32 * k,
    }
}

struct PathSink<'a> {
    pb: &'a mut PathBuilder,
    x: f32,
    y: f32,
    k: f32,
}

impl OutlineBuilder for PathSink<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.x + x * self.k, self.y - y * self.k);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.x + x * self.k, self.y - y * self.k);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb.quad_to(
            self.x + x1 * self.k,
            self.y - y1 * self.k,
            self.x + x * self.k,
            self.y - y * self.k,
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            self.x + x1 * self.k,
            self.y - y1 * self.k,
            self.x + x2 * self.k,
            self.y - y2 * self.k,
            self.x + x * self.k,
            self.y - y * self.k,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Splits a line into runs by the first font that has each character.
fn runs(line: &str) -> Vec<(usize, String)> {
    let files = fonts();
    let mut out: Vec<(usize, String)> = Vec::new();
    for ch in line.chars() {
        let idx = files
            .iter()
            .position(|f| {
                Face::parse(&f.data, f.index)
                    .ok()
                    .and_then(|face| face.glyph_index(ch))
                    .is_some()
            })
            .unwrap_or(0);
        match out.last_mut() {
            Some((i, s)) if *i == idx || ch == ' ' => s.push(ch),
            _ => out.push((idx, ch.to_string())),
        }
    }
    out
}

/// Lays out one line; calls `glyph` with (face, glyph id, x, baseline) and returns the advance.
fn layout_line(
    line: &str,
    size: f32,
    weight: f32,
    mut glyph: impl FnMut(&Face, ttf_parser::GlyphId, f32),
) -> f32 {
    let mut x = 0.;
    for (idx, text) in runs(line) {
        let Some(file) = fonts().get(idx) else {
            continue;
        };
        let Some(ttf) = face(file, weight) else {
            continue;
        };
        let k = size / ttf.units_per_em() as f32;
        let mut hb = rustybuzz::Face::from_face(ttf.clone());
        if ttf.is_variable() {
            hb.set_variations(&[rustybuzz::Variation {
                tag: Tag::from_bytes(b"wght"),
                value: weight,
            }]);
        }
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(&text);
        let shaped = rustybuzz::shape(&hb, &[], buffer);
        for (info, pos) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            glyph(
                &ttf,
                ttf_parser::GlyphId(info.glyph_id as u16),
                x + pos.x_offset as f32 * k,
            );
            x += pos.x_advance as f32 * k;
        }
    }
    x
}

/// Size of the text box, like NSString.boundingRect with line fragments.
pub fn measure(text: &str, size: f64, weight: f32) -> (f64, f64) {
    #[cfg(target_os = "macos")]
    if let Some(m) = crate::platform::macos_text::measure(text, size, weight) {
        return m;
    }
    let size = size as f32;
    let m = metrics(size, weight);
    let mut w: f32 = 0.;
    let lines: Vec<&str> = text.split('\n').collect();
    for line in &lines {
        w = w.max(layout_line(line, size, weight, |_, _, _| {}));
    }
    (w as f64, (m.line_height * lines.len() as f32) as f64)
}

/// Glyph outlines with the text box's top-left corner at the origin.
pub fn path(text: &str, size: f64, weight: f32) -> Option<Path> {
    #[cfg(target_os = "macos")]
    if let Some(p) = crate::platform::macos_text::path(text, size, weight) {
        return Some(p);
    }
    let size = size as f32;
    let m = metrics(size, weight);
    let mut pb = PathBuilder::new();
    for (i, line) in text.split('\n').enumerate() {
        let baseline = m.ascent + m.line_height * i as f32;
        layout_line(line, size, weight, |face, gid, x| {
            let k = size / face.units_per_em() as f32;
            let mut sink = PathSink {
                pb: &mut pb,
                x,
                y: baseline,
                k,
            };
            face.outline_glyph(gid, &mut sink);
        });
    }
    pb.finish()
}

pub fn path_at(text: &str, size: f64, weight: f32, x: f64, y: f64) -> Option<Path> {
    path(text, size, weight)?.transform(Transform::from_translate(x as f32, y as f32))
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_glyph_paths() {
        assert!(!super::fonts().is_empty(), "nenhuma fonte");
        let (w, h) = super::measure("Olá Kirei", 32., super::BOLD);
        eprintln!("measure {w} {h}");
        let p = super::path("Olá Kirei", 32., super::BOLD).expect("path");
        eprintln!("bounds {:?}", p.bounds());
        assert!(w > 50.);
    }
}
