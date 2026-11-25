use std::sync::{Arc, OnceLock};

use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable, Hsla, MouseButton, RenderImage, SharedString,
    Window, div, hsla, img, prelude::*, px,
};
use tiny_skia::{Paint, Pixmap, Transform};

use super::view::input_frame;
use super::{BackgroundKind, ColorTarget, Editor, Popup};
use crate::l10n::{l, lf};
use crate::model::{Art, Background, Rgba, Style, WindowFrame, solid_presets};
use crate::render::{RenderInput, SourceImage, render};
use crate::settings;
use crate::ui::controls::{chevron, inspector_row, slider_row, tabs, toggle_row};
use crate::ui::{Theme, icon, render_image, tooltip_view};

fn sample() -> &'static SourceImage {
    static SAMPLE: OnceLock<SourceImage> = OnceLock::new();
    SAMPLE.get_or_init(|| {
        let (w, h) = (480u32, 300u32);
        let mut pm = Pixmap::new(w, h).expect("sample");
        pm.fill(tiny_skia::Color::from_rgba8(250, 250, 250, 255));
        let mut fill = |x: f32, y: f32, w: f32, h: f32, c: [u8; 3]| {
            let mut p = Paint::default();
            p.set_color_rgba8(c[0], c[1], c[2], 255);
            if let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) {
                pm.fill_rect(r, &p, Transform::identity(), None);
            }
        };
        fill(0., 0., w as f32, 46., [230, 230, 230]);
        for (i, width) in [260., 340., 200., 300.].into_iter().enumerate() {
            fill(36., 84. + i as f32 * 44., width, 16., [209, 209, 209]);
        }
        fill(36., 230., 120., 34., [62, 140, 99]);
        SourceImage { pixmap: pm }
    })
}

fn style_thumb(style: &Style) -> Option<Arc<RenderImage>> {
    let src = sample();
    let pm = render(&RenderInput {
        source: src,
        style,
        crop: src.full_rect(),
        edge_color: Rgba::hex("#F2F2F2"),
        annotations: &[],
        effects: None,
        hidden: None,
        scale: 0.3,
    })?;
    Some(render_image(&pm))
}

fn art_thumb(art: &Art) -> Option<Arc<RenderImage>> {
    let pm = crate::render::art::THUMB_CACHE.image(art, 96., 96.)?;
    Some(render_image(&pm))
}

fn section_title(text: impl Into<SharedString>, theme: &Theme) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.text)
        .child(text.into())
}

fn swatch_shape(selected: bool, theme: &Theme) -> Div {
    div()
        .relative()
        .w_full()
        .h(px(30.))
        .rounded(px(6.))
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .when(selected, |d| d.border_2().border_color(theme.accent))
}

impl Editor {
    pub(super) fn inspector(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sections = settings::get().inspector_sections;
        let open = |k: &str| sections.get(k).copied().unwrap_or(false);
        let gallery = self.style_gallery(theme, cx);
        let background = self.background_picker(theme, window, cx);
        let frame = self.collapsible(
            "frame",
            l("Frame"),
            Some(self.style.frame.title()),
            open("frame"),
            theme,
            cx,
            |this, theme, cx| this.frame_controls(theme, cx),
        );
        let aspect_label = self.style.aspect.label();
        let short = aspect_label.split(" · ").next().unwrap_or("").to_string();
        let layout = self.collapsible(
            "layout",
            l("Layout"),
            Some(short),
            open("layout"),
            theme,
            cx,
            |this, theme, cx| this.layout_controls(theme, cx),
        );
        let advanced = self.collapsible(
            "advanced",
            l("Advanced"),
            None,
            open("advanced"),
            theme,
            cx,
            |this, theme, cx| this.advanced_controls(theme, cx),
        );
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.panel)
            .child(div().h(px(52.)).flex_none())
            .child(crate::ui::divider_h(theme))
            .child(
                div()
                    .id("inspector-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(24.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.))
                                    .child(section_title(l("Styles"), theme))
                                    .child(gallery),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.))
                                    .child(section_title(l("Background"), theme))
                                    .child(background),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.))
                                    .child(frame)
                                    .child(layout)
                                    .child(advanced),
                            ),
                    ),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn collapsible(
        &mut self,
        key: &'static str,
        title: String,
        summary: Option<String>,
        expanded: bool,
        theme: &Theme,
        cx: &mut Context<Self>,
        content: impl FnOnce(&mut Self, &Theme, &mut Context<Self>) -> Div,
    ) -> Div {
        let header = div()
            .id(key)
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .child(section_title(title, theme))
            .child(div().flex_1())
            .when_some(summary.filter(|_| !expanded), |d, s| {
                d.child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme.secondary)
                        .child(s),
                )
            })
            .child(chevron(expanded, theme))
            .on_click(cx.listener(move |_, _, _, cx| {
                settings::update(|s| {
                    let v = s.inspector_sections.get(key).copied().unwrap_or(false);
                    s.inspector_sections.insert(key.into(), !v);
                });
                cx.notify();
            }));
        let mut d = div().flex().flex_col().gap(px(12.)).child(header);
        if expanded {
            d = d.child(content(self, theme, cx));
        }
        d
    }

    fn style_gallery(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let presets = settings::presets();
        let mut grid = div().flex().flex_wrap().gap(px(8.));
        let card_w = px(76.);
        grid = grid.child(
            div()
                .id("save-style")
                .w(card_w)
                .flex()
                .flex_col()
                .gap(px(4.))
                .cursor_pointer()
                .child(
                    div()
                        .h(px(54.))
                        .rounded(px(7.))
                        .border_1()
                        .border_dashed()
                        .border_color(theme.tertiary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("plus", 13., theme.secondary)),
                )
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(theme.secondary)
                        .flex()
                        .justify_center()
                        .child(l("Save")),
                )
                .tooltip(|_, cx| tooltip_view(l("Save the current style").into(), cx))
                .on_click(cx.listener(|this, ev: &ClickEvent, window, cx| {
                    let n = settings::presets().len() + 1;
                    this.preset_input.update(cx, |t, cx| {
                        t.set_text(lf("Style %d", n), cx);
                        t.select_all_text(cx);
                    });
                    let focus = this.preset_input.read(cx).focus_handle(cx);
                    window.focus(&focus);
                    this.open_popup(Popup::SavePreset, ev.position(), cx);
                })),
        );
        for p in presets {
            let key = serde_json::to_string(&p.style).unwrap_or_default();
            let thumb = match self.preset_thumbs.get(&key) {
                Some(t) => Some(t.clone()),
                None => {
                    let t = style_thumb(&p.style);
                    if let Some(t) = &t {
                        self.preset_thumbs.insert(key, t.clone());
                    }
                    t
                }
            };
            let selected = p.style == self.style;
            let style = p.style.clone();
            let id = p.id;
            let name: SharedString = p.name.clone().into();
            let tip = name.clone();
            grid = grid.child(
                div()
                    .id(SharedString::from(format!("preset-{id}")))
                    .w(card_w)
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .cursor_pointer()
                    .child(
                        div()
                            .h(px(54.))
                            .rounded(px(7.))
                            .overflow_hidden()
                            .border_1()
                            .border_color(theme.border)
                            .when(selected, |d| d.border_2().border_color(theme.accent))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(thumb, |d, t| d.child(img(t).w_full().h_full())),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(theme.text)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .flex()
                            .justify_center()
                            .child(name),
                    )
                    .tooltip(move |_, cx| tooltip_view(tip.clone(), cx))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_style(style.clone(), cx)))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            this.open_popup(Popup::PresetMenu(id), ev.position, cx)
                        }),
                    ),
            );
        }
        grid
    }

    fn background_picker(&mut self, theme: &Theme, _: &mut Window, cx: &mut Context<Self>) -> Div {
        let kinds = [
            BackgroundKind::Art,
            BackgroundKind::Solid,
            BackgroundKind::Image,
        ];
        let selected = kinds.iter().position(|k| *k == self.bg_kind).unwrap_or(1);
        let entity = cx.entity();
        let mut col = div().flex().flex_col().gap(px(12.)).child(tabs(
            "bg-kind",
            vec![l("Art").into(), l("Color").into(), l("Image").into()],
            selected,
            theme,
            move |i, _, cx| {
                entity.update(cx, |this, cx| {
                    this.bg_kind = kinds[i];
                    cx.notify();
                })
            },
        ));
        match self.bg_kind {
            BackgroundKind::Art => {
                let presets = Art::presets();
                let visible = if self.show_all_backgrounds {
                    presets.len()
                } else {
                    12
                };
                let current = match &self.style.background {
                    Background::Art { art } => Some(art.clone()),
                    _ => None,
                };
                let mut grid = div().grid().grid_cols(6).gap(px(6.));
                for (i, art) in presets.into_iter().take(visible).enumerate() {
                    let sel = current.as_ref().is_some_and(|c| c.is_variant_of(&art));
                    let key = serde_json::to_string(&art).unwrap_or_default();
                    let thumb = match self.art_thumb_keys.get(&key) {
                        Some(t) => Some(t.clone()),
                        None => {
                            let t = art_thumb(&art);
                            if let Some(t) = &t {
                                self.art_thumb_keys.insert(key, t.clone());
                            }
                            t
                        }
                    };
                    let base = art.palette[0].gpui();
                    grid = grid.child(
                        swatch_shape(sel, theme)
                            .id(("art", i))
                            .cursor_pointer()
                            .bg(base)
                            .when_some(thumb, |d, t| d.child(img(t).size_full()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !sel {
                                    let art = art.clone();
                                    this.update_style(cx, |s| {
                                        s.background = Background::Art { art }
                                    });
                                }
                            })),
                    );
                }
                col = col.child(grid);
                let mut row = div().flex().items_center();
                row = row.child(self.more_button(Art::presets().len(), 12, theme, cx));
                row = row.child(div().flex_1());
                if let Some(art) = current {
                    row = row.child(
                        div()
                            .id("vary")
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .cursor_pointer()
                            .text_size(px(11.))
                            .text_color(theme.secondary)
                            .child(icon("shuffle", 11., theme.secondary))
                            .child(l("Vary"))
                            .tooltip(|_, cx| {
                                tooltip_view(l("Keeps the colors and draws new shapes").into(), cx)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let a = art.reseeded();
                                this.update_style(cx, |s| {
                                    s.background = Background::Art { art: a }
                                });
                            })),
                    );
                }
                col = col.child(row);
            }
            BackgroundKind::Solid => {
                let solids = solid_presets();
                let visible = if self.show_all_backgrounds {
                    solids.len()
                } else {
                    6
                };
                let current = match &self.style.background {
                    Background::Solid { color } => Some(*color),
                    _ => None,
                };
                let mut grid = div().grid().grid_cols(8).gap(px(6.));
                grid = grid.child(
                    swatch_shape(self.style.background.is_none(), theme)
                        .id("bg-none")
                        .cursor_pointer()
                        .bg(hsla(0., 0., 1., 1.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon("nosign", 11., theme.secondary))
                        .tooltip(|_, cx| tooltip_view(l("Transparent").into(), cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.update_style(cx, |s| s.background = Background::None {})
                        })),
                );
                for (i, c) in solids.iter().copied().take(visible).enumerate() {
                    grid = grid.child(
                        swatch_shape(current == Some(c), theme)
                            .id(("solid", i))
                            .cursor_pointer()
                            .bg(c.gpui())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.update_style(cx, |s| s.background = Background::solid(c))
                            })),
                    );
                }
                let custom = current.is_some_and(|c| !solids.contains(&c));
                grid = grid.child(
                    swatch_shape(custom, theme)
                        .id("bg-custom")
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when_some(current.filter(|_| custom), |d, c| d.bg(c.gpui()))
                        .child(icon("paintpalette.fill", 13., theme.secondary))
                        .tooltip(|_, cx| tooltip_view(l("Custom color").into(), cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.pick_color(ColorTarget::Background, cx)
                        })),
                );
                col = col
                    .child(grid)
                    .child(self.more_button(solids.len(), 6, theme, cx));
            }
            BackgroundKind::Image => {
                col = col.child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(
                            crate::ui::controls::button("pick-image", l("Image…"), theme)
                                .flex_1()
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.pick_background_image(cx)),
                                ),
                        )
                        .child(
                            crate::ui::controls::button("wallpaper", l("Wallpaper"), theme)
                                .flex_1()
                                .tooltip(|_, cx| {
                                    tooltip_view(l("Use the current wallpaper").into(), cx)
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.use_wallpaper(cx))),
                        ),
                );
                if matches!(self.style.background, Background::Image { .. }) {
                    let v = self.style.background_blur;
                    let e = cx.entity();
                    col = col.child(slider_row(
                        "bg-blur",
                        l("Blur"),
                        v,
                        0.,
                        1.,
                        None,
                        theme,
                        move |v, _, cx| {
                            e.update(cx, |t, cx| t.update_style(cx, |s| s.background_blur = v))
                        },
                    ));
                }
            }
        }
        col
    }

    fn more_button(
        &mut self,
        total: usize,
        row: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if total <= row {
            return div().into_any_element();
        }
        div()
            .id("more-bg")
            .cursor_pointer()
            .text_size(px(11.))
            .text_color(theme.secondary)
            .child(if self.show_all_backgrounds {
                l("Show Less")
            } else {
                l("Show More")
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.show_all_backgrounds = !this.show_all_backgrounds;
                cx.notify();
            }))
            .into_any_element()
    }

    fn pick_background_image(&mut self, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let path = path.to_string_lossy().to_string();
            this.update(cx, |this, cx| {
                this.update_style(cx, |s| s.background = Background::Image { path })
            })
            .ok();
        })
        .detach();
    }

    fn use_wallpaper(&mut self, cx: &mut Context<Self>) {
        match crate::platform::desktop_image_path().filter(|p| {
            std::path::Path::new(p).is_file() && crate::render::load_image_file(p).is_some()
        }) {
            Some(path) => self.update_style(cx, |s| s.background = Background::Image { path }),
            None => match crate::capture::wallpaper_snapshot() {
                Some(path) => self.update_style(cx, |s| s.background = Background::Image { path }),
                None => self.show_toast(l("Couldn't read the wallpaper"), cx),
            },
        }
    }

    fn frame_controls(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let mut row = div().flex().gap(px(6.));
        for (i, f) in WindowFrame::ALL.into_iter().enumerate() {
            let active = self.style.frame == f;
            let tip: SharedString = f.title().into();
            row = row.child(
                div()
                    .id(("frame", i))
                    .flex_1()
                    .h(px(34.))
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if active {
                        theme.accent_soft
                    } else {
                        theme.fill
                    })
                    .border_1()
                    .border_color(if active {
                        theme.accent
                    } else {
                        Hsla::transparent_black()
                    })
                    .child(frame_glyph(f, theme))
                    .tooltip(move |_, cx| tooltip_view(tip.clone(), cx))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.update_style(cx, |s| s.frame = f)),
                    ),
            );
        }
        row
    }

    fn layout_controls(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = self.style.clone();
        let mut grid = div().flex().flex_col().gap(px(3.));
        for row in 0..3usize {
            let mut r = div().flex().gap(px(3.));
            for col in 0..3usize {
                let i = row * 3 + col;
                let active = s.alignment == i as i64;
                r = r.child(
                    div()
                        .id(("align", i))
                        .w(px(22.))
                        .h(px(14.))
                        .rounded(px(3.))
                        .cursor_pointer()
                        .bg(if active {
                            theme.accent
                        } else {
                            theme.fill_strong
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.update_style(cx, |s| s.alignment = i as i64)
                        })),
                );
            }
            grid = grid.child(r);
        }
        let slider = |id: &'static str,
                      title: String,
                      v: f64,
                      max: f64,
                      f: fn(&mut Style, f64),
                      cx: &mut Context<Self>| {
            let e = cx.entity();
            slider_row(id, title, v, 0., max, None, theme, move |v, _, cx| {
                e.update(cx, |t, cx| t.update_style(cx, |s| f(s, v)))
            })
        };
        let e = cx.entity();
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                inspector_row(l("Aspect Ratio"), theme).child(
                    crate::ui::controls::button("aspect", s.aspect.label(), theme)
                        .flex_1()
                        .on_click(cx.listener(|this, ev: &ClickEvent, _, cx| {
                            this.open_popup(Popup::Aspect, ev.position(), cx)
                        })),
                ),
            )
            .child(
                inspector_row(l("Position"), theme).child(grid.id("alignment").tooltip(|_, cx| {
                    tooltip_view(l("Where the capture sits on the background").into(), cx)
                })),
            )
            .child(
                toggle_row(
                    "auto-balance",
                    l("Auto-balance"),
                    s.auto_balance,
                    theme,
                    move |v, _, cx| {
                        e.update(cx, |t, cx| t.update_style(cx, |s| s.auto_balance = v))
                    },
                )
                .pt(px(4.)),
            )
            .child(slider(
                "padding",
                l("Padding"),
                s.padding,
                0.3,
                |s, v| s.padding = v,
                cx,
            ))
            .child(slider(
                "corners",
                l("Corners"),
                s.corner_radius,
                0.05,
                |s, v| s.corner_radius = v,
                cx,
            ))
            .child(slider(
                "shadow",
                l("Shadow"),
                s.shadow,
                1.,
                |s, v| s.shadow = v,
                cx,
            ))
    }

    fn advanced_controls(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = self.style.clone();
        let mut col = div().flex().flex_col().gap(px(10.)).child(
            inspector_row(l("Signature"), theme).child(
                input_frame(theme)
                    .flex_1()
                    .child(self.watermark_input.clone()),
            ),
        );
        let e = cx.entity();
        col = col.child(slider_row(
            "inset",
            l("Inset"),
            s.inset,
            0.,
            0.08,
            None,
            theme,
            move |v, _, cx| e.update(cx, |t, cx| t.update_style(cx, |s| s.inset = v)),
        ));
        if let Background::Gradient { angle, .. } = &s.background {
            let e = cx.entity();
            col = col.child(slider_row(
                "angle",
                l("Angle"),
                *angle,
                0.,
                360.,
                Some("°"),
                theme,
                move |v, _, cx| {
                    e.update(cx, |t, cx| {
                        t.update_style(cx, |s| {
                            if let Background::Gradient { angle, .. } = &mut s.background {
                                *angle = v;
                            }
                        })
                    })
                },
            ));
        }
        if !s.background.is_none() {
            let e = cx.entity();
            col = col.child(slider_row(
                "grain",
                l("Grain"),
                s.grain,
                0.,
                1.,
                None,
                theme,
                move |v, _, cx| e.update(cx, |t, cx| t.update_style(cx, |s| s.grain = v)),
            ));
        }
        let e = cx.entity();
        col.child(toggle_row(
            "hairline",
            l("Fine edge"),
            s.hairline,
            theme,
            move |v, _, cx| e.update(cx, |t, cx| t.update_style(cx, |s| s.hairline = v)),
        ))
    }
}

fn frame_glyph(frame: WindowFrame, theme: &Theme) -> AnyElement {
    if frame == WindowFrame::None {
        return icon("rectangle.slash", 13., theme.secondary).into_any_element();
    }
    let (bar, body) = if frame.is_dark() {
        (hsla(0., 0., 0.25, 1.), hsla(0., 0., 0.15, 1.))
    } else {
        (hsla(0., 0., 0.88, 1.), hsla(0., 0., 1., 1.))
    };
    let dot = |c: Hsla| div().size(px(3.)).rounded_full().bg(c);
    let mut top = div()
        .h(px(7.))
        .px(px(3.))
        .flex()
        .items_center()
        .gap(px(1.5))
        .bg(bar)
        .child(dot(hsla(0., 0.9, 0.6, 1.)))
        .child(dot(hsla(45. / 360., 0.95, 0.55, 1.)))
        .child(dot(hsla(130. / 360., 0.6, 0.5, 1.)));
    if frame.is_browser() {
        top = top.child(
            div()
                .ml(px(1.))
                .w(px(10.))
                .h(px(3.))
                .rounded_full()
                .bg(body),
        );
    }
    div()
        .w(px(30.))
        .h(px(22.))
        .rounded(px(3.))
        .overflow_hidden()
        .border_1()
        .border_color(Hsla {
            a: 0.2,
            ..theme.text
        })
        .flex()
        .flex_col()
        .child(top)
        .child(div().flex_1().bg(body))
        .into_any_element()
}
