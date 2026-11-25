use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, AsyncApp, Bounds, Context, Global, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels,
    Point, RenderImage, Task, Window, WindowHandle, div, hsla, img, point, prelude::*, px, size,
};
use tiny_skia::Pixmap;

use crate::l10n::{l, lf};
use crate::render::{EffectImages, RenderInput, SourceImage, render};
use crate::ui::icon;
use crate::{analysis, editor, export, feedback, hud, platform, settings};

/// Hotkey id for the system-wide ⌘C that copies the thumbnail while it is on screen.
pub const COPY_HOTKEY: u32 = 999;

#[derive(Default)]
struct Current(Option<WindowHandle<Thumbnail>>);

impl Global for Current {}

/// Post-capture thumbnail in the screen corner: drag it out, copy, save, pin or edit.
/// `from` is where the capture was on screen and its pixels; the image flies from there into
/// the thumbnail first, like the Swift app.
pub fn show(
    source: Arc<SourceImage>,
    scale: f64,
    from: Option<(Bounds<Pixels>, Arc<RenderImage>)>,
    cx: &mut App,
) {
    close_current(cx);
    let (w, h) = (248., 176.);
    let visible = platform::visible_frame_at_mouse();
    let origin = point(
        visible.origin.x + visible.size.width - px(w + 20.),
        visible.origin.y + visible.size.height - px(h + 20.),
    );
    let bounds = Bounds::new(origin, size(px(w), px(h)));
    let from = from.filter(|(r, _)| r.size.width > px(8.) && r.size.height > px(8.));
    let hidden = from.is_some();
    let options = crate::ui::popup_options(bounds, cx);
    let handle = cx
        .open_window(options, |window, cx| {
            platform::set_window_level(window, 25);
            platform::set_window_shadow(window, true);
            if hidden {
                platform::set_window_alpha(window, 0.);
            }
            cx.new(|cx| Thumbnail::new(source, scale, cx))
        })
        .ok();
    if let (Some(h), Some((rect, image))) = (handle, from) {
        let slot = Bounds::new(
            bounds.origin + point(px(16.), px(16.)),
            bounds.size - size(px(32.), px(32.)),
        );
        crate::flash::fly(
            image,
            rect,
            slot,
            move |cx| {
                h.update(cx, |_, w, _| platform::set_window_alpha(w, 1.))
                    .ok();
            },
            cx,
        );
    }
    cx.set_global(Current(handle));
    if handle.is_some() && crate::shell::is_started() {
        platform::hotkeys::register(COPY_HOTKEY, 0x08, settings::MOD_CMD).ok();
    }
}

fn close_current(cx: &mut App) {
    if let Some(h) = cx.default_global::<Current>().0.take() {
        h.update(cx, |_, w, _| w.remove_window()).ok();
    }
    platform::hotkeys::unregister(COPY_HOTKEY);
}

pub fn copy_current(cx: &mut App) {
    if let Some(h) = cx.default_global::<Current>().0 {
        h.update(cx, |t, w, cx| t.copy(w, cx)).ok();
    }
}

pub struct Thumbnail {
    source: Arc<SourceImage>,
    scale: f64,
    final_image: Option<Arc<Pixmap>>,
    image: Option<Arc<RenderImage>>,
    hover: bool,
    press: Option<Point<Pixels>>,
    _render: Task<()>,
    dismiss: Option<Task<()>>,
}

impl Thumbnail {
    fn new(source: Arc<SourceImage>, scale: f64, cx: &mut Context<Self>) -> Self {
        let src = source.clone();
        let render_task = cx.spawn(async move |this, cx: &mut AsyncApp| {
            let pm = cx
                .background_executor()
                .spawn(async move { render_styled(&src) })
                .await;
            let Some(pm) = pm else { return };
            let image = crate::ui::render_image(&pm);
            this.update(cx, |t, cx| {
                t.final_image = Some(Arc::new(pm));
                t.image = Some(image);
                cx.notify();
            })
            .ok();
        });
        let mut t = Self {
            source,
            scale,
            final_image: None,
            image: None,
            hover: false,
            press: None,
            _render: render_task,
            dismiss: None,
        };
        t.schedule_dismiss(cx);
        t
    }

    fn schedule_dismiss(&mut self, cx: &mut Context<Self>) {
        if self.hover {
            self.dismiss = None;
            return;
        }
        self.dismiss = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            cx.background_executor().timer(Duration::from_secs(7)).await;
            this.update(cx, |_, cx| cx.defer(close_current)).ok();
        }));
    }

    fn pixmap(&self) -> Option<Arc<Pixmap>> {
        self.final_image
            .clone()
            .or_else(|| render_styled(&self.source).map(Arc::new))
    }

    fn copy(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(pm) = self.pixmap() else { return };
        if platform::copy_image(&pm, self.scale, settings::get().export_retina).is_ok() {
            feedback::play_copy();
            hud::show(l("Image copied"), cx);
        }
        cx.defer(close_current);
    }

    fn save(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(pm) = self.pixmap() else { return };
        let s = settings::get();
        let folder = s.save_folder();
        let path = export::unique_path(&folder, &export::file_name(s.format.ext()));
        match export::save(&pm, self.scale, &path, s.format, s.export_retina) {
            Ok(()) => hud::show_with(
                lf(
                    "Saved to %@",
                    folder
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                ),
                "folder.fill",
                cx,
            ),
            Err(_) => hud::show_with(l("Couldn't save"), "exclamationmark.triangle.fill", cx),
        }
        cx.defer(close_current);
    }

    fn pin(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(pm) = self.pixmap() else { return };
        crate::pinned::pin((*pm).clone(), self.scale, cx);
        cx.defer(close_current);
    }

    fn edit(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let (source, scale) = (self.source.clone(), self.scale);
        cx.defer(move |cx| {
            close_current(cx);
            editor::open_source(source, scale, cx);
        });
    }

    fn drag_out(&mut self, window: &mut Window) {
        let Some(pm) = self.pixmap() else { return };
        let dir = std::env::temp_dir().join("Kirei");
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let path = dir.join(export::file_name("png"));
        if export::save(
            &pm,
            self.scale,
            &path,
            settings::Format::Png,
            settings::get().export_retina,
        )
        .is_ok()
        {
            platform::drag_file(window, &path);
        }
    }
}

/// The capture with the last style (and redaction when that setting is on).
fn render_styled(source: &SourceImage) -> Option<Pixmap> {
    let style = settings::last_style();
    let crop = if style.auto_balance {
        analysis::balanced_rect(source)
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
    let effects = (!annotations.is_empty()).then(|| EffectImages::new(source));
    render(&RenderInput {
        source,
        style: &style,
        crop,
        edge_color: analysis::edge_color(source, crop),
        annotations: &annotations,
        effects: effects.as_ref(),
        hidden: None,
        scale: 1.,
    })
}

fn action(
    id: &'static str,
    title: String,
    symbol: &'static str,
    cx: &mut Context<Thumbnail>,
    f: fn(&mut Thumbnail, &mut Window, &mut Context<Thumbnail>),
) -> impl IntoElement {
    let white = hsla(0., 0., 1., 1.);
    div()
        .id(id)
        .px(px(12.))
        .py(px(6.))
        .rounded_full()
        .bg(hsla(0., 0., 1., 0.2))
        .border_1()
        .border_color(hsla(0., 0., 1., 0.3))
        .flex()
        .items_center()
        .gap(px(5.))
        .text_size(px(13.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(white)
        .cursor_pointer()
        .child(icon(symbol, 12., white))
        .child(title)
        .on_click(cx.listener(move |t, _, w, cx| f(t, w, cx)))
}

impl Render for Thumbnail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = crate::ui::Theme::get(window);
        let mut card = div()
            .id("thumb")
            .relative()
            .size_full()
            .rounded(px(16.))
            .bg(theme.panel)
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .on_hover(cx.listener(|t, h: &bool, _, cx| {
                t.hover = *h;
                t.schedule_dismiss(cx);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|t, ev: &MouseDownEvent, w, cx| {
                    if ev.click_count >= 2 {
                        t.edit(w, cx);
                    } else {
                        t.press = Some(ev.position);
                    }
                }),
            )
            .on_mouse_move(cx.listener(|t, ev: &MouseMoveEvent, w, _| {
                let Some(start) = t.press else { return };
                if ev.pressed_button != Some(MouseButton::Left) {
                    t.press = None;
                    return;
                }
                let d = ev.position - start;
                if f32::from(d.x).hypot(f32::from(d.y)) > 4. {
                    t.press = None;
                    t.drag_out(w);
                }
            }));
        if let Some(image) = self.image.clone() {
            card = card.child(img(image).max_w(px(224.)).max_h(px(152.)));
        }
        if self.hover {
            let white = hsla(0., 0., 1., 0.9);
            card = card
                .child(div().absolute().inset_0().bg(hsla(0., 0., 0., 0.35)))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(action("copy", l("Copy"), "doc.on.doc", cx, Thumbnail::copy))
                                .child(action(
                                    "save",
                                    l("Save"),
                                    "square.and.arrow.down",
                                    cx,
                                    Thumbnail::save,
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(action(
                                    "edit",
                                    l("Edit"),
                                    "slider.horizontal.3",
                                    cx,
                                    Thumbnail::edit,
                                ))
                                .child(action("pin", l("Pin"), "pin", cx, Thumbnail::pin)),
                        ),
                )
                .child(
                    div()
                        .id("close")
                        .absolute()
                        .top(px(10.))
                        .left(px(10.))
                        .size(px(20.))
                        .rounded_full()
                        .bg(hsla(0., 0., 0.15, 0.85))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(icon("xmark", 9., white))
                        .tooltip(|_, cx| crate::ui::tooltip_view(l("Close").into(), cx))
                        .on_click(cx.listener(|_, _, _, cx| cx.defer(close_current))),
                );
        }
        div().size_full().p(px(4.)).child(card)
    }
}
