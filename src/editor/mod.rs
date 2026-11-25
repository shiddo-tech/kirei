mod canvas;
mod inspector;
#[cfg(test)]
mod tests;
mod view;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, KeyBinding, Pixels, Point,
    RenderImage, Subscription, Task, TitlebarOptions, Window, WindowBounds, WindowOptions, actions,
    point, px, size,
};
use uuid::Uuid;

use crate::analysis;
use crate::capture::Captured;
use crate::l10n::{l, lf};
use crate::model::{Annotation, AnnotationKind, Rect, Rgba, Style, Tool, annotation_palette};
use crate::render::{CanvasLayout, EffectImages, RenderInput, SourceImage, render};
use crate::settings;
use crate::ui::text_input::{TextInput, TextInputEvent};

actions!(
    kirei_editor,
    [
        CopyImage,
        Save,
        SaveAs,
        Undo,
        Redo,
        Duplicate,
        SelectAll,
        Pin,
        ZoomIn,
        ZoomOut,
        ActualSize,
        ZoomToFit,
        ToggleInspector,
        CopyText,
        Redact,
        CloseWindow,
        BringToFront,
        SendToBack,
    ]
);

pub const CONTEXT: &str = "KireiEditor";

pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("cmd-c", CopyImage, c),
        KeyBinding::new("cmd-s", Save, c),
        KeyBinding::new("cmd-shift-s", SaveAs, c),
        KeyBinding::new("cmd-z", Undo, c),
        KeyBinding::new("cmd-shift-z", Redo, c),
        KeyBinding::new("cmd-d", Duplicate, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("cmd-p", Pin, c),
        KeyBinding::new("cmd-=", ZoomIn, c),
        KeyBinding::new("cmd-+", ZoomIn, c),
        KeyBinding::new("cmd--", ZoomOut, c),
        KeyBinding::new("cmd-0", ActualSize, c),
        KeyBinding::new("cmd-9", ZoomToFit, c),
        KeyBinding::new("alt-cmd-i", ToggleInspector, c),
        KeyBinding::new("cmd-w", CloseWindow, c),
        KeyBinding::new("cmd-]", BringToFront, c),
        KeyBinding::new("cmd-[", SendToBack, c),
        KeyBinding::new("cmd-shift-t", CopyText, c),
        KeyBinding::new("cmd-shift-r", Redact, c),
    ]);
}

pub const ZOOM_STEPS: [f64; 10] = [0.25, 0.33, 0.5, 0.67, 0.75, 1., 1.5, 2., 3., 4.];

#[derive(Clone)]
struct Snapshot {
    annotations: Vec<Annotation>,
    crop: Option<Rect>,
    name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ToolGroup {
    Select,
    Arrow,
    Shape,
    Highlight,
    Text,
    Counter,
    Redact,
    Crop,
}

impl ToolGroup {
    pub const ALL: [ToolGroup; 8] = [
        Self::Select,
        Self::Arrow,
        Self::Shape,
        Self::Highlight,
        Self::Text,
        Self::Counter,
        Self::Redact,
        Self::Crop,
    ];

    pub fn tools(self) -> &'static [Tool] {
        match self {
            Self::Select => &[Tool::Select],
            Self::Arrow => &[Tool::Arrow],
            Self::Shape => &[Tool::Rect, Tool::Ellipse],
            Self::Highlight => &[Tool::Highlight],
            Self::Text => &[Tool::Text],
            Self::Counter => &[Tool::Counter],
            Self::Redact => &[Tool::Pixelate, Tool::Blur],
            Self::Crop => &[Tool::Crop],
        }
    }

    pub fn containing(tool: Tool) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.tools().contains(&tool))
    }
}

pub fn uses_color(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Arrow | Tool::Rect | Tool::Ellipse | Tool::Highlight | Tool::Text | Tool::Counter
    )
}

pub fn symbol(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "cursorarrow",
        Tool::Arrow => "arrow.up.right",
        Tool::Rect => "rectangle",
        Tool::Ellipse => "circle",
        Tool::Highlight => "highlighter",
        Tool::Text => "textformat",
        Tool::Counter => "1.circle",
        Tool::Pixelate => "square.grid.3x3.fill",
        Tool::Blur => "drop.halffull",
        Tool::Crop => "crop",
    }
}

#[derive(Clone, PartialEq)]
struct RenderKey {
    style: Style,
    crop: Rect,
    edge: Rgba,
    scale: f64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Popup {
    StyleColor,
    Smart,
    More,
    Shortcuts,
    Zoom,
    Save,
    SavePreset,
    ToolGroup(ToolGroup),
    Context,
    PresetMenu(Uuid),
    Aspect,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorTarget {
    Annotation,
    Background,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackgroundKind {
    Art,
    Solid,
    Image,
}

pub struct Editor {
    pub source: Arc<SourceImage>,
    pub pixel_scale: f64,
    pub balanced: Rect,
    pub full: Rect,
    pub style: Style,
    pub annotations: Vec<Annotation>,
    pub manual_crop: Option<Rect>,
    pub tool: Tool,
    pub selection: HashSet<Uuid>,
    pub editing: Option<Uuid>,
    pub color: Rgba,
    pub stroke_level: usize,
    pub edge: Rgba,
    pub busy: Option<String>,
    pub zoom: Option<f64>,
    pub fit_zoom: f64,
    pub effects: Option<Arc<EffectImages>>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,

    base: Option<Arc<RenderImage>>,
    base_key: Option<RenderKey>,
    render_generation: u64,
    _render_task: Option<Task<()>>,
    overlay: Option<Arc<RenderImage>>,
    overlay_key: Option<u64>,
    pub(crate) canvas: canvas::CanvasState,

    pub last_in_group: HashMap<ToolGroup, Tool>,
    pub show_inspector: bool,
    pub popup: Option<(Popup, Point<Pixels>)>,
    pub bg_kind: BackgroundKind,
    pub show_all_backgrounds: bool,
    pub text_input: Entity<TextInput>,
    pub watermark_input: Entity<TextInput>,
    pub preset_input: Entity<TextInput>,
    thumb: Option<Arc<RenderImage>>,
    thumb_key: Option<RenderKey>,
    _thumb_task: Option<Task<()>>,
    pub preset_thumbs: HashMap<String, Arc<RenderImage>>,
    pub art_thumbs: HashMap<i64, Arc<RenderImage>>,
    pub art_thumb_keys: HashMap<String, Arc<RenderImage>>,
    pub toast: Option<String>,
    _toast_task: Option<Task<()>>,
    color_target: Option<ColorTarget>,
    _color_task: Option<Task<()>>,
    pub focus: FocusHandle,
    pub thumb_press: Option<Point<Pixels>>,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub fn open(captured: Captured, cx: &mut App) {
    let Some(source) = SourceImage::from_bgra(captured.width, captured.height, &captured.bgra)
    else {
        return;
    };
    open_source(Arc::new(source), captured.scale as f64, cx);
}

pub fn open_source(source: Arc<SourceImage>, pixel_scale: f64, cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.), px(820.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("Kirei".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(16.), px(19.))),
        }),
        focus: !crate::dev_mode(),
        window_min_size: Some(size(px(980.), px(620.))),
        ..Default::default()
    };
    let opened = cx.open_window(options, move |window, cx| {
        let editor = cx.new(|cx| Editor::new(source, pixel_scale, window, cx));
        editor.update(cx, |e, _| window.focus(&e.focus));
        editor
    });
    if let Err(e) = opened {
        eprintln!("kirei: editor não abriu: {e:#}");
    }
    crate::activate(cx);
}

impl Editor {
    fn new(
        source: Arc<SourceImage>,
        pixel_scale: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let full = source.full_rect();
        let balanced = analysis::balanced_rect(&source);
        let style = settings::last_style();
        let text_input = cx.new(|cx| {
            let mut t = TextInput::new(l("Type…"), cx);
            t.bold = true;
            t
        });
        let watermark_input = cx.new(|cx| {
            let mut t = TextInput::new(l("@yourname"), cx);
            t.set_text(style.watermark.clone(), cx);
            t
        });
        let preset_input = cx.new(|cx| TextInput::new(l("Name"), cx));
        let mut subs = vec![
            cx.subscribe_in(&text_input, window, Self::on_text_event),
            cx.subscribe(&watermark_input, |this, _, ev: &TextInputEvent, cx| {
                if let TextInputEvent::Changed(text) = ev {
                    this.style.watermark = text.clone();
                    this.style_changed(cx);
                }
            }),
            cx.subscribe_in(
                &preset_input,
                window,
                |this, _, ev: &TextInputEvent, window, cx| match ev {
                    TextInputEvent::Submit => this.save_preset(window, cx),
                    TextInputEvent::Cancel => this.close_popup(window, cx),
                    _ => {}
                },
            ),
        ];
        let focus = cx.focus_handle();
        let input_focus = text_input.read(cx).focus_handle(cx);
        subs.push(cx.on_blur(&input_focus, window, |this, window, cx| {
            if this.editing.is_some() {
                this.finish_text(window, cx);
            }
        }));
        subs.push(cx.observe_window_appearance(window, |this, _, cx| {
            this.preset_thumbs.clear();
            cx.notify();
        }));

        let fx_source = source.clone();
        let fx_task = cx.spawn(async move |this, cx| {
            let fx = cx
                .background_executor()
                .spawn(async move { EffectImages::new(&fx_source) })
                .await;
            this.update(cx, |this, cx| {
                this.effects = Some(Arc::new(fx));
                this.overlay_key = None;
                cx.notify();
            })
            .ok();
        });
        fx_task.detach();

        let mut editor = Self {
            source,
            pixel_scale,
            balanced,
            full,
            style,
            annotations: Vec::new(),
            manual_crop: None,
            tool: Tool::Select,
            selection: HashSet::new(),
            editing: None,
            color: annotation_palette()[0],
            stroke_level: 1,
            edge: Rgba::WHITE,
            busy: None,
            zoom: None,
            fit_zoom: 1.,
            effects: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            base: None,
            base_key: None,
            render_generation: 0,
            _render_task: None,
            overlay: None,
            overlay_key: None,
            canvas: canvas::CanvasState::default(),
            last_in_group: HashMap::new(),
            show_inspector: settings::get()
                .inspector_sections
                .get("visible")
                .copied()
                .unwrap_or(true),
            popup: None,
            bg_kind: BackgroundKind::Solid,
            show_all_backgrounds: false,
            text_input,
            watermark_input,
            preset_input,
            thumb: None,
            thumb_key: None,
            _thumb_task: None,
            preset_thumbs: HashMap::new(),
            art_thumbs: HashMap::new(),
            art_thumb_keys: HashMap::new(),
            toast: None,
            _toast_task: None,
            color_target: None,
            _color_task: None,
            focus,
            thumb_press: None,
            _subscriptions: subs,
        };
        editor.update_edge_color();
        editor.sync_background_kind();
        if settings::get().auto_redact {
            editor.auto_redact(cx);
        }
        editor
    }

    // MARK: Geometry

    pub fn crop(&self) -> Rect {
        if let Some(m) = self.manual_crop {
            return m;
        }
        if self.style.auto_balance {
            self.balanced
        } else {
            self.full
        }
    }

    /// While cropping, the whole image is shown so the crop can grow.
    pub fn display_crop(&self) -> Rect {
        if self.tool == Tool::Crop {
            self.full
        } else {
            self.crop()
        }
    }

    pub fn layout(&self) -> CanvasLayout {
        CanvasLayout::new(&self.style, self.display_crop())
    }

    /// Reference unit for default annotation sizes.
    pub fn unit(&self) -> f64 {
        let c = self.crop();
        (c.w.max(c.h) / 320.).max(2.)
    }

    pub fn stroke_width(&self) -> f64 {
        self.unit() * [1.2, 2., 3.2][self.stroke_level.min(2)]
    }

    pub fn font_size(&self) -> f64 {
        self.unit() * [9., 13., 19.][self.stroke_level.min(2)]
    }

    pub fn selected(&self) -> Option<&Annotation> {
        if self.selection.len() != 1 {
            return None;
        }
        let id = self.selection.iter().next()?;
        self.annotations.iter().find(|a| a.id == *id)
    }

    pub fn selected_annotations(&self) -> Vec<Annotation> {
        self.annotations
            .iter()
            .filter(|a| self.selection.contains(&a.id))
            .cloned()
            .collect()
    }

    fn update_edge_color(&mut self) {
        self.edge = analysis::edge_color(&self.source, self.crop());
    }

    pub fn set_tool(&mut self, tool: Tool, cx: &mut Context<Self>) {
        if tool != self.tool && tool != Tool::Select {
            self.selection.clear();
        }
        self.tool = tool;
        self.canvas.crop_draft = None;
        if let Some(g) = ToolGroup::containing(tool) {
            self.last_in_group.insert(g, tool);
        }
        self.dirty(cx);
    }

    pub fn set_style(&mut self, style: Style, cx: &mut Context<Self>) {
        let old = std::mem::replace(&mut self.style, style);
        if old.auto_balance != self.style.auto_balance || old.inset != self.style.inset {
            self.update_edge_color();
        }
        if old.watermark != self.style.watermark {
            let text = self.style.watermark.clone();
            self.watermark_input.update(cx, |t, cx| {
                if t.text() != text {
                    t.set_text(text, cx)
                }
            });
        }
        self.sync_background_kind();
        self.style_changed(cx);
    }

    pub fn update_style(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Style)) {
        let mut s = self.style.clone();
        f(&mut s);
        self.set_style(s, cx);
    }

    fn style_changed(&mut self, cx: &mut Context<Self>) {
        settings::set_last_style(&self.style);
        self.dirty(cx);
    }

    pub fn dirty(&mut self, cx: &mut Context<Self>) {
        self.overlay_key = None;
        cx.notify();
    }

    fn sync_background_kind(&mut self) {
        use crate::model::Background as B;
        self.bg_kind = match self.style.background {
            B::Image { .. } => BackgroundKind::Image,
            B::Art { .. } | B::Gradient { .. } | B::Mesh { .. } => BackgroundKind::Art,
            B::None {} | B::Solid { .. } => BackgroundKind::Solid,
        };
        let hidden = match &self.style.background {
            B::Solid { color } => {
                let solids = crate::model::solid_presets();
                !solids[..6].contains(color) && solids.contains(color)
            }
            B::Art { art } => !crate::model::Art::presets()[..12]
                .iter()
                .any(|p| art.is_variant_of(p)),
            _ => false,
        };
        if hidden {
            self.show_all_backgrounds = true;
        }
    }

    // MARK: Undo

    fn snapshot(&self, name: &str) -> Snapshot {
        Snapshot {
            annotations: self.annotations.clone(),
            crop: self.manual_crop,
            name: name.to_string(),
        }
    }

    pub fn register_undo(&mut self, name: &str) {
        let snap = self.snapshot(name);
        self.push_undo(snap);
    }

    fn push_undo(&mut self, snap: Snapshot) {
        self.undo_stack.push(snap);
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn restore(&mut self, snap: Snapshot) {
        let crop_changed = self.manual_crop != snap.crop;
        self.annotations = snap.annotations;
        self.manual_crop = snap.crop;
        let ids: HashSet<Uuid> = self.annotations.iter().map(|a| a.id).collect();
        self.selection.retain(|id| ids.contains(id));
        if crop_changed {
            self.update_edge_color();
        }
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) {
        if let Some(snap) = self.undo_stack.pop() {
            let current = self.snapshot(&snap.name);
            self.restore(snap);
            self.redo_stack.push(current);
            self.dirty(cx);
        }
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) {
        if let Some(snap) = self.redo_stack.pop() {
            let current = self.snapshot(&snap.name);
            self.restore(snap);
            self.undo_stack.push(current);
            self.dirty(cx);
        }
    }

    // MARK: Editing

    pub fn update_annotation(&mut self, id: Uuid, f: impl FnOnce(&mut Annotation)) {
        if let Some(a) = self.annotations.iter_mut().find(|a| a.id == id) {
            f(a);
        }
    }

    pub fn add(&mut self, a: Annotation) {
        self.register_undo(&l("Add"));
        self.annotations.push(a);
    }

    pub fn begin_drawing(&mut self, a: Annotation) {
        self.annotations.push(a);
    }

    pub fn commit_drawing(&mut self, id: Uuid) {
        let mut before = self.snapshot(&l("Add"));
        before.annotations.retain(|a| a.id != id);
        self.push_undo(before);
    }

    pub fn cancel_drawing(&mut self, id: Uuid) {
        self.annotations.retain(|a| a.id != id);
    }

    pub fn delete(&mut self, ids: &HashSet<Uuid>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        self.register_undo(&l("Delete"));
        self.annotations.retain(|a| !ids.contains(&a.id));
        self.selection.retain(|id| !ids.contains(id));
        self.dirty(cx);
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.tool = Tool::Select;
        self.selection = self.annotations.iter().map(|a| a.id).collect();
        self.dirty(cx);
    }

    pub fn duplicate(&mut self, ids: &HashSet<Uuid>, offset: Option<(f64, f64)>, undo: bool) {
        let originals: Vec<Annotation> = self
            .annotations
            .iter()
            .filter(|a| ids.contains(&a.id))
            .cloned()
            .collect();
        if originals.is_empty() {
            return;
        }
        if undo {
            self.register_undo(&l("Duplicate"));
        }
        let (dx, dy) = offset.unwrap_or((self.unit() * 6., self.unit() * 6.));
        let copies: Vec<Annotation> = originals
            .into_iter()
            .map(|mut c| {
                c.id = Uuid::new_v4();
                c.offset(dx, dy);
                c
            })
            .collect();
        self.selection = copies.iter().map(|c| c.id).collect();
        self.annotations.extend(copies);
    }

    pub fn bring_to_front(&mut self, ids: &HashSet<Uuid>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        self.register_undo(&l("Bring to Front"));
        let (moved, rest): (Vec<_>, Vec<_>) = self
            .annotations
            .drain(..)
            .partition(|a| ids.contains(&a.id));
        self.annotations = rest;
        self.annotations.extend(moved);
        self.dirty(cx);
    }

    pub fn send_to_back(&mut self, ids: &HashSet<Uuid>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        self.register_undo(&l("Send to Back"));
        let (mut moved, rest): (Vec<_>, Vec<_>) = self
            .annotations
            .drain(..)
            .partition(|a| ids.contains(&a.id));
        moved.extend(rest);
        self.annotations = moved;
        self.dirty(cx);
    }

    pub fn nudge_selection(&mut self, dx: f64, dy: f64, cx: &mut Context<Self>) {
        if self.selection.is_empty() {
            return;
        }
        self.register_undo(&l("Move"));
        let sel = self.selection.clone();
        for a in self.annotations.iter_mut().filter(|a| sel.contains(&a.id)) {
            a.offset(dx, dy);
        }
        self.dirty(cx);
    }

    pub fn apply_color(&mut self, c: Rgba, cx: &mut Context<Self>) {
        self.color = c;
        if !self.selection.is_empty() {
            self.register_undo(&l("Color"));
            let sel = self.selection.clone();
            for a in self.annotations.iter_mut().filter(|a| sel.contains(&a.id)) {
                a.color = c;
            }
        }
        self.dirty(cx);
    }

    pub fn apply_stroke(&mut self, level: usize, cx: &mut Context<Self>) {
        self.stroke_level = level;
        if !self.selection.is_empty() {
            self.register_undo(&l("Thickness"));
            let (w, f) = (self.stroke_width(), self.font_size());
            let sel = self.selection.clone();
            for a in self.annotations.iter_mut().filter(|a| sel.contains(&a.id)) {
                a.width = w;
                if a.kind == AnnotationKind::Text {
                    a.font_size = f;
                }
            }
        }
        self.dirty(cx);
    }

    pub fn set_crop(&mut self, r: Option<Rect>, cx: &mut Context<Self>) {
        self.register_undo(&l("Crop"));
        self.manual_crop = r.map(|r| r.integral().intersection(&self.full));
        self.update_edge_color();
        self.dirty(cx);
    }

    pub fn clear_annotations(&mut self, cx: &mut Context<Self>) {
        if self.annotations.is_empty() {
            return;
        }
        self.register_undo(&l("Clear"));
        self.annotations.clear();
        self.selection.clear();
        self.dirty(cx);
    }

    // MARK: Text editing

    pub fn start_editing(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.annotations.iter().find(|a| a.id == id).cloned() else {
            return;
        };
        self.register_undo(&if a.kind == AnnotationKind::Counter {
            l("Edit Number")
        } else {
            l("Edit Text")
        });
        self.selection = HashSet::from([id]);
        self.begin_text_input(&a, window, cx);
    }

    fn begin_text_input(&mut self, a: &Annotation, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = Some(a.id);
        let text = a.text.clone();
        let placeholder = if a.kind == AnnotationKind::Counter {
            let mut auto = self.annotations.clone();
            if let Some(x) = auto.iter_mut().find(|x| x.id == a.id) {
                x.text.clear();
            }
            crate::model::counter_labels(&auto)
                .get(&a.id)
                .cloned()
                .unwrap_or_default()
        } else {
            l("Type…")
        };
        let color = a.color.gpui();
        let counter = a.kind == AnnotationKind::Counter;
        let size = match self.canvas.view {
            Some(v) if !counter => (a.font_size * v.scale).max(9.) as f32,
            _ => 13.,
        };
        self.text_input.update(cx, |t, cx| {
            t.font_size = px(size);
            t.set_placeholder(placeholder);
            t.set_text(text, cx);
            t.select_all_text(cx);
            t.bold = !counter;
            t.color = (!counter).then(|| color.into());
        });
        let focus = self.text_input.read(cx).focus_handle(cx);
        window.focus(&focus);
        self.dirty(cx);
    }

    fn on_text_event(
        &mut self,
        _: &Entity<TextInput>,
        ev: &TextInputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match ev {
            TextInputEvent::Changed(text) => {
                if let Some(id) = self.editing {
                    let text = text.clone();
                    self.update_annotation(id, |a| a.text = text);
                    self.dirty(cx);
                }
            }
            TextInputEvent::Submit | TextInputEvent::Cancel => {
                self.finish_text(window, cx);
            }
            TextInputEvent::Blur => {}
        }
    }

    pub fn finish_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.editing.take() else {
            return;
        };
        if let Some(a) = self.annotations.iter().find(|a| a.id == id).cloned() {
            if a.kind == AnnotationKind::Counter {
                self.update_annotation(id, |a| a.text = a.text.trim().to_string());
            } else if a.text.trim().is_empty() {
                self.annotations.retain(|a| a.id != id);
                self.selection.remove(&id);
            }
        }
        window.focus(&self.focus);
        self.dirty(cx);
    }

    // MARK: Zoom

    pub fn effective_zoom(&self) -> f64 {
        self.zoom.unwrap_or(self.fit_zoom)
    }

    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        let z = self.effective_zoom();
        self.zoom = Some(
            ZOOM_STEPS
                .into_iter()
                .find(|s| *s > z + 0.001)
                .unwrap_or(ZOOM_STEPS[ZOOM_STEPS.len() - 1]),
        );
        self.canvas.scroll = point(px(0.), px(0.));
        self.dirty(cx);
    }

    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        let z = self.effective_zoom();
        let next = ZOOM_STEPS
            .into_iter()
            .rev()
            .find(|s| *s < z - 0.001)
            .unwrap_or(ZOOM_STEPS[0]);
        // Zooming out past the fit size returns to "fit".
        self.zoom = if next <= self.fit_zoom {
            None
        } else {
            Some(next)
        };
        self.dirty(cx);
    }

    pub fn set_zoom(&mut self, z: Option<f64>, cx: &mut Context<Self>) {
        self.zoom = z;
        self.canvas.scroll = point(px(0.), px(0.));
        self.dirty(cx);
    }

    // MARK: Popups and feedback

    pub fn open_popup(&mut self, p: Popup, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.popup = Some((p, at));
        cx.notify();
    }

    pub fn close_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.popup.take().is_some() && self.editing.is_none() {
            window.focus(&self.focus);
        }
        cx.notify();
    }

    pub fn show_toast(&mut self, text: String, cx: &mut Context<Self>) {
        self.toast = Some(text);
        cx.notify();
        self._toast_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1600))
                .await;
            this.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            })
            .ok();
        }));
    }

    /// Opens the system color panel and follows its color until it closes.
    pub fn pick_color(&mut self, target: ColorTarget, cx: &mut Context<Self>) {
        let start = match target {
            ColorTarget::Annotation => self.color,
            ColorTarget::Background => match &self.style.background {
                crate::model::Background::Solid { color } => *color,
                _ => Rgba::WHITE,
            },
        };
        crate::platform::color_panel::open([start.r, start.g, start.b]);
        self.color_target = Some(target);
        let mut last = Some([start.r, start.g, start.b]);
        self._color_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                let current = crate::platform::color_panel::current();
                let visible = crate::platform::color_panel::is_visible();
                let keep = this
                    .update(cx, |this, cx| {
                        if current != last
                            && let Some([r, g, b]) = current
                        {
                            let c = Rgba::new(r, g, b, 1.);
                            match this.color_target {
                                Some(ColorTarget::Annotation) => this.apply_color(c, cx),
                                Some(ColorTarget::Background) => this.update_style(cx, |s| {
                                    s.background = crate::model::Background::solid(c)
                                }),
                                None => {}
                            }
                        }
                        this.color_target.is_some() && visible
                    })
                    .unwrap_or(false);
                last = current;
                if !keep {
                    break;
                }
            }
        }));
    }

    // MARK: Rendering

    pub fn base_image(&self) -> Option<Arc<RenderImage>> {
        self.base.clone()
    }

    /// Renders the composition (without annotations) off the main thread.
    pub fn request_base(&mut self, scale: f64, window: &mut Window, cx: &mut Context<Self>) {
        let key = RenderKey {
            style: self.style.clone(),
            crop: self.display_crop(),
            edge: self.edge,
            scale: (scale * 1000.).round() / 1000.,
        };
        if self.base_key.as_ref() == Some(&key) {
            return;
        }
        self.base_key = Some(key.clone());
        self.render_generation += 1;
        let generation = self.render_generation;
        let source = self.source.clone();
        let window_handle = window.window_handle();
        self._render_task = Some(cx.spawn(async move |this, cx| {
            let image = cx
                .background_executor()
                .spawn(async move {
                    let pm = render(&RenderInput {
                        source: &source,
                        style: &key.style,
                        crop: key.crop,
                        edge_color: key.edge,
                        annotations: &[],
                        effects: None,
                        hidden: None,
                        scale: key.scale,
                    })?;
                    let pm = if key.style.background.is_none() {
                        canvas::on_checkerboard(&pm, key.scale)
                    } else {
                        pm
                    };
                    Some(crate::ui::render_image(&pm))
                })
                .await;
            let Some(image) = image else { return };
            window_handle
                .update(cx, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        if this.render_generation != generation {
                            cx.drop_image(image, Some(window));
                            return;
                        }
                        let old = this.base.replace(image);
                        crate::ui::drop_image(old, window, cx);
                        cx.notify();
                    })
                    .ok();
                })
                .ok();
        }));
    }

    /// Small render of the final look for the drag thumbnail in the bottom bar.
    pub fn request_thumb(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let crop = self.crop();
        let layout = CanvasLayout::new(&self.style, crop);
        let key = RenderKey {
            style: self.style.clone(),
            crop,
            edge: self.edge,
            scale: 128. / layout.canvas_w.max(layout.canvas_h),
        };
        if self.thumb_key.as_ref() == Some(&key) {
            return;
        }
        self.thumb_key = Some(key.clone());
        let source = self.source.clone();
        let window_handle = window.window_handle();
        self._thumb_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let image = cx
                .background_executor()
                .spawn(async move {
                    render(&RenderInput {
                        source: &source,
                        style: &key.style,
                        crop: key.crop,
                        edge_color: key.edge,
                        annotations: &[],
                        effects: None,
                        hidden: None,
                        scale: key.scale,
                    })
                    .map(|pm| crate::ui::render_image(&pm))
                })
                .await;
            let Some(image) = image else { return };
            window_handle
                .update(cx, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        let old = this.thumb.replace(image);
                        crate::ui::drop_image(old, window, cx);
                        cx.notify();
                    })
                    .ok();
                })
                .ok();
        }));
    }

    pub fn thumb_image(&self) -> Option<Arc<RenderImage>> {
        self.thumb.clone()
    }

    pub fn render_final(&mut self) -> Option<tiny_skia::Pixmap> {
        let effects = match &self.effects {
            Some(fx) => fx.clone(),
            None => Arc::new(EffectImages::new(&self.source)),
        };
        render(&RenderInput {
            source: &self.source,
            style: &self.style,
            crop: self.crop(),
            edge_color: self.edge,
            annotations: &self.annotations,
            effects: Some(&effects),
            hidden: None,
            scale: 1.,
        })
    }

    // MARK: Export

    fn final_image(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<tiny_skia::Pixmap> {
        if self.editing.is_some() {
            self.finish_text(window, cx);
        }
        self.render_final()
    }

    pub fn copy_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pm) = self.final_image(window, cx) else {
            return;
        };
        let s = settings::get();
        match crate::platform::copy_image(&pm, self.pixel_scale, s.export_retina) {
            Ok(()) => {
                self.show_toast(l("Image copied"), cx);
                crate::feedback::play_copy();
            }
            Err(e) => self.show_toast(format!("{e:#}"), cx),
        }
    }

    pub fn save_quick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pm) = self.final_image(window, cx) else {
            return;
        };
        let s = settings::get();
        let folder = s.save_folder();
        let path = crate::export::unique_path(&folder, &crate::export::file_name(s.format.ext()));
        match crate::export::save(&pm, self.pixel_scale, &path, s.format, s.export_retina) {
            Ok(()) => {
                let name = folder
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.show_toast(lf("Saved to %@", name), cx);
            }
            Err(_) => self.show_toast(l("Couldn't save"), cx),
        }
    }

    pub fn save_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pm) = self.final_image(window, cx) else {
            return;
        };
        let s = settings::get();
        let name = crate::export::file_name(s.format.ext());
        let rx = cx.prompt_for_new_path(&s.save_folder(), Some(&name));
        let scale = self.pixel_scale;
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = rx.await else { return };
            let format = path
                .extension()
                .and_then(|e| settings::Format::from_ext(&e.to_string_lossy()))
                .unwrap_or(s.format);
            let path = path.with_extension(format.ext());
            let ok = crate::export::save(&pm, scale, &path, format, s.export_retina).is_ok();
            this.update(cx, |this, cx| {
                this.show_toast(
                    if ok {
                        l("Image saved")
                    } else {
                        l("Couldn't save")
                    },
                    cx,
                )
            })
            .ok();
        })
        .detach();
    }

    pub fn temp_file(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<std::path::PathBuf> {
        let pm = self.final_image(window, cx)?;
        let dir = std::env::temp_dir().join("Kirei");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(crate::export::file_name("png"));
        let s = settings::get();
        crate::export::save(
            &pm,
            self.pixel_scale,
            &path,
            settings::Format::Png,
            s.export_retina,
        )
        .ok()?;
        Some(path)
    }

    pub fn share(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.temp_file(window, cx) else {
            return;
        };
        if let Some(view) = crate::platform::ns_view(window) {
            crate::platform::share_file(
                view,
                &path,
                f64::from(f32::from(at.x)),
                f64::from(f32::from(at.y)),
            );
        }
    }

    pub fn pin(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pm) = self.final_image(window, cx) else {
            return;
        };
        crate::pinned::pin(pm, self.pixel_scale, cx);
    }

    pub fn copy_text(&mut self, cx: &mut Context<Self>) {
        self.busy = Some(l("Recognizing text…"));
        cx.notify();
        let Some(crop) = self.source.crop(self.crop()) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let text = cx
                .background_executor()
                .spawn(async move { crate::ocr::text(&crop) })
                .await;
            this.update(cx, |this, cx| {
                this.busy = None;
                if text.is_empty() {
                    this.show_toast(l("No text found"), cx);
                    return;
                }
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
                let lines = text.lines().count();
                this.show_toast(
                    if lines == 1 {
                        l("Text copied (1 line)")
                    } else {
                        lf("Text copied (%d lines)", lines)
                    },
                    cx,
                );
            })
            .ok();
        })
        .detach();
    }

    pub fn auto_redact(&mut self, cx: &mut Context<Self>) {
        self.busy = Some(l("Looking for sensitive data…"));
        cx.notify();
        let source = self.source.clone();
        cx.spawn(async move |this, cx| {
            let rects = cx
                .background_executor()
                .spawn(async move { crate::ocr::sensitive_rects(&source.pixmap) })
                .await;
            this.update(cx, |this, cx| {
                this.busy = None;
                if !rects.is_empty() {
                    this.register_undo(&l("Hide Data"));
                    for r in &rects {
                        this.annotations.push(Annotation::new(
                            AnnotationKind::Pixelate,
                            crate::model::pt(r.x, r.y),
                            crate::model::pt(r.max_x(), r.max_y()),
                            Rgba::BLACK,
                            0.,
                        ));
                    }
                }
                let n = rects.len();
                this.show_toast(
                    match n {
                        0 => l("No sensitive data found"),
                        1 => l("1 item redacted"),
                        n => lf("%d items redacted", n),
                    },
                    cx,
                );
                this.dirty(cx);
            })
            .ok();
        })
        .detach();
    }

    // MARK: Presets

    pub fn save_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.preset_input.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        let style = self.style.clone();
        settings::update_presets(|p| {
            p.push(settings::Preset {
                id: Uuid::new_v4(),
                name,
                style,
            })
        });
        self.close_popup(window, cx);
    }

    /// Dev-only state injection for screenshots (`KIREI_UI_STATE`).
    pub fn debug_apply(&mut self, spec: &str, window: &mut Window, cx: &mut Context<Self>) {
        for item in spec.split(',').map(str::trim) {
            let (k, v) = item.split_once('=').unwrap_or((item, ""));
            match k {
                "annotations" => {
                    self.annotations = crate::selftest::sample_annotations(self.crop(), self.unit())
                }
                "select" => {
                    let i: usize = v.parse().unwrap_or(0);
                    if let Some(a) = self.annotations.get(i) {
                        self.selection.insert(a.id);
                    }
                }
                "tool" => {
                    if let Some(t) = Tool::ALL.into_iter().find(|t| t.shortcut() == v) {
                        self.set_tool(t, cx);
                    }
                }
                "edit" => {
                    let i: usize = v.parse().unwrap_or(0);
                    if let Some(id) = self.annotations.get(i).map(|a| a.id) {
                        self.start_editing(id, window, cx);
                    }
                }
                "popup" => {
                    let p = match v {
                        "smart" => Popup::Smart,
                        "more" => Popup::More,
                        "style" => Popup::StyleColor,
                        "zoom" => Popup::Zoom,
                        "context" => Popup::Context,
                        "shortcuts" => Popup::Shortcuts,
                        "preset" => Popup::SavePreset,
                        _ => continue,
                    };
                    self.open_popup(p, point(px(700.), px(300.)), cx);
                }
                "sections" => settings::update(|s| {
                    for k in ["frame", "layout", "advanced"] {
                        s.inspector_sections.insert(k.into(), true);
                    }
                }),
                "bg" => {
                    self.bg_kind = match v {
                        "art" => BackgroundKind::Art,
                        "image" => BackgroundKind::Image,
                        _ => BackgroundKind::Solid,
                    }
                }
                "style" => {
                    if let Ok(style) = std::fs::read(v)
                        .map_err(anyhow::Error::from)
                        .and_then(|d| Ok(serde_json::from_slice::<Style>(&d)?))
                    {
                        self.set_style(style, cx);
                    }
                }
                "zoom" => self.zoom = v.parse().ok(),
                "toast" => self.show_toast(v.to_string(), cx),
                _ => {}
            }
        }
        self.dirty(cx);
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.show_inspector = !self.show_inspector;
        let v = self.show_inspector;
        settings::update(|s| {
            s.inspector_sections.insert("visible".into(), v);
        });
        self.base_key = None;
        cx.notify();
    }
}
