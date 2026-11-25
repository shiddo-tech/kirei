use std::path::Path;

use crate::model::{Annotation, AnnotationKind, Pt, Style, annotation_palette, pt};
use crate::render::{EffectImages, RenderInput, SourceImage, render};
use crate::{analysis, export, platform, settings};

pub fn load(path: &Path) -> Option<(SourceImage, f64)> {
    let img = platform::decode_image_file(path)?;
    let src = SourceImage::from_rgba(img.width(), img.height(), img.as_raw())?;
    Some((src, platform::file_scale(path)))
}

/// Developer checks without UI, mirroring the Swift app's flags so outputs can be compared.
pub fn run_from_args() -> bool {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--render-test") if args.len() >= 4 => {
            let style = args
                .get(4)
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|d| serde_json::from_slice::<Style>(&d).ok())
                .unwrap_or_default();
            if let Err(e) = render_test(Path::new(&args[2]), Path::new(&args[3]), style) {
                eprintln!("render-test: {e:#}");
                std::process::exit(1);
            }
            true
        }
        Some("--record-test") if args.len() >= 3 => {
            let out = std::path::PathBuf::from(&args[2]);
            // Window filters need a window-server connection, which the real app always has.
            crate::platform::hide_from_dock();
            let started = match args.get(3).and_then(|w| w.parse().ok()) {
                Some(id) => crate::capture::Recording::start_window(id, out),
                None => crate::capture::Recording::start_area(100., 100., 641., 361., out),
            };
            let rec = match started {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("record-test: {e:#}");
                    std::process::exit(1)
                }
            };
            std::thread::sleep(std::time::Duration::from_secs(2));
            println!("record-test: {:?}", rec.stop());
            true
        }
        Some("--gif-test") if args.len() == 3 => {
            let video = Path::new(&args[2]);
            let poster = crate::platform::video_poster(video);
            let gif = crate::platform::video_to_gif(video, 12., 960.);
            println!(
                "poster={:?} gif={:?}",
                poster.map(|p| p.dimensions()),
                gif.map(|g| g.display().to_string())
            );
            true
        }
        Some("--scroll-test") if args.len() == 4 => {
            let Some((src, scale)) = load(Path::new(&args[2])) else {
                std::process::exit(1)
            };
            let Some(out) = crate::scroll::self_test(&src.pixmap) else {
                std::process::exit(1)
            };
            export::save(
                &out,
                scale,
                Path::new(&args[3]),
                settings::Format::Png,
                true,
            )
            .ok();
            true
        }
        Some("--preview-test") if args.len() == 5 => {
            let scale: f64 = args[4].parse().unwrap_or(1.);
            let Some((src, _)) = load(Path::new(&args[2])) else {
                std::process::exit(1)
            };
            let style = settings::last_style();
            let crop = if style.auto_balance {
                analysis::balanced_rect(&src)
            } else {
                src.full_rect()
            };
            let edge = analysis::edge_color(&src, crop);
            let out = render(&RenderInput {
                source: &src,
                style: &style,
                crop,
                edge_color: edge,
                annotations: &[],
                effects: None,
                hidden: None,
                scale,
            })
            .expect("render");
            export::save(&out, 1., Path::new(&args[3]), settings::Format::Png, true).ok();
            println!("out: {} x {}", out.width(), out.height());
            true
        }
        _ => false,
    }
}

/// The annotations the Swift `--render-test` adds, so both renders can be compared.
pub fn sample_annotations(c: crate::model::Rect, unit: f64) -> Vec<Annotation> {
    let pal = annotation_palette();
    let mut list = vec![
        Annotation::new(
            AnnotationKind::Arrow,
            pt(c.x + 40., c.max_y() - 40.),
            pt(c.mid_x(), c.mid_y()),
            pal[0],
            unit * 2.,
        ),
        Annotation::new(
            AnnotationKind::Rect,
            pt(c.x + 20., c.y + 20.),
            pt(c.mid_x(), c.y + 120.),
            pal[4],
            unit * 2.,
        ),
        Annotation::new(
            AnnotationKind::Counter,
            pt(c.max_x() - 60., c.y + 60.),
            Pt::default(),
            pal[5],
            unit * 2.,
        ),
    ];
    let mut text = Annotation::new(
        AnnotationKind::Text,
        pt(c.mid_x(), c.max_y() - 120.),
        Pt::default(),
        pal[1],
        unit * 2.,
    );
    text.text = "Olá Kirei".into();
    text.font_size = unit * 13.;
    list.push(text);
    list.push(Annotation::new(
        AnnotationKind::Highlight,
        pt(c.x + 30., c.mid_y()),
        pt(c.mid_x(), c.mid_y() + 40.),
        pal[2],
        unit,
    ));
    list
}

/// `KIREI_UI_STATE` (comma-separated) preloads editor state for screenshots in `--edit` runs.
pub fn apply_ui_state(cx: &mut gpui::App) {
    let Ok(spec) = std::env::var("KIREI_UI_STATE") else {
        return;
    };
    let Some(handle) = cx
        .windows()
        .into_iter()
        .find_map(|w| w.downcast::<crate::editor::Editor>())
    else {
        return;
    };
    handle
        .update(cx, |editor, window, cx| {
            editor.debug_apply(&spec, window, cx)
        })
        .ok();
}

fn render_test(input: &Path, output: &Path, style: Style) -> anyhow::Result<()> {
    let t0 = std::time::Instant::now();
    let (src, scale) = load(input).ok_or_else(|| anyhow::anyhow!("não abriu {input:?}"))?;
    let balanced = analysis::balanced_rect(&src);
    let full = src.full_rect();
    println!("balanced: {balanced:?} full: {full:?}");
    let crop = if style.auto_balance { balanced } else { full };
    let rects = crate::ocr::sensitive_rects(&src.pixmap);
    println!("sensitive: {}", rects.len());
    println!(
        "ocr: {}",
        crate::ocr::text(&src.pixmap).replace('\n', " | ")
    );
    let unit = (crop.w.max(crop.h) / 320.).max(2.);
    let mut list: Vec<Annotation> = rects
        .iter()
        .map(|r| {
            Annotation::new(
                AnnotationKind::Pixelate,
                pt(r.x, r.y),
                pt(r.max_x(), r.max_y()),
                crate::model::Rgba::BLACK,
                0.,
            )
        })
        .collect();
    list.extend(sample_annotations(crop, unit));
    let effects = EffectImages::new(&src);
    let edge = analysis::edge_color(&src, crop);
    let out = render(&RenderInput {
        source: &src,
        style: &style,
        crop,
        edge_color: edge,
        annotations: &list,
        effects: Some(&effects),
        hidden: None,
        scale: 1.,
    })
    .ok_or_else(|| anyhow::anyhow!("render falhou"))?;
    export::save(&out, scale, output, settings::Format::Png, true)?;
    println!(
        "out: {} x {} ({} ms)",
        out.width(),
        out.height(),
        t0.elapsed().as_millis()
    );
    Ok(())
}
