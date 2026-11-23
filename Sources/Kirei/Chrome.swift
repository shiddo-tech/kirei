import AppKit
import SwiftUI

extension View {
    /// Liquid Glass on macOS 26, regular material before it.
    @ViewBuilder
    func kireiGlass<S: Shape>(in shape: S) -> some View {
        if #available(macOS 26, *) {
            self.glassEffect(.regular, in: shape)
        } else {
            self.background(.regularMaterial, in: shape)
                .overlay(shape.stroke(Color.primary.opacity(0.08), lineWidth: 0.5))
        }
    }
}

struct VisualEffect: NSViewRepresentable {
    var material: NSVisualEffectView.Material = .sidebar
    var blending: NSVisualEffectView.BlendingMode = .behindWindow

    func makeNSView(context: Context) -> NSVisualEffectView {
        let v = PassthroughEffectView()
        v.material = material
        v.blendingMode = blending
        v.state = .followsWindowActiveState
        return v
    }

    func updateNSView(_ v: NSVisualEffectView, context: Context) {
        v.material = material
        v.blendingMode = blending
    }
}

/// Decoration only: clicks pass to whatever sits under it (e.g. the window drag area).
private final class PassthroughEffectView: NSVisualEffectView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

/// Empty bar areas under a transparent title bar: drag moves the window, double-click zooms it.
struct WindowDragArea: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { DragView() }
    func updateNSView(_ nsView: NSView, context: Context) {}

    private final class DragView: NSView {
        override var mouseDownCanMoveWindow: Bool { true }
        override func mouseDown(with event: NSEvent) {
            if event.clickCount == 2 {
                window?.performZoom(nil)
            } else {
                window?.performDrag(with: event)
            }
        }
    }
}

/// Canvas backdrop: a dim surface with a faint dot grid.
struct CanvasBackdrop: View {
    var body: some View {
        ZStack {
            Color(nsColor: .underPageBackgroundColor)
            Canvas { ctx, size in
                let step: CGFloat = 18, r: CGFloat = 0.9
                var path = Path()
                for y in stride(from: step / 2, to: size.height, by: step) {
                    for x in stride(from: step / 2, to: size.width, by: step) {
                        path.addEllipse(in: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2))
                    }
                }
                ctx.fill(path, with: .color(.primary.opacity(0.09)))
            }
        }
    }
}

/// Small rendered previews of styles and backgrounds, cached by value.
enum StyleThumbnails {
    private static var cache: [Int: CGImage] = [:]

    private static let sample: CGImage? = {
        let w = 480, h = 300
        guard let ctx = Renderer.makeContext(width: w, height: h) else { return nil }
        ctx.setFillColor(CGColor(gray: 0.98, alpha: 1))
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        ctx.setFillColor(CGColor(gray: 0.9, alpha: 1))
        ctx.fill(CGRect(x: 0, y: h - 46, width: w, height: 46))
        ctx.setFillColor(CGColor(gray: 0.82, alpha: 1))
        for (i, width) in [260, 340, 200, 300].enumerated() {
            ctx.fill(CGRect(x: 36, y: h - 100 - i * 44, width: width, height: 16))
        }
        ctx.setFillColor(CGColor(srgbRed: 0.243, green: 0.549, blue: 0.388, alpha: 1))
        ctx.fill(CGRect(x: 36, y: 36, width: 120, height: 34))
        return ctx.makeImage()
    }()

    static func style(_ style: Style) -> CGImage? {
        var hasher = Hasher()
        hasher.combine("style")
        hasher.combine(style)
        let key = hasher.finalize()
        if let c = cache[key] { return c }
        guard let sample else { return nil }
        let crop = CGRect(x: 0, y: 0, width: sample.width, height: sample.height)
        let img = Renderer.render(source: sample, style: style, crop: crop, edgeColor: RGBA(hex: "#F2F2F2"),
                                  annotations: [], effects: nil, scale: 0.3)
        cache[key] = img
        return img
    }

    static func background(_ bg: Background, size: Int = 64) -> CGImage? {
        var hasher = Hasher()
        hasher.combine("bg")
        hasher.combine(bg)
        hasher.combine(size)
        let key = hasher.finalize()
        if let c = cache[key] { return c }
        guard let ctx = Renderer.makeContext(width: size, height: size) else { return nil }
        Renderer.drawBackground(bg, in: CGRect(x: 0, y: 0, width: size, height: size), ctx: ctx)
        let img = ctx.makeImage()
        cache[key] = img
        return img
    }
}
