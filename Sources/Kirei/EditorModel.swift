import AppKit
import SwiftUI
import Combine

final class EditorModel: ObservableObject {
    let source: CGImage
    /// Pixels per point of the original capture (2 on Retina).
    let pixelScale: CGFloat
    let balancedRect: CGRect
    let fullRect: CGRect

    @Published var style: Style { didSet { styleChanged(oldValue) } }
    @Published var annotations: [Annotation] = []
    @Published var manualCrop: CGRect? { didSet { updateEdgeColor() } }
    @Published var tool: Tool = .select { didSet { if tool != oldValue && tool != .select { selection = [] } } }
    @Published var selection: Set<UUID> = []
    @Published var editingTextID: UUID?
    @Published var color: RGBA = Palette.annotation[0]
    @Published var strokeLevel: Int = 1
    @Published private(set) var edgeColor: RGBA = .white
    @Published var busy: String?
    /// Screen pixels per capture pixel (1 = 100%); nil fits the whole image in the window.
    @Published var zoom: CGFloat?
    /// Zoom that currently fits the window, reported by the canvas.
    @Published var fitZoom: CGFloat = 1

    weak var undoManager: UndoManager?
    @Published private(set) var effects: EffectImages?

    init(image: CGImage, pixelScale: CGFloat, style: Style = PresetStore.shared.lastStyle) {
        source = image
        self.pixelScale = pixelScale
        fullRect = CGRect(x: 0, y: 0, width: image.width, height: image.height)
        balancedRect = ImageAnalysis.balancedRect(image)
        self.style = style
        updateEdgeColor()
        // Build pixelate/blur images off the main thread.
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let fx = EffectImages(source: image)
            DispatchQueue.main.async { self?.effects = fx }
        }
    }

    var crop: CGRect {
        if let m = manualCrop { return m }
        return style.autoBalance ? balancedRect : fullRect
    }

    /// While cropping, the whole image is shown so the crop can grow.
    var displayCrop: CGRect { tool == .crop ? fullRect : crop }

    var layout: CanvasLayout { CanvasLayout(style: style, crop: displayCrop) }

    /// Reference unit for default annotation sizes.
    var unit: CGFloat { max(2, max(crop.width, crop.height) / 320) }

    var strokeWidth: CGFloat { unit * [1.2, 2, 3.2][strokeLevel] }
    var fontSize: CGFloat { unit * [9, 13, 19][strokeLevel] }

    var selected: Annotation? {
        guard selection.count == 1, let id = selection.first else { return nil }
        return annotations.first { $0.id == id }
    }

    var selectedAnnotations: [Annotation] { annotations.filter { selection.contains($0.id) } }

    private func styleChanged(_ old: Style) {
        if old.autoBalance != style.autoBalance || old.inset != style.inset { updateEdgeColor() }
        PresetStore.shared.lastStyle = style
    }

    private func updateEdgeColor() {
        edgeColor = ImageAnalysis.edgeColor(source, crop: crop)
    }

    // MARK: - Zoom

    static let zoomSteps: [CGFloat] = [0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.5, 2, 3, 4]

    var effectiveZoom: CGFloat { zoom ?? fitZoom }

    func zoomIn() {
        let z = effectiveZoom
        zoom = Self.zoomSteps.first { $0 > z + 0.001 } ?? Self.zoomSteps.last
    }

    func zoomOut() {
        let z = effectiveZoom
        let next = Self.zoomSteps.last { $0 < z - 0.001 } ?? Self.zoomSteps.first!
        // Zooming out past the fit size returns to "fit".
        zoom = next <= fitZoom ? nil : next
    }

    func zoomToFit() { zoom = nil }
    func zoomActualSize() { zoom = 1 }

    // MARK: - Undo

    private struct Snapshot { var annotations: [Annotation]; var crop: CGRect? }

    private func snapshot() -> Snapshot { Snapshot(annotations: annotations, crop: manualCrop) }

    func registerUndo(_ name: String) {
        registerUndo(snapshot(), name: name)
    }

    private func registerUndo(_ snap: Snapshot, name: String) {
        undoManager?.registerUndo(withTarget: self) { m in
            let current = m.snapshot()
            m.annotations = snap.annotations
            m.manualCrop = snap.crop
            let ids = Set(m.annotations.map(\.id))
            m.selection.formIntersection(ids)
            m.registerUndo(current, name: name)
        }
        undoManager?.setActionName(name)
    }

    // MARK: - Editing

    func add(_ a: Annotation) {
        registerUndo(L("Add"))
        annotations.append(a)
    }

    /// Starts an annotation being drawn; undo is registered once it is committed.
    func beginDrawing(_ a: Annotation) {
        annotations.append(a)
    }

    func commitDrawing(_ id: UUID) {
        var before = snapshot()
        before.annotations.removeAll { $0.id == id }
        registerUndo(before, name: L("Add"))
    }

    func cancelDrawing(_ id: UUID) {
        annotations.removeAll { $0.id == id }
    }

    func update(_ id: UUID, _ change: (inout Annotation) -> Void) {
        guard let i = annotations.firstIndex(where: { $0.id == id }) else { return }
        change(&annotations[i])
    }

    func deleteSelected() {
        delete(selection)
    }

    func delete(_ ids: Set<UUID>) {
        guard !ids.isEmpty else { return }
        registerUndo(L("Delete"))
        annotations.removeAll { ids.contains($0.id) }
        selection.subtract(ids)
    }

    func selectAll() {
        selection = Set(annotations.map(\.id))
    }

    @discardableResult
    func duplicate(_ ids: Set<UUID>, offset: CGSize? = nil, registerUndo undo: Bool = true) -> [UUID] {
        let originals = annotations.filter { ids.contains($0.id) }
        guard !originals.isEmpty else { return [] }
        if undo { registerUndo(L("Duplicate")) }
        let d = offset ?? CGSize(width: unit * 6, height: unit * 6)
        let copies = originals.map { a -> Annotation in
            var c = a
            c.id = UUID()
            c.offset(by: d)
            return c
        }
        annotations.append(contentsOf: copies)
        selection = Set(copies.map(\.id))
        return copies.map(\.id)
    }

    func bringToFront(_ ids: Set<UUID>) {
        guard !ids.isEmpty else { return }
        registerUndo(L("Bring to Front"))
        let moved = annotations.filter { ids.contains($0.id) }
        annotations.removeAll { ids.contains($0.id) }
        annotations.append(contentsOf: moved)
    }

    func sendToBack(_ ids: Set<UUID>) {
        guard !ids.isEmpty else { return }
        registerUndo(L("Send to Back"))
        let moved = annotations.filter { ids.contains($0.id) }
        annotations.removeAll { ids.contains($0.id) }
        annotations.insert(contentsOf: moved, at: 0)
    }

    func nudgeSelection(by d: CGSize) {
        guard !selection.isEmpty else { return }
        registerUndo(L("Move"))
        for i in annotations.indices where selection.contains(annotations[i].id) {
            annotations[i].offset(by: d)
        }
    }

    func applyColorToSelection(_ c: RGBA) {
        color = c
        guard !selection.isEmpty else { return }
        registerUndo(L("Color"))
        for i in annotations.indices where selection.contains(annotations[i].id) {
            annotations[i].color = c
        }
    }

    func applyStrokeToSelection(_ level: Int) {
        strokeLevel = level
        guard !selection.isEmpty else { return }
        registerUndo(L("Thickness"))
        for i in annotations.indices where selection.contains(annotations[i].id) {
            annotations[i].width = strokeWidth
            if annotations[i].kind == .text { annotations[i].fontSize = fontSize }
        }
    }

    func setCrop(_ r: CGRect?) {
        registerUndo(L("Crop"))
        manualCrop = r.map { $0.integral.intersection(fullRect) }
    }

    func clearAnnotations() {
        guard !annotations.isEmpty else { return }
        registerUndo(L("Clear"))
        annotations.removeAll()
        selection = []
    }

    // MARK: - Analysis

    func autoRedact(completion: ((Int) -> Void)? = nil) {
        busy = L("Looking for sensitive data…")
        let img = source
        DispatchQueue.global(qos: .userInitiated).async {
            let rects = ImageAnalysis.sensitiveRects(img)
            DispatchQueue.main.async {
                self.busy = nil
                if !rects.isEmpty {
                    self.registerUndo(L("Hide Data"))
                    for r in rects {
                        self.annotations.append(Annotation(kind: .pixelate, p1: r.origin,
                                                           p2: CGPoint(x: r.maxX, y: r.maxY),
                                                           color: .black, width: 0))
                    }
                }
                completion?(rects.count)
            }
        }
    }

    func copyText(completion: @escaping (String) -> Void) {
        busy = L("Recognizing text…")
        guard let img = source.cropping(to: crop.integral) else { return }
        DispatchQueue.global(qos: .userInitiated).async {
            let text = ImageAnalysis.text(in: img)
            DispatchQueue.main.async {
                self.busy = nil
                completion(text)
            }
        }
    }

    // MARK: - Export

    func renderFinal() -> CGImage? {
        Renderer.render(source: source, style: style, crop: crop, edgeColor: edgeColor,
                        annotations: annotations, effects: effects ?? EffectImages(source: source))
    }
}
