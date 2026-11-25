use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    App, AsyncApp, Bounds, Context, Global, Pixels, Task, Window, WindowHandle, canvas, div, hsla,
    point, prelude::*, px, size,
};

use crate::capture::{self, Captured, RegionGrabber};
use crate::l10n::{l, lf};
use crate::render::SourceImage;
use crate::scroll::{Frame, Stitcher};
use crate::settings::CaptureAction;
use crate::ui::{Theme, icon};

#[derive(Default)]
struct Current(Option<(WindowHandle<Border>, WindowHandle<Controls>)>);

impl Global for Current {}

pub fn is_active(cx: &App) -> bool {
    cx.try_global::<Current>().is_some_and(|c| c.0.is_some())
}

/// Scrolling capture: pick an area, scroll down, and the frames are stitched into one tall image.
pub fn start(cx: &mut App) {
    if is_active(cx) || crate::overlay::is_active(cx) {
        return;
    }
    cx.spawn(async move |cx: &mut AsyncApp| {
        let shots = cx
            .background_executor()
            .spawn(async { capture::freeze_displays() })
            .await;
        cx.update(|cx| match shots {
            Ok(shots) => crate::overlay::begin(shots, false, cx, |c: Captured, cx| {
                if let Some(r) = c.screen_rect {
                    begin_session(r, cx);
                }
            }),
            Err(_) => crate::hud::show_with(
                l("Couldn't capture the screen"),
                "exclamationmark.triangle.fill",
                cx,
            ),
        })
        .ok();
    })
    .detach();
}

fn begin_session(rect: Bounds<Pixels>, cx: &mut App) {
    let (x, y) = (
        f32::from(rect.origin.x) as f64,
        f32::from(rect.origin.y) as f64,
    );
    let (w, h) = (
        f32::from(rect.size.width) as f64,
        f32::from(rect.size.height) as f64,
    );
    let Ok((grabber, scale)) = RegionGrabber::new(x, y, w, h) else {
        crate::hud::show_with(
            l("Couldn't capture the screen"),
            "exclamationmark.triangle.fill",
            cx,
        );
        return;
    };
    let popup = |bounds: Bounds<Pixels>, cx: &App| crate::ui::popup_options(bounds, cx);
    let border_bounds = Bounds::new(
        rect.origin - point(px(3.), px(3.)),
        rect.size + size(px(6.), px(6.)),
    );
    let Ok(border) = cx.open_window(popup(border_bounds, cx), |window, cx| {
        crate::platform::set_window_level(window, 25);
        crate::platform::set_ignores_mouse(window, true);
        crate::platform::set_window_shadow(window, false);
        cx.new(|_| Border)
    }) else {
        return;
    };
    let visible = crate::platform::visible_frame_at_mouse();
    let panel = size(px(340.), px(48.));
    let mut origin = point(
        rect.origin.x + (rect.size.width - panel.width) / 2.,
        rect.origin.y + rect.size.height + px(12.),
    );
    if origin.y + panel.height > visible.origin.y + visible.size.height - px(8.) {
        origin.y = visible.origin.y + visible.size.height - panel.height - px(24.);
    }
    origin.x = origin
        .x
        .max(visible.origin.x + px(8.))
        .min(visible.origin.x + visible.size.width - panel.width - px(8.));
    let grabber = Arc::new(grabber);
    let Ok(controls) = cx.open_window(popup(Bounds::new(origin, panel), cx), move |window, cx| {
        crate::platform::set_window_level(window, 25);
        cx.new(|cx| Controls::new(grabber, scale, cx))
    }) else {
        border.update(cx, |_, w, _| w.remove_window()).ok();
        return;
    };
    cx.set_global(Current(Some((border, controls))));
}

fn finish(result: Option<(tiny_skia::Pixmap, f64)>, cx: &mut App) {
    if let Some((border, controls)) = cx.default_global::<Current>().0.take() {
        border.update(cx, |_, w, _| w.remove_window()).ok();
        controls.update(cx, |_, w, _| w.remove_window()).ok();
    }
    let Some((pm, scale)) = result else { return };
    let bgra: Vec<u8> = pm
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.blue(), c.green(), c.red(), c.alpha()]
        })
        .collect();
    crate::flow::handle(
        Captured {
            width: pm.width(),
            height: pm.height(),
            scale: scale as f32,
            bgra,
            screen_rect: None,
            window_id: None,
        },
        CaptureAction::Scrolling,
        cx,
    );
}

struct Border;

impl Render for Border {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let accent: gpui::Hsla = crate::platform::accent_color().into();
        div().size_full().child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    crate::ui::paint_dashed_frame(bounds, accent, window);
                },
            )
            .size_full(),
        )
    }
}

pub struct Controls {
    stitcher: Arc<Mutex<Stitcher>>,
    scale: f64,
    height_pt: u32,
    _timer: Task<()>,
}

impl Controls {
    fn new(grabber: Arc<RegionGrabber>, scale: f64, cx: &mut Context<Self>) -> Self {
        let stitcher = Arc::new(Mutex::new(Stitcher::default()));
        let st = stitcher.clone();
        let timer = cx.spawn(async move |this, cx: &mut AsyncApp| {
            loop {
                let (g, st) = (grabber.clone(), st.clone());
                let height = cx
                    .background_executor()
                    .spawn(async move {
                        let (w, h, bgra) = g.grab().ok()?;
                        let src = SourceImage::from_bgra(w, h, &bgra)?;
                        let mut s = st.lock().ok()?;
                        s.add(Frame::new(src.pixmap)).then(|| s.height())
                    })
                    .await;
                let alive = this
                    .update(cx, |c, cx| {
                        if let Some(h) = height {
                            c.height_pt = (h as f64 / c.scale) as u32;
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
            }
        });
        Self {
            stitcher,
            scale,
            height_pt: 0,
            _timer: timer,
        }
    }

    fn done(&mut self, cx: &mut Context<Self>) {
        let result = self
            .stitcher
            .lock()
            .ok()
            .and_then(|s| s.result())
            .map(|pm| (pm, self.scale));
        cx.defer(move |cx| finish(result, cx));
    }
}

impl Render for Controls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        let white = hsla(0., 0., 1., 1.);
        div().size_full().p(px(2.)).child(
            div()
                .size_full()
                .rounded_full()
                .bg(theme.panel)
                .border_1()
                .border_color(theme.border)
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(10.))
                .child(icon("arrow.down", 13., theme.secondary))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.text)
                                .child(l("Scroll down slowly")),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.secondary)
                                .child(lf("%d pt captured", self.height_pt)),
                        ),
                )
                .child(
                    div()
                        .id("cancel")
                        .cursor_pointer()
                        .text_size(px(13.))
                        .text_color(theme.secondary)
                        .child(l("Cancel"))
                        .on_click(cx.listener(|_, _, _, cx| cx.defer(|cx| finish(None, cx)))),
                )
                .child(
                    div()
                        .id("done")
                        .cursor_pointer()
                        .px(px(12.))
                        .py(px(5.))
                        .rounded_full()
                        .bg(theme.accent)
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(white)
                        .child(l("Done"))
                        .on_click(cx.listener(|c, _, _, cx| c.done(cx))),
                ),
        )
    }
}
