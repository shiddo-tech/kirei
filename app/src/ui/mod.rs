pub mod controls;
pub mod icons;
pub mod text_input;

use std::sync::Arc;

use gpui::{
    AnyElement, App, Div, Hsla, IntoElement, ParentElement, RenderImage, SharedString, Stateful,
    Styled, Window, WindowAppearance, div, hsla, prelude::*, px,
};
use image::Frame;
use smallvec::smallvec;
use tiny_skia::Pixmap;

pub use icons::icon;

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub window: Hsla,
    pub canvas: Hsla,
    pub panel: Hsla,
    pub text: Hsla,
    pub secondary: Hsla,
    pub tertiary: Hsla,
    pub border: Hsla,
    pub fill: Hsla,
    pub fill_strong: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub danger: Hsla,
}

impl Theme {
    pub fn get(window: &Window) -> Self {
        let dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        Self::new(dark)
    }

    pub fn new(dark: bool) -> Self {
        let accent: Hsla = crate::platform::accent_color().into();
        let ink = |a: f32| {
            if dark {
                hsla(0., 0., 1., a)
            } else {
                hsla(0., 0., 0., a)
            }
        };
        Self {
            dark,
            window: if dark {
                hsla(0., 0., 0.14, 1.)
            } else {
                hsla(0., 0., 0.98, 1.)
            },
            canvas: if dark {
                hsla(0., 0., 0.11, 1.)
            } else {
                hsla(240. / 360., 0.06, 0.93, 1.)
            },
            panel: if dark {
                hsla(0., 0., 0.17, 1.)
            } else {
                hsla(0., 0., 0.96, 1.)
            },
            text: ink(0.88),
            secondary: ink(0.55),
            tertiary: ink(0.3),
            border: ink(0.12),
            fill: ink(0.05),
            fill_strong: ink(0.11),
            accent,
            accent_soft: Hsla { a: 0.16, ..accent },
            danger: hsla(4. / 360., 0.9, 0.58, 1.),
        }
    }
}

/// Converts a premultiplied tiny-skia pixmap into the straight BGRA that gpui expects.
pub fn render_image(pm: &Pixmap) -> Arc<RenderImage> {
    let mut bgra = Vec::with_capacity(pm.data().len());
    for p in pm.pixels() {
        let c = p.demultiply();
        bgra.extend_from_slice(&[c.blue(), c.green(), c.red(), c.alpha()]);
    }
    let buffer = image::RgbaImage::from_raw(pm.width(), pm.height(), bgra).expect("pixmap");
    Arc::new(RenderImage::new(smallvec![Frame::new(buffer)]))
}

/// Frees the GPU texture of an image that is about to be replaced.
pub fn drop_image(image: Option<Arc<RenderImage>>, window: &mut Window, cx: &mut App) {
    if let Some(image) = image {
        cx.drop_image(image, Some(window));
    }
}

pub fn hairline_v(theme: &Theme) -> Div {
    div().w(px(0.5)).h(px(12.)).bg(theme.border)
}

pub fn divider_h(theme: &Theme) -> Div {
    div().h(px(1.)).w_full().bg(theme.border)
}

pub fn divider_v(theme: &Theme) -> Div {
    div().w(px(1.)).h_full().bg(theme.border)
}

pub fn label(text: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_size(px(13.))
        .text_color(theme.text)
        .child(text.into())
}

/// Icon-only button with hover state and a tooltip.
pub fn icon_button(
    id: impl Into<gpui::ElementId>,
    symbol: &str,
    tooltip: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    let tip: SharedString = tooltip.into();
    let hover = theme.fill_strong;
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(30.))
        .h(px(26.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(icon(symbol, 15., theme.secondary))
        .tooltip(move |_, cx| tooltip_view(tip.clone(), cx))
}

pub fn tooltip_view(text: SharedString, cx: &mut App) -> gpui::AnyView {
    cx.new(|_| Tooltip(text)).into()
}

struct Tooltip(SharedString);

impl Render for Tooltip {
    fn render(&mut self, window: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        div()
            .px_2()
            .py_1()
            .rounded(px(6.))
            .bg(if theme.dark {
                hsla(0., 0., 0.22, 0.98)
            } else {
                hsla(0., 0., 1., 0.98)
            })
            .border_1()
            .border_color(theme.border)
            .shadow_md()
            .text_size(px(12.))
            .text_color(theme.text)
            .child(self.0.clone())
    }
}

pub fn any(e: impl IntoElement) -> AnyElement {
    e.into_any_element()
}

/// Options for a borderless popup at `bounds` in global points. gpui positions a window relative
/// to its display, so this picks the display under the rect's center and converts the origin.
pub fn popup_options(bounds: gpui::Bounds<gpui::Pixels>, cx: &gpui::App) -> gpui::WindowOptions {
    let center = bounds.center();
    let placed = cx.displays().into_iter().find_map(|d| {
        let origin = crate::platform::display_origin(u32::from(d.id()))?;
        let frame = gpui::Bounds::new(origin, d.bounds().size);
        frame.contains(&center).then(|| (d.id(), origin))
    });
    let (display_id, local) = match placed {
        Some((id, origin)) => (
            Some(id),
            gpui::Bounds::new(bounds.origin - origin, bounds.size),
        ),
        None => (None, bounds),
    };
    gpui::WindowOptions {
        window_bounds: Some(gpui::WindowBounds::Windowed(local)),
        display_id,
        titlebar: None,
        focus: false,
        show: true,
        kind: gpui::WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        window_background: gpui::WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }
}
