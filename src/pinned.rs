use std::sync::Arc;

use gpui::{
    App, Bounds, Context, FocusHandle, Global, KeyDownEvent, MouseButton, MouseDownEvent,
    RenderImage, ScrollWheelEvent, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, div, hsla, img, point, prelude::*, px, size,
};
use tiny_skia::Pixmap;

use crate::l10n::l;
use crate::ui::controls::{MenuItem, menu};
use crate::ui::{Theme, icon};

#[derive(Default)]
struct Pinned(Vec<WindowHandle<PinnedImage>>);

impl Global for Pinned {}

pub fn count(cx: &App) -> usize {
    cx.try_global::<Pinned>().map(|p| p.0.len()).unwrap_or(0)
}

pub fn close_all(cx: &mut App) {
    let windows = std::mem::take(&mut cx.default_global::<Pinned>().0);
    for w in windows {
        w.update(cx, |_, window, _| window.remove_window()).ok();
    }
}

/// An image floating above every window, as a reference while you work. Drag to move, scroll to
/// resize, double-click to edit, right-click for options.
pub fn pin(pm: Pixmap, scale: f64, cx: &mut App) {
    let natural = (pm.width() as f64 / scale, pm.height() as f64 / scale);
    let display = cx.primary_display();
    let visible = display.as_ref().map(|d| d.bounds()).unwrap_or(Bounds::new(
        point(px(0.), px(0.)),
        size(px(1440.), px(900.)),
    ));
    let vw = f32::from(visible.size.width) as f64;
    let vh = f32::from(visible.size.height) as f64;
    let fit = (vw * 0.5 / natural.0).min(vh * 0.5 / natural.1).min(1.);
    let window_size = size(px((natural.0 * fit) as f32), px((natural.1 * fit) as f32));
    let image = crate::ui::render_image(&pm);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            window_size,
            cx,
        ))),
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: true,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let aspect = natural.0 / natural.1.max(1.);
    let pm = Arc::new(pm);
    let handle = cx.open_window(options, move |window, cx| {
        crate::platform::set_window_level(window, 3);
        crate::platform::set_window_shadow(window, true);
        cx.new(|cx| {
            let focus = cx.focus_handle();
            window.focus(&focus);
            PinnedImage {
                image,
                pixmap: pm,
                scale,
                aspect,
                natural_width: natural.0,
                focus,
                hover: false,
                menu_at: None,
                opacity: 1.,
            }
        })
    });
    if let Ok(h) = handle {
        cx.default_global::<Pinned>().0.push(h);
    }
}

pub struct PinnedImage {
    image: Arc<RenderImage>,
    pixmap: Arc<Pixmap>,
    scale: f64,
    aspect: f64,
    natural_width: f64,
    focus: FocusHandle,
    hover: bool,
    menu_at: Option<gpui::Point<gpui::Pixels>>,
    opacity: f64,
}

impl PinnedImage {
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let me = window.window_handle();
        cx.default_global::<Pinned>()
            .0
            .retain(|h| AnyHandle::from(*h) != me);
        window.remove_window();
    }

    fn resize_by(&mut self, factor: f64, window: &mut Window) {
        let b = window.bounds();
        let w = (f32::from(b.size.width) as f64 * factor).clamp(60., 4000.);
        let h = w / self.aspect;
        crate::platform::set_window_frame_centered(window, w, h);
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let retina = crate::settings::get().export_retina;
        if crate::platform::copy_image(&self.pixmap, self.scale, retina).is_ok() {
            crate::feedback::play_copy();
            crate::hud::show(l("Image copied"), cx);
        }
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let src = crate::render::SourceImage {
            pixmap: (*self.pixmap).clone(),
        };
        crate::editor::open_source(Arc::new(src), self.scale, cx);
        self.close(window, cx);
    }

    fn menu_items(&self, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let e = cx.entity();
        let act = |f: fn(&mut PinnedImage, &mut Window, &mut Context<PinnedImage>)| {
            let e = e.clone();
            move |w: &mut Window, cx: &mut App| e.update(cx, |t, cx| f(t, w, cx))
        };
        let mut items = vec![
            MenuItem::new(l("Copy"), act(|t, _, cx| t.copy(cx))),
            MenuItem::new(l("Edit"), act(|t, w, cx| t.edit(w, cx))),
            MenuItem::new(
                l("Actual Size"),
                act(|t, w, _| {
                    let current = f32::from(w.bounds().size.width) as f64;
                    t.resize_by(t.natural_width / current.max(1.), w)
                }),
            ),
            MenuItem::separator(),
        ];
        for p in [100, 75, 50, 25] {
            let e = e.clone();
            items.push(
                MenuItem::new(format!("{} {p}%", l("Opacity")), move |w, cx| {
                    e.update(cx, |t, _| {
                        t.opacity = p as f64 / 100.;
                        crate::platform::set_window_alpha(w, t.opacity);
                    })
                })
                .checked((self.opacity * 100. - p as f64).abs() < 1.),
            );
        }
        items.push(MenuItem::new(
            l("Click Through"),
            act(|_, w, cx| {
                crate::platform::set_ignores_mouse(w, true);
                crate::hud::show(l("Click-through on. Close it from the menu bar."), cx);
            }),
        ));
        items.push(MenuItem::separator());
        items.push(MenuItem::new(l("Close"), act(|t, w, cx| t.close(w, cx))));
        items
    }
}

type AnyHandle = gpui::AnyWindowHandle;

impl Render for PinnedImage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        let menu_view = self.menu_at.map(|at| {
            let e = cx.entity();
            menu(at, self.menu_items(cx), &theme, move |_, cx| {
                e.update(cx, |t, cx| {
                    t.menu_at = None;
                    cx.notify()
                })
            })
        });
        div()
            .id("pinned")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .rounded(px(8.))
            .overflow_hidden()
            .border_1()
            .border_color(hsla(0., 0., 1., if self.hover { 0.35 } else { 0.12 }))
            .on_hover(cx.listener(|t, h: &bool, _, cx| {
                t.hover = *h;
                cx.notify()
            }))
            .child(img(self.image.clone()).size_full())
            .when(self.hover, |d| {
                d.child(
                    div()
                        .id("close")
                        .absolute()
                        .top(px(8.))
                        .left(px(8.))
                        .size(px(20.))
                        .rounded_full()
                        .bg(hsla(0., 0., 0.15, 0.8))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(icon("xmark", 9., hsla(0., 0., 1., 0.9)))
                        .on_click(cx.listener(|t, _, w, cx| t.close(w, cx))),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|t, ev: &MouseDownEvent, w, cx| {
                    if ev.click_count >= 2 {
                        t.edit(w, cx);
                    } else {
                        w.start_window_move();
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|t, ev: &MouseDownEvent, _, cx| {
                    t.menu_at = Some(ev.position);
                    cx.notify();
                }),
            )
            .on_scroll_wheel(cx.listener(|t, ev: &ScrollWheelEvent, w, _| {
                let d = ev.delta.pixel_delta(px(6.));
                t.resize_by(1. + f32::from(d.y) as f64 * 0.01, w);
            }))
            .on_key_down(cx.listener(|t, ev: &KeyDownEvent, w, cx| {
                let k = &ev.keystroke;
                if k.key == "escape" {
                    t.close(w, cx);
                } else if k.key == "c" && k.modifiers.platform {
                    t.copy(cx);
                }
            }))
            .when_some(menu_view, |d, m| d.child(m))
    }
}
