use std::sync::Arc;

use gpui::{App, AsyncApp, Global};

use crate::capture::{self, Captured};
use crate::l10n::l;
use crate::render::{RenderInput, SourceImage, render};
use crate::settings::{self, AfterCapture, CaptureAction};
use crate::{analysis, editor, feedback, history, hud, overlay, platform, welcome};

#[derive(Default)]
struct CaptureInFlight(bool);

#[derive(Default)]
struct Timed(bool);

impl Global for Timed {}

impl Global for CaptureInFlight {}

/// Runs a capture action from a hotkey or a menu.
pub fn perform(action: CaptureAction, cx: &mut App) {
    if action == CaptureAction::Clipboard {
        return from_clipboard(cx);
    }
    if !platform::screen_capture_allowed() {
        platform::request_screen_capture();
        welcome::open_at(welcome::Step::Permission, Some(action), cx);
        return;
    }
    perform_unchecked(action, cx);
}

/// Runs the action without checking the Screen Recording permission ("Try Anyway").
pub fn perform_unchecked(action: CaptureAction, cx: &mut App) {
    match action {
        CaptureAction::Scrolling => crate::scrolling::start(cx),
        CaptureAction::Record => crate::recorder::toggle(cx),
        _ => start_capture(action, cx),
    }
}

/// Actions still missing in this port are left out of the menus and hotkeys.
pub fn is_ported(action: CaptureAction) -> bool {
    let _ = action;
    true
}

fn start_capture(action: CaptureAction, cx: &mut App) {
    if overlay::is_active(cx) || cx.default_global::<CaptureInFlight>().0 {
        return;
    }
    let delay = settings::get().capture_delay;
    if delay > 0
        && matches!(
            action,
            CaptureAction::Area | CaptureAction::Window | CaptureAction::Fullscreen
        )
        && !cx.default_global::<Timed>().0
    {
        crate::countdown::start(
            delay,
            move |cx| {
                cx.set_global(Timed(true));
                start_capture(action, cx);
                cx.set_global(Timed(false));
            },
            cx,
        );
        return;
    }
    cx.set_global(CaptureInFlight(true));
    cx.spawn(async move |cx: &mut AsyncApp| {
        let shots = cx
            .background_executor()
            .spawn(async { capture::freeze_displays() })
            .await;
        cx.update(|cx| {
            cx.set_global(CaptureInFlight(false));
            match shots {
                Ok(shots) => match action {
                    CaptureAction::Fullscreen => fullscreen(shots, cx),
                    _ => {
                        overlay::begin(shots, action == CaptureAction::Window, cx, move |c, cx| {
                            handle(c, action, cx)
                        })
                    }
                },
                Err(e) => {
                    eprintln!("kirei: falha na captura: {e:#}");
                    hud::show_with(
                        l("Couldn't capture the screen"),
                        "exclamationmark.triangle.fill",
                        cx,
                    );
                }
            }
        })
        .ok();
    })
    .detach();
}

/// Captures the screen under the mouse.
fn fullscreen(shots: Vec<capture::DisplayShot>, cx: &mut App) {
    let mouse = platform::mouse_location();
    let frame = |d: &std::rc::Rc<dyn gpui::PlatformDisplay>| {
        let origin = platform::display_origin(u32::from(d.id())).unwrap_or_default();
        gpui::Bounds::new(origin, d.bounds().size)
    };
    let displays = cx.displays();
    let display = displays
        .iter()
        .find(|d| frame(d).contains(&mouse))
        .cloned()
        .or_else(|| cx.primary_display());
    let shot = display
        .as_ref()
        .and_then(|d| shots.iter().find(|s| s.display_id == u32::from(d.id())))
        .or(shots.first());
    let Some(shot) = shot else { return };
    let scale = display
        .as_ref()
        .map(|d| shot.width as f32 / f32::from(d.bounds().size.width))
        .unwrap_or(2.);
    let mut captured = shot.whole(scale);
    captured.screen_rect = display.map(|d| frame(&d));
    handle(captured, CaptureAction::Fullscreen, cx);
}

fn from_clipboard(cx: &mut App) {
    match platform::pasteboard_image() {
        Some((img, scale)) => {
            if let Some(src) = SourceImage::from_rgba(img.width(), img.height(), img.as_raw()) {
                editor::open_source(Arc::new(src), scale, cx);
            }
        }
        None => hud::show_with(l("No image on the clipboard"), "doc.on.clipboard", cx),
    }
}

pub fn handle(captured: Captured, action: CaptureAction, cx: &mut App) {
    if let Some(r) = captured.screen_rect {
        crate::flash::flash(r, cx);
    }
    let Some(source) = SourceImage::from_bgra(captured.width, captured.height, &captured.bgra)
    else {
        return;
    };
    let source = Arc::new(source);
    let scale = captured.scale as f64;
    if action != CaptureAction::Text {
        let pm = source.pixmap.clone();
        cx.background_executor()
            .spawn(async move {
                history::add(&pm, scale);
            })
            .detach();
    }
    feedback::play_capture();
    if action == CaptureAction::Text {
        return ocr_to_clipboard(source, cx);
    }
    match settings::get().after_capture {
        AfterCapture::CopyStyled => quick_copy(source, scale, cx),
        AfterCapture::Preview => {
            let from = captured.screen_rect.map(|r| (r, captured.render_image()));
            crate::thumbnail::show(source, scale, from, cx)
        }
        AfterCapture::OpenEditor => editor::open_source(source, scale, cx),
    }
}

fn ocr_to_clipboard(source: Arc<SourceImage>, cx: &mut App) {
    hud::show_with(l("Recognizing text…"), "text.viewfinder", cx);
    cx.spawn(async move |cx: &mut AsyncApp| {
        let text = cx
            .background_executor()
            .spawn(async move { crate::ocr::text(&source.pixmap) })
            .await;
        cx.update(|cx| {
            if text.is_empty() {
                hud::show_with(l("No text found"), "text.viewfinder", cx);
            } else {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                hud::show_with(l("Text copied"), "text.viewfinder", cx);
            }
        })
        .ok();
    })
    .detach();
}

/// "Copy styled image directly": renders with the last style (and redaction) and copies.
fn quick_copy(source: Arc<SourceImage>, scale: f64, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let pm = cx
            .background_executor()
            .spawn(async move {
                let style = settings::last_style();
                let crop = if style.auto_balance {
                    analysis::balanced_rect(&source)
                } else {
                    source.full_rect()
                };
                let annotations: Vec<crate::model::Annotation> = if settings::get().auto_redact {
                    crate::ocr::sensitive_rects(&source.pixmap)
                        .into_iter()
                        .map(|r| {
                            crate::model::Annotation::new(
                                crate::model::AnnotationKind::Pixelate,
                                crate::model::pt(r.x, r.y),
                                crate::model::pt(r.max_x(), r.max_y()),
                                crate::model::Rgba::BLACK,
                                0.,
                            )
                        })
                        .collect()
                } else {
                    vec![]
                };
                let effects =
                    (!annotations.is_empty()).then(|| crate::render::EffectImages::new(&source));
                render(&RenderInput {
                    source: &source,
                    style: &style,
                    crop,
                    edge_color: analysis::edge_color(&source, crop),
                    annotations: &annotations,
                    effects: effects.as_ref(),
                    hidden: None,
                    scale: 1.,
                })
            })
            .await;
        cx.update(|cx| {
            let Some(pm) = pm else { return };
            if platform::copy_image(&pm, scale, settings::get().export_retina).is_ok() {
                feedback::play_copy();
                hud::show(l("Styled image copied"), cx);
            }
        })
        .ok();
    })
    .detach();
}

pub fn open_image(cx: &mut App) {
    let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: true,
        prompt: None,
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let Ok(Ok(Some(paths))) = rx.await else {
            return;
        };
        cx.update(|cx| {
            for p in paths {
                if let Some((src, scale)) = crate::selftest::load(&p) {
                    editor::open_source(Arc::new(src), scale, cx);
                }
            }
        })
        .ok();
    })
    .detach();
}
