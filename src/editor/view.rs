use std::collections::HashSet;

use gpui::{
    AnyElement, ClickEvent, Context, Corner, Div, FocusHandle, Hsla, KeyDownEvent, MouseButton,
    Pixels, Point, SharedString, Window, div, hsla, img, point, prelude::*, px,
};

use super::{
    ActualSize, CloseWindow, ColorTarget, CopyImage, CopyText, Duplicate, Editor, Pin, Popup,
    Redact, Redo, Save, SaveAs, SelectAll, ToggleInspector, ToolGroup, Undo, ZOOM_STEPS, ZoomIn,
    ZoomOut, ZoomToFit, symbol, uses_color,
};
use crate::l10n::l;
use crate::model::{AnnotationKind, Tool, annotation_palette};
use crate::ui::controls::{MenuItem, button, menu, popover, primary_button};
use crate::ui::{Theme, divider_h, divider_v, hairline_v, icon, icon_button, tooltip_view};

const BAR: f32 = 52.;

fn below(ev: &ClickEvent) -> Point<Pixels> {
    let p = ev.position();
    point(p.x - px(12.), p.y + px(14.))
}

impl Editor {
    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() || !self.focus.is_focused(window) {
            return;
        }
        let m = ev.keystroke.modifiers;
        if m.platform || m.alt || m.control {
            return;
        }
        let key = ev.keystroke.key.as_str();
        match key {
            "backspace" | "delete" => {
                let sel = self.selection.clone();
                self.delete(&sel, cx);
            }
            "escape" => {
                if self.popup.is_some() {
                    self.close_popup(window, cx);
                } else if !self.selection.is_empty() {
                    self.selection.clear();
                    self.dirty(cx);
                } else {
                    self.set_tool(Tool::Select, cx);
                }
            }
            "left" | "right" | "up" | "down" => {
                if self.selection.is_empty() {
                    return;
                }
                let step = if m.shift { 10. } else { 1. } * self.pixel_scale;
                let (dx, dy) = match key {
                    "left" => (-step, 0.),
                    "right" => (step, 0.),
                    "up" => (0., -step),
                    _ => (0., step),
                };
                self.nudge_selection(dx, dy, cx);
            }
            k => {
                if let Some(t) = Tool::ALL.into_iter().find(|t| t.shortcut() == k) {
                    self.set_tool(t, cx);
                } else {
                    return;
                }
            }
        }
        cx.stop_propagation();
    }

    fn toolbar(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let mut tools = div().flex().gap(px(2.));
        for g in ToolGroup::ALL {
            let group = g.tools();
            let active = group.contains(&self.tool);
            let current = if active {
                self.tool
            } else {
                self.last_in_group.get(&g).copied().unwrap_or(group[0])
            };
            let is_group = group.len() > 1;
            let tip: SharedString = if is_group {
                format!("{} · {}", current.title(), l("Click again to switch")).into()
            } else {
                current.title().into()
            };
            let hover = theme.fill_strong;
            tools = tools.child(
                div()
                    .id(("tool", g as usize))
                    .relative()
                    .w(px(30.))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(active, |d| d.bg(theme.fill_strong))
                    .when(!active, |d| d.hover(move |s| s.bg(hover)))
                    .child(icon(
                        symbol(current),
                        14.,
                        if active { theme.text } else { theme.secondary },
                    ))
                    .when(is_group, |d| {
                        d.child(div().absolute().right(px(3.)).bottom(px(3.)).child(icon(
                            "arrowtriangle.right.fill",
                            4.,
                            theme.secondary,
                        )))
                    })
                    .tooltip(move |_, cx| tooltip_view(tip.clone(), cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let tools = g.tools();
                        if active && tools.len() > 1 {
                            let i = tools.iter().position(|t| *t == current).unwrap_or(0);
                            this.set_tool(tools[(i + 1) % tools.len()], cx);
                        } else {
                            this.set_tool(current, cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            if g.tools().len() > 1 {
                                this.open_popup(Popup::ToolGroup(g), ev.position, cx);
                            }
                        }),
                    ),
            );
        }

        let shows_style = uses_color(self.tool) || !self.selection.is_empty();
        let mut bar = div()
            .h(px(BAR))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(84.))
            .pr(px(14.))
            .child(tools);
        if shows_style {
            let color = self.color.gpui();
            bar = bar.child(
                div()
                    .id("style-color")
                    .p(px(5.))
                    .cursor_pointer()
                    .child(
                        div()
                            .size(px(16.))
                            .rounded_full()
                            .bg(color)
                            .border_1()
                            .border_color(Hsla {
                                a: 0.25,
                                ..theme.text
                            }),
                    )
                    .tooltip(|_, cx| tooltip_view(l("Color and thickness").into(), cx))
                    .on_click(cx.listener(|this, ev: &ClickEvent, _, cx| {
                        this.open_popup(Popup::StyleColor, below(ev), cx)
                    })),
            );
        }
        bar.child(div().flex_1())
            .child(
                icon_button("smart", "wand.and.stars", l("Smart tools"), theme).on_click(
                    cx.listener(|this, ev: &ClickEvent, _, cx| {
                        this.open_popup(Popup::Smart, below(ev), cx)
                    }),
                ),
            )
            .child(
                icon_button("more", "ellipsis.circle", l("More options"), theme).on_click(
                    cx.listener(|this, ev: &ClickEvent, _, cx| {
                        this.open_popup(Popup::More, below(ev), cx)
                    }),
                ),
            )
            .child(
                icon_button(
                    "inspector",
                    "sidebar.right",
                    format!(
                        "{} (⌥⌘I)",
                        if self.show_inspector {
                            l("Hide Inspector")
                        } else {
                            l("Show Inspector")
                        }
                    ),
                    theme,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_inspector(cx))),
            )
    }

    fn bottom_bar(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let layout = crate::render::CanvasLayout::new(&self.style, self.crop());
        let zoom_label = match self.zoom {
            None => l("Fit"),
            Some(_) => format!("{}%", (self.effective_zoom() * 100.).round() as i64),
        };
        let thumb = self.thumb_image();
        let capsule = |d: Div| d.rounded_full().bg(theme.fill);
        div()
            .h(px(56.))
            .flex_none()
            .px(px(16.))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(
                div()
                    .id("drag-thumb")
                    .w(px(64.))
                    .h(px(40.))
                    .rounded(px(6.))
                    .overflow_hidden()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.fill)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when_some(thumb, |d, t| d.child(img(t).max_w(px(64.)).max_h(px(40.))))
                    .tooltip(|_, cx| {
                        tooltip_view(
                            l("Drag the final image to Finder, Slack, email…").into(),
                            cx,
                        )
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, ev: &gpui::MouseDownEvent, _, _| {
                            this.thumb_press = Some(ev.position);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, ev: &gpui::MouseMoveEvent, window, cx| {
                        let Some(start) = this.thumb_press else {
                            return;
                        };
                        if ev.pressed_button != Some(MouseButton::Left) {
                            this.thumb_press = None;
                            return;
                        }
                        let d = ev.position - start;
                        if f32::from(d.x).hypot(f32::from(d.y)) > 4. {
                            this.thumb_press = None;
                            if let Some(path) = this.temp_file(window, cx) {
                                crate::platform::drag_file(window, &path);
                            }
                        }
                    })),
            )
            .child(
                capsule(div())
                    .h(px(26.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.secondary)
                            .child(format!(
                                "{} × {}",
                                layout.canvas_w as i64, layout.canvas_h as i64
                            )),
                    )
                    .child(hairline_v(theme))
                    .child(
                        div()
                            .id("zoom")
                            .text_size(px(11.))
                            .text_color(theme.secondary)
                            .cursor_pointer()
                            .child(zoom_label)
                            .tooltip(|_, cx| {
                                tooltip_view(
                                    l("100% shows each captured pixel as one screen pixel (⌘0)")
                                        .into(),
                                    cx,
                                )
                            })
                            .on_click(cx.listener(|this, ev: &ClickEvent, _, cx| {
                                let p = ev.position();
                                this.open_popup(Popup::Zoom, point(p.x, p.y - px(8.)), cx)
                            })),
                    ),
            )
            .child(div().flex_1())
            .child(
                capsule(div())
                    .flex()
                    .items_center()
                    .px(px(2.))
                    .child(
                        icon_button("share", "square.and.arrow.up", l("Share"), theme).on_click(
                            cx.listener(|this, ev: &ClickEvent, window, cx| {
                                this.share(ev.position(), window, cx)
                            }),
                        ),
                    )
                    .child(hairline_v(theme))
                    .child(
                        icon_button(
                            "save",
                            "square.and.arrow.down",
                            l("Save to the default folder (⌘S)"),
                            theme,
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.save_quick(window, cx)))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, ev: &gpui::MouseDownEvent, _, cx| {
                                this.open_popup(Popup::Save, ev.position, cx)
                            }),
                        ),
                    ),
            )
            .child(
                crate::ui::controls::pill("copy", l("Copy"), theme, true)
                    .child(div().text_size(px(10.)).opacity(0.6).child("⌘C"))
                    .tooltip(|_, cx| tooltip_view(l("Copy image (⌘C)").into(), cx))
                    .on_click(cx.listener(|this, _, window, cx| this.copy_image(window, cx))),
            )
    }

    fn popup_view(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (kind, at) = self.popup?;
        let entity = cx.entity();
        let dismiss = {
            let e = entity.clone();
            move |window: &mut Window, cx: &mut gpui::App| {
                e.update(cx, |this, cx| this.close_popup(window, cx));
            }
        };
        let act = |f: fn(&mut Editor, &mut Window, &mut Context<Editor>)| {
            let e = entity.clone();
            move |window: &mut Window, cx: &mut gpui::App| {
                e.update(cx, |this, cx| f(this, window, cx))
            }
        };
        let items: Vec<MenuItem> = match kind {
            Popup::Smart => vec![
                MenuItem::new(l("Copy Text (OCR)"), act(|t, _, cx| t.copy_text(cx))),
                MenuItem::new(l("Hide Sensitive Data"), act(|t, _, cx| t.auto_redact(cx))),
                MenuItem::new(l("Pin to Screen"), act(|t, w, cx| t.pin(w, cx))),
                MenuItem::separator(),
                MenuItem::new(
                    l("Remove All Annotations"),
                    act(|t, _, cx| t.clear_annotations(cx)),
                )
                .enabled(!self.annotations.is_empty()),
                MenuItem::new(l("Reset Crop"), act(|t, _, cx| t.set_crop(None, cx)))
                    .enabled(self.manual_crop.is_some()),
            ],
            Popup::More => vec![
                MenuItem::new(
                    l("Restore Default"),
                    act(|t, _, cx| t.set_style(crate::model::Style::default(), cx)),
                ),
                MenuItem::new(format!("{}…", l("Shortcuts")), {
                    let e = entity.clone();
                    move |_, cx| e.update(cx, |this, cx| this.open_popup(Popup::Shortcuts, at, cx))
                }),
            ],
            Popup::Zoom => {
                let mut items = vec![
                    MenuItem::new(
                        format!("{} (⌘+)", l("Zoom In")),
                        act(|t, _, cx| t.zoom_in(cx)),
                    ),
                    MenuItem::new(
                        format!("{} (⌘−)", l("Zoom Out")),
                        act(|t, _, cx| t.zoom_out(cx)),
                    ),
                    MenuItem::new(l("Fit in Window"), act(|t, _, cx| t.set_zoom(None, cx))),
                    MenuItem::separator(),
                ];
                for z in ZOOM_STEPS {
                    let e = entity.clone();
                    items.push(
                        MenuItem::new(format!("{}%", (z * 100.).round() as i64), move |_, cx| {
                            e.update(cx, |this, cx| this.set_zoom(Some(z), cx))
                        })
                        .checked(self.zoom == Some(z)),
                    );
                }
                let anchor = point(at.x, at.y - px(24. * items.len() as f32 + 20.));
                return Some(menu(anchor, items, theme, dismiss));
            }
            Popup::Save => vec![MenuItem::new(
                l("Save As…"),
                act(|t, w, cx| t.save_as(w, cx)),
            )],
            Popup::ToolGroup(g) => g
                .tools()
                .iter()
                .map(|t| {
                    let tool = *t;
                    let e = entity.clone();
                    MenuItem::new(tool.title(), move |_, cx| {
                        e.update(cx, |this, cx| this.set_tool(tool, cx))
                    })
                    .checked(self.tool == tool)
                })
                .collect(),
            Popup::Context => self.context_items(&entity),
            Popup::StyleColor => return Some(self.style_popover(at, theme, cx, dismiss)),
            Popup::Shortcuts => return Some(shortcuts_popover(at, theme, dismiss)),
            Popup::SavePreset => {
                return Some(self.save_preset_popover(at, theme, window, cx, dismiss));
            }
            Popup::PresetMenu(id) => self.preset_items(id, &entity),
            Popup::Aspect => self.aspect_items(&entity),
        };
        Some(menu(at, items, theme, dismiss))
    }

    fn context_items(&self, entity: &gpui::Entity<Editor>) -> Vec<MenuItem> {
        let targets = self.menu_targets();
        let e = entity.clone();
        if targets.is_empty() {
            return vec![
                MenuItem::new(l("Select All"), move |_, cx| {
                    e.update(cx, |t, cx| t.select_all(cx))
                })
                .enabled(!self.annotations.is_empty()),
            ];
        }
        let mut items = Vec::new();
        if targets.len() == 1
            && let Some(id) = targets.iter().next().copied()
            && let Some(kind) = self.annotations.iter().find(|a| a.id == id).map(|a| a.kind)
            && matches!(kind, AnnotationKind::Text | AnnotationKind::Counter)
        {
            let e = entity.clone();
            items.push(MenuItem::new(
                if kind == AnnotationKind::Counter {
                    l("Edit Number")
                } else {
                    l("Edit Text")
                },
                move |w, cx| e.update(cx, |t, cx| t.start_editing(id, w, cx)),
            ));
            items.push(MenuItem::separator());
        }
        let with = |f: fn(&mut Editor, &HashSet<uuid::Uuid>, &mut Context<Editor>)| {
            let e = entity.clone();
            let t = targets.clone();
            move |_: &mut Window, cx: &mut gpui::App| e.update(cx, |this, cx| f(this, &t, cx))
        };
        items.push(MenuItem::new(
            l("Duplicate"),
            with(|t, ids, cx| {
                t.duplicate(ids, None, true);
                t.dirty(cx)
            }),
        ));
        items.push(MenuItem::new(
            l("Bring to Front"),
            with(|t, ids, cx| {
                t.selection = ids.clone();
                t.bring_to_front(ids, cx)
            }),
        ));
        items.push(MenuItem::new(
            l("Send to Back"),
            with(|t, ids, cx| {
                t.selection = ids.clone();
                t.send_to_back(ids, cx)
            }),
        ));
        items.push(MenuItem::separator());
        items.push(MenuItem::new(l("Delete"), with(|t, ids, cx| t.delete(ids, cx))).destructive());
        items
    }

    fn style_popover(
        &self,
        at: Point<Pixels>,
        theme: &Theme,
        cx: &mut Context<Self>,
        dismiss: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        let mut swatches = div().flex().gap(px(8.)).items_center();
        for (i, c) in annotation_palette().into_iter().enumerate() {
            let selected = self.color == c;
            swatches = swatches.child(
                div()
                    .id(("swatch", i))
                    .size(px(20.))
                    .rounded_full()
                    .bg(c.gpui())
                    .border_1()
                    .border_color(Hsla {
                        a: 0.25,
                        ..theme.text
                    })
                    .cursor_pointer()
                    .when(selected, |d| d.border_2().border_color(theme.accent))
                    .on_click(cx.listener(move |this, _, _, cx| this.apply_color(c, cx))),
            );
        }
        swatches = swatches.child(
            div()
                .id("custom-color")
                .cursor_pointer()
                .child(icon("paintpalette.fill", 18., theme.secondary))
                .tooltip(|_, cx| tooltip_view(l("Custom color").into(), cx))
                .on_click(
                    cx.listener(|this, _, _, cx| this.pick_color(ColorTarget::Annotation, cx)),
                ),
        );
        let mut seg = div()
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(7.))
            .bg(theme.fill);
        for level in 0..3usize {
            let active = self.stroke_level == level;
            let weight = [0., 0.4, 0.8][level];
            seg = seg.child(
                div()
                    .id(("stroke", level))
                    .flex_1()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(active, |d| d.bg(theme.fill_strong))
                    .child(crate::ui::icons::icon_weighted(
                        "line.diagonal",
                        13.,
                        weight,
                        theme.text,
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| this.apply_stroke(level, cx))),
            );
        }
        let content = div()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(swatches)
            .child(
                seg.id("thickness")
                    .tooltip(|_, cx| tooltip_view(l("Thickness / text size").into(), cx)),
            );
        popover(at, Corner::TopLeft, content, theme, dismiss)
    }

    fn save_preset_popover(
        &self,
        at: Point<Pixels>,
        theme: &Theme,
        _: &mut Window,
        cx: &mut Context<Self>,
        dismiss: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> AnyElement {
        let content =
            div()
                .p(px(14.))
                .w(px(230.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child(l("Save Style")),
                )
                .child(input_frame(theme).child(self.preset_input.clone()))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(button("preset-cancel", l("Cancel"), theme).on_click(
                            cx.listener(|this, _, window, cx| this.close_popup(window, cx)),
                        ))
                        .child(primary_button("preset-save", l("Save"), theme).on_click(
                            cx.listener(|this, _, window, cx| this.save_preset(window, cx)),
                        )),
                );
        popover(at, Corner::TopLeft, content, theme, dismiss)
    }

    fn preset_items(&self, id: uuid::Uuid, entity: &gpui::Entity<Editor>) -> Vec<MenuItem> {
        let Some(preset) = crate::settings::presets().into_iter().find(|p| p.id == id) else {
            return vec![];
        };
        let (e1, e2) = (entity.clone(), entity.clone());
        let style = preset.style.clone();
        vec![
            MenuItem::new(l("Apply Style"), move |_, cx| {
                e1.update(cx, |t, cx| t.set_style(style.clone(), cx))
            }),
            MenuItem::new(l("Update with Current Style"), move |_, cx| {
                let current = e2.read(cx).style.clone();
                crate::settings::update_presets(|p| {
                    if let Some(x) = p.iter_mut().find(|x| x.id == id) {
                        x.style = current;
                    }
                });
                e2.update(cx, |t, cx| {
                    t.preset_thumbs.clear();
                    cx.notify()
                });
            }),
            MenuItem::separator(),
            MenuItem::new(l("Delete"), move |_, _| {
                crate::settings::update_presets(|p| p.retain(|x| x.id != id));
            })
            .destructive(),
        ]
    }

    fn aspect_items(&self, entity: &gpui::Entity<Editor>) -> Vec<MenuItem> {
        crate::model::AspectRatio::ALL
            .into_iter()
            .map(|a| {
                let e = entity.clone();
                MenuItem::new(a.label(), move |_, cx| {
                    e.update(cx, |t, cx| t.update_style(cx, |s| s.aspect = a))
                })
                .checked(self.style.aspect == a)
            })
            .collect()
    }

    /// Inline editor over a text or counter annotation being edited.
    fn text_editor(&self, theme: &Theme) -> Option<AnyElement> {
        let id = self.editing?;
        let a = self.annotations.iter().find(|a| a.id == id)?;
        let view = self.canvas.view?;
        let origin = view.bounds.origin;
        if a.kind == AnnotationKind::Counter {
            let c = view.to_local(a.p1);
            let x = c.x + a.counter_radius() * view.scale + 8.;
            return Some(
                div()
                    .absolute()
                    .left(origin.x + px(x as f32))
                    .top(origin.y + px(c.y as f32 - 11.))
                    .w(px(64.))
                    .child(input_frame(theme).child(self.text_input.clone()))
                    .into_any_element(),
            );
        }
        let p = view.to_local(a.p1);
        Some(
            div()
                .absolute()
                .left(origin.x + px(p.x as f32 - 2.))
                .top(origin.y + px(p.y as f32 - 2.))
                .min_w(px(60.))
                .p(px(2.))
                .rounded(px(3.))
                .border_1()
                .border_dashed()
                .border_color(theme.accent)
                .child(self.text_input.clone())
                .into_any_element(),
        )
    }
}

pub fn input_frame(theme: &Theme) -> Div {
    div()
        .px(px(6.))
        .py(px(3.))
        .rounded(px(5.))
        .border_1()
        .border_color(theme.border)
        .bg(if theme.dark {
            hsla(0., 0., 0.1, 1.)
        } else {
            hsla(0., 0., 1., 1.)
        })
        .text_color(theme.text)
}

fn shortcuts_popover(
    at: Point<Pixels>,
    theme: &Theme,
    dismiss: impl Fn(&mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let rows = [
        ("⌘C".to_string(), l("Copy image")),
        ("⌘S".into(), l("Save")),
        ("⌘Z".into(), l("Undo")),
        ("⌫".into(), l("Delete annotation")),
        (l("⇧ click"), l("Add to selection")),
        ("⌘A".into(), l("Select All")),
        ("⌘D".into(), l("Duplicate")),
        (l("⌥ drag"), l("Duplicate while moving")),
        (l("Double-click"), l("Edit text or number")),
        ("← → ↑ ↓".into(), l("Nudge (⇧ = 10 px)")),
        ("V A R O H T N P B C".into(), l("Tools")),
        ("⌥⌘I".into(), l("Show or hide inspector")),
    ];
    let mut grid = div()
        .p(px(14.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .text_size(px(13.));
    for (k, v) in rows {
        grid = grid.child(
            div()
                .flex()
                .gap(px(14.))
                .child(
                    div()
                        .w(px(150.))
                        .font_family("Menlo")
                        .text_color(theme.secondary)
                        .child(k),
                )
                .child(div().text_color(theme.text).child(v)),
        );
    }
    popover(at, Corner::TopRight, grid, theme, dismiss)
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(window);
        self.request_thumb(window, cx);
        let toolbar = self.toolbar(&theme, cx);
        let bottom = self.bottom_bar(&theme, cx);
        let canvas = self.render_canvas(window, cx);
        let popup = self.popup_view(&theme, window, cx);
        let text_editor = self.text_editor(&theme);
        let inspector = self
            .show_inspector
            .then(|| self.inspector(&theme, window, cx));
        let focus: FocusHandle = self.focus.clone();

        let mut stage = div()
            .flex_1()
            .relative()
            .overflow_hidden()
            .bg(theme.canvas)
            .child(canvas);
        if let Some(busy) = self.busy.clone() {
            stage = stage.child(
                div()
                    .absolute()
                    .top(px(12.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(toast_capsule(&theme).child(busy)),
            );
        }
        if let Some(toast) = self.toast.clone() {
            stage = stage.child(
                div()
                    .absolute()
                    .bottom(px(16.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(toast_capsule(&theme).child(toast)),
            );
        }

        div()
            .key_context(super::CONTEXT)
            .track_focus(&focus)
            .size_full()
            .flex()
            .bg(theme.window)
            .text_color(theme.text)
            .font_family(".SystemUIFont")
            .on_key_down(cx.listener(Self::on_key))
            .on_action(cx.listener(|t, _: &CopyImage, w, cx| t.copy_image(w, cx)))
            .on_action(cx.listener(|t, _: &Save, w, cx| t.save_quick(w, cx)))
            .on_action(cx.listener(|t, _: &SaveAs, w, cx| t.save_as(w, cx)))
            .on_action(cx.listener(|t, _: &Undo, _, cx| t.undo(cx)))
            .on_action(cx.listener(|t, _: &Redo, _, cx| t.redo(cx)))
            .on_action(cx.listener(|t, _: &Duplicate, _, cx| {
                let sel = t.selection.clone();
                t.duplicate(&sel, None, true);
                t.dirty(cx)
            }))
            .on_action(cx.listener(|t, _: &SelectAll, _, cx| t.select_all(cx)))
            .on_action(cx.listener(|t, _: &Pin, w, cx| t.pin(w, cx)))
            .on_action(cx.listener(|t, _: &ZoomIn, _, cx| t.zoom_in(cx)))
            .on_action(cx.listener(|t, _: &ZoomOut, _, cx| t.zoom_out(cx)))
            .on_action(cx.listener(|t, _: &ActualSize, _, cx| t.set_zoom(Some(1.), cx)))
            .on_action(cx.listener(|t, _: &ZoomToFit, _, cx| t.set_zoom(None, cx)))
            .on_action(cx.listener(|t, _: &ToggleInspector, _, cx| t.toggle_inspector(cx)))
            .on_action(cx.listener(|t, _: &CopyText, _, cx| t.copy_text(cx)))
            .on_action(cx.listener(|t, _: &Redact, _, cx| t.auto_redact(cx)))
            .on_action(cx.listener(|_, _: &CloseWindow, w, _| w.remove_window()))
            .on_action(cx.listener(|t, _: &super::BringToFront, _, cx| {
                let sel = t.selection.clone();
                t.bring_to_front(&sel, cx)
            }))
            .on_action(cx.listener(|t, _: &super::SendToBack, _, cx| {
                let sel = t.selection.clone();
                t.send_to_back(&sel, cx)
            }))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(toolbar)
                    .child(divider_h(&theme))
                    .child(stage)
                    .child(divider_h(&theme))
                    .child(bottom),
            )
            .when_some(inspector, |d, i| {
                d.child(divider_v(&theme))
                    .child(div().w(px(284.)).flex_none().h_full().child(i))
            })
            .when_some(text_editor, |d, t| d.child(t))
            .when_some(popup, |d, p| d.child(p))
    }
}

fn toast_capsule(theme: &Theme) -> Div {
    div()
        .px(px(14.))
        .py(px(8.))
        .rounded_full()
        .bg(if theme.dark {
            hsla(0., 0., 0.22, 0.95)
        } else {
            hsla(0., 0., 1., 0.95)
        })
        .border_1()
        .border_color(theme.border)
        .shadow_md()
        .text_size(px(13.))
        .text_color(theme.text)
}
