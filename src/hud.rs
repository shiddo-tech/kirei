use std::time::Duration;

use gpui::{
    App, Bounds, Context, Global, Task, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, div, point, prelude::*, px, size,
};

use crate::ui::{Theme, icon};

#[derive(Default)]
struct Current(Option<WindowHandle<Hud>>, Option<Task<()>>);

impl Global for Current {}

/// A short message in a capsule near the bottom of the active screen.
pub fn show(message: impl Into<String>, cx: &mut App) {
    show_with(message, "checkmark.circle.fill", cx)
}

pub fn show_with(message: impl Into<String>, symbol: &'static str, cx: &mut App) {
    let message = message.into();
    if let Some(old) = cx.default_global::<Current>().0.take() {
        old.update(cx, |_, w, _| w.remove_window()).ok();
    }
    let width = 60. + message.chars().count() as f32 * 7.6;
    let display = cx.primary_display();
    let Some(display) = display else { return };
    let visible = display.bounds();
    let origin = point(
        visible.origin.x + (visible.size.width - px(width)) / 2.,
        visible.origin.y + visible.size.height - px(80. + 46.),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            origin,
            size(px(width), px(46.)),
        ))),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        display_id: Some(display.id()),
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |window, cx| {
            crate::platform::set_ignores_mouse(window, true);
            crate::platform::set_window_shadow(window, false);
            cx.new(|_| Hud { message, symbol })
        })
        .ok();
    let task = cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(1600))
            .await;
        cx.update(|cx| {
            if let Some(h) = cx.default_global::<Current>().0.take() {
                h.update(cx, |_, w, _| w.remove_window()).ok();
            }
        })
        .ok();
    });
    cx.set_global(Current(handle, Some(task)));
}

struct Hud {
    message: String,
    symbol: &'static str,
}

impl Render for Hud {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        div().size_full().p(px(3.)).child(
            div()
                .size_full()
                .rounded_full()
                .bg(theme.panel)
                .border_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .text_size(px(14.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text)
                .child(icon(self.symbol, 17., theme.text))
                .child(self.message.clone()),
        )
    }
}
