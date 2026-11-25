use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, Corner, Div, ElementId, Global, Hsla, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Stateful, Window, anchored, canvas,
    deferred, div, hsla, prelude::*, px,
};

use super::{Theme, icon};

#[derive(Default)]
struct DragOwner(Option<ElementId>);

impl Global for DragOwner {}

type Change = Rc<dyn Fn(f64, &mut Window, &mut App)>;

/// Horizontal slider; `on_change` gets the new value while the knob is dragged.
pub fn slider(
    id: impl Into<ElementId>,
    value: f64,
    min: f64,
    max: f64,
    theme: &Theme,
    on_change: impl Fn(f64, &mut Window, &mut App) + 'static,
) -> Div {
    let id: ElementId = id.into();
    let t = ((value - min) / (max - min)).clamp(0., 1.) as f32;
    let on_change: Change = Rc::new(on_change);
    let knob_bg = if theme.dark {
        hsla(0., 0., 0.85, 1.)
    } else {
        hsla(0., 0., 1., 1.)
    };
    div()
        .relative()
        .flex_1()
        .h(px(18.))
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(7.5))
                .h(px(3.))
                .rounded_full()
                .bg(theme.fill_strong),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(7.5))
                .h(px(3.))
                .w(gpui::relative(t))
                .rounded_full()
                .bg(theme.accent),
        )
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(gpui::relative(t))
                .ml(px(-7.))
                .size(px(14.))
                .rounded_full()
                .bg(knob_bg)
                .border_1()
                .border_color(theme.border)
                .shadow_sm(),
        )
        .child(
            canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window, _| {
                    let value_at = move |p: Point<Pixels>| {
                        let w = f32::from(bounds.size.width).max(1.);
                        let f = ((f32::from(p.x - bounds.origin.x)) / w).clamp(0., 1.) as f64;
                        min + (max - min) * f
                    };
                    let (down_id, down_change) = (id.clone(), on_change.clone());
                    window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                        if phase.bubble()
                            && ev.button == MouseButton::Left
                            && bounds.contains(&ev.position)
                        {
                            cx.set_global(DragOwner(Some(down_id.clone())));
                            down_change(value_at(ev.position), window, cx);
                            cx.stop_propagation();
                        }
                    });
                    let (move_id, move_change) = (id.clone(), on_change.clone());
                    window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, cx| {
                        if phase.bubble()
                            && ev.pressed_button == Some(MouseButton::Left)
                            && cx.try_global::<DragOwner>().and_then(|d| d.0.as_ref())
                                == Some(&move_id)
                        {
                            move_change(value_at(ev.position), window, cx);
                        }
                    });
                    let up_id = id.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase.bubble()
                            && cx.try_global::<DragOwner>().and_then(|d| d.0.as_ref())
                                == Some(&up_id)
                        {
                            cx.set_global(DragOwner(None));
                        }
                    });
                },
            )
            .absolute()
            .left(px(-7.))
            .right(px(-7.))
            .top_0()
            .bottom_0(),
        )
}

pub fn switch(
    id: impl Into<ElementId>,
    on: bool,
    theme: &Theme,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(26.))
        .h(px(15.))
        .rounded_full()
        .p(px(1.5))
        .flex()
        .when(on, |d| d.justify_end())
        .bg(if on { theme.accent } else { theme.fill_strong })
        .cursor_pointer()
        .child(
            div()
                .size(px(12.))
                .rounded_full()
                .bg(hsla(0., 0., 1., 1.))
                .shadow_sm(),
        )
        .on_click(move |_, window, cx| on_toggle(!on, window, cx))
}

/// Row with a label on the left and a toggle on the right.
pub fn toggle_row(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    on: bool,
    theme: &Theme,
    on_toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(13.))
        .text_color(theme.text)
        .child(div().flex_1().child(title.into()))
        .child(switch(id, on, theme, on_toggle))
}

/// Label on the left with a fixed width, control on the right.
pub fn inspector_row(title: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(13.))
        .text_color(theme.text)
        .child(
            div()
                .w(px(76.))
                .flex_none()
                .overflow_hidden()
                .child(title.into()),
        )
}

/// Slider with a label and the value as 0–100 (or the raw value with a unit).
pub fn slider_row(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    value: f64,
    min: f64,
    max: f64,
    unit: Option<&str>,
    theme: &Theme,
    on_change: impl Fn(f64, &mut Window, &mut App) + 'static,
) -> Div {
    let display = match unit {
        Some(u) => format!("{}{u}", value.round() as i64),
        None => format!("{}", (((value - min) / (max - min)) * 100.).round() as i64),
    };
    inspector_row(title, theme)
        .child(slider(id, value, min, max, theme, on_change))
        .child(
            div()
                .w(px(30.))
                .flex_none()
                .flex()
                .justify_end()
                .text_color(theme.secondary)
                .child(display),
        )
}

pub struct MenuItem {
    pub label: SharedString,
    pub detail: Option<SharedString>,
    pub enabled: bool,
    pub destructive: bool,
    pub checked: bool,
    pub separator: bool,
    pub action: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            detail: None,
            enabled: true,
            destructive: false,
            checked: false,
            separator: false,
            action: Some(Rc::new(action)),
        }
    }

    pub fn separator() -> Self {
        Self {
            label: SharedString::default(),
            detail: None,
            enabled: false,
            destructive: false,
            checked: false,
            separator: true,
            action: None,
        }
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn checked(mut self, on: bool) -> Self {
        self.checked = on;
        self
    }

    pub fn detail(mut self, d: impl Into<SharedString>) -> Self {
        self.detail = Some(d.into());
        self
    }
}

pub fn popup_surface(theme: &Theme) -> Div {
    div()
        .rounded(px(9.))
        .border_1()
        .border_color(theme.border)
        .bg(if theme.dark {
            hsla(0., 0., 0.2, 0.99)
        } else {
            hsla(0., 0., 0.99, 0.99)
        })
        .shadow_lg()
}

/// A floating menu at `position` (window coordinates). `dismiss` runs after a choice or a click
/// outside.
pub fn menu(
    position: Point<Pixels>,
    items: Vec<MenuItem>,
    theme: &Theme,
    dismiss: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let dismiss: Rc<dyn Fn(&mut Window, &mut App)> = Rc::new(dismiss);
    let outside = dismiss.clone();
    let mut list = popup_surface(theme)
        .id("menu")
        .min_w(px(180.))
        .py(px(5.))
        .flex()
        .flex_col()
        .text_size(px(13.))
        .occlude()
        .on_mouse_down_out(move |_, window, cx| outside(window, cx));
    for (i, item) in items.into_iter().enumerate() {
        if item.separator {
            list = list.child(div().my(px(4.)).mx(px(10.)).h(px(1.)).bg(theme.border));
            continue;
        }
        let color = if !item.enabled {
            theme.tertiary
        } else if item.destructive {
            theme.danger
        } else {
            theme.text
        };
        let accent = theme.accent;
        let dismiss = dismiss.clone();
        let action = item.action.clone();
        let enabled = item.enabled;
        list = list.child(
            div()
                .id(("menu-item", i))
                .mx(px(5.))
                .px(px(8.))
                .h(px(24.))
                .rounded(px(5.))
                .flex()
                .items_center()
                .gap_2()
                .text_color(color)
                .when(enabled, |d| {
                    d.hover(move |s| s.bg(accent).text_color(hsla(0., 0., 1., 1.)))
                })
                .child(div().w(px(12.)).when(item.checked, |d| d.child("✓")))
                .child(div().flex_1().child(item.label))
                .when_some(item.detail, |d, det| {
                    d.child(div().opacity(0.6).text_size(px(12.)).child(det))
                })
                .on_click(move |_, window, cx| {
                    if !enabled {
                        return;
                    }
                    dismiss(window, cx);
                    if let Some(a) = action.as_ref() {
                        a(window, cx);
                    }
                }),
        );
    }
    deferred(
        anchored()
            .position(position)
            .anchor(Corner::TopLeft)
            .snap_to_window_with_margin(px(8.))
            .child(list),
    )
    .with_priority(2)
    .into_any_element()
}

/// A floating panel with arbitrary content, dismissed by clicking outside.
pub fn popover(
    position: Point<Pixels>,
    anchor: Corner,
    content: impl IntoElement,
    theme: &Theme,
    dismiss: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    deferred(
        anchored()
            .position(position)
            .anchor(anchor)
            .snap_to_window_with_margin(px(8.))
            .child(
                popup_surface(theme)
                    .id("popover")
                    .occlude()
                    .on_mouse_down_out(move |_, window, cx| dismiss(window, cx))
                    .child(content),
            ),
    )
    .with_priority(2)
    .into_any_element()
}

/// Segmented tabs that stretch to the column width.
pub fn tabs(
    id: &'static str,
    items: Vec<SharedString>,
    selected: usize,
    theme: &Theme,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> Div {
    let on_select = Rc::new(on_select);
    let mut row = div()
        .flex()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(7.))
        .bg(theme.fill);
    for (i, title) in items.into_iter().enumerate() {
        let on_select = on_select.clone();
        let active = i == selected;
        row = row.child(
            div()
                .id((id, i))
                .flex_1()
                .flex()
                .justify_center()
                .py(px(3.))
                .rounded(px(5.))
                .text_size(px(13.))
                .cursor_pointer()
                .when(active, |d| d.bg(theme.fill_strong))
                .text_color(if active { theme.text } else { theme.secondary })
                .child(title)
                .on_click(move |_, window, cx| on_select(i, window, cx)),
        );
    }
    row
}

pub fn chevron(expanded: bool, theme: &Theme) -> Div {
    icon(
        if expanded {
            "chevron.down"
        } else {
            "chevron.right"
        },
        10.,
        theme.tertiary,
    )
}

/// Pill-shaped button.
pub fn pill(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    theme: &Theme,
    prominent: bool,
) -> Stateful<Div> {
    let hover = if prominent {
        Hsla {
            a: 0.22,
            ..theme.accent
        }
    } else {
        theme.fill_strong
    };
    div()
        .id(id)
        .px(px(12.))
        .h(px(28.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .text_size(px(12.))
        .cursor_pointer()
        .bg(if prominent {
            theme.accent_soft
        } else {
            theme.fill
        })
        .text_color(if prominent { theme.accent } else { theme.text })
        .hover(move |s| s.bg(hover))
        .child(text.into())
}

/// Regular push button.
pub fn button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = theme.fill_strong;
    div()
        .id(id)
        .px(px(10.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .rounded(px(6.))
        .border_1()
        .border_color(theme.border)
        .bg(theme.fill)
        .text_size(px(13.))
        .text_color(theme.text)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(text.into())
}

pub fn primary_button(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    theme: &Theme,
) -> Stateful<Div> {
    let accent = theme.accent;
    div()
        .id(id)
        .px(px(12.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .bg(accent)
        .text_size(px(13.))
        .text_color(hsla(0., 0., 1., 1.))
        .cursor_pointer()
        .hover(move |s| s.opacity(0.9))
        .child(text.into())
}
