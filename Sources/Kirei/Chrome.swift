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
        ctx.translateBy(x: 0, y: CGFloat(size))
        ctx.scaleBy(x: 1, y: -1)
        Renderer.drawBackground(bg, in: CGRect(x: 0, y: 0, width: size, height: size), ctx: ctx)
        let img = ctx.makeImage()
        cache[key] = img
        return img
    }
}

/// Dashed frame whose dashes are fitted to each side, so every corner is drawn solid.
/// A single dash pattern around a rounded rect lets the corners fall into gaps.
struct DashedFrame: View {
    var color: Color
    var lineWidth: CGFloat = 2
    var dash: CGFloat = 6
    var gap: CGFloat = 4
    var radius: CGFloat = 4

    var body: some View {
        Canvas { ctx, size in
            let r = CGRect(origin: .zero, size: size).insetBy(dx: lineWidth / 2, dy: lineWidth / 2)
            let c = min(radius, r.width / 2, r.height / 2)
            let solid = StrokeStyle(lineWidth: lineWidth, lineCap: .butt)

            ctx.stroke(Self.arcsOnly(r, c), with: .color(color), style: solid)

            let sides: [(CGPoint, CGPoint)] = [
                (CGPoint(x: r.minX + c, y: r.minY), CGPoint(x: r.maxX - c, y: r.minY)),
                (CGPoint(x: r.maxX, y: r.minY + c), CGPoint(x: r.maxX, y: r.maxY - c)),
                (CGPoint(x: r.maxX - c, y: r.maxY), CGPoint(x: r.minX + c, y: r.maxY)),
                (CGPoint(x: r.minX, y: r.maxY - c), CGPoint(x: r.minX, y: r.minY + c)),
            ]
            var dashes = Path()
            for (a, b) in sides {
                let length = hypot(b.x - a.x, b.y - a.y)
                guard length > 0 else { continue }
                let ux = (b.x - a.x) / length, uy = (b.y - a.y) / length
                // Both ends start and finish on a dash: stretch the gaps so the count fits exactly.
                let n = max(1, Int(((length + gap) / (dash + gap)).rounded()))
                let d = n < 2 ? length : min(dash, length)
                let step = n < 2 ? 0 : (length - d) / CGFloat(n - 1)
                for i in 0..<n {
                    let t = CGFloat(i) * step
                    dashes.move(to: CGPoint(x: a.x + ux * t, y: a.y + uy * t))
                    dashes.addLine(to: CGPoint(x: a.x + ux * (t + d), y: a.y + uy * (t + d)))
                }
            }
            ctx.stroke(dashes, with: .color(color), style: solid)
        }
        .allowsHitTesting(false)
    }

    private static func arcsOnly(_ r: CGRect, _ c: CGFloat) -> Path {
        var p = Path()
        let arcs: [(CGPoint, Double)] = [
            (CGPoint(x: r.minX + c, y: r.minY + c), 180),
            (CGPoint(x: r.maxX - c, y: r.minY + c), 270),
            (CGPoint(x: r.maxX - c, y: r.maxY - c), 0),
            (CGPoint(x: r.minX + c, y: r.maxY - c), 90),
        ]
        for (center, start) in arcs {
            let a = CGFloat(start * Double.pi / 180)
            p.move(to: CGPoint(x: center.x + c * cos(a), y: center.y + c * sin(a)))
            p.addArc(center: center, radius: c, startAngle: .degrees(start), endAngle: .degrees(start + 90), clockwise: false)
        }
        return p
    }
}
