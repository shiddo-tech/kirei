use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, Div, FocusHandle, FontWeight, Global, Hsla,
    KeyDownEvent, SharedString, Stateful, TitlebarOptions, Window, WindowBounds, WindowHandle,
    WindowOptions, div, hsla, img, linear_color_stop, linear_gradient, point, prelude::*, px, rgb,
    rgba, size,
};

use crate::l10n::l;
use crate::settings::{self, CaptureAction};
use crate::{brand, platform};

/// Dev view (`--show welcome-denied`): the window as a user without the grant sees it.
pub static FORCE_DENIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn granted() -> bool {
    !FORCE_DENIED.load(std::sync::atomic::Ordering::Relaxed) && platform::screen_capture_allowed()
}

const WIDTH: f32 = 460.;
const HEIGHT: f32 = 586.;
const HERO: f32 = 232.;
const PERMISSION_CARD: f32 = 148.;

fn window_height(granted: bool) -> f32 {
    if granted {
        HEIGHT
    } else {
        HEIGHT + PERMISSION_CARD
    }
}

/// One screen shows everything; the step only tells callers what the user came for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Welcome,
    Permission,
    Shortcuts,
}

#[derive(Default)]
struct WelcomeWindow(Option<WindowHandle<Welcome>>);

impl Global for WelcomeWindow {}

fn marker_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/Kirei/welcomed"))
}

pub fn open_on_first_launch(cx: &mut App) {
    let Some(marker) = marker_path() else {
        return;
    };
    if marker.exists() && platform::screen_capture_allowed() {
        return;
    }
    if let Some(dir) = marker.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    std::fs::write(&marker, b"").ok();
    open(cx);
}

pub fn is(window: &gpui::AnyWindowHandle) -> bool {
    window.downcast::<Welcome>().is_some()
}

pub fn open(cx: &mut App) {
    open_at(Step::Welcome, None, cx);
}

/// `try_anyway` is the capture that asked for the permission; macOS may report a new grant only
/// after a relaunch, so the user can still try it from the permission card.
pub fn open_at(_step: Step, try_anyway: Option<CaptureAction>, cx: &mut App) {
    if let Some(handle) = cx.default_global::<WelcomeWindow>().0
        && handle
            .update(cx, |w, window, cx| {
                w.try_anyway = try_anyway;
                cx.notify();
                window.activate_window();
            })
            .is_ok()
    {
        crate::activate(cx);
        return;
    }

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(WIDTH), px(window_height(granted()))),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(l("Welcome to Kirei").into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        focus: !crate::dev_mode(),
        is_resizable: false,
        is_minimizable: false,
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |window, cx| {
            cx.new(|cx| {
                let welcome = Welcome::new(try_anyway, cx);
                window.focus(&welcome.focus);
                welcome
            })
        })
        .ok();
    cx.set_global(WelcomeWindow(handle));
    crate::activate(cx);
}

struct Welcome {
    granted: bool,
    try_anyway: Option<CaptureAction>,
    focus: FocusHandle,
}

impl Welcome {
    fn new(try_anyway: Option<CaptureAction>, cx: &mut Context<Self>) -> Self {
        // macOS often reports a new grant only after a relaunch, so polling is a bonus.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let granted = granted();
                let alive = this.update(cx, |this, cx| {
                    if this.granted != granted {
                        this.granted = granted;
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            granted: granted(),
            try_anyway,
            focus: cx.focus_handle(),
        }
    }
}

pub(crate) fn white(alpha: f32) -> Hsla {
    hsla(0., 0., 1., alpha)
}

pub(crate) fn shadow(color: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(0.),
    }
}

pub(crate) fn corner(top: bool, left: bool) -> Div {
    const LEN: f32 = 18.;
    const W: f32 = 2.5;
    let line = white(0.92);
    let mut c = div().absolute().w(px(LEN)).h(px(LEN));
    c = if top { c.top(px(-W)) } else { c.bottom(px(-W)) };
    c = if left {
        c.left(px(-W))
    } else {
        c.right(px(-W))
    };
    let horizontal = div().absolute().w(px(LEN)).h(px(W)).bg(line).rounded_full();
    let vertical = div().absolute().w(px(W)).h(px(LEN)).bg(line).rounded_full();
    c.child(if top {
        horizontal.top_0()
    } else {
        horizontal.bottom_0()
    })
    .child(if left {
        vertical.left_0()
    } else {
        vertical.right_0()
    })
}

fn hero() -> impl IntoElement {
    let frame_w = 196.;
    let frame_h = 136.;
    div()
        .relative()
        .flex_none()
        .w_full()
        .h(px(HERO))
        .overflow_hidden()
        .bg(linear_gradient(
            155.,
            linear_color_stop(rgb(0x6cc095), 0.),
            linear_color_stop(rgb(0x1d4a33), 1.),
        ))
        .child(
            div()
                .absolute()
                .top(px(-120.))
                .left(px(-80.))
                .size(px(320.))
                .rounded_full()
                .bg(white(0.07)),
        )
        .child(
            div()
                .absolute()
                .bottom(px(-150.))
                .right(px(-90.))
                .size(px(300.))
                .rounded_full()
                .bg(hsla(0., 0., 0., 0.16)),
        )
        .child(
            div()
                .absolute()
                .bottom_0()
                .left_0()
                .w_full()
                .h(px(72.))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(rgba(0x0e100f00), 0.),
                    linear_color_stop(rgb(0x0e100f), 1.),
                )),
        )
        .child(
            div()
                .absolute()
                .top(px((HERO - frame_h) / 2. + 6.))
                .left(px((WIDTH - frame_w) / 2.))
                .w(px(frame_w))
                .h(px(frame_h))
                .rounded(px(3.))
                .bg(white(0.07))
                .border_1()
                .border_color(white(0.22))
                .flex()
                .items_center()
                .justify_center()
                .child(corner(true, true))
                .child(corner(true, false))
                .child(corner(false, true))
                .child(corner(false, false))
                .child(
                    div()
                        .size(px(92.))
                        .shadow(vec![
                            shadow(hsla(0.42, 0.6, 0.08, 0.55), 18., 34.),
                            shadow(hsla(0., 0., 0., 0.25), 4., 10.),
                        ])
                        .rounded(px(22.))
                        .child(img(brand::app_icon()).size_full()),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(-2.))
                        .top(px(-27.))
                        .px(px(7.))
                        .py(px(2.))
                        .rounded(px(5.))
                        .bg(hsla(0., 0., 0., 0.55))
                        .text_size(px(11.))
                        .text_color(white(0.9))
                        .child("1024 × 1024"),
                ),
        )
}

pub(crate) fn keycap(key: String) -> impl IntoElement {
    div()
        .min_w(px(26.))
        .h(px(26.))
        .px(px(7.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .bg(linear_gradient(
            180.,
            linear_color_stop(rgb(0x2c302d), 0.),
            linear_color_stop(rgb(0x202321), 1.),
        ))
        .border_1()
        .border_color(rgb(0x3a3f3b))
        .shadow(vec![shadow(hsla(0., 0., 0., 0.6), 1.5, 0.)])
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(0xeef2ef))
        .child(key)
}

/// "⌥⇧1" → ["⌥", "⇧", "1"]; modifiers get one cap each, the key keeps its whole name.
pub(crate) fn keycaps(display: &str) -> Vec<String> {
    const MODIFIERS: [char; 5] = ['⌃', '⌥', '⇧', '⌘', '⇪'];
    let mut caps: Vec<String> = Vec::new();
    let mut rest = String::new();
    for c in display.chars() {
        if MODIFIERS.contains(&c) {
            caps.push(c.to_string());
        } else {
            rest.push(c);
        }
    }
    let rest = rest.trim();
    if !rest.is_empty() {
        caps.push(rest.to_string());
    }
    caps
}

fn hot_key_caps(action: CaptureAction) -> Vec<String> {
    settings::get()
        .hot_key(action)
        .filter(|hk| !hk.is_cleared())
        .map(|hk| keycaps(&hk.display))
        .unwrap_or_else(|| vec!["—".into()])
}

fn area_glyph() -> impl IntoElement {
    let line = brand::green_light();
    let seg = |w: f32, h: f32| div().absolute().w(px(w)).h(px(h)).bg(line).rounded_full();
    div()
        .relative()
        .w(px(16.))
        .h(px(12.))
        .child(seg(6., 1.6).top_0().left_0())
        .child(seg(1.6, 5.).top_0().left_0())
        .child(seg(6., 1.6).bottom_0().right_0())
        .child(seg(1.6, 5.).bottom_0().right_0())
}

fn screen_glyph() -> impl IntoElement {
    div()
        .w(px(16.))
        .h(px(12.))
        .rounded(px(2.5))
        .border(px(1.6))
        .border_color(brand::green_light())
}

fn shortcut_row(
    glyph: AnyElement,
    title: String,
    detail: String,
    keys: Vec<String>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .px_4()
        .py(px(13.))
        .child(
            div()
                .flex_none()
                .size(px(34.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(9.))
                .bg(hsla(0.41, 0.4, 0.5, 0.12))
                .border_1()
                .border_color(hsla(0.41, 0.4, 0.6, 0.18))
                .child(glyph),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(0xeef2ef))
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x7d857f))
                        .child(detail),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .gap(px(5.))
                .children(keys.into_iter().map(keycap)),
        )
}

pub(crate) fn button(
    id: &'static str,
    label: impl Into<SharedString>,
    primary: bool,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .px(px(18.))
        .h(px(36.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(10.))
        .cursor_pointer()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label.into());
    if primary {
        base.bg(linear_gradient(
            180.,
            linear_color_stop(rgb(0x4fa77a), 0.),
            linear_color_stop(brand::green(), 1.),
        ))
        .border_1()
        .border_color(rgb(0x5cb487))
        .shadow(vec![
            shadow(hsla(0.41, 0.5, 0.35, 0.45), 6., 18.),
            shadow(hsla(0., 0., 0., 0.4), 1., 2.),
        ])
        .text_color(rgb(0xffffff))
        .hover(|s| s.opacity(0.92))
    } else {
        base.bg(rgb(0x1c1f1d))
            .border_1()
            .border_color(rgb(0x30342f))
            .text_color(rgb(0xdfe5e0))
            .hover(|s| s.bg(rgb(0x252926)))
    }
}

pub(crate) fn status_dot(color: Hsla) -> impl IntoElement {
    div()
        .size(px(8.))
        .rounded_full()
        .bg(color)
        .shadow(vec![BoxShadow {
            color: color.opacity(0.7),
            offset: point(px(0.), px(0.)),
            blur_radius: px(8.),
            spread_radius: px(1.),
        }])
}

impl Welcome {
    fn permission(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.granted {
            return div()
                .flex()
                .items_center()
                .gap(px(9.))
                .child(status_dot(hsla(0.40, 0.62, 0.62, 1.)))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(0x9aa39c))
                        .child(l("Screen Recording is on")),
                )
                .into_any_element();
        }
        let amber = hsla(0.11, 0.9, 0.6, 1.);
        let actions = div()
            .flex()
            .gap_2()
            .child(
                button("settings", l("Open Settings"), false).on_click(|_, _, cx| {
                    platform::request_screen_capture();
                    cx.open_url(platform::SCREEN_CAPTURE_SETTINGS_URL);
                }),
            )
            .child(
                button("restart", l("Restart Kirei"), false)
                    .on_click(|_, _, cx| platform::relaunch(cx)),
            );
        let try_anyway = self.try_anyway.map(|action| {
            div()
                .id("try")
                .cursor_pointer()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0xc9b88f))
                .hover(|s| s.text_color(rgb(0xf5e3c0)))
                .child(l("Try Anyway"))
                .on_click(cx.listener(move |_, _, window, cx| {
                    window.remove_window();
                    cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(300))
                            .await;
                        cx.update(|cx| crate::flow::perform_unchecked(action, cx))
                            .ok();
                    })
                    .detach();
                }))
        });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded(px(12.))
            .bg(hsla(0.11, 0.9, 0.5, 0.07))
            .border_1()
            .border_color(hsla(0.11, 0.9, 0.6, 0.22))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .child(status_dot(amber))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(0xf5e3c0))
                            .child(l("Allow Screen Recording")),
                    )
                    .child(div().flex_1())
                    .children(try_anyway),
            )
            .child(
                div()
                    .w_full()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(0xa8a091))
                    .child(l("Kirei needs this permission to capture the screen. Nothing leaves your Mac. After turning it on, restart Kirei.")),
            )
            .child(actions)
            .into_any_element()
    }
}

impl Render for Welcome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let wanted = px(window_height(self.granted));
        if window.bounds().size.height != wanted {
            window.resize(size(px(WIDTH), wanted));
        }
        let shortcuts = div()
            .flex_none()
            .flex()
            .flex_col()
            .rounded(px(14.))
            .bg(rgb(0x151816))
            .border_1()
            .border_color(rgb(0x232724))
            .shadow(vec![shadow(hsla(0., 0., 0., 0.35), 8., 24.)])
            .child(shortcut_row(
                area_glyph().into_any_element(),
                CaptureAction::Area.title(),
                l("Drag to choose. Esc cancels."),
                hot_key_caps(CaptureAction::Area),
            ))
            .child(div().mx_4().h(px(1.)).bg(rgb(0x222623)))
            .child(shortcut_row(
                screen_glyph().into_any_element(),
                CaptureAction::Fullscreen.title(),
                l("The main screen, in one press."),
                hot_key_caps(CaptureAction::Fullscreen),
            ));

        let granted = self.granted;
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .child(if granted {
                self.permission(cx)
            } else {
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0x6f7771))
                    .child(l("Lives in the menu bar"))
                    .into_any_element()
            })
            .child(
                button("continue", l("Get Started"), true)
                    .on_click(|_, window, _| window.remove_window()),
            );

        div()
            .id("welcome")
            .track_focus(&self.focus)
            .on_key_down(|ev: &KeyDownEvent, window, _| {
                if matches!(ev.keystroke.key.as_str(), "escape" | "enter") {
                    window.remove_window();
                }
            })
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0e100f))
            .child(hero())
            .child(
                div()
                    .flex_none()
                    .h(px(window_height(granted) - HERO))
                    .flex()
                    .flex_col()
                    .px(px(32.))
                    .pb(px(26.))
                    .gap(px(22.))
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(brand::green_light())
                                    .child("綺麗")
                                    .child(div().text_color(rgb(0x4d5650)).child("·"))
                                    .child(l("beautiful, in Japanese")),
                            )
                            .child(
                                div()
                                    .text_size(px(28.))
                                    .line_height(px(34.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xf4f7f5))
                                    .child(l("Welcome to Kirei")),
                            )
                            .child(
                                div()
                                    .w(px(330.))
                                    .text_size(px(14.))
                                    .line_height(px(20.))
                                    .text_color(rgb(0x8f9891))
                                    .child(l(
                                        "Beautiful, share-ready screenshots in one shortcut.",
                                    )),
                            ),
                    )
                    .child(shortcuts)
                    .when(!granted, |el| el.child(self.permission(cx)))
                    .child(footer),
            )
    }
}
