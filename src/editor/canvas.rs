use std::collections::HashSet;

use gpui::{
    Bounds, BoxShadow, Context, Corners, CursorStyle, Hitbox, HitboxBehavior, Modifiers,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent,
    Window, canvas, hsla, point, prelude::*, px, size,
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, StrokeDash, Transform};
use uuid::Uuid;

use super::{Editor, Popup};
use crate::l10n::l;
use crate::model::{Annotation, AnnotationKind, Pt, Rect, Tool, distance_to_segment, pt};
use crate::render::{CanvasLayout, draw_annotations};

const MARGIN: f32 = 28.;

#[derive(Clone)]
pub enum DragState {
    Idle,
    Creating(Uuid),
    Moving {
        starts: Vec<Annotation>,
        origin: Pt,
        duplicate: bool,
        collapse_to: Option<Uuid>,
    },
    Handle {
        id: Uuid,
        index: usize,
        start: Annotation,
    },
    Cropping(Pt),
    Marquee {
        origin: Pt,
        base: HashSet<Uuid>,
    },
}

#[derive(Default)]
pub struct CanvasState {
    pub drag: Option<DragState>,
    pub drag_start_view: Option<Point<Pixels>>,
    pub crop_draft: Option<Rect>,
    pub undo_pending: Option<String>,
    pub marquee: Option<Rect>,
    pub hover: Option<Uuid>,
    pub guide_x: Option<f64>,
    pub guide_y: Option<f64>,
    pub drag_scale: f64,
    pub scroll: Point<Pixels>,
    pub view: Option<View>,
    pub modifiers: Modifiers,
}

/// Where the composition sits inside the canvas element.
#[derive(Clone, Copy)]
pub struct View {
    pub bounds: Bounds<Pixels>,
    /// Canvas rect in element-local points.
    pub fit: Rect,
    /// Points per canvas pixel.
    pub scale: f64,
    pub layout: CanvasLayout,
    pub display_scale: f64,
}

impl View {
    pub fn to_source(&self, p: Point<Pixels>) -> Pt {
        let lx = f32::from(p.x - self.bounds.origin.x) as f64;
        let ly = f32::from(p.y - self.bounds.origin.y) as f64;
        self.layout.to_source(pt(
            (lx - self.fit.x) / self.scale,
            (ly - self.fit.y) / self.scale,
        ))
    }

    /// Source pixel → element-local point.
    pub fn to_local(&self, p: Pt) -> Pt {
        let c = self.layout.to_canvas(p);
        pt(self.fit.x + c.x * self.scale, self.fit.y + c.y * self.scale)
    }

    pub fn to_window(&self, p: Pt) -> Point<Pixels> {
        let l = self.to_local(p);
        point(
            self.bounds.origin.x + px(l.x as f32),
            self.bounds.origin.y + px(l.y as f32),
        )
    }

    pub fn local_rect(&self, r: Rect) -> Rect {
        let a = self.to_local(pt(r.x, r.y));
        Rect::new(a.x, a.y, r.w * self.scale, r.h * self.scale)
    }
}

pub fn handles(a: &Annotation) -> Vec<Pt> {
    match a.kind {
        AnnotationKind::Arrow => vec![a.p1, a.p2],
        k if k.is_box() => {
            let r = a.rect();
            vec![
                pt(r.x, r.y),
                pt(r.max_x(), r.y),
                pt(r.max_x(), r.max_y()),
                pt(r.x, r.max_y()),
            ]
        }
        AnnotationKind::Text => {
            let b = a.bounds();
            vec![pt(b.max_x(), b.max_y())]
        }
        _ => vec![],
    }
}

/// Composites a transparent render over a checkerboard, for the "transparent" background.
pub fn on_checkerboard(pm: &Pixmap, scale: f64) -> Pixmap {
    let mut out = Pixmap::new(pm.width(), pm.height()).unwrap_or_else(|| pm.clone());
    out.fill(tiny_skia::Color::WHITE);
    let cell = ((10. * scale * 2.).round() as u32).max(4);
    let mut paint = Paint::default();
    paint.set_color_rgba8(224, 224, 224, 255);
    for y in (0..pm.height()).step_by(cell as usize) {
        for x in (0..pm.width()).step_by(cell as usize) {
            if ((x / cell) + (y / cell)) % 2 == 0
                && let Some(r) =
                    tiny_skia::Rect::from_xywh(x as f32, y as f32, cell as f32, cell as f32)
            {
                out.fill_rect(r, &paint, Transform::identity(), None);
            }
        }
    }
    out.draw_pixmap(
        0,
        0,
        pm.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    out
}

impl Editor {
    fn view_for(&self, bounds: Bounds<Pixels>, display_scale: f64) -> View {
        let layout = self.layout();
        let (cw, ch) = (layout.canvas_w, layout.canvas_h);
        let (bw, bh) = (
            f32::from(bounds.size.width) as f64,
            f32::from(bounds.size.height) as f64,
        );
        let m = MARGIN as f64;
        let fit_scale = ((bw - m * 2.).max(10.) / cw).min((bh - m * 2.).max(10.) / ch);
        let fit_zoom = fit_scale * display_scale;
        let s = self.zoom.unwrap_or(fit_zoom) / display_scale;
        let content_w = bw.max(cw * s + m * 2.);
        let content_h = bh.max(ch * s + m * 2.);
        let sx = f32::from(self.canvas.scroll.x) as f64;
        let sy = f32::from(self.canvas.scroll.y) as f64;
        let fit = Rect::new(
            ((content_w - cw * s) / 2.).round() - sx,
            ((content_h - ch * s) / 2.).round() - sy,
            cw * s,
            ch * s,
        );
        View {
            bounds,
            fit,
            scale: s,
            layout,
            display_scale,
        }
    }

    fn clamp_scroll(&mut self, view: &View) {
        let (bw, bh) = (
            f32::from(view.bounds.size.width) as f64,
            f32::from(view.bounds.size.height) as f64,
        );
        let m = MARGIN as f64;
        let max_x = (view.layout.canvas_w * view.scale + m * 2. - bw).max(0.) / 2.;
        let max_y = (view.layout.canvas_h * view.scale + m * 2. - bh).max(0.) / 2.;
        self.canvas.scroll = point(
            px((f32::from(self.canvas.scroll.x) as f64).clamp(-max_x, max_x) as f32),
            px((f32::from(self.canvas.scroll.y) as f64).clamp(-max_y, max_y) as f32),
        );
    }

    /// The canvas element: composition, annotation layer and all pointer handling.
    pub fn render_canvas(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let base = self.base_image();
        let display_scale = window.scale_factor() as f64;
        let checker = self.style.background.is_none();
        let editor = entity.clone();
        canvas(
            move |bounds, window, cx| {
                let view = editor.update(cx, |this, cx| {
                    let view = this.view_for(bounds, display_scale);
                    let fit_zoom = {
                        let v = this.view_for_fit(bounds, display_scale);
                        v.scale * display_scale
                    };
                    if (this.fit_zoom - fit_zoom).abs() > 1e-6 {
                        this.fit_zoom = fit_zoom;
                    }
                    this.canvas.view = Some(view);
                    this.request_base((view.scale * display_scale).min(1.), window, cx);
                    this.refresh_overlay(&view, window, cx);
                    view
                });
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                (view, hitbox)
            },
            move |bounds, (view, hitbox): (View, Hitbox), window, cx| {
                window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                    let fit = Bounds::new(
                        point(
                            bounds.origin.x + px(view.fit.x as f32),
                            bounds.origin.y + px(view.fit.y as f32),
                        ),
                        size(px(view.fit.w as f32), px(view.fit.h as f32)),
                    );
                    if let Some(base) = base.clone() {
                        if !checker {
                            window.paint_shadows(
                                fit,
                                Corners::default(),
                                &[BoxShadow {
                                    color: hsla(0., 0., 0., 0.22),
                                    offset: point(px(0.), px(6.)),
                                    blur_radius: px(18.),
                                    spread_radius: px(0.),
                                }],
                            );
                        }
                        window
                            .paint_image(fit, Corners::default(), base, 0, false)
                            .ok();
                    }
                    if let Some(overlay) = entity.read(cx).overlay.clone() {
                        window
                            .paint_image(bounds, Corners::default(), overlay, 0, false)
                            .ok();
                    }
                });

                let cursor = entity.read(cx).cursor_at(window.mouse_position(), &view);
                window.set_cursor_style(cursor, &hitbox);

                let e = entity.clone();
                let hb = hitbox.clone();
                window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                    if !phase.bubble() || !hb.is_hovered(window) {
                        return;
                    }
                    e.update(cx, |this, cx| match ev.button {
                        MouseButton::Left => this.mouse_down(ev, window, cx),
                        MouseButton::Right => this.right_click(ev, window, cx),
                        _ => {}
                    });
                });
                let e = entity.clone();
                let hb = hitbox.clone();
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, cx| {
                    if !phase.bubble() {
                        return;
                    }
                    e.update(cx, |this, cx| {
                        this.canvas.modifiers = ev.modifiers;
                        if this.canvas.drag.is_some()
                            && ev.pressed_button == Some(MouseButton::Left)
                        {
                            this.mouse_drag(ev.position, window, cx);
                        } else if hb.is_hovered(window) {
                            this.hover(ev.position, cx);
                        }
                    });
                });
                let e = entity.clone();
                window.on_mouse_event(move |ev: &MouseUpEvent, phase, window, cx| {
                    if !phase.bubble() || ev.button != MouseButton::Left {
                        return;
                    }
                    e.update(cx, |this, cx| {
                        if this.canvas.drag.is_some() {
                            this.mouse_up(ev.position, window, cx);
                        }
                    });
                });
                let e = entity.clone();
                let hb = hitbox;
                window.on_mouse_event(move |ev: &ScrollWheelEvent, phase, window, cx| {
                    if !phase.bubble() || !hb.should_handle_scroll(window) {
                        return;
                    }
                    e.update(cx, |this, cx| {
                        if this.zoom.is_none() {
                            return;
                        }
                        let d = ev.delta.pixel_delta(px(16.));
                        this.canvas.scroll =
                            point(this.canvas.scroll.x - d.x, this.canvas.scroll.y - d.y);
                        if let Some(view) = this.canvas.view {
                            this.clamp_scroll(&view);
                        }
                        this.dirty(cx);
                    });
                });
            },
        )
        .size_full()
    }

    fn view_for_fit(&self, bounds: Bounds<Pixels>, display_scale: f64) -> View {
        let layout = self.layout();
        let (bw, bh) = (
            f32::from(bounds.size.width) as f64,
            f32::from(bounds.size.height) as f64,
        );
        let m = MARGIN as f64;
        let s = ((bw - m * 2.).max(10.) / layout.canvas_w)
            .min((bh - m * 2.).max(10.) / layout.canvas_h);
        View {
            bounds,
            fit: Rect::default(),
            scale: s,
            layout,
            display_scale,
        }
    }

    // MARK: Overlay

    fn overlay_hash(&self, view: &View) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!(
            "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{}",
            self.annotations,
            self.selection
                .iter()
                .collect::<std::collections::BTreeSet<_>>(),
            self.editing,
            self.canvas.marquee,
            self.canvas.crop_draft,
            (self.canvas.guide_x, self.canvas.guide_y),
            (view.fit, view.scale, view.bounds.size),
            self.tool,
            self.manual_crop,
            self.effects.is_some()
        )
        .hash(&mut h);
        h.finish()
    }

    fn refresh_overlay(&mut self, view: &View, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.overlay_hash(view);
        if self.overlay_key == Some(key) && self.overlay.is_some() {
            return;
        }
        self.overlay_key = Some(key);
        let image = self
            .draw_overlay(view)
            .map(|pm| crate::ui::render_image(&pm));
        let old = std::mem::replace(&mut self.overlay, image);
        crate::ui::drop_image(old, window, cx);
    }

    fn draw_overlay(&self, view: &View) -> Option<Pixmap> {
        let ds = view.display_scale;
        let w = (f32::from(view.bounds.size.width) as f64 * ds).round() as u32;
        let h = (f32::from(view.bounds.size.height) as f64 * ds).round() as u32;
        let mut pm = Pixmap::new(w.max(1), h.max(1))?;
        let layout = view.layout;
        let k = view.scale * ds;
        // Source pixels → overlay device pixels.
        let ts = Transform::from_scale(ds as f32, ds as f32)
            .pre_translate(view.fit.x as f32, view.fit.y as f32)
            .pre_scale(view.scale as f32, view.scale as f32)
            .pre_translate(
                (layout.image_rect.x - layout.crop.x) as f32,
                (layout.image_rect.y - layout.crop.y) as f32,
            );
        let clip = {
            let r = Rect::new(
                view.fit.x * ds,
                view.fit.y * ds,
                view.fit.w * ds,
                view.fit.h * ds,
            );
            let mut m = tiny_skia::Mask::new(pm.width(), pm.height())?;
            if let Some(rr) = r.skia() {
                m.fill_path(
                    &PathBuilder::from_rect(rr),
                    FillRule::Winding,
                    false,
                    Transform::identity(),
                );
            }
            m
        };
        let hidden = self.editing.filter(|id| {
            self.annotations
                .iter()
                .any(|a| a.id == *id && a.kind == AnnotationKind::Text)
        });
        let mut layer = Pixmap::new(pm.width(), pm.height())?;
        draw_annotations(
            &mut layer,
            &self.annotations,
            self.effects.as_deref(),
            &layout,
            ts,
            k,
            hidden,
        );
        pm.draw_pixmap(
            0,
            0,
            layer.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            Some(&clip),
        );

        // Selection chrome, in view space.
        let accent: gpui::Rgba = crate::platform::accent_color();
        let accent_c = tiny_skia::Color::from_rgba(accent.r, accent.g, accent.b, 1.)
            .unwrap_or(tiny_skia::Color::BLACK);
        let mut ap = Paint::default();
        ap.set_color(accent_c);
        ap.anti_alias = true;
        let dts = Transform::from_scale(ds as f32, ds as f32);
        let stroke_rect = |pm: &mut Pixmap, r: Rect, paint: &Paint, width: f32, dash: bool| {
            if let Some(rr) = r.skia() {
                let path = PathBuilder::from_rect(rr);
                let stroke = Stroke {
                    width,
                    dash: if dash {
                        StrokeDash::new(vec![5., 3.], 0.)
                    } else {
                        None
                    },
                    ..Default::default()
                };
                pm.stroke_path(&path, paint, &stroke, dts, None);
            }
        };
        for a in self
            .annotations
            .iter()
            .filter(|a| self.selection.contains(&a.id))
        {
            if self.editing == Some(a.id) {
                continue;
            }
            if a.kind != AnnotationKind::Arrow || self.selection.len() > 1 {
                let r = view.local_rect(a.bounds()).inset(-4., -4.);
                stroke_rect(&mut pm, r, &ap, 1.5, true);
            }
        }
        if let Some(a) = self.selected()
            && self.editing != Some(a.id)
        {
            let mut white = Paint::default();
            white.set_color(tiny_skia::Color::WHITE);
            white.anti_alias = true;
            for hpt in handles(a) {
                let p = view.to_local(hpt);
                if let Some(c) = PathBuilder::from_circle(p.x as f32, p.y as f32, 5.) {
                    pm.fill_path(&c, &white, FillRule::Winding, dts, None);
                    pm.stroke_path(
                        &c,
                        &ap,
                        &Stroke {
                            width: 1.,
                            ..Default::default()
                        },
                        dts,
                        None,
                    );
                }
            }
        }

        if self.canvas.guide_x.is_some() || self.canvas.guide_y.is_some() {
            let c = view.local_rect(self.crop());
            let mut pink = Paint::default();
            pink.set_color_rgba8(255, 45, 85, 255);
            pink.anti_alias = true;
            let mut pb = PathBuilder::new();
            if let Some(x) = self.canvas.guide_x {
                let vx = view.to_local(pt(x, 0.)).x as f32;
                pb.move_to(vx, c.y as f32);
                pb.line_to(vx, c.max_y() as f32);
            }
            if let Some(y) = self.canvas.guide_y {
                let vy = view.to_local(pt(0., y)).y as f32;
                pb.move_to(c.x as f32, vy);
                pb.line_to(c.max_x() as f32, vy);
            }
            if let Some(p) = pb.finish() {
                pm.stroke_path(
                    &p,
                    &pink,
                    &Stroke {
                        width: 1.,
                        ..Default::default()
                    },
                    dts,
                    None,
                );
            }
        }

        if let Some(m) = self.canvas.marquee {
            let r = view.local_rect(m);
            if let Some(rr) = r.skia() {
                let mut fill = Paint::default();
                fill.set_color(
                    tiny_skia::Color::from_rgba(accent.r, accent.g, accent.b, 0.12)
                        .unwrap_or(accent_c),
                );
                pm.fill_rect(rr, &fill, dts, None);
            }
            stroke_rect(&mut pm, r, &ap, 1., false);
        }

        if self.tool == Tool::Crop {
            let current = self.canvas.crop_draft.unwrap_or(self.crop());
            let full = view.fit.union(&view.local_rect(self.full));
            let cr = view.local_rect(current);
            let mut pb = PathBuilder::new();
            if let (Some(a), Some(b)) = (full.skia(), cr.skia()) {
                pb.push_rect(a);
                pb.push_rect(b);
            }
            if let Some(p) = pb.finish() {
                let mut dim = Paint::default();
                dim.set_color_rgba8(0, 0, 0, 115);
                pm.fill_path(&p, &dim, FillRule::EvenOdd, dts, None);
            }
            let mut white = Paint::default();
            white.set_color(tiny_skia::Color::WHITE);
            white.anti_alias = true;
            stroke_rect(&mut pm, cr, &white, 1.5, false);
            let mut thirds = Paint::default();
            thirds.set_color_rgba8(255, 255, 255, 102);
            let mut pb = PathBuilder::new();
            for i in 1..=2 {
                let x = (cr.x + cr.w * i as f64 / 3.) as f32;
                let y = (cr.y + cr.h * i as f64 / 3.) as f32;
                pb.move_to(x, cr.y as f32);
                pb.line_to(x, cr.max_y() as f32);
                pb.move_to(cr.x as f32, y);
                pb.line_to(cr.max_x() as f32, y);
            }
            if let Some(p) = pb.finish() {
                pm.stroke_path(
                    &p,
                    &thirds,
                    &Stroke {
                        width: 0.8,
                        ..Default::default()
                    },
                    dts,
                    None,
                );
            }
        }
        Some(pm)
    }

    // MARK: Hit testing

    fn tolerance(&self, view: &View) -> f64 {
        8. / view.scale
    }

    fn handle_hit(&self, p: Pt, tol: f64) -> Option<(Annotation, usize)> {
        let a = self.selected()?;
        if self.editing == Some(a.id) {
            return None;
        }
        handles(a)
            .iter()
            .position(|h| (h.x - p.x).hypot(h.y - p.y) <= tol * 1.4)
            .map(|i| (a.clone(), i))
    }

    /// With a drawing tool active, only outlines and arrows are grabbed, plus anything already
    /// selected except the inside of an outlined shape, so drawing on top of another shape still
    /// draws.
    fn grabbable(&self, p: Pt, tol: f64) -> Option<Annotation> {
        self.annotations
            .iter()
            .rev()
            .find(|a| {
                a.grab_test(p, tol)
                    || (self.selection.contains(&a.id)
                        && !matches!(a.kind, AnnotationKind::Rect | AnnotationKind::Ellipse)
                        && a.hit_test(p, tol))
            })
            .cloned()
    }

    fn cursor_at(&self, position: Point<Pixels>, view: &View) -> CursorStyle {
        if let Some(DragState::Moving { .. }) = self.canvas.drag {
            return CursorStyle::ClosedHand;
        }
        if self.editing.is_some() || self.tool == Tool::Crop {
            return CursorStyle::Crosshair;
        }
        let p = view.to_source(position);
        let tol = self.tolerance(view);
        if self.handle_hit(p, tol).is_some() {
            return CursorStyle::Crosshair;
        }
        let hit = self.annotations.iter().rev().find(|a| a.hit_test(p, tol));
        let grab = if self.tool == Tool::Select {
            hit.is_some()
        } else {
            self.grabbable(p, tol).is_some()
                || (self.tool == Tool::Counter
                    && hit.is_some_and(|h| h.kind == AnnotationKind::Counter))
        };
        if grab {
            CursorStyle::OpenHand
        } else if self.tool == Tool::Select {
            CursorStyle::Arrow
        } else {
            CursorStyle::Crosshair
        }
    }

    fn hover(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(view) = self.canvas.view else { return };
        let p = view.to_source(position);
        let tol = self.tolerance(&view);
        let hit = self
            .annotations
            .iter()
            .rev()
            .find(|a| a.hit_test(p, tol))
            .map(|a| a.id);
        if hit != self.canvas.hover {
            self.canvas.hover = hit;
        }
        cx.notify();
    }

    // MARK: Gestures

    fn mouse_down(&mut self, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.popup.is_some() {
            self.close_popup(window, cx);
        }
        let Some(view) = self.canvas.view else { return };
        if self.editing.is_some() {
            self.finish_text(window, cx);
        }
        window.focus(&self.focus);
        let p = view.to_source(ev.position);
        let tol = self.tolerance(&view);
        let flags = ev.modifiers;
        let additive = flags.shift || flags.platform;
        let clicks = ev.click_count;
        self.canvas.drag = Some(DragState::Idle);
        self.canvas.drag_start_view = Some(ev.position);
        self.canvas.drag_scale = view.scale;
        self.canvas.modifiers = flags;

        if self.tool != Tool::Crop
            && let Some((a, i)) = self.handle_hit(p, tol)
        {
            self.canvas.undo_pending = Some(l("Resize"));
            self.canvas.drag = Some(DragState::Handle {
                id: a.id,
                index: i,
                start: a,
            });
            self.dirty(cx);
            return;
        }

        match self.tool {
            Tool::Select => {
                let Some(hit) = self
                    .annotations
                    .iter()
                    .rev()
                    .find(|a| a.hit_test(p, tol))
                    .cloned()
                else {
                    if !additive {
                        self.selection.clear();
                    }
                    self.canvas.drag = Some(DragState::Marquee {
                        origin: p,
                        base: self.selection.clone(),
                    });
                    self.dirty(cx);
                    return;
                };
                if clicks >= 2 && matches!(hit.kind, AnnotationKind::Text | AnnotationKind::Counter)
                {
                    self.canvas.drag = None;
                    self.start_editing(hit.id, window, cx);
                    return;
                }
                let mut collapse = None;
                if additive {
                    if self.selection.contains(&hit.id) {
                        self.selection.remove(&hit.id);
                        self.dirty(cx);
                        return;
                    }
                    self.selection.insert(hit.id);
                } else if !self.selection.contains(&hit.id) {
                    self.selection = HashSet::from([hit.id]);
                } else if self.selection.len() > 1 {
                    collapse = Some(hit.id);
                }
                self.start_moving(p, flags.alt, collapse);
            }
            Tool::Crop => {
                self.canvas.drag = Some(DragState::Cropping(p));
            }
            Tool::Text => {
                if let Some(hit) = self
                    .annotations
                    .iter()
                    .rev()
                    .find(|a| a.kind == AnnotationKind::Text && a.hit_test(p, tol))
                    .cloned()
                {
                    self.canvas.drag = None;
                    self.start_editing(hit.id, window, cx);
                    return;
                }
                if let Some(hit) = self.grabbable(p, tol) {
                    self.select_and_move(&hit, p, flags.alt);
                    self.dirty(cx);
                    return;
                }
                let mut a =
                    Annotation::new(AnnotationKind::Text, p, p, self.color, self.stroke_width());
                a.font_size = self.font_size();
                self.add(a.clone());
                self.selection = HashSet::from([a.id]);
                self.canvas.drag = None;
                self.begin_text_input(&a, window, cx);
            }
            tool => {
                if tool == Tool::Counter
                    && let Some(hit) = self
                        .annotations
                        .iter()
                        .rev()
                        .find(|a| a.kind == AnnotationKind::Counter && a.hit_test(p, tol))
                        .cloned()
                {
                    if clicks >= 2 {
                        self.canvas.drag = None;
                        self.start_editing(hit.id, window, cx);
                    } else {
                        self.select_and_move(&hit, p, flags.alt);
                    }
                    self.dirty(cx);
                    return;
                }
                if let Some(hit) = self.grabbable(p, tol) {
                    self.select_and_move(&hit, p, flags.alt);
                    self.dirty(cx);
                    return;
                }
                let Some(kind) = tool.annotation_kind() else {
                    return;
                };
                let mut a = Annotation::new(kind, p, p, self.color, self.stroke_width());
                a.font_size = self.font_size();
                let id = a.id;
                self.begin_drawing(a);
                self.canvas.drag = Some(DragState::Creating(id));
            }
        }
        self.dirty(cx);
    }

    fn select_and_move(&mut self, hit: &Annotation, p: Pt, duplicating: bool) {
        if !self.selection.contains(&hit.id) {
            self.selection = HashSet::from([hit.id]);
        }
        self.start_moving(p, duplicating, None);
    }

    /// ⌥-drag moves copies and leaves the originals in place. The copies are made only once the
    /// pointer actually moves, so a plain ⌥-click doesn't stack a hidden duplicate.
    fn start_moving(&mut self, p: Pt, duplicating: bool, collapse_to: Option<Uuid>) {
        self.canvas.undo_pending = if duplicating { None } else { Some(l("Move")) };
        self.canvas.drag = Some(DragState::Moving {
            starts: self.selected_annotations(),
            origin: p,
            duplicate: duplicating,
            collapse_to,
        });
    }

    fn mouse_drag(&mut self, position: Point<Pixels>, _: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.canvas.view else { return };
        let p = view.to_source(position);
        let Some(drag) = self.canvas.drag.clone() else {
            return;
        };
        if let Some(name) = self.canvas.undo_pending.clone() {
            let register = match &drag {
                DragState::Moving { origin, .. } => (p.x - origin.x).hypot(p.y - origin.y) > 0.5,
                DragState::Handle { .. } => true,
                _ => false,
            };
            if register {
                self.register_undo(&name);
                self.canvas.undo_pending = None;
            }
        }
        if let DragState::Moving {
            origin,
            duplicate: true,
            ..
        } = &drag
            && (p.x - origin.x).hypot(p.y - origin.y) > 0.5
        {
            let sel = self.selection.clone();
            self.duplicate(&sel, Some((0., 0.)), true);
            self.canvas.drag = Some(DragState::Moving {
                starts: self.selected_annotations(),
                origin: *origin,
                duplicate: false,
                collapse_to: None,
            });
        }
        let Some(current) = self.canvas.drag.clone() else {
            return;
        };
        match current {
            DragState::Idle => {}
            DragState::Creating(id) => self.update_annotation(id, |a| {
                if a.kind == AnnotationKind::Counter {
                    a.p1 = p;
                    a.p2 = p;
                } else {
                    a.p2 = p;
                }
            }),
            DragState::Moving {
                starts,
                origin,
                duplicate,
                ..
            } => {
                if !duplicate {
                    let (mut dx, mut dy) = (p.x - origin.x, p.y - origin.y);
                    self.snap(&mut dx, &mut dy, &starts);
                    for start in &starts {
                        let s = start.clone();
                        self.update_annotation(start.id, |a| {
                            *a = s;
                            a.offset(dx, dy);
                        });
                    }
                }
            }
            DragState::Handle { id, index, start } => {
                self.update_annotation(id, |a| match a.kind {
                    AnnotationKind::Arrow => {
                        if index == 0 {
                            a.p1 = p
                        } else {
                            a.p2 = p
                        }
                    }
                    AnnotationKind::Text => {
                        let b = start.bounds();
                        let f = ((p.x - b.x) / b.w.max(1.)).max((p.y - b.y) / b.h.max(1.));
                        a.font_size = (start.font_size * f).max(6.);
                    }
                    _ => {
                        let r = start.rect();
                        // Opposite corner stays fixed.
                        let fixed = [
                            pt(r.max_x(), r.max_y()),
                            pt(r.x, r.max_y()),
                            pt(r.x, r.y),
                            pt(r.max_x(), r.y),
                        ][index.min(3)];
                        a.p1 = fixed;
                        a.p2 = p;
                    }
                })
            }
            DragState::Cropping(origin) => {
                self.canvas.crop_draft =
                    Some(Rect::from_points(origin, p).intersection(&self.full));
            }
            DragState::Marquee { origin, base } => {
                let r = Rect::from_points(origin, p);
                self.canvas.marquee = Some(r);
                let mut sel = base.clone();
                sel.extend(
                    self.annotations
                        .iter()
                        .filter(|a| r.intersects(&a.bounds()))
                        .map(|a| a.id),
                );
                self.selection = sel;
            }
        }
        self.dirty(cx);
    }

    /// Smart guides: the moving group's edges and center snap to the screenshot's center and
    /// edges and to the other annotations. Holding ⌘ moves freely.
    fn snap(&mut self, dx: &mut f64, dy: &mut f64, starts: &[Annotation]) {
        let free = self.canvas.modifiers.platform;
        let Some(first) = starts.first() else {
            return;
        };
        if free {
            self.canvas.guide_x = None;
            self.canvas.guide_y = None;
            return;
        }
        let mut group = starts
            .iter()
            .skip(1)
            .fold(first.bounds(), |acc, a| acc.union(&a.bounds()));
        group.x += *dx;
        group.y += *dy;
        let ids: HashSet<Uuid> = starts.iter().map(|a| a.id).collect();
        let c = self.crop();
        let mut xs = vec![c.x, c.mid_x(), c.max_x()];
        let mut ys = vec![c.y, c.mid_y(), c.max_y()];
        for a in self.annotations.iter().filter(|a| !ids.contains(&a.id)) {
            let b = a.bounds();
            xs.extend([b.x, b.mid_x(), b.max_x()]);
            ys.extend([b.y, b.mid_y(), b.max_y()]);
        }
        let tol = 6. / self.canvas.drag_scale.max(0.0001);
        let best = |mine: [f64; 3], targets: &[f64]| -> Option<(f64, f64)> {
            let mut out: Option<(f64, f64)> = None;
            for m in mine {
                for &t in targets {
                    if (t - m).abs() <= tol
                        && (t - m).abs() < out.map(|o| o.0.abs()).unwrap_or(f64::INFINITY)
                    {
                        out = Some((t - m, t));
                    }
                }
            }
            out
        };
        let sx = best([group.x, group.mid_x(), group.max_x()], &xs);
        let sy = best([group.y, group.mid_y(), group.max_y()], &ys);
        if let Some(s) = sx {
            *dx += s.0;
        }
        if let Some(s) = sy {
            *dy += s.0;
        }
        let engaged = sx.is_some_and(|s| Some(s.1) != self.canvas.guide_x)
            || sy.is_some_and(|s| Some(s.1) != self.canvas.guide_y);
        if engaged {
            crate::platform::haptic_align();
        }
        self.canvas.guide_x = sx.map(|s| s.1);
        self.canvas.guide_y = sy.map(|s| s.1);
    }

    fn mouse_up(&mut self, position: Point<Pixels>, _: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.canvas.view else { return };
        let moved = self
            .canvas
            .drag_start_view
            .map(|s| {
                let d = position - s;
                f32::from(d.x).hypot(f32::from(d.y)) > 3.
            })
            .unwrap_or(false);
        let Some(drag) = self.canvas.drag.take() else {
            return;
        };
        match drag {
            DragState::Creating(id) => {
                let Some(a) = self.annotations.iter().find(|a| a.id == id).cloned() else {
                    return;
                };
                let min = 4. / view.scale;
                let too_small = match a.kind {
                    AnnotationKind::Counter => false,
                    AnnotationKind::Arrow => distance_to_segment(a.p2, a.p1, a.p1) < min * 3.,
                    _ => a.rect().w < min || a.rect().h < min,
                };
                if too_small {
                    self.cancel_drawing(id);
                    self.selection.clear();
                } else {
                    self.commit_drawing(id);
                    self.selection = if a.kind == AnnotationKind::Counter {
                        HashSet::new()
                    } else {
                        HashSet::from([id])
                    };
                }
            }
            DragState::Moving { collapse_to, .. } => {
                self.canvas.guide_x = None;
                self.canvas.guide_y = None;
                if !moved && let Some(c) = collapse_to {
                    self.selection = HashSet::from([c]);
                }
                self.canvas.undo_pending = None;
            }
            DragState::Handle { .. } => self.canvas.undo_pending = None,
            DragState::Cropping(_) => {
                if let Some(r) = self.canvas.crop_draft.take()
                    && r.w > 8.
                    && r.h > 8.
                {
                    self.set_crop(Some(r), cx);
                    self.set_tool(Tool::Select, cx);
                }
            }
            DragState::Marquee { .. } => self.canvas.marquee = None,
            DragState::Idle => {}
        }
        self.dirty(cx);
    }

    fn right_click(&mut self, ev: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.canvas.view else { return };
        let p = view.to_source(ev.position);
        let tol = self.tolerance(&view);
        self.canvas.hover = self
            .annotations
            .iter()
            .rev()
            .find(|a| a.hit_test(p, tol))
            .map(|a| a.id);
        self.open_popup(Popup::Context, ev.position, cx);
    }

    /// Right-clicking an unselected annotation acts on it alone; otherwise on the selection.
    pub fn menu_targets(&self) -> HashSet<Uuid> {
        match self.canvas.hover {
            Some(h) if !self.selection.contains(&h) => HashSet::from([h]),
            _ => self.selection.clone(),
        }
    }
}
