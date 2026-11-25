use std::time::Duration;

use gpui::{
    App, AsyncApp, Bounds, Context, Global, MouseButton, Task, Window, WindowHandle, div, hsla,
    point, prelude::*, px, size,
};

use crate::l10n::l;
use crate::ui::Theme;

#[derive(Default)]
struct Current(Option<WindowHandle<Countdown>>);

impl Global for Current {}

/// Self-timer: a large count in the middle of the screen. Clicking it cancels.
pub fn start(seconds: u32, done: impl FnOnce(&mut App) + 'static, cx: &mut App) {
    cancel(cx);
    let visible = crate::platform::visible_frame_at_mouse();
    let side = px(168.);
    let origin = point(
        visible.origin.x + (visible.size.width - side) / 2.,
        visible.origin.y + (visible.size.height - side) / 2.,
    );
    let options = crate::ui::popup_options(Bounds::new(origin, size(side, side)), cx);
    let mut done = Some(done);
    let handle = cx
        .open_window(options, |window, cx| {
            crate::platform::set_window_level(window, 25);
            crate::platform::set_window_shadow(window, false);
            cx.new(|cx| {
                let ticker = cx.spawn(async move |this, cx: &mut AsyncApp| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        let finished = this
                            .update(cx, |c: &mut Countdown, cx| {
                                if c.value > 1 {
                                    c.value -= 1;
                                    crate::feedback::play_tick();
                                    cx.notify();
                                    false
                                } else {
                                    true
                                }
                            })
                            .unwrap_or(true);
                        if finished {
                            break;
                        }
                    }
                    let finish = this
                        .update(cx, |c: &mut Countdown, _| c.done.take())
                        .ok()
                        .flatten();
                    cx.update(|cx| cancel(cx)).ok();
                    // Give the window server a frame to remove the panel before capturing.
                    cx.background_executor()
                        .timer(Duration::from_millis(120))
                        .await;
                    if let Some(f) = finish {
                        cx.update(f).ok();
                    }
                });
                Countdown {
                    value: seconds,
                    done: done
                        .take()
                        .map(|d| Box::new(d) as Box<dyn FnOnce(&mut App)>),
                    _ticker: ticker,
                }
            })
        })
        .ok();
    crate::feedback::play_tick();
    cx.set_global(Current(handle));
}

fn cancel(cx: &mut App) {
    if let Some(h) = cx.default_global::<Current>().0.take() {
        h.update(cx, |_, w, _| w.remove_window()).ok();
    }
}

pub struct Countdown {
    value: u32,
    done: Option<Box<dyn FnOnce(&mut App)>>,
    _ticker: Task<()>,
}

impl Render for Countdown {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("countdown")
                    .size(px(150.))
                    .rounded_full()
                    .bg(if theme.dark {
                        hsla(0., 0., 0.18, 0.92)
                    } else {
                        hsla(0., 0., 0.98, 0.92)
                    })
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(2.))
                    .cursor_pointer()
                    .child(
                        div()
                            .text_size(px(64.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child(self.value.to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.secondary)
                            .child(l("Click to cancel")),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|c, _, _, cx| {
                            c.done = None;
                            cx.defer(cancel);
                        }),
                    ),
            )
    }
}
