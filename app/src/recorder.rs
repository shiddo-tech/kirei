use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, App, AsyncApp, Bounds, Context, Global, MouseButton, MouseDownEvent,
    MouseMoveEvent, Pixels, Point, RenderImage, Task, Window, WindowHandle, WindowOptions, canvas,
    div, hsla, img, outline, point, prelude::*, px, size,
};
use smallvec::smallvec;

use crate::capture::{self, Captured, Recording};
use crate::l10n::{l, lf};
use crate::ui::{Theme, icon};
use crate::{export, feedback, hud, platform, settings};

const RED: gpui::Hsla = gpui::Hsla {
    h: 0.0,
    s: 0.86,
    l: 0.55,
    a: 1.,
};

#[derive(Default)]
struct State {
    recording: Option<Recording>,
    failed: Option<Arc<AtomicBool>>,
    ended: Option<Arc<AtomicBool>>,
    starting: bool,
    stopping: bool,
    border: Option<WindowHandle<Border>>,
    controls: Option<WindowHandle<Controls>>,
}

impl Global for State {}

pub fn is_recording(cx: &App) -> bool {
    cx.try_global::<State>()
        .is_some_and(|s| s.recording.is_some() || s.starting || s.stopping)
}

/// Record Screen action: starts picking an area, or stops the running recording.
pub fn toggle(cx: &mut App) {
    if is_recording(cx) {
        stop(cx);
    } else {
        start(cx);
    }
}

fn start(cx: &mut App) {
    if crate::overlay::is_active(cx) || crate::scrolling::is_active(cx) {
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
                    begin(r, c.window_id, cx);
                }
            }),
            Err(_) => hud::show_with(
                l("Couldn't capture the screen"),
                "exclamationmark.triangle.fill",
                cx,
            ),
        })
        .ok();
    })
    .detach();
}

fn begin(rect: Bounds<Pixels>, window_id: Option<u32>, cx: &mut App) {
    if is_recording(cx) {
        return;
    }
    let dir = std::env::temp_dir().join("KireiRecordings");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join(export::file_name("mp4"));
    cx.default_global::<State>().starting = true;
    show_chrome(rect, cx);
    let (x, y) = (
        f32::from(rect.origin.x) as f64,
        f32::from(rect.origin.y) as f64,
    );
    let (w, h) = (
        f32::from(rect.size.width) as f64,
        f32::from(rect.size.height) as f64,
    );
    cx.spawn(async move |cx: &mut AsyncApp| {
        let result = cx
            .background_executor()
            .spawn(async move {
                match window_id {
                    Some(id) => Recording::start_window(id, path),
                    None => Recording::start_area(x, y, w, h, path),
                }
            })
            .await;
        cx.update(|cx| {
            let state = cx.default_global::<State>();
            state.starting = false;
            match result {
                Ok(rec) => {
                    state.failed = Some(rec.failed.clone());
                    state.ended = Some(rec.ended.clone());
                    state.recording = Some(rec);
                    feedback::play_tick();
                }
                Err(e) => {
                    eprintln!("kirei: não começou a gravar: {e:#}");
                    teardown(cx);
                    hud::show_with(
                        l("Couldn't start recording"),
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

pub fn stop(cx: &mut App) {
    let state = cx.default_global::<State>();
    if state.stopping {
        return;
    }
    let Some(rec) = state.recording.take() else {
        return;
    };
    state.stopping = true;
    hide_chrome(cx);
    cx.spawn(async move |cx: &mut AsyncApp| {
        let failed = rec.failed.clone();
        let path = cx
            .background_executor()
            .spawn(async move { rec.stop() })
            .await;
        cx.update(|cx| {
            cx.default_global::<State>().stopping = false;
            teardown(cx);
            match path {
                Some(path) => {
                    feedback::play_capture();
                    preview::show(path, cx);
                }
                None if failed.load(Ordering::Relaxed) => hud::show_with(
                    l("Couldn't save the recording"),
                    "exclamationmark.triangle.fill",
                    cx,
                ),
                None => hud::show_with(l("Nothing was recorded"), "record.circle", cx),
            }
        })
        .ok();
    })
    .detach();
}

fn teardown(cx: &mut App) {
    hide_chrome(cx);
    let state = cx.default_global::<State>();
    state.recording = None;
    state.failed = None;
    state.ended = None;
}

fn popup(bounds: Bounds<Pixels>, cx: &App) -> WindowOptions {
    crate::ui::popup_options(bounds, cx)
}

/// Dev view (`--show recording-controls`): the on-screen chrome without recording anything.
pub fn show_chrome_preview(cx: &mut App) {
    let v = platform::visible_frame_at_mouse();
    let rect = Bounds::new(
        v.origin + point(px(120.), px(120.)),
        size(px(480.), px(300.)),
    );
    show_chrome(rect, cx);
}

/// A dashed frame around the recorded area and a small stop control under it. Kirei's own
/// windows are left out of the capture, so neither shows in the video.
fn show_chrome(rect: Bounds<Pixels>, cx: &mut App) {
    let border_bounds = Bounds::new(
        rect.origin - point(px(3.), px(3.)),
        rect.size + size(px(6.), px(6.)),
    );
    let border = cx
        .open_window(popup(border_bounds, cx), |window, cx| {
            platform::set_window_level(window, 25);
            platform::set_ignores_mouse(window, true);
            platform::set_window_shadow(window, false);
            cx.new(|_| Border)
        })
        .ok();
    let visible = platform::visible_frame_at_mouse();
    let panel = size(px(176.), px(44.));
    let (v_top, v_bottom) = (visible.origin.y, visible.origin.y + visible.size.height);
    let mut origin = point(
        rect.origin.x + (rect.size.width - panel.width) / 2.,
        rect.origin.y + rect.size.height + px(12.),
    );
    if origin.y + panel.height > v_bottom - px(8.) {
        origin.y = (rect.origin.y - px(12.) - panel.height).max(v_top + px(8.));
    }
    if rect.size.height > visible.size.height - px(120.) {
        origin.y = v_bottom - px(24.) - panel.height;
    }
    origin.x = origin
        .x
        .min(visible.origin.x + visible.size.width - panel.width - px(8.))
        .max(visible.origin.x + px(8.));
    let mut options = popup(Bounds::new(origin, panel), cx);
    options.is_movable = true;
    let controls = cx
        .open_window(options, |window, cx| {
            platform::set_window_level(window, 25);
            platform::set_window_shadow(window, true);
            cx.new(Controls::new)
        })
        .ok();
    let state = cx.default_global::<State>();
    state.border = border;
    state.controls = controls;
}

fn hide_chrome(cx: &mut App) {
    let state = cx.default_global::<State>();
    let (border, controls) = (state.border.take(), state.controls.take());
    if let Some(b) = border {
        b.update(cx, |_, w, _| w.remove_window()).ok();
    }
    if let Some(c) = controls {
        c.update(cx, |_, w, _| w.remove_window()).ok();
    }
}

struct Border;

impl Render for Border {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let color = RED.opacity(0.85);
        div().size_full().child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    window.paint_quad(
                        outline(bounds, color, gpui::BorderStyle::Dashed)
                            .corner_radii(px(4.))
                            .border_widths(px(2.)),
                    );
                },
            )
            .size_full(),
        )
    }
}

struct Controls {
    start: Instant,
    _tick: Task<()>,
}

impl Controls {
    fn new(cx: &mut Context<Self>) -> Self {
        let tick = cx.spawn(async move |this, cx: &mut AsyncApp| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let alive = this
                    .update(cx, |_, cx| {
                        let over = cx.try_global::<State>().is_some_and(|s| {
                            [&s.failed, &s.ended]
                                .into_iter()
                                .flatten()
                                .any(|f| f.load(Ordering::Relaxed))
                        });
                        if over {
                            cx.defer(stop);
                        }
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        });
        Self {
            start: Instant::now(),
            _tick: tick,
        }
    }
}

impl Render for Controls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        let white = hsla(0., 0., 1., 1.);
        let s = self.start.elapsed().as_secs();
        div().size_full().p(px(2.)).child(
            div()
                .id("controls")
                .size_full()
                .rounded_full()
                .bg(theme.panel)
                .border_1()
                .border_color(theme.border)
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(10.))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                .child(
                    div().size(px(9.)).rounded_full().bg(RED).with_animation(
                        "pulse",
                        Animation::new(Duration::from_millis(1600))
                            .repeat()
                            .with_easing(gpui::pulsating_between(0.35, 1.)),
                        |dot, t| dot.opacity(t),
                    ),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child(format!("{}:{:02}", s / 60, s % 60)),
                )
                .child(
                    div()
                        .id("stop")
                        .cursor_pointer()
                        .px(px(10.))
                        .py(px(5.))
                        .rounded_full()
                        .bg(RED)
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(white)
                        .child(icon("stop.fill", 11., white))
                        .child(l("Stop"))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|_, _, _, cx| cx.defer(stop))),
                ),
        )
    }
}

pub mod preview {
    use super::*;

    #[derive(Default)]
    struct Current(Option<WindowHandle<RecordingPreview>>);

    impl Global for Current {}

    /// After recording: a card in the screen corner to save the MP4, make a GIF or copy the file.
    pub fn show(path: PathBuf, cx: &mut App) {
        close(cx);
        let (w, h) = (300., 220.);
        let visible = platform::visible_frame_at_mouse();
        let origin = point(
            visible.origin.x + visible.size.width - px(w + 20.),
            visible.origin.y + visible.size.height - px(h + 20.),
        );
        let options = popup(Bounds::new(origin, size(px(w), px(h))), cx);
        let handle = cx
            .open_window(options, |window, cx| {
                platform::set_window_level(window, 25);
                platform::set_window_shadow(window, true);
                platform::set_window_alpha(window, 0.);
                cx.new(|cx| RecordingPreview::new(path, cx))
            })
            .ok();
        if let Some(h) = handle {
            fade(h, 0., 1., 250, false, cx);
        }
        cx.set_global(Current(handle));
    }

    pub fn close(cx: &mut App) {
        if let Some(h) = cx.default_global::<Current>().0.take() {
            fade(h, 1., 0., 200, true, cx);
        }
    }

    fn fade(
        h: WindowHandle<RecordingPreview>,
        from: f64,
        to: f64,
        ms: u64,
        remove: bool,
        cx: &mut App,
    ) {
        cx.spawn(async move |cx: &mut AsyncApp| {
            const STEPS: u64 = 12;
            for i in 1..=STEPS {
                cx.background_executor()
                    .timer(Duration::from_millis(ms / STEPS))
                    .await;
                let a = from + (to - from) * i as f64 / STEPS as f64;
                h.update(cx, |_, w, _| platform::set_window_alpha(w, a))
                    .ok();
            }
            if remove {
                h.update(cx, |_, w, _| w.remove_window()).ok();
            }
        })
        .detach();
    }

    pub struct RecordingPreview {
        path: PathBuf,
        poster: Option<Arc<RenderImage>>,
        working: bool,
        press: Option<Point<Pixels>>,
        _load: Task<()>,
    }

    impl RecordingPreview {
        fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
            let p = path.clone();
            let load = cx.spawn(async move |this, cx: &mut AsyncApp| {
                let poster = cx
                    .background_executor()
                    .spawn(async move { platform::video_poster(&p) })
                    .await;
                let Some(mut poster) = poster else { return };
                for px in poster.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                let image = Arc::new(RenderImage::new(smallvec![image::Frame::new(poster)]));
                this.update(cx, |t, cx| {
                    t.poster = Some(image);
                    cx.notify();
                })
                .ok();
            });
            Self {
                path,
                poster: None,
                working: false,
                press: None,
                _load: load,
            }
        }

        fn save(file: &Path, ext: &str, cx: &mut App) {
            let folder = settings::get().save_folder();
            let dest = export::unique_path(&folder, &export::file_name(ext));
            match std::fs::copy(file, &dest) {
                Ok(_) => {
                    hud::show_with(
                        lf(
                            "Saved to %@",
                            folder
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default(),
                        ),
                        "folder.fill",
                        cx,
                    );
                    close(cx);
                }
                Err(_) => hud::show_with(l("Couldn't save"), "exclamationmark.triangle.fill", cx),
            }
        }

        fn save_mp4(&mut self, _: &mut Window, cx: &mut Context<Self>) {
            let path = self.path.clone();
            cx.defer(move |cx| Self::save(&path, "mp4", cx));
        }

        fn save_gif(&mut self, _: &mut Window, cx: &mut Context<Self>) {
            self.working = true;
            cx.notify();
            let path = self.path.clone();
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let gif = cx
                    .background_executor()
                    .spawn(async move { platform::video_to_gif(&path, 12., 960.) })
                    .await;
                this.update(cx, |t, cx| {
                    t.working = false;
                    cx.notify();
                })
                .ok();
                cx.update(|cx| match gif {
                    Some(gif) => Self::save(&gif, "gif", cx),
                    None => hud::show_with(
                        l("Couldn't create the GIF"),
                        "exclamationmark.triangle.fill",
                        cx,
                    ),
                })
                .ok();
            })
            .detach();
        }

        fn copy(&mut self, _: &mut Window, cx: &mut Context<Self>) {
            if platform::copy_file(&self.path) {
                feedback::play_copy();
                hud::show(l("Video copied"), cx);
            }
        }
    }

    fn button(
        id: &'static str,
        title: String,
        symbol: &'static str,
        theme: &Theme,
        enabled: bool,
        cx: &mut Context<RecordingPreview>,
        f: fn(&mut RecordingPreview, &mut Window, &mut Context<RecordingPreview>),
    ) -> impl IntoElement {
        let mut b = div()
            .id(id)
            .px(px(8.))
            .py(px(5.))
            .rounded_full()
            .bg(theme.text.opacity(0.08))
            .flex()
            .items_center()
            .gap(px(4.))
            .text_size(px(11.))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.text)
            .whitespace_nowrap()
            .child(icon(symbol, 11., theme.text))
            .child(title);
        if enabled {
            b = b
                .cursor_pointer()
                .on_click(cx.listener(move |t, _, w, cx| f(t, w, cx)));
        } else {
            b = b.opacity(0.4);
        }
        b
    }

    impl Render for RecordingPreview {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let theme = Theme::get(window);
            let white = hsla(0., 0., 1., 0.9);
            let mut poster = div()
                .id("poster")
                .relative()
                .flex_1()
                .w_full()
                .min_h_0()
                .rounded(px(8.))
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|t, ev: &MouseDownEvent, _, _| {
                        if ev.click_count >= 2 {
                            platform::open_file(&t.path);
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
                        platform::drag_file(w, &t.path);
                    }
                }));
            poster = match self.poster.clone() {
                Some(image) => poster.child(img(image).max_w_full().max_h_full().rounded(px(8.))),
                None => poster.child(icon("film", 28., theme.secondary)),
            };
            poster = poster.child(
                div()
                    .id("close")
                    .absolute()
                    .top(px(6.))
                    .left(px(6.))
                    .size(px(20.))
                    .rounded_full()
                    .bg(hsla(0., 0., 0.15, 0.85))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(icon("xmark", 9., white))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|_, _, _, cx| cx.defer(close))),
            );
            let enabled = !self.working;
            let mut buttons = div()
                .relative()
                .flex()
                .gap(px(6.))
                .child(button(
                    "mp4",
                    l("Save MP4"),
                    "square.and.arrow.down",
                    &theme,
                    enabled,
                    cx,
                    Self::save_mp4,
                ))
                .child(button(
                    "gif",
                    l("Save GIF"),
                    "photo.stack",
                    &theme,
                    enabled,
                    cx,
                    Self::save_gif,
                ))
                .child(button(
                    "copy",
                    l("Copy"),
                    "doc.on.doc",
                    &theme,
                    enabled,
                    cx,
                    Self::copy,
                ));
            if self.working {
                buttons = buttons.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            icon("hourglass", 13., theme.text).with_animation(
                                "working",
                                Animation::new(Duration::from_millis(1000))
                                    .repeat()
                                    .with_easing(gpui::pulsating_between(0.3, 1.)),
                                |e, t| e.opacity(t),
                            ),
                        ),
                );
            }
            div().size_full().p(px(4.)).child(
                div()
                    .size_full()
                    .rounded(px(16.))
                    .bg(theme.panel)
                    .border_1()
                    .border_color(theme.border)
                    .p(px(10.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.))
                    .child(poster)
                    .child(buttons),
            )
        }
    }
}
