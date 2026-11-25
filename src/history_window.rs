use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    AnyWindowHandle, App, Bounds, Context, Global, MouseButton, MouseDownEvent, Pixels, Point,
    RenderImage, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, div, img,
    point, prelude::*, px, size,
};
use uuid::Uuid;

use crate::history::{self, HistoryItem};
use crate::l10n::l;
use crate::render::SourceImage;
use crate::ui::controls::{MenuItem, button, menu};
use crate::ui::{Theme, icon};

#[derive(Default)]
struct Current(Option<WindowHandle<HistoryWindow>>);

impl Global for Current {}

pub fn is(w: &AnyWindowHandle) -> bool {
    w.downcast::<HistoryWindow>().is_some()
}

pub fn open(cx: &mut App) {
    if let Some(h) = cx.default_global::<Current>().0
        && h.update(cx, |_, w, _| w.activate_window()).is_ok()
    {
        crate::activate(cx);
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(760.), px(560.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some(l("Capture History").into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.), px(14.))),
        }),
        focus: !crate::dev_mode(),
        window_min_size: Some(size(px(480.), px(360.))),
        ..Default::default()
    };
    let handle = cx
        .open_window(options, |_, cx| {
            cx.new(|_| HistoryWindow {
                thumbs: HashMap::new(),
                hover: None,
                menu: None,
                press: None,
            })
        })
        .ok();
    cx.set_global(Current(handle));
    crate::activate(cx);
}

pub struct HistoryWindow {
    thumbs: HashMap<Uuid, Option<Arc<RenderImage>>>,
    hover: Option<Uuid>,
    menu: Option<(Uuid, Point<Pixels>)>,
    press: Option<(Uuid, Point<Pixels>)>,
}

fn relative(item: &HistoryItem) -> String {
    crate::platform::relative_date(item.unix_time())
}

fn load_source(item: &HistoryItem) -> Option<Arc<SourceImage>> {
    history::load(item).map(|pixmap| Arc::new(SourceImage { pixmap }))
}

impl HistoryWindow {
    fn thumb(&mut self, item: &HistoryItem) -> Option<Arc<RenderImage>> {
        self.thumbs
            .entry(item.id)
            .or_insert_with(|| {
                let path = history::thumb_path(item);
                let pm = crate::render::load_image_file(&path.to_string_lossy())?;
                Some(crate::ui::render_image(&pm))
            })
            .clone()
    }

    fn edit(item: &HistoryItem, cx: &mut App) {
        if let Some(src) = load_source(item) {
            crate::editor::open_source(src, item.scale, cx);
        }
    }

    fn menu_items(&self, item: HistoryItem, cx: &mut Context<Self>) -> Vec<MenuItem> {
        let e = cx.entity();
        let (i1, i2, i3, i4) = (item.clone(), item.clone(), item.clone(), item.clone());
        vec![
            MenuItem::new(l("Edit"), move |_, cx| Self::edit(&i1, cx)),
            MenuItem::new(l("Copy"), move |_, _| {
                if let Some(pm) = history::load(&i2)
                    && crate::platform::copy_image(
                        &pm,
                        i2.scale,
                        crate::settings::get().export_retina,
                    )
                    .is_ok()
                {
                    crate::feedback::play_copy();
                }
            }),
            MenuItem::new(l("Pin to Screen"), move |_, cx| {
                if let Some(pm) = history::load(&i3) {
                    crate::pinned::pin(pm, i3.scale, cx);
                }
            }),
            MenuItem::new(l("Show in Finder"), move |_, cx| {
                cx.reveal_path(&history::file_path(&i4))
            }),
            MenuItem::separator(),
            MenuItem::new(l("Delete"), move |_, cx| {
                history::remove(item.id);
                e.update(cx, |t, cx| {
                    t.thumbs.remove(&item.id);
                    cx.notify()
                });
            })
            .destructive(),
        ]
    }
}

impl Render for HistoryWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        let items = history::items();
        let keep = crate::settings::get().keep_history;
        let menu_view = self.menu.and_then(|(id, at)| {
            let item = items.iter().find(|i| i.id == id)?.clone();
            let e = cx.entity();
            Some(menu(at, self.menu_items(item, cx), &theme, move |_, cx| {
                e.update(cx, |t, cx| {
                    t.menu = None;
                    cx.notify()
                })
            }))
        });
        let header = div()
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .px(px(16.))
            .pl(px(90.))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(l("Capture History")),
            )
            .when(!items.is_empty(), |d| {
                d.child(
                    button("clear", l("Clear History"), &theme).on_click(cx.listener(
                        |t, _, _, cx| {
                            history::clear();
                            t.thumbs.clear();
                            cx.notify();
                        },
                    )),
                )
            });
        let body = if items.is_empty() {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .child(icon("photo.stack", 36., theme.tertiary))
                .child(
                    div()
                        .text_size(px(17.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(l("No captures yet")),
                )
                .child(div().text_size(px(13.)).text_color(theme.secondary).child(if keep {
                    l("Your captures from the last 14 days show up here.")
                } else {
                    l("Turn on “Keep capture history” in Settings to see your captures here.")
                }))
                .into_any_element()
        } else {
            let mut grid = div().p(px(22.)).flex().flex_wrap().gap(px(18.));
            for item in items {
                let thumb = self.thumb(&item);
                let hover = self.hover == Some(item.id);
                let id = item.id;
                let size_label = format!(
                    "{}×{}",
                    (item.width as f64 / item.scale) as i64,
                    (item.height as f64 / item.scale) as i64
                );
                let it = item.clone();
                grid = grid.child(
                    div()
                        .id(gpui::SharedString::from(format!("h-{id}")))
                        .w(px(220.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .cursor_pointer()
                        .on_hover(cx.listener(move |t, h: &bool, _, cx| {
                            t.hover = if *h { Some(id) } else { None };
                            cx.notify()
                        }))
                        .child(
                            div()
                                .h(px(150.))
                                .rounded(px(12.))
                                .bg(theme.fill)
                                .border(px(if hover { 2. } else { 1. }))
                                .border_color(if hover { theme.accent } else { theme.border })
                                .p(px(12.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .when_some(thumb, |d, t| {
                                    d.child(img(t).max_w_full().max_h_full().rounded(px(6.)))
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .text_size(px(13.))
                                .child(
                                    div()
                                        .flex_1()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(relative(&item)),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.secondary)
                                        .child(size_label),
                                ),
                        )
                        .tooltip(|_, cx| {
                            crate::ui::tooltip_view(l("Double-click to edit").into(), cx)
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |t, ev: &MouseDownEvent, _, cx| {
                                if ev.click_count >= 2 {
                                    t.press = None;
                                    Self::edit(&it, cx);
                                } else {
                                    t.press = Some((id, ev.position));
                                }
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |t, ev: &MouseDownEvent, _, cx| {
                                t.menu = Some((id, ev.position));
                                cx.notify()
                            }),
                        ),
                );
            }
            let items_for_drag = history::items();
            div()
                .id("history-scroll")
                .on_mouse_move(cx.listener(move |t, ev: &gpui::MouseMoveEvent, window, _| {
                    let Some((id, start)) = t.press else { return };
                    if ev.pressed_button != Some(MouseButton::Left) {
                        t.press = None;
                        return;
                    }
                    let d = ev.position - start;
                    if f32::from(d.x).hypot(f32::from(d.y)) > 4.
                        && let Some(item) = items_for_drag.iter().find(|i| i.id == id)
                    {
                        t.press = None;
                        crate::platform::drag_file(window, &history::file_path(item));
                    }
                }))
                .flex_1()
                .overflow_y_scroll()
                .child(grid)
                .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.window)
            .text_color(theme.text)
            .child(header)
            .child(crate::ui::divider_h(&theme))
            .child(body)
            .when_some(menu_view, |d, m| d.child(m))
    }
}
