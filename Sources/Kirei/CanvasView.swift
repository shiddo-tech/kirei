import SwiftUI
import AppKit

/// Interactive preview: cached composition image + live annotation layer.
struct CanvasView: View {
    @ObservedObject var model: EditorModel
    @State private var base: CGImage?
    @State private var renderGeneration = 0
    @State private var baseID = 0
    @State private var renderedStyle: Style?
    @State private var drag: DragState?
    @State private var cropDraft: CGRect?
    @State private var undoPending: String?
    @State private var marquee: CGRect?
    @State private var hoverID: UUID?
    @FocusState private var textFocused: Bool

    enum DragState {
        case idle
        case creating(UUID)
        case moving(starts: [Annotation], origin: CGPoint, duplicate: Bool, collapseTo: UUID?)
        case handle(UUID, index: Int, start: Annotation)
        case cropping(origin: CGPoint)
        case marquee(origin: CGPoint, base: Set<UUID>)
    }

    private struct RenderKey: Equatable {
        var style: Style
        var crop: CGRect
        var edge: RGBA
        var scale: CGFloat
    }

    @Environment(\.displayScale) private var displayScale

    private static let margin: CGFloat = 28

    var body: some View {
        GeometryReader { geo in
            let layout = model.layout
            let fitZoom = fitScale(canvas: layout.canvasSize, in: geo.size) * displayScale
            // Points per canvas pixel. Zoom is in screen pixels per capture pixel (1 = 100%).
            let s = (model.zoom ?? fitZoom) / displayScale
            let content = CGSize(width: max(geo.size.width, layout.canvasSize.width * s + Self.margin * 2),
                                 height: max(geo.size.height, layout.canvasSize.height * s + Self.margin * 2))
            let fit = CGRect(x: ((content.width - layout.canvasSize.width * s) / 2).rounded(),
                             y: ((content.height - layout.canvasSize.height * s) / 2).rounded(),
                             width: layout.canvasSize.width * s, height: layout.canvasSize.height * s)
            // Render at exactly the monitor's pixel density; beyond 100% the image is magnified.
            let key = RenderKey(style: model.style, crop: model.displayCrop, edge: model.edgeColor,
                                scale: min(1, s * displayScale))

            Group {
                if model.zoom == nil {
                    stage(layout: layout, fit: fit, scale: s, size: content)
                } else {
                    ScrollView([.horizontal, .vertical]) {
                        stage(layout: layout, fit: fit, scale: s, size: content)
                    }
                }
            }
            .onAppear { rerender(key); model.fitZoom = fitZoom }
            .onChange(of: key) { _, k in rerender(k) }
            .onChange(of: fitZoom) { _, z in model.fitZoom = z }
        }
        .onChange(of: model.tool) { _, _ in cropDraft = nil }
    }

    private func stage(layout: CanvasLayout, fit: CGRect, scale s: CGFloat, size: CGSize) -> some View {
        ZStack(alignment: .topLeading) {
            Checkerboard()
                .frame(width: fit.width, height: fit.height)
                .offset(x: fit.minX, y: fit.minY)
                .opacity(model.style.background == .none ? 1 : 0)

            if let base {
                // When the render already matches the screen (up to rounding), show it at its exact
                // pixel size: stretching by a fraction of a pixel would resample and soften it.
                let w = CGFloat(base.width) / displayScale, h = CGFloat(base.height) / displayScale
                let exact = abs(w - fit.width) < 1 && abs(h - fit.height) < 1
                Image(decorative: base, scale: displayScale)
                    .resizable()
                    .interpolation(s * displayScale >= 2 ? .none : .high)
                    .frame(width: exact ? w : fit.width, height: exact ? h : fit.height)
                    .offset(x: fit.minX, y: fit.minY)
                    .shadow(color: .black.opacity(0.22), radius: 18, y: 6)
                    .id(baseID)
                    .transition(.opacity)
            }

            Canvas { ctx, _ in
                ctx.withCGContext { cg in
                    drawOverlay(cg, layout: layout, fit: fit, scale: s)
                }
            }
            .allowsHitTesting(false)

            textEditor(layout: layout, fit: fit, scale: s)
        }
        .frame(width: size.width, height: size.height)
        .contentShape(Rectangle())
        .gesture(dragGesture(layout: layout, fit: fit, scale: s))
        .onContinuousHover { phase in
            switch phase {
            case .active(let loc): hover(at: toSource(loc, layout: layout, fit: fit, scale: s), scale: s)
            case .ended: hoverID = nil; NSCursor.arrow.set()
            }
        }
        .contextMenu { contextMenuItems }
    }

    // MARK: Geometry

    /// Points per canvas pixel that fit the whole composition in the view.
    private func fitScale(canvas: CGSize, in size: CGSize) -> CGFloat {
        let avail = CGSize(width: max(10, size.width - Self.margin * 2), height: max(10, size.height - Self.margin * 2))
        return min(avail.width / canvas.width, avail.height / canvas.height)
    }

    /// View point → source-pixel point.
    private func toSource(_ p: CGPoint, layout: CanvasLayout, fit: CGRect, scale: CGFloat) -> CGPoint {
        layout.toSource(CGPoint(x: (p.x - fit.minX) / scale, y: (p.y - fit.minY) / scale))
    }

    private func toView(_ p: CGPoint, layout: CanvasLayout, fit: CGRect, scale: CGFloat) -> CGPoint {
        let c = layout.toCanvas(p)
        return CGPoint(x: fit.minX + c.x * scale, y: fit.minY + c.y * scale)
    }

    // MARK: Rendering

    private func rerender(_ key: RenderKey) {
        renderGeneration += 1
        let gen = renderGeneration
        let src = model.source
        // Crossfade only discrete changes; slider drags must track the pointer without lag.
        let animate = renderedStyle.map { old in
            old.background != key.style.background || old.frame != key.style.frame
                || old.aspect != key.style.aspect || old.alignment != key.style.alignment
        } ?? false
        renderedStyle = key.style
        DispatchQueue.global(qos: .userInteractive).async {
            let img = Renderer.render(source: src, style: key.style, crop: key.crop, edgeColor: key.edge,
                                      annotations: [], effects: nil, scale: key.scale)
            DispatchQueue.main.async {
                guard gen == renderGeneration else { return }
                if animate {
                    withAnimation(.easeOut(duration: 0.22)) { base = img; baseID += 1 }
                } else {
                    base = img
                }
            }
        }
    }

    private func drawOverlay(_ cg: CGContext, layout: CanvasLayout, fit: CGRect, scale: CGFloat) {
        cg.saveGState()
        cg.translateBy(x: fit.minX, y: fit.minY)
        cg.scaleBy(x: scale, y: scale)
        cg.clip(to: CGRect(origin: .zero, size: layout.canvasSize))
        cg.translateBy(x: layout.imageRect.minX - layout.crop.minX, y: layout.imageRect.minY - layout.crop.minY)
        AnnotationRenderer.drawAll(model.annotations, ctx: cg, effects: model.effects, layout: layout,
                                   hiddenID: model.editingTextID.flatMap { id in
                                       model.annotations.first { $0.id == id }?.kind == .text ? id : nil
                                   })
        cg.restoreGState()

        // Selection chrome, in view space.
        cg.setStrokeColor(NSColor.controlAccentColor.cgColor)
        cg.setLineWidth(1.5)
        for a in model.selectedAnnotations where model.editingTextID != a.id {
            let r = viewRect(a.bounds, layout: layout, fit: fit, scale: scale).insetBy(dx: -4, dy: -4)
            cg.setLineDash(phase: 0, lengths: [5, 3])
            if a.kind != .arrow || model.selection.count > 1 { cg.stroke(r) }
            cg.setLineDash(phase: 0, lengths: [])
        }
        if let a = model.selected, model.editingTextID != a.id {
            for h in handles(a) {
                let p = toView(h, layout: layout, fit: fit, scale: scale)
                let hr = CGRect(x: p.x - 5, y: p.y - 5, width: 10, height: 10)
                cg.setFillColor(.white)
                cg.fillEllipse(in: hr)
                cg.setStrokeColor(NSColor.controlAccentColor.cgColor)
                cg.strokeEllipse(in: hr)
            }
        }

        if let m = marquee {
            let r = viewRect(m, layout: layout, fit: fit, scale: scale)
            cg.setFillColor(NSColor.controlAccentColor.withAlphaComponent(0.12).cgColor)
            cg.fill(r)
            cg.setStrokeColor(NSColor.controlAccentColor.cgColor)
            cg.setLineWidth(1)
            cg.stroke(r)
        }

        // Crop overlay.
        if model.tool == .crop {
            let current = cropDraft ?? model.crop
            let full = CGRect(origin: .zero, size: CGSize(width: fit.width, height: fit.height)).offsetBy(dx: fit.minX, dy: fit.minY)
            let fullSrc = viewRect(model.fullRect, layout: layout, fit: fit, scale: scale)
            let cr = viewRect(current, layout: layout, fit: fit, scale: scale)
            cg.saveGState()
            cg.addRect(full.union(fullSrc))
            cg.addRect(cr)
            cg.setFillColor(CGColor(gray: 0, alpha: 0.45))
            cg.fillPath(using: .evenOdd)
            cg.setStrokeColor(.white)
            cg.setLineWidth(1.5)
            cg.stroke(cr)
            // Rule of thirds.
            cg.setStrokeColor(CGColor(gray: 1, alpha: 0.4))
            cg.setLineWidth(0.8)
            for i in 1...2 {
                let x = cr.minX + cr.width * CGFloat(i) / 3, y = cr.minY + cr.height * CGFloat(i) / 3
                cg.move(to: CGPoint(x: x, y: cr.minY)); cg.addLine(to: CGPoint(x: x, y: cr.maxY))
                cg.move(to: CGPoint(x: cr.minX, y: y)); cg.addLine(to: CGPoint(x: cr.maxX, y: y))
            }
            cg.strokePath()
            cg.restoreGState()
        }
    }

    private func viewRect(_ r: CGRect, layout: CanvasLayout, fit: CGRect, scale: CGFloat) -> CGRect {
        let a = toView(r.origin, layout: layout, fit: fit, scale: scale)
        return CGRect(x: a.x, y: a.y, width: r.width * scale, height: r.height * scale)
    }

    private func handles(_ a: Annotation) -> [CGPoint] {
        switch a.kind {
        case .arrow: return [a.p1, a.p2]
        case _ where a.kind.isBox:
            let r = a.rect
            return [CGPoint(x: r.minX, y: r.minY), CGPoint(x: r.maxX, y: r.minY),
                    CGPoint(x: r.maxX, y: r.maxY), CGPoint(x: r.minX, y: r.maxY)]
        case .text:
            return [CGPoint(x: a.bounds.maxX, y: a.bounds.maxY)]
        default: return []
        }
    }

    // MARK: Text editing

    @ViewBuilder
    private func textEditor(layout: CanvasLayout, fit: CGRect, scale: CGFloat) -> some View {
        if let id = model.editingTextID, let a = model.annotations.first(where: { $0.id == id }), a.kind == .counter {
            let c = toView(a.p1, layout: layout, fit: fit, scale: scale)
            var auto = model.annotations
            let _ = auto.indices.filter { auto[$0].id == id }.forEach { auto[$0].text = "" }
            TextField(AnnotationRenderer.counterLabels(auto)[id] ?? "", text: Binding(
                get: { model.annotations.first { $0.id == id }?.text ?? "" },
                set: { v in model.update(id) { $0.text = v } }
            ))
            .textFieldStyle(.roundedBorder)
            .frame(width: 64)
            .fixedSize()
            .offset(x: c.x + a.counterRadius * scale + 8, y: c.y - 11)
            .focused($textFocused)
            .onSubmit { finishText() }
            .onAppear { DispatchQueue.main.async { textFocused = true } }
            .onChange(of: textFocused) { _, f in if !f { finishText() } }
        } else if let id = model.editingTextID, let a = model.annotations.first(where: { $0.id == id }) {
            let p = toView(a.p1, layout: layout, fit: fit, scale: scale)
            TextField(L("Type…"), text: Binding(
                get: { model.annotations.first { $0.id == id }?.text ?? "" },
                set: { v in model.update(id) { $0.text = v } }
            ), axis: .vertical)
            .textFieldStyle(.plain)
            .font(.system(size: max(9, a.fontSize * scale), weight: .bold))
            .foregroundStyle(a.color.color)
            .fixedSize()
            .frame(minWidth: 60, alignment: .topLeading)
            .padding(2)
            .background(RoundedRectangle(cornerRadius: 3).strokeBorder(Color.accentColor, style: StrokeStyle(lineWidth: 1, dash: [4, 2])))
            .offset(x: p.x - 2, y: p.y - 2)
            .focused($textFocused)
            .onSubmit { finishText() }
            .onAppear { DispatchQueue.main.async { textFocused = true } }
            .onChange(of: textFocused) { _, f in if !f { finishText() } }
        }
    }

    private func startEditing(_ id: UUID) {
        guard let a = model.annotations.first(where: { $0.id == id }) else { return }
        model.registerUndo(a.kind == .counter ? L("Edit Number") : L("Edit Text"))
        model.selection = [id]
        model.editingTextID = id
    }

    private func finishText() {
        guard let id = model.editingTextID else { return }
        model.editingTextID = nil
        guard let a = model.annotations.first(where: { $0.id == id }) else { return }
        if a.kind == .counter {
            model.update(id) { $0.text = $0.text.trimmingCharacters(in: .whitespacesAndNewlines) }
        } else if a.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            model.annotations.removeAll { $0.id == id }
            model.selection.remove(id)
        }
    }

    // MARK: Gestures

    private func dragGesture(layout: CanvasLayout, fit: CGRect, scale: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 0, coordinateSpace: .local)
            .onChanged { v in
                let p = toSource(v.location, layout: layout, fit: fit, scale: scale)
                if drag == nil {
                    let start = toSource(v.startLocation, layout: layout, fit: fit, scale: scale)
                    begin(at: start, scale: scale)
                }
                update(to: p)
            }
            .onEnded { v in
                let p = toSource(v.location, layout: layout, fit: fit, scale: scale)
                end(at: p, moved: hypot(v.translation.width, v.translation.height) > 3, scale: scale)
                drag = nil
            }
    }

    private func begin(at p: CGPoint, scale: CGFloat) {
        if model.editingTextID != nil { finishText() }
        let tol = 8 / scale
        let flags = NSApp.currentEvent?.modifierFlags ?? []
        let additive = flags.contains(.shift) || flags.contains(.command)
        let clicks = NSApp.currentEvent?.clickCount ?? 1
        drag = .idle

        if model.tool != .crop, let (a, i) = handleHit(p, tolerance: tol) {
            undoPending = L("Resize")
            drag = .handle(a.id, index: i, start: a)
            return
        }

        switch model.tool {
        case .select:
            guard let hit = model.annotations.last(where: { $0.hitTest(p, tolerance: tol) }) else {
                if !additive { model.selection = [] }
                drag = .marquee(origin: p, base: model.selection)
                return
            }
            if clicks >= 2, hit.kind == .text || hit.kind == .counter {
                startEditing(hit.id)
                return
            }
            var collapse: UUID?
            if additive {
                if model.selection.contains(hit.id) {
                    model.selection.remove(hit.id)
                    return
                }
                model.selection.insert(hit.id)
            } else if !model.selection.contains(hit.id) {
                model.selection = [hit.id]
            } else if model.selection.count > 1 {
                collapse = hit.id
            }
            startMoving(at: p, duplicating: flags.contains(.option), collapseTo: collapse)

        case .crop:
            drag = .cropping(origin: p)

        case .text:
            // Clicking an existing text edits it.
            if let hit = model.annotations.last(where: { $0.kind == .text && $0.hitTest(p, tolerance: tol) }) {
                startEditing(hit.id)
                return
            }
            if let hit = grabbable(at: p, tolerance: tol) {
                select(hit, andMoveFrom: p, flags: flags)
                return
            }
            let a = Annotation(kind: .text, p1: p, p2: p, color: model.color, width: model.strokeWidth,
                               text: "", fontSize: model.fontSize)
            model.add(a)
            model.selection = [a.id]
            model.editingTextID = a.id

        default:
            if model.tool == .counter,
               let hit = model.annotations.last(where: { $0.kind == .counter && $0.hitTest(p, tolerance: tol) }) {
                if clicks >= 2 { startEditing(hit.id) } else { select(hit, andMoveFrom: p, flags: flags) }
                return
            }
            if let hit = grabbable(at: p, tolerance: tol) {
                select(hit, andMoveFrom: p, flags: flags)
                return
            }
            guard let kind = model.tool.annotationKind else { return }
            let a = Annotation(kind: kind, p1: p, p2: p, color: model.color, width: model.strokeWidth,
                               fontSize: model.fontSize)
            model.beginDrawing(a)
            drag = .creating(a.id)
        }
    }

    private func handleHit(_ p: CGPoint, tolerance tol: CGFloat) -> (Annotation, Int)? {
        guard let a = model.selected, model.editingTextID != a.id else { return nil }
        for (i, h) in handles(a).enumerated() where hypot(h.x - p.x, h.y - p.y) <= tol * 1.4 {
            return (a, i)
        }
        return nil
    }

    /// With a drawing tool active, only outlines and arrows are grabbed, plus anything already
    /// selected except the inside of an outlined shape, so drawing on top of another shape still draws.
    private func grabbable(at p: CGPoint, tolerance tol: CGFloat) -> Annotation? {
        model.annotations.last { a in
            if a.grabTest(p, tolerance: tol) { return true }
            let outlined: Set<AnnotationKind> = [.rect, .ellipse]
            return model.selection.contains(a.id) && !outlined.contains(a.kind) && a.hitTest(p, tolerance: tol)
        }
    }

    private func select(_ hit: Annotation, andMoveFrom p: CGPoint, flags: NSEvent.ModifierFlags) {
        if !model.selection.contains(hit.id) { model.selection = [hit.id] }
        startMoving(at: p, duplicating: flags.contains(.option))
    }

    /// ⌥-drag moves copies and leaves the originals in place. The copies are made only once the
    /// pointer actually moves, so a plain ⌥-click doesn't stack a hidden duplicate.
    private func startMoving(at p: CGPoint, duplicating: Bool, collapseTo: UUID? = nil) {
        undoPending = duplicating ? nil : L("Move")
        drag = .moving(starts: model.selectedAnnotations, origin: p, duplicate: duplicating, collapseTo: collapseTo)
        NSCursor.closedHand.set()
    }

    private func update(to p: CGPoint) {
        guard let drag else { return }
        if let name = undoPending {
            switch drag {
            case .moving(_, let origin, _, _) where hypot(p.x - origin.x, p.y - origin.y) > 0.5:
                model.registerUndo(name); undoPending = nil
            case .handle:
                model.registerUndo(name); undoPending = nil
            default: break
            }
        }
        if case .moving(_, let origin, true, _) = drag, hypot(p.x - origin.x, p.y - origin.y) > 0.5 {
            model.duplicate(model.selection, offset: .zero)
            self.drag = .moving(starts: model.selectedAnnotations, origin: origin, duplicate: false, collapseTo: nil)
        }
        guard let current = self.drag else { return }
        switch current {
        case .idle:
            break
        case .creating(let id):
            model.update(id) { a in
                if a.kind == .counter { a.p1 = p; a.p2 = p } else { a.p2 = p }
            }
        case .moving(let starts, let origin, let duplicate, _):
            guard !duplicate else { break }
            let d = CGSize(width: p.x - origin.x, height: p.y - origin.y)
            for start in starts {
                model.update(start.id) { a in
                    a = start
                    a.offset(by: d)
                }
            }
        case .handle(let id, let index, let start):
            model.update(id) { a in
                switch a.kind {
                case .arrow:
                    if index == 0 { a.p1 = p } else { a.p2 = p }
                case .text:
                    let b = start.bounds
                    let f = max((p.x - b.minX) / max(1, b.width), (p.y - b.minY) / max(1, b.height))
                    a.fontSize = max(6, start.fontSize * f)
                default:
                    let r = start.rect
                    // Opposite corner stays fixed.
                    let fixed = [CGPoint(x: r.maxX, y: r.maxY), CGPoint(x: r.minX, y: r.maxY),
                                 CGPoint(x: r.minX, y: r.minY), CGPoint(x: r.maxX, y: r.minY)][index]
                    a.p1 = fixed; a.p2 = p
                }
            }
        case .cropping(let origin):
            cropDraft = CGRect(x: min(origin.x, p.x), y: min(origin.y, p.y),
                               width: abs(p.x - origin.x), height: abs(p.y - origin.y))
                .intersection(model.fullRect)
        case .marquee(let origin, let base):
            let r = CGRect(x: min(origin.x, p.x), y: min(origin.y, p.y),
                           width: abs(p.x - origin.x), height: abs(p.y - origin.y))
            marquee = r
            let inside = model.annotations.filter { r.intersects($0.bounds) }.map(\.id)
            model.selection = base.union(inside)
        }
    }

    private func end(at p: CGPoint, moved: Bool, scale: CGFloat) {
        guard let drag else { return }
        switch drag {
        case .creating(let id):
            guard let a = model.annotations.first(where: { $0.id == id }) else { return }
            let minSize = 4 / scale
            let tooSmall: Bool = {
                switch a.kind {
                case .counter: return false
                case .arrow: return hypot(a.p2.x - a.p1.x, a.p2.y - a.p1.y) < minSize * 3
                default: return a.rect.width < minSize || a.rect.height < minSize
                }
            }()
            if tooSmall {
                model.cancelDrawing(id)
                model.selection = []
            } else {
                model.commitDrawing(id)
                model.selection = a.kind == .counter ? [] : [id]
            }
        case .moving(_, _, _, let collapse):
            if !moved, let collapse { model.selection = [collapse] }
            undoPending = nil
            NSCursor.openHand.set()
        case .handle:
            undoPending = nil
            NSCursor.openHand.set()
        case .cropping:
            if let r = cropDraft {
                if r.width > 8 && r.height > 8 {
                    model.setCrop(r)
                    model.tool = .select
                }
                cropDraft = nil
            }
        case .marquee:
            marquee = nil
        case .idle:
            break
        }
    }

    // MARK: Hover & context menu

    private func hover(at p: CGPoint, scale: CGFloat) {
        guard drag == nil else { return }
        let tol = 8 / scale
        let hit = model.annotations.last { $0.hitTest(p, tolerance: tol) }
        if hoverID != hit?.id { hoverID = hit?.id }
        guard model.editingTextID == nil, model.tool != .crop else { return }
        if handleHit(p, tolerance: tol) != nil {
            NSCursor.crosshair.set()
        } else if model.tool == .select ? hit != nil
                    : grabbable(at: p, tolerance: tol) != nil || (model.tool == .counter && hit?.kind == .counter) {
            NSCursor.openHand.set()
        } else {
            NSCursor.arrow.set()
        }
    }

    /// Right-clicking an unselected annotation acts on it alone; otherwise on the selection.
    private var menuTargets: Set<UUID> {
        if let h = hoverID, !model.selection.contains(h) { return [h] }
        return model.selection
    }

    @ViewBuilder
    private var contextMenuItems: some View {
        let targets = menuTargets
        if targets.isEmpty {
            Button(L("Select All")) { model.tool = .select; model.selectAll() }
                .disabled(model.annotations.isEmpty)
        } else {
            if targets.count == 1, let id = targets.first,
               let kind = model.annotations.first(where: { $0.id == id })?.kind, kind == .text || kind == .counter {
                Button(kind == .counter ? L("Edit Number") : L("Edit Text")) { startEditing(id) }
                Divider()
            }
            Button(L("Duplicate")) { model.duplicate(targets) }
            Button(L("Bring to Front")) { model.selection = targets; model.bringToFront(targets) }
            Button(L("Send to Back")) { model.selection = targets; model.sendToBack(targets) }
            Divider()
            Button(L("Delete"), role: .destructive) { model.delete(targets) }
        }
    }
}

/// Transparency checkerboard.
struct Checkerboard: View {
    var body: some View {
        Canvas { ctx, size in
            let s: CGFloat = 10
            ctx.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.white))
            for y in stride(from: 0, to: size.height, by: s) {
                for x in stride(from: 0, to: size.width, by: s) where (Int(x / s) + Int(y / s)) % 2 == 0 {
                    ctx.fill(Path(CGRect(x: x, y: y, width: s, height: s)), with: .color(Color(white: 0.88)))
                }
            }
        }
    }
}
