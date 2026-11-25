use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

use crate::render::straight_rgba;
use crate::settings::Format;

/// Applies the "Retina size" setting: off halves a 2x image to 1 pixel per point.
pub fn sized(pm: &Pixmap, scale: f64, retina: bool) -> (Pixmap, f64) {
    if retina || scale <= 1. {
        return (pm.clone(), scale);
    }
    let w = ((pm.width() as f64 / scale).round() as u32).max(1);
    let h = ((pm.height() as f64 / scale).round() as u32).max(1);
    let Some(mut out) = Pixmap::new(w, h) else {
        return (pm.clone(), scale);
    };
    out.draw_pixmap(
        0,
        0,
        pm.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..Default::default()
        },
        Transform::from_scale(w as f32 / pm.width() as f32, h as f32 / pm.height() as f32),
        None,
    );
    (out, 1.)
}

pub fn png(pm: &Pixmap, scale: f64) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, pm.width(), pm.height());
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let ppm = (72. * scale / 0.0254).round() as u32;
        enc.set_pixel_dims(Some(png::PixelDimensions {
            xppu: ppm,
            yppu: ppm,
            unit: png::Unit::Meter,
        }));
        let mut writer = enc.write_header()?;
        writer.write_image_data(&straight_rgba(pm))?;
    }
    Ok(out)
}

/// JPEG has no alpha: composite on white.
pub fn jpeg(pm: &Pixmap) -> Result<Vec<u8>> {
    let mut rgb = Vec::with_capacity((pm.width() * pm.height() * 3) as usize);
    for p in pm.pixels() {
        let inv = 255 - p.alpha() as u32;
        rgb.push((p.red() as u32 + inv) as u8);
        rgb.push((p.green() as u32 + inv) as u8);
        rgb.push((p.blue() as u32 + inv) as u8);
    }
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
    image::ImageEncoder::write_image(
        enc,
        &rgb,
        pm.width(),
        pm.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}

pub fn encode(pm: &Pixmap, format: Format, scale: f64, retina: bool) -> Result<Vec<u8>> {
    let (pm, scale) = sized(pm, scale, retina);
    match format {
        Format::Png => png(&pm, scale),
        Format::Jpeg => jpeg(&pm),
        Format::Heic => crate::platform::encode_heic(&pm, scale)
            .or_else(|_| jpeg(&pm))
            .context("HEIC"),
    }
}

pub fn file_name(ext: &str) -> String {
    let now = crate::platform::local_time();
    let pattern = crate::l10n::l("yyyy-MM-dd 'at' HH.mm.ss");
    format!("Kirei {}.{ext}", format_date(&pattern, &now))
}

/// Minimal subset of Unicode date patterns used by the translations.
fn format_date(pattern: &str, t: &crate::platform::LocalTime) -> String {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            for q in chars.by_ref() {
                if q == '\'' {
                    break;
                }
                out.push(q);
            }
            continue;
        }
        let mut n = 1;
        while chars.peek() == Some(&c) {
            chars.next();
            n += 1;
        }
        let v = match c {
            'y' => Some(t.year as u32),
            'M' => Some(t.month),
            'd' => Some(t.day),
            'H' => Some(t.hour),
            'm' => Some(t.minute),
            's' => Some(t.second),
            _ => None,
        };
        match v {
            Some(v) if c == 'y' => out.push_str(&v.to_string()),
            Some(v) => out.push_str(&format!("{v:0n$}")),
            None => (0..n).for_each(|_| out.push(c)),
        }
    }
    out
}

pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    if !path.exists() {
        return path;
    }
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    (2..)
        .map(|i| dir.join(format!("{stem} {i}.{ext}")))
        .find(|p| !p.exists())
        .unwrap_or(path)
}

pub fn save(pm: &Pixmap, scale: f64, path: &Path, format: Format, retina: bool) -> Result<()> {
    std::fs::write(path, encode(pm, format, scale, retina)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Pixmap {
        let mut pm = Pixmap::new(200, 100).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(30, 120, 200, 255));
        pm
    }

    #[test]
    fn encodes_every_format() {
        let dir = std::env::temp_dir().join(format!("kirei-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pm = sample();
        for format in Format::ALL {
            let path = dir.join(format!("out.{}", format.ext()));
            save(&pm, 2., &path, format, true).unwrap();
            let img = crate::platform::decode_image_file(&path).expect("decodifica");
            assert_eq!((img.width(), img.height()), (200, 100), "{format:?}");
            let px = img.get_pixel(100, 50);
            assert!((px[2] as i32 - 200).abs() < 6, "{format:?} {px:?}");
        }
        assert_eq!(crate::platform::file_scale(&dir.join("out.png")), 2.);
        save(&pm, 2., &dir.join("half.png"), Format::Png, false).unwrap();
        let half = crate::platform::decode_image_file(&dir.join("half.png")).unwrap();
        assert_eq!((half.width(), half.height()), (100, 50));
        assert_eq!(crate::platform::file_scale(&dir.join("half.png")), 1.);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn formats_file_names() {
        let t = crate::platform::LocalTime {
            year: 2026,
            month: 3,
            day: 7,
            hour: 9,
            minute: 5,
            second: 1,
        };
        assert_eq!(
            format_date("yyyy-MM-dd 'at' HH.mm.ss", &t),
            "2026-03-07 at 09.05.01"
        );
    }
}
