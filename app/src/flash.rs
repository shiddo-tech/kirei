use std::time::Duration;

use gpui::{App, AsyncApp, Bounds, Pixels, Window, div, hsla, prelude::*};

struct Flash;

impl Render for Flash {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().size_full().bg(hsla(0., 0., 1., 1.))
    }
}

/// A brief white flash over the captured region, like a camera shutter.
pub fn flash(rect: Bounds<Pixels>, cx: &mut App) {
    let options = crate::ui::popup_options(rect, cx);
    let Ok(handle) = cx.open_window(options, |window, cx| {
        crate::platform::set_window_level(window, 1000);
        crate::platform::set_ignores_mouse(window, true);
        crate::platform::set_window_shadow(window, false);
        crate::platform::set_window_alpha(window, 0.55);
        cx.new(|_| Flash)
    }) else {
        return;
    };
    cx.spawn(async move |cx: &mut AsyncApp| {
        const STEPS: u32 = 14;
        for i in 1..=STEPS {
            cx.background_executor()
                .timer(Duration::from_millis(20))
                .await;
            let t = i as f64 / STEPS as f64;
            let alpha = 0.55 * (1. - t) * (1. - t);
            handle
                .update(cx, |_, window, _| {
                    crate::platform::set_window_alpha(window, alpha)
                })
                .ok();
        }
        handle
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    })
    .detach();
}

struct Flying(std::sync::Arc<gpui::RenderImage>);

impl Render for Flying {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .rounded(gpui::px(10.))
            .overflow_hidden()
            .child(
                gpui::img(self.0.clone())
                    .size_full()
                    .object_fit(gpui::ObjectFit::Contain),
            )
    }
}

/// CSS-style cubic bezier from (0,0) to (1,1) through (x1,y1) and (x2,y2), evaluated at `t`.
fn bezier(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    let curve = |a: f64, b: f64, u: f64| {
        3. * a * u * (1. - u).powi(2) + 3. * b * u * u * (1. - u) + u.powi(3)
    };
    let (mut lo, mut hi) = (0., 1.);
    let mut u = t;
    for _ in 0..30 {
        let x = curve(x1, x2, u);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            lo = u
        } else {
            hi = u
        }
        u = (lo + hi) / 2.;
    }
    curve(y1, y2, u)
}

/// Flies the captured image from where it was on screen into `target`, with a little overshoot.
pub fn fly(
    image: std::sync::Arc<gpui::RenderImage>,
    from: Bounds<Pixels>,
    target: Bounds<Pixels>,
    done: impl FnOnce(&mut App) + 'static,
    cx: &mut App,
) {
    let Ok(handle) = cx.open_window(crate::ui::popup_options(from, cx), |window, cx| {
        crate::platform::set_window_level(window, 1000);
        crate::platform::set_ignores_mouse(window, true);
        crate::platform::set_window_shadow(window, true);
        cx.new(|_| Flying(image))
    }) else {
        return done(cx);
    };
    let (fw, fh) = (f32::from(from.size.width), f32::from(from.size.height));
    let s = (f32::from(target.size.width) / fw).min(f32::from(target.size.height) / fh);
    let end = Bounds::new(
        target.center() - gpui::point(gpui::px(fw * s / 2.), gpui::px(fh * s / 2.)),
        gpui::size(gpui::px(fw * s), gpui::px(fh * s)),
    );
    cx.spawn(async move |cx: &mut AsyncApp| {
        const STEPS: u32 = 25;
        let lerp = |a: Pixels, b: Pixels, k: f32| a + (b - a) * k;
        for i in 1..=STEPS {
            cx.background_executor()
                .timer(Duration::from_millis(420 / STEPS as u64))
                .await;
            let k = bezier(0.3, 1.25, 0.5, 1., i as f64 / STEPS as f64) as f32;
            let r = Bounds::new(
                gpui::point(
                    lerp(from.origin.x, end.origin.x, k),
                    lerp(from.origin.y, end.origin.y, k),
                ),
                gpui::size(
                    lerp(from.size.width, end.size.width, k),
                    lerp(from.size.height, end.size.height, k),
                ),
            );
            handle
                .update(cx, |_, window, _| {
                    crate::platform::set_window_frame(window, r)
                })
                .ok();
        }
        cx.update(done).ok();
        for i in 1..=8 {
            cx.background_executor()
                .timer(Duration::from_millis(22))
                .await;
            let a = 1. - i as f64 / 8.;
            handle
                .update(cx, |_, window, _| {
                    crate::platform::set_window_alpha(window, a)
                })
                .ok();
        }
        handle
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    })
    .detach();
}
