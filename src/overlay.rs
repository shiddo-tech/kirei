use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, AsyncApp, Bounds, Context, Corners, CursorStyle, FocusHandle, Global, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderImage, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, canvas, div,
    fill, hsla, img, outline, point, prelude::*, px, size,
};

use crate::capture::{self, Captured, DisplayShot, PickableWindow};
use crate::l10n::l;

const MIN_DRAG: f32 = 4.0;

type Done = Rc<dyn Fn(Captured, &mut App)>;

struct OverlaySession {
    windows: Vec<WindowHandle<SelectionOverlay>>,
    window_mode: bool,
    pickable: Vec<PickableWindow>,
    hovered: Option<PickableWindow>,
}

impl Global for OverlaySession {}

pub fn is_active(cx: &App) -> bool {
    cx.try_global::<OverlaySession>()
        .is_some_and(|s| !s.windows.is_empty())
}

/// Full-screen overlay (one window per display) over the frozen screens, for picking an area or
/// a window, in the spirit of the system screenshot UI.
pub fn begin(
    shots: Vec<DisplayShot>,
    window_mode: bool,
    cx: &mut App,
    on_done: impl Fn(Captured, &mut App) + 'static,
) {
    if is_active(cx) {
        close_all(cx);
    }
    let on_done: Done = Rc::new(on_done);
    let displays = cx.displays();
    let pickable = capture::pickable_windows();
    let mouse = crate::platform::mouse_location();
    let hovered = window_mode.then(|| window_at(&pickable, mouse)).flatten();
    // open_window renders right away, and render reads this global.
    cx.set_global(OverlaySession {
        windows: Vec::new(),
        window_mode,
        pickable,
        hovered,
    });

    for shot in shots {
        let Some(display) = displays
            .iter()
            .find(|d| u32::from(d.id()) == shot.display_id)
        else {
            continue;
        };
        let display_bounds = display.bounds();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: display_bounds.size,
            })),
            display_id: Some(display.id()),
            titlebar: None,
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };
        let scale = shot.width as f32 / f32::from(display_bounds.size.width);
        let origin =
            crate::platform::display_origin(shot.display_id).unwrap_or(display_bounds.origin);
        let on_done = on_done.clone();
        let handle = cx.open_window(options, move |window, cx| {
            crate::platform::set_window_level(window, 1000);
            crate::platform::cover_screen(window, Bounds::new(origin, display_bounds.size));
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                SelectionOverlay {
                    image: shot.render_image(),
                    shot: Arc::new(shot),
                    origin,
                    scale,
                    focus,
                    drag_start: None,
                    drag_end: None,
                    hover: None,
                    on_done,
                }
            })
        });
        match handle {
            Ok(handle) => cx.global_mut::<OverlaySession>().windows.push(handle),
            Err(e) => eprintln!("kirei: overlay não abriu: {e:#}"),
        }
    }

    cx.activate(true);
}

fn window_at(list: &[PickableWindow], p: Point<Pixels>) -> Option<PickableWindow> {
    let (x, y) = (f32::from(p.x) as f64, f32::from(p.y) as f64);
    list.iter()
        .find(|w| x >= w.x && x <= w.x + w.w && y >= w.y && y <= w.y + w.h)
        .copied()
}

fn close_all(cx: &mut App) {
    let windows = std::mem::take(&mut cx.global_mut::<OverlaySession>().windows);
    for handle in windows {
        handle
            .update(cx, |_, window, _| window.remove_window())
            .ok();
    }
}

fn refresh_all(cx: &mut App) {
    let windows = cx.global::<OverlaySession>().windows.clone();
    for h in windows {
        h.update(cx, |_, _, cx| cx.notify()).ok();
    }
}

fn toggle_mode(cx: &mut App) {
    let mouse = crate::platform::mouse_location();
    let s = cx.global_mut::<OverlaySession>();
    s.window_mode = !s.window_mode;
    s.hovered = if s.window_mode {
        window_at(&s.pickable, mouse)
    } else {
        None
    };
    let windows = s.windows.clone();
    for h in windows {
        h.update(cx, |o, _, cx| {
            o.drag_start = None;
            o.drag_end = None;
            cx.notify()
        })
        .ok();
    }
}

/// Captures the picked window after the overlay has left the screen.
fn finish_window(id: u32, on_done: Done, cx: &mut App) {
    close_all(cx);
    cx.spawn(async move |cx: &mut AsyncApp| {
        cx.background_executor()
            .timer(std::time::Duration::from_millis(120))
            .await;
        let result = cx
            .background_executor()
            .spawn(async move { capture::capture_window(id) })
            .await;
        cx.update(|cx| match result {
            Ok(c) => on_done(c, cx),
            Err(e) => {
                eprintln!("kirei: janela não capturada: {e:#}");
                crate::hud::show_with(
                    l("Couldn't capture the screen"),
                    "exclamationmark.triangle.fill",
                    cx,
                );
            }
        })
        .ok();
    })
    .detach();
}

pub struct SelectionOverlay {
    shot: Arc<DisplayShot>,
    image: Arc<RenderImage>,
    origin: Point<Pixels>,
    scale: f32,
    focus: FocusHandle,
    drag_start: Option<Point<Pixels>>,
    drag_end: Option<Point<Pixels>>,
    hover: Option<Point<Pixels>>,
    on_done: Done,
}

impl SelectionOverlay {
    /// Snapped to whole points, the same grid the capture uses.
    fn selection(&self, full: gpui::Size<Pixels>) -> Option<Bounds<Pixels>> {
        let (a, b) = (self.drag_start?, self.drag_end?);
        let r = |v: Pixels| px(f32::from(v).round());
        let x0 = r(a.x.min(b.x)).max(px(0.));
        let y0 = r(a.y.min(b.y)).max(px(0.));
        let x1 = r(a.x.max(b.x)).min(full.width);
        let y1 = r(a.y.max(b.y)).min(full.height);
        Some(Bounds::from_corners(point(x0, y0), point(x1, y1)))
    }

    fn finish_area(&mut self, sel: Bounds<Pixels>, cx: &mut Context<Self>) {
        let s = self.scale;
        let x0 = (f32::from(sel.origin.x) * s).round() as u32;
        let y0 = (f32::from(sel.origin.y) * s).round() as u32;
        let x1 = (f32::from(sel.origin.x + sel.size.width) * s).round() as u32;
        let y1 = (f32::from(sel.origin.y + sel.size.height) * s).round() as u32;
        let captured = self.shot.crop(x0, y0, x1 - x0, y1 - y0, s).map(|mut c| {
            c.screen_rect = Some(Bounds::new(sel.origin + self.origin, sel.size));
            c
        });
        let on_done = self.on_done.clone();
        cx.defer(move |cx| {
            close_all(cx);
            if let Some(c) = captured {
                on_done(c, cx);
            }
        });
    }

    fn mouse_down(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let session = cx.global::<OverlaySession>();
        if session.window_mode {
            if let Some(w) = session.hovered {
                let on_done = self.on_done.clone();
                cx.defer(move |cx| finish_window(w.id, on_done, cx));
            }
            return;
        }
        self.drag_start = Some(ev.position);
        self.drag_end = Some(ev.position);
        cx.notify();
    }

    fn mouse_move(&mut self, ev: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.hover = Some(ev.position);
        if ev.dragging() && self.drag_start.is_some() {
            self.drag_end = Some(ev.position);
        }
        let global = point(self.origin.x + ev.position.x, self.origin.y + ev.position.y);
        let s = cx.global_mut::<OverlaySession>();
        if s.window_mode {
            let h = window_at(&s.pickable, global);
            if h.map(|w| w.id) != s.hovered.map(|w| w.id) {
                s.hovered = h;
                cx.defer(refresh_all);
            }
        }
        cx.notify();
    }

    fn mouse_up(&mut self, ev: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag_start.is_none() {
            return;
        }
        self.drag_end = Some(ev.position);
        let full = window.bounds().size;
        match self.selection(full) {
            Some(sel)
                if f32::from(sel.size.width) >= MIN_DRAG
                    && f32::from(sel.size.height) >= MIN_DRAG =>
            {
                self.finish_area(sel, cx)
            }
            _ => {
                // A plain click switches to window mode and takes the window under the cursor.
                self.drag_start = None;
                self.drag_end = None;
                let on_done = self.on_done.clone();
                cx.defer(move |cx| {
                    toggle_mode(cx);
                    if let Some(w) = cx.global::<OverlaySession>().hovered {
                        finish_window(w.id, on_done, cx);
                    }
                });
            }
        }
    }
}

fn hint(text: String) -> impl IntoElement {
    div()
        .absolute()
        .bottom(px(40.))
        .left_0()
        .right_0()
        .flex()
        .justify_center()
        .child(
            div()
                .px(px(14.))
                .py(px(7.))
                .rounded_full()
                .bg(hsla(0., 0., 0., 0.62))
                .text_size(px(13.))
                .text_color(hsla(0., 0., 1., 0.95))
                .child(text),
        )
}

impl Render for SelectionOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let full = window.bounds().size;
        let dim = hsla(0., 0., 0., 0.32);
        let session = cx.global::<OverlaySession>();
        let window_mode = session.window_mode;
        let hovered = session.hovered;
        let selection = if window_mode {
            None
        } else {
            self.selection(full)
        };
        let image = self.image.clone();
        let hover = self.hover;
        let shot = self.shot.clone();
        let scale = self.scale;
        let origin = self.origin;
        let accent: gpui::Hsla = crate::platform::accent_color().into();

        let painter = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let reveal = |window: &mut Window, r: Bounds<Pixels>| {
                    window.with_content_mask(Some(gpui::ContentMask { bounds: r }), |window| {
                        window
                            .paint_image(bounds, Corners::default(), image.clone(), 0, false)
                            .ok();
                    });
                };
                window.paint_quad(fill(bounds, dim));
                if window_mode {
                    if let Some(w) = hovered {
                        let r = Bounds::new(
                            point(px(w.x as f32) - origin.x, px(w.y as f32) - origin.y),
                            size(px(w.w as f32), px(w.h as f32)),
                        );
                        reveal(window, r);
                        window.paint_quad(fill(r, gpui::Hsla { a: 0.18, ..accent }));
                        window.paint_quad(
                            outline(r, accent, gpui::BorderStyle::Solid).border_widths(px(3.)),
                        );
                    }
                    return;
                }
                if let Some(sel) = selection {
                    reveal(window, sel);
                    window.paint_quad(
                        outline(
                            Bounds::new(
                                sel.origin - point(px(0.5), px(0.5)),
                                sel.size + size(px(1.), px(1.)),
                            ),
                            hsla(0., 0., 1., 0.95),
                            gpui::BorderStyle::Solid,
                        )
                        .border_widths(px(1.)),
                    );
                } else if let Some(p) = hover {
                    let line = hsla(0., 0., 1., 0.55);
                    window.paint_quad(fill(
                        Bounds::new(point(px(0.), p.y.round()), size(bounds.size.width, px(1.))),
                        line,
                    ));
                    window.paint_quad(fill(
                        Bounds::new(point(p.x.round(), px(0.)), size(px(1.), bounds.size.height)),
                        line,
                    ));
                }
            },
        )
        .absolute()
        .size_full();

        let mut root = div()
            .id("selection-overlay")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .cursor(if window_mode {
                CursorStyle::PointingHand
            } else {
                CursorStyle::Crosshair
            })
            .child(img(self.image.clone()).absolute().size_full())
            .child(painter)
            .on_key_down(cx.listener(
                |_, ev: &KeyDownEvent, _, cx| match ev.keystroke.key.as_str() {
                    "escape" => cx.defer(close_all),
                    "space" => cx.defer(toggle_mode),
                    _ => {}
                },
            ))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|_, _: &MouseDownEvent, _, cx| cx.defer(close_all)),
            )
            .on_mouse_move(cx.listener(Self::mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up));

        if window_mode {
            root = root.child(hint(l(
                "Click a window to capture it · Space: select area · Esc: cancel",
            )));
        } else if let Some(sel) = selection {
            root = root.child(size_label(sel, scale));
            if let Some(m) = hover {
                root = root.child(loupe(m, &shot, scale, full));
            }
        } else {
            if let Some(m) = hover {
                root = root.child(loupe(m, &shot, scale, full));
            }
            root = root.child(hint(l(
                "Drag to select an area · Space or click: pick a window · Esc: cancel",
            )));
        }
        root
    }
}

fn size_label(sel: Bounds<Pixels>, scale: f32) -> impl IntoElement {
    let w = (f32::from(sel.size.width) * scale).round();
    let h = (f32::from(sel.size.height) * scale).round();
    let top = if f32::from(sel.origin.y) > 28. {
        sel.origin.y - px(26.)
    } else {
        sel.origin.y + sel.size.height + px(6.)
    };
    div()
        .absolute()
        .left(sel.origin.x)
        .top(top)
        .px_2()
        .py(px(2.))
        .rounded_md()
        .bg(hsla(0., 0., 0., 0.75))
        .text_xs()
        .text_color(hsla(0., 0., 1., 1.))
        .child(format!("{w} × {h}"))
}

/// Magnified pixels around the cursor with a pixel grid, the center pixel and the coordinates.
fn loupe(
    m: Point<Pixels>,
    shot: &DisplayShot,
    scale: f32,
    full: gpui::Size<Pixels>,
) -> impl IntoElement {
    const CELLS: i32 = 15;
    const CELL: f32 = 8.;
    let side = CELLS as f32 * CELL;
    let px_x = (f32::from(m.x) * scale).floor() as i32;
    let px_y = (f32::from(m.y) * scale).floor() as i32;
    let half = CELLS / 2;
    let mut x = f32::from(m.x) + 22.;
    let mut y = f32::from(m.y) + 22.;
    if x + side > f32::from(full.width) - 8. {
        x = f32::from(m.x) - side - 22.;
    }
    if y + side + 30. > f32::from(full.height) - 8. {
        y = f32::from(m.y) - side - 52.;
    }
    let mut grid = div().absolute().inset_0().flex().flex_wrap();
    for row in 0..CELLS {
        for col in 0..CELLS {
            let (sx, sy) = (px_x - half + col, px_y - half + row);
            let color =
                if sx >= 0 && sy >= 0 && (sx as u32) < shot.width && (sy as u32) < shot.height {
                    let i = ((sy as u32 * shot.width + sx as u32) * 4) as usize;
                    let (b, g, r) = (shot.bgra[i], shot.bgra[i + 1], shot.bgra[i + 2]);
                    gpui::Rgba {
                        r: r as f32 / 255.,
                        g: g as f32 / 255.,
                        b: b as f32 / 255.,
                        a: 1.,
                    }
                    .into()
                } else {
                    hsla(0., 0., 0.1, 1.)
                };
            grid = grid.child(
                div()
                    .size(px(CELL))
                    .bg(color)
                    .border_r(px(0.5))
                    .border_b(px(0.5))
                    .border_color(hsla(0., 0., 0., 0.14)),
            );
        }
    }
    let c = half as f32 * CELL;
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .relative()
                .size(px(side))
                .rounded(px(14.))
                .overflow_hidden()
                .bg(hsla(0., 0., 0.1, 1.))
                .border(px(1.5))
                .border_color(hsla(0., 0., 1., 0.85))
                .shadow_lg()
                .child(grid)
                .child(
                    div()
                        .absolute()
                        .left(px(c - 1.))
                        .top(px(c - 1.))
                        .size(px(CELL + 2.))
                        .border(px(1.5))
                        .border_color(hsla(0., 0., 1., 1.)),
                ),
        )
        .child(
            div()
                .px(px(7.))
                .py(px(2.))
                .rounded_full()
                .bg(hsla(0., 0., 0., 0.72))
                .text_size(px(11.))
                .text_color(hsla(0., 0., 1., 1.))
                .child(format!(
                    "{}, {}",
                    f32::from(m.x) as i32,
                    f32::from(m.y) as i32
                )),
        )
}
