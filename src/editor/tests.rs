use std::sync::Arc;

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px, size};

use super::Editor;
use crate::model::{AnnotationKind, Pt, Tool, pt};
use crate::render::SourceImage;

fn source() -> Arc<SourceImage> {
    let (w, h) = (800u32, 500u32);
    let mut rgba = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if (x / 50 + y / 50) % 2 == 0 {
                rgba[i] = 90;
                rgba[i + 1] = 60;
                rgba[i + 2] = 200;
            }
        }
    }
    Arc::new(SourceImage::from_rgba(w, h, &rgba).unwrap())
}

fn setup(cx: &mut TestAppContext) -> (gpui::Entity<Editor>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::ui::text_input::bind_keys(cx);
        super::bind_keys(cx);
    });
    let (editor, vcx) = cx.add_window_view(|window, cx| {
        window.resize(size(px(1280.), px(820.)));
        Editor::new(source(), 1., window, cx)
    });
    vcx.run_until_parked();
    (editor, vcx)
}

fn at(
    editor: &gpui::Entity<Editor>,
    cx: &mut VisualTestContext,
    p: Pt,
) -> gpui::Point<gpui::Pixels> {
    cx.run_until_parked();
    editor.read_with(cx, |e, _| {
        e.canvas.view.expect("canvas pintado").to_window(p)
    })
}

fn drag(
    cx: &mut VisualTestContext,
    a: gpui::Point<gpui::Pixels>,
    b: gpui::Point<gpui::Pixels>,
    m: Modifiers,
) {
    drag_with(cx, a, b, m, m);
}

/// `down` applies to the press, `held` to the movement (⌘ while moving disables snapping).
fn drag_with(
    cx: &mut VisualTestContext,
    a: gpui::Point<gpui::Pixels>,
    b: gpui::Point<gpui::Pixels>,
    down: Modifiers,
    held: Modifiers,
) {
    cx.simulate_mouse_move(a, None, down);
    cx.simulate_mouse_down(a, MouseButton::Left, down);
    for i in 1..=8 {
        let t = i as f32 / 8.;
        let p = point(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        cx.simulate_mouse_move(p, Some(MouseButton::Left), held);
    }
    cx.simulate_mouse_up(b, MouseButton::Left, held);
    cx.run_until_parked();
}

fn set_tool(editor: &gpui::Entity<Editor>, cx: &mut VisualTestContext, t: Tool) {
    editor.update(cx, |e, cx| e.set_tool(t, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn draws_each_kind_and_undoes(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx);
    let tools = [
        (Tool::Arrow, AnnotationKind::Arrow),
        (Tool::Rect, AnnotationKind::Rect),
        (Tool::Ellipse, AnnotationKind::Ellipse),
        (Tool::Highlight, AnnotationKind::Highlight),
        (Tool::Pixelate, AnnotationKind::Pixelate),
        (Tool::Blur, AnnotationKind::Blur),
    ];
    for (i, (tool, kind)) in tools.iter().enumerate() {
        set_tool(&editor, cx, *tool);
        let y = 40. + i as f64 * 70.;
        let a = at(&editor, cx, pt(100., y));
        let b = at(&editor, cx, pt(300., y + 50.));
        drag(cx, a, b, Modifiers::none());
        editor.read_with(cx, |e, _| {
            assert_eq!(e.annotations.len(), i + 1, "{tool:?}");
            assert_eq!(e.annotations[i].kind, *kind);
            assert!(e.selection.contains(&e.annotations[i].id));
        });
    }
    set_tool(&editor, cx, Tool::Counter);
    let c = at(&editor, cx, pt(600., 100.));
    drag(cx, c, c, Modifiers::none());
    editor.read_with(cx, |e, _| {
        assert_eq!(e.annotations.last().unwrap().kind, AnnotationKind::Counter);
        assert!(e.selection.is_empty());
    });
    editor.update(cx, |e, cx| {
        e.undo(cx);
        e.undo(cx);
    });
    editor.read_with(cx, |e, _| assert_eq!(e.annotations.len(), 5));
    editor.update(cx, |e, cx| e.redo(cx));
    editor.read_with(cx, |e, _| assert_eq!(e.annotations.len(), 6));
}

#[gpui::test]
fn tiny_drag_is_cancelled(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx);
    set_tool(&editor, cx, Tool::Rect);
    let a = at(&editor, cx, pt(100., 100.));
    drag(cx, a, point(a.x + px(1.), a.y + px(1.)), Modifiers::none());
    editor.read_with(cx, |e, _| assert!(e.annotations.is_empty()));
}

#[gpui::test]
fn selects_moves_duplicates_and_marquees(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx);
    set_tool(&editor, cx, Tool::Highlight);
    for y in [100., 300.] {
        let a = at(&editor, cx, pt(100., y));
        let b = at(&editor, cx, pt(250., y + 40.));
        drag(cx, a, b, Modifiers::none());
    }
    set_tool(&editor, cx, Tool::Select);
    let empty = at(&editor, cx, pt(700., 450.));
    drag(cx, empty, empty, Modifiers::none());
    editor.read_with(cx, |e, _| assert!(e.selection.is_empty()));

    // Marquee over both.
    let a = at(&editor, cx, pt(50., 50.));
    let b = at(&editor, cx, pt(400., 400.));
    drag(cx, a, b, Modifiers::none());
    editor.read_with(cx, |e, _| assert_eq!(e.selection.len(), 2));

    // ⇧-click removes one from the selection.
    let first = at(&editor, cx, pt(150., 120.));
    drag(cx, first, first, Modifiers::shift());
    editor.read_with(cx, |e, _| assert_eq!(e.selection.len(), 1));

    // Move the remaining one by 40 px (⌘ disables snapping).
    let second = at(&editor, cx, pt(150., 320.));
    let to = at(&editor, cx, pt(190., 320.));
    drag_with(cx, second, to, Modifiers::none(), Modifiers::command());
    editor.read_with(cx, |e, _| {
        let moved = e
            .annotations
            .iter()
            .find(|a| e.selection.contains(&a.id))
            .unwrap();
        assert!(
            (moved.rect().x - 140.).abs() < 1.5,
            "x = {}",
            moved.rect().x
        );
    });

    // ⌥-drag leaves the original and moves a copy.
    let from = at(&editor, cx, pt(200., 320.));
    let to = at(&editor, cx, pt(200., 420.));
    drag_with(cx, from, to, Modifiers::alt(), Modifiers::command());
    editor.read_with(cx, |e, _| {
        assert_eq!(e.annotations.len(), 3);
        let copy = e.annotations.last().unwrap();
        assert!(e.selection.contains(&copy.id));
        assert!((copy.rect().y - 400.).abs() < 1.5, "y = {}", copy.rect().y);
    });
}

#[gpui::test]
fn resizes_by_handle_and_snaps_to_center(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx);
    set_tool(&editor, cx, Tool::Rect);
    let a = at(&editor, cx, pt(100., 100.));
    let b = at(&editor, cx, pt(200., 200.));
    drag(cx, a, b, Modifiers::none());
    // Bottom-right handle.
    let h = at(&editor, cx, pt(200., 200.));
    let to = at(&editor, cx, pt(260., 230.));
    drag(cx, h, to, Modifiers::none());
    editor.read_with(cx, |e, _| {
        let r = e.annotations[0].rect();
        assert!(
            (r.max_x() - 260.).abs() < 1.5 && (r.max_y() - 230.).abs() < 1.5,
            "{r:?}"
        );
        assert!((r.x - 100.).abs() < 0.5);
    });

    // Drag near the image center: the shape's center snaps to it.
    set_tool(&editor, cx, Tool::Select);
    let (center, rc) = editor.read_with(cx, |e, _| {
        let c = e.crop();
        (pt(c.mid_x(), c.mid_y()), e.annotations[0].rect())
    });
    let grab = at(&editor, cx, pt(rc.x + 2., rc.mid_y()));
    let target = at(
        &editor,
        cx,
        pt(
            rc.x + 2. + center.x - rc.mid_x() + 2.,
            rc.mid_y() + center.y - rc.mid_y() - 2.,
        ),
    );
    drag(cx, grab, target, Modifiers::none());
    editor.read_with(cx, |e, _| {
        let r = e.annotations[0].rect();
        assert!(
            (r.mid_x() - center.x).abs() < 0.01,
            "{} vs {}",
            r.mid_x(),
            center.x
        );
        assert!(
            (r.mid_y() - center.y).abs() < 0.01,
            "{} vs {}",
            r.mid_y(),
            center.y
        );
    });
}

#[gpui::test]
fn crops_and_resets(cx: &mut TestAppContext) {
    let (editor, cx) = setup(cx);
    set_tool(&editor, cx, Tool::Crop);
    let a = at(&editor, cx, pt(100., 50.));
    let b = at(&editor, cx, pt(500., 350.));
    drag(cx, a, b, Modifiers::none());
    editor.read_with(cx, |e, _| {
        let c = e.crop();
        assert_eq!(e.tool, Tool::Select);
        assert!(
            (c.w - 400.).abs() <= 2. && (c.h - 300.).abs() <= 2.,
            "{c:?}"
        );
    });
    editor.update(cx, |e, cx| e.undo(cx));
    editor.read_with(cx, |e, _| assert!(e.manual_crop.is_none()));
}
