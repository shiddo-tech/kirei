import AppKit
import CoreImage
import SwiftUI

/// Continuous (squircle) corners, the same curve macOS uses for windows and icons.
func squirclePath(_ rect: CGRect, radius: CGFloat) -> CGPath {
    let r = max(0, min(radius, min(rect.width, rect.height) / 2))
    guard r > 0 else { return CGPath(rect: rect, transform: nil) }
    return RoundedRectangle(cornerRadius: r, style: .continuous).path(in: rect).cgPath
}

/// Geometry of the final composition, in output pixels (scale 1 = export size).
struct CanvasLayout {
    let canvasSize: CGSize
    /// Where the (cropped) screenshot is drawn on the canvas.
    let imageRect: CGRect
    /// The part of the source image being shown, in source pixels.
    let crop: CGRect
    let inset: CGFloat
    let corner: CGFloat
    let shadowBlur: CGFloat
    let shadowOffset: CGFloat
    let shadowAlpha: CGFloat
    let windowFrame: WindowFrame
    /// Height of the mockup title/address bar above the image; 0 without a frame.
    let chrome: CGFloat

    var frameRect: CGRect {
        CGRect(x: imageRect.minX - inset, y: imageRect.minY - inset - chrome,
               width: imageRect.width + inset * 2, height: imageRect.height + inset * 2 + chrome)
    }
    var frameCorner: CGFloat { chrome > 0 ? max(corner, chrome * 0.3) : corner }
    var imageCorner: CGFloat { chrome > 0 ? 0 : inset > 0 ? corner * 0.6 : corner }

    /// Source-pixel point → canvas point.
    func toCanvas(_ p: CGPoint) -> CGPoint {
        CGPoint(x: imageRect.minX + p.x - crop.minX, y: imageRect.minY + p.y - crop.minY)
    }

    func toSource(_ p: CGPoint) -> CGPoint {
        CGPoint(x: p.x - imageRect.minX + crop.minX, y: p.y - imageRect.minY + crop.minY)
    }

    init(style: Style, crop: CGRect) {
        self.crop = crop
        let R = max(crop.width, crop.height)
        inset = (style.inset * R).rounded()
        corner = style.cornerRadius * R
        let pad = (style.padding * R).rounded()
        let s = style.shadow
        shadowBlur = s * 0.045 * R
        shadowOffset = s * 0.012 * R
        shadowAlpha = s > 0 ? 0.18 + 0.32 * s : 0
        windowFrame = style.frame
        chrome = style.frame == .none ? 0 : max(22, (R * (style.frame.isBrowser ? 0.042 : 0.032)).rounded())

        let frameW = crop.width + inset * 2
        let frameH = crop.height + inset * 2 + chrome
        var w = frameW + pad * 2
        var h = frameH + pad * 2
        if let ratio = style.aspect.value {
            if w / h < ratio { w = h * ratio } else { h = w / ratio }
        }
        w = w.rounded(); h = h.rounded()
        canvasSize = CGSize(width: w, height: h)

        // Position moves the frame through all the space around it (padding included), so it
        // works with any ratio. The side it moves toward keeps room for the shadow.
        let margin = min(pad, max(pad * 0.25, shadowBlur)).rounded()
        let col = CGFloat(style.alignment % 3), row = CGFloat(style.alignment / 3)
        let freeW: CGFloat = w - frameW - margin * 2
        let freeH: CGFloat = h - frameH - margin * 2
        let fx: CGFloat = margin + freeW * col / 2
        let fy: CGFloat = margin + freeH * row / 2
        imageRect = CGRect(x: (fx + inset).rounded(), y: (fy + inset + chrome).rounded(),
                           width: crop.width, height: crop.height)
    }
}

/// Pre-computed full-size effect images used by pixelate/blur annotations.
final class EffectImages {
    let pixelated: CGImage?
    let blurred: CGImage?

    init(source: CGImage) {
        let ci = CIImage(cgImage: source)
        let extent = ci.extent
        let R = max(extent.width, extent.height)
        let context = CIContext(options: [.useSoftwareRenderer: false])

        let pix = CIFilter(name: "CIPixellate")!
        pix.setValue(ci.clampedToExtent(), forKey: kCIInputImageKey)
        pix.setValue(max(14, R / 60), forKey: kCIInputScaleKey)
        pix.setValue(CIVector(x: 0, y: 0), forKey: kCIInputCenterKey)
        pixelated = pix.outputImage.flatMap { context.createCGImage($0.cropped(to: extent), from: extent) }

        let blur = CIFilter(name: "CIGaussianBlur")!
        blur.setValue(ci.clampedToExtent(), forKey: kCIInputImageKey)
        blur.setValue(max(10, R / 70), forKey: kCIInputRadiusKey)
        blurred = blur.outputImage.flatMap { context.createCGImage($0.cropped(to: extent), from: extent) }
    }
}

enum Renderer {
    static let colorSpace = CGColorSpace(name: CGColorSpace.sRGB)!

    static func makeContext(width: Int, height: Int) -> CGContext? {
        CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                  space: colorSpace, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
    }

    /// Draws a CGImage upright inside `rect` in a y-down context.
    static func drawImage(_ image: CGImage, in rect: CGRect, ctx: CGContext) {
        ctx.saveGState()
        ctx.translateBy(x: rect.minX, y: rect.maxY)
        ctx.scaleBy(x: 1, y: -1)
        ctx.draw(image, in: CGRect(origin: .zero, size: rect.size))
        ctx.restoreGState()
    }

    /// Scale from user space to device pixels, used to size shadows correctly.
    static func deviceScale(_ ctx: CGContext) -> CGFloat {
        let m = ctx.ctm
        return max(0.0001, hypot(m.a, m.b))
    }

    /// Renders the full composition. `scale` < 1 is used for fast previews.
    static func render(source: CGImage, style: Style, crop: CGRect, edgeColor: RGBA,
                       annotations: [Annotation], effects: EffectImages?, scale: CGFloat = 1) -> CGImage? {
        let layout = CanvasLayout(style: style, crop: crop)
        let W = max(1, Int((layout.canvasSize.width * scale).rounded()))
        let H = max(1, Int((layout.canvasSize.height * scale).rounded()))
        guard let ctx = makeContext(width: W, height: H) else { return nil }
        ctx.interpolationQuality = .high
        ctx.translateBy(x: 0, y: CGFloat(H))
        ctx.scaleBy(x: 1, y: -1)
        ctx.scaleBy(x: scale, y: scale)

        drawComposition(source: source, style: style, layout: layout, edgeColor: edgeColor, ctx: ctx)
        if !annotations.isEmpty {
            ctx.saveGState()
            ctx.translateBy(x: layout.imageRect.minX - crop.minX, y: layout.imageRect.minY - crop.minY)
            AnnotationRenderer.drawAll(annotations, ctx: ctx, effects: effects, layout: layout)
            ctx.restoreGState()
        }
        return ctx.makeImage()
    }

    static func drawComposition(source: CGImage, style: Style, layout: CanvasLayout, edgeColor: RGBA, ctx: CGContext) {
        let canvas = CGRect(origin: .zero, size: layout.canvasSize)
        drawBackground(style.background, in: canvas, blur: style.backgroundBlur, ctx: ctx)
        if style.grain > 0, style.background != .none {
            drawGrain(in: canvas, amount: style.grain, ctx: ctx)
        } else if style.background.isSmooth {
            drawGrain(in: canvas, amount: 0.05, ctx: ctx)
        }

        let k = deviceScale(ctx)
        // Two shadows, like real depth: a wide ambient one and a short contact one under the edge.
        // Nested transparency layers draw the content once, so translucent pixels (window corners)
        // don't darken from being composited twice.
        let a = layout.shadowAlpha
        ctx.saveGState()
        if a > 0 {
            ctx.setShadow(offset: CGSize(width: 0, height: -layout.shadowOffset * 1.6 * k),
                          blur: layout.shadowBlur * 1.25 * k,
                          color: CGColor(gray: 0, alpha: a * 0.72))
        }
        ctx.beginTransparencyLayer(auxiliaryInfo: nil)
        if a > 0 {
            let contact = max(1.5, layout.shadowBlur * 0.14)
            ctx.setShadow(offset: CGSize(width: 0, height: -contact * 0.35 * k),
                          blur: contact * k,
                          color: CGColor(gray: 0, alpha: min(0.5, a * 0.55)))
        }
        ctx.beginTransparencyLayer(auxiliaryInfo: nil)
        if layout.chrome > 0 {
            drawWindowFrame(layout: layout, edgeColor: edgeColor, ctx: ctx)
        } else if layout.inset > 0 {
            ctx.addPath(squirclePath(layout.frameRect, radius: layout.corner))
            ctx.setFillColor(edgeColor.cgColor)
            ctx.fillPath()
        }
        ctx.saveGState()
        if layout.chrome > 0 {
            ctx.addPath(squirclePath(layout.frameRect, radius: layout.frameCorner))
            ctx.clip()
        }
        let r = min(layout.imageCorner, min(layout.imageRect.width, layout.imageRect.height) / 2)
        ctx.addPath(squirclePath(layout.imageRect, radius: r))
        ctx.clip()
        if let cropped = source.cropping(to: layout.crop.integral) {
            drawImage(cropped, in: layout.imageRect, ctx: ctx)
        }
        ctx.restoreGState()
        if style.hairline, style.background != .none || layout.chrome > 0 || layout.inset > 0 {
            drawHairline(layout: layout, edgeColor: edgeColor, ctx: ctx)
        }
        ctx.endTransparencyLayer()
        ctx.endTransparencyLayer()
        ctx.restoreGState()

        if !style.watermark.trimmingCharacters(in: .whitespaces).isEmpty {
            drawWatermark(style.watermark, layout: layout, ctx: ctx)
        }
    }

    /// A half-point inner edge keeps the screenshot separate from a background of a similar color.
    static func drawHairline(layout: CanvasLayout, edgeColor: RGBA, ctx: CGContext) {
        let outer = layout.chrome > 0 || layout.inset > 0
        let rect = outer ? layout.frameRect : layout.imageRect
        let radius = outer ? (layout.chrome > 0 ? layout.frameCorner : layout.corner)
                           : min(layout.imageCorner, min(rect.width, rect.height) / 2)
        let luminance = 0.2126 * edgeColor.r + 0.7152 * edgeColor.g + 0.0722 * edgeColor.b
        let dark = layout.chrome > 0 ? layout.windowFrame.isDark : luminance < 0.5
        let w = max(1, (max(rect.width, rect.height) / 1400).rounded())
        let path = squirclePath(rect, radius: radius)
        ctx.saveGState()
        ctx.addPath(path)
        ctx.clip()
        ctx.addPath(path)
        ctx.setLineWidth(w * 2)
        ctx.setStrokeColor(dark ? CGColor(gray: 1, alpha: 0.14) : CGColor(gray: 0, alpha: 0.1))
        ctx.strokePath()
        ctx.restoreGState()
    }

    /// Mockup window: title bar with traffic lights (and an address field for the browser look).
    static func drawWindowFrame(layout: CanvasLayout, edgeColor: RGBA, ctx: CGContext) {
        let f = layout.frameRect, h = layout.chrome, fc = layout.frameCorner
        let dark = layout.windowFrame.isDark
        ctx.saveGState()
        ctx.addPath(squirclePath(f, radius: fc))
        ctx.clip()
        ctx.setFillColor(layout.inset > 0 ? edgeColor.cgColor : RGBA(hex: dark ? "#1E1E20" : "#FFFFFF").cgColor)
        ctx.fill(f)
        let bar = CGRect(x: f.minX, y: f.minY, width: f.width, height: h)
        let top = RGBA(hex: dark ? "#3A3A3D" : "#F6F6F6"), bottom = RGBA(hex: dark ? "#2C2C2F" : "#E8E8E8")
        if let g = CGGradient(colorsSpace: colorSpace, colors: [top.cgColor, bottom.cgColor] as CFArray, locations: nil) {
            ctx.saveGState()
            ctx.clip(to: bar)
            ctx.drawLinearGradient(g, start: CGPoint(x: 0, y: bar.minY), end: CGPoint(x: 0, y: bar.maxY), options: [])
            ctx.restoreGState()
        }
        let line = max(1, h / 36)
        ctx.setFillColor(RGBA(hex: dark ? "#000000" : "#C8C8C8", alpha: dark ? 0.6 : 1).cgColor)
        ctx.fill(CGRect(x: bar.minX, y: bar.maxY - line, width: bar.width, height: line))

        let d = h * 0.34, gap = d * 0.62
        let lights = ["#FF5F57", "#FEBC2E", "#28C840"]
        var x = f.minX + h * 0.42
        for hex in lights {
            let c = CGRect(x: x, y: bar.midY - d / 2, width: d, height: d)
            ctx.setFillColor(RGBA(hex: hex).cgColor)
            ctx.fillEllipse(in: c)
            ctx.setStrokeColor(CGColor(gray: 0, alpha: 0.12))
            ctx.setLineWidth(max(0.5, d * 0.05))
            ctx.strokeEllipse(in: c.insetBy(dx: d * 0.025, dy: d * 0.025))
            x += d + gap
        }

        if layout.windowFrame.isBrowser {
            let w = min(f.width * 0.46, max(f.width * 0.3, h * 9))
            let field = CGRect(x: f.midX - w / 2, y: bar.midY - h * 0.29, width: w, height: h * 0.58)
            ctx.addPath(squirclePath(field, radius: field.height * 0.3))
            ctx.setFillColor(RGBA(hex: dark ? "#1C1C1E" : "#FFFFFF", alpha: dark ? 0.9 : 0.95).cgColor)
            ctx.fillPath()
            let dot = field.height * 0.22
            ctx.setFillColor(RGBA(hex: dark ? "#8E8E93" : "#AEAEB2").cgColor)
            ctx.fill(CGRect(x: field.midX - w * 0.18, y: field.midY - dot / 2, width: w * 0.36, height: dot))
        }
        ctx.restoreGState()
    }

    static func drawWatermark(_ text: String, layout: CanvasLayout, ctx: CGContext) {
        let canvas = layout.canvasSize
        let size = max(11, min(canvas.width, canvas.height) * 0.022)
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: size, weight: .semibold),
            .foregroundColor: NSColor(white: 1, alpha: 0.85),
        ]
        let t = text as NSString
        let ts = t.size(withAttributes: attrs)
        let margin = size * 1.1
        ctx.saveGState()
        ctx.setShadow(offset: .zero, blur: size * 0.5 * deviceScale(ctx), color: CGColor(gray: 0, alpha: 0.35))
        AnnotationRenderer.drawText(text, attributes: attrs,
                                    at: CGPoint(x: canvas.width - ts.width - margin, y: canvas.height - ts.height - margin * 0.7),
                                    ctx: ctx)
        ctx.restoreGState()
    }

    private static let grainTile: CGImage? = {
        let n = 192
        var bytes = [UInt8](repeating: 0, count: n * n * 4)
        var rng = SystemRandomNumberGenerator()
        for i in 0..<(n * n) {
            let v = UInt8.random(in: 0...255, using: &rng)
            let a: UInt8 = 60
            let pv = UInt8(Int(v) * Int(a) / 255)
            bytes[i * 4] = pv; bytes[i * 4 + 1] = pv; bytes[i * 4 + 2] = pv; bytes[i * 4 + 3] = a
        }
        guard let provider = CGDataProvider(data: Data(bytes) as CFData) else { return nil }
        return CGImage(width: n, height: n, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: n * 4,
                       space: colorSpace, bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                       provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }()

    /// Tiled in canvas pixels, so the preview and the export show the same grain.
    static func drawGrain(in rect: CGRect, amount: Double, ctx: CGContext) {
        guard let tile = grainTile else { return }
        ctx.saveGState()
        ctx.clip(to: rect)
        ctx.setAlpha(CGFloat(min(1, amount)))
        ctx.setBlendMode(.overlay)
        ctx.interpolationQuality = .none
        ctx.draw(tile, in: CGRect(x: 0, y: 0, width: tile.width, height: tile.height), byTiling: true)
        ctx.restoreGState()
    }

    static func drawBackground(_ bg: Background, in rect: CGRect, blur: Double = 0, ctx: CGContext) {
        switch bg {
        case .none:
            break
        case .solid(let c):
            ctx.setFillColor(c.cgColor)
            ctx.fill(rect)
        case .gradient(let colors, let angle):
            let cgColors = colors.map(\.cgColor) as CFArray
            guard let g = CGGradient(colorsSpace: colorSpace, colors: cgColors, locations: nil) else { return }
            // Angle in degrees, CSS-like: 90 = left→right, 180 = top→bottom.
            let rad = (angle - 90) * .pi / 180
            let dx = cos(rad), dy = sin(rad)
            let half = (abs(rect.width * dx) + abs(rect.height * dy)) / 2
            let c = CGPoint(x: rect.midX, y: rect.midY)
            ctx.drawLinearGradient(g, start: CGPoint(x: c.x - dx * half, y: c.y - dy * half),
                                   end: CGPoint(x: c.x + dx * half, y: c.y + dy * half),
                                   options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
        case .image(let path):
            guard let img = blur > 0 ? ImageCache.shared.blurred(path: path, amount: blur)
                                     : ImageCache.shared.image(path: path) else {
                ctx.setFillColor(RGBA(hex: "#8E8E93").cgColor)
                ctx.fill(rect)
                return
            }
            // Aspect fill.
            let iw = CGFloat(img.width), ih = CGFloat(img.height)
            let s = max(rect.width / iw, rect.height / ih)
            let dw = iw * s, dh = ih * s
            ctx.saveGState()
            ctx.clip(to: rect)
            drawImage(img, in: CGRect(x: rect.midX - dw / 2, y: rect.midY - dh / 2, width: dw, height: dh), ctx: ctx)
            ctx.restoreGState()
        case .mesh(let colors):
            drawMesh(colors, in: rect, ctx: ctx)
        }
    }

    /// A mesh-gradient look built from soft radial blooms, so it also renders on macOS 14 and in exports.
    static func drawMesh(_ colors: [RGBA], in rect: CGRect, ctx: CGContext) {
        guard let base = colors.first else { return }
        ctx.saveGState()
        ctx.clip(to: rect)
        ctx.setFillColor(base.cgColor)
        ctx.fill(rect)
        let anchors: [(CGFloat, CGFloat, CGFloat)] = [(0.12, 0.1, 0.85), (0.92, 0.3, 0.75), (0.4, 1.0, 0.9)]
        let R = max(rect.width, rect.height)
        for (i, c) in colors.dropFirst().prefix(3).enumerated() {
            let (ax, ay, ar) = anchors[i]
            let center = CGPoint(x: rect.minX + rect.width * ax, y: rect.minY + rect.height * ay)
            guard let g = CGGradient(colorsSpace: colorSpace,
                                     colors: [c.with(alpha: 0.95).cgColor, c.with(alpha: 0.45).cgColor, c.with(alpha: 0).cgColor] as CFArray,
                                     locations: [0, 0.45, 1]) else { continue }
            ctx.drawRadialGradient(g, startCenter: center, startRadius: 0, endCenter: center, endRadius: R * ar, options: [])
        }
        ctx.restoreGState()
    }
}

final class ImageCache {
    static let shared = ImageCache()
    private var cache: [String: CGImage] = [:]
    private var blurCache: [String: CGImage] = [:]
    private let lock = NSLock()
    private let ciContext = CIContext(options: [.useSoftwareRenderer: false])

    /// Blur radius is a fraction of the image size, so the preview and the export look the same.
    /// The image is downscaled first: a blurred background loses nothing and the slider stays fast.
    func blurred(path: String, amount: Double) -> CGImage? {
        let q = (min(1, max(0, amount)) * 50).rounded() / 50
        guard q > 0 else { return image(path: path) }
        let key = "\(path)|\(q)"
        lock.lock()
        if let c = blurCache[key] { lock.unlock(); return c }
        lock.unlock()
        guard let src = image(path: path) else { return nil }

        var ci = CIImage(cgImage: src)
        let maxSide = CGFloat(max(src.width, src.height))
        let down = min(1, 1600 / maxSide)
        if down < 1 { ci = ci.transformed(by: CGAffineTransform(scaleX: down, y: down)) }
        let extent = ci.extent.integral
        let radius = q * 0.04 * Double(max(extent.width, extent.height))
        guard let f = CIFilter(name: "CIGaussianBlur") else { return src }
        f.setValue(ci.clampedToExtent(), forKey: kCIInputImageKey)
        f.setValue(radius, forKey: kCIInputRadiusKey)
        guard let out = f.outputImage?.cropped(to: extent),
              let result = ciContext.createCGImage(out, from: extent) else { return src }

        lock.lock()
        if blurCache.count > 8 { blurCache.removeAll() }
        blurCache[key] = result
        lock.unlock()
        return result
    }

    func image(path: String) -> CGImage? {
        lock.lock(); defer { lock.unlock() }
        if let c = cache[path] { return c }
        guard let ns = NSImage(contentsOfFile: path),
              let cg = ns.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
        cache[path] = cg
        return cg
    }
}

enum AnnotationRenderer {
    /// Draws annotations in source-pixel coordinates (ctx already transformed).
    static func drawAll(_ list: [Annotation], ctx: CGContext, effects: EffectImages?, layout: CanvasLayout,
                        hiddenID: UUID? = nil) {
        let labels = counterLabels(list)
        for a in list where a.id != hiddenID {
            draw(a, label: labels[a.id] ?? "", ctx: ctx, effects: effects, layout: layout)
        }
    }

    /// Counters number themselves in stacking order. A counter set to a number restarts the
    /// sequence from it; one set to other text (a letter, say) shows it without taking a number.
    static func counterLabels(_ list: [Annotation]) -> [UUID: String] {
        var n = 0
        var out: [UUID: String] = [:]
        for a in list where a.kind == .counter {
            let custom = a.text.trimmingCharacters(in: .whitespacesAndNewlines)
            if custom.isEmpty { n += 1 } else if let v = Int(custom) { n = v }
            out[a.id] = custom.isEmpty ? "\(n)" : custom
        }
        return out
    }

    static func draw(_ a: Annotation, label: String, ctx: CGContext, effects: EffectImages?, layout: CanvasLayout) {
        let k = Renderer.deviceScale(ctx)
        ctx.saveGState()
        defer { ctx.restoreGState() }
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)

        switch a.kind {
        case .pixelate, .blur:
            guard let img = (a.kind == .pixelate ? effects?.pixelated : effects?.blurred) else { return }
            let crop = layout.crop
            let r = layout.imageCorner
            ctx.addPath(squirclePath(crop, radius: r))
            ctx.clip()
            ctx.clip(to: a.rect)
            Renderer.drawImage(img, in: CGRect(x: 0, y: 0, width: img.width, height: img.height), ctx: ctx)

        case .highlight:
            ctx.setFillColor(a.color.with(alpha: 0.38).cgColor)
            let r = a.rect
            ctx.addPath(CGPath(roundedRect: r, cornerWidth: min(4, r.width / 2), cornerHeight: min(4, r.height / 2), transform: nil))
            ctx.fillPath()

        case .rect:
            softShadow(ctx, k: k, width: a.width)
            let r = a.rect
            let cr = min(a.width * 1.5, min(r.width, r.height) / 2)
            ctx.addPath(CGPath(roundedRect: r, cornerWidth: cr, cornerHeight: cr, transform: nil))
            ctx.setStrokeColor(a.color.cgColor)
            ctx.setLineWidth(a.width)
            ctx.strokePath()

        case .ellipse:
            softShadow(ctx, k: k, width: a.width)
            ctx.addEllipse(in: a.rect)
            ctx.setStrokeColor(a.color.cgColor)
            ctx.setLineWidth(a.width)
            ctx.strokePath()

        case .arrow:
            softShadow(ctx, k: k, width: a.width)
            drawArrow(from: a.p1, to: a.p2, width: a.width, color: a.color.cgColor, ctx: ctx)

        case .counter:
            softShadow(ctx, k: k, width: a.width)
            let r = a.counterRadius
            let circle = CGRect(x: a.p1.x - r, y: a.p1.y - r, width: r * 2, height: r * 2)
            ctx.setFillColor(a.color.cgColor)
            ctx.fillEllipse(in: circle)
            ctx.setShadow(offset: .zero, blur: 0, color: nil)
            ctx.setStrokeColor(CGColor(gray: 1, alpha: 0.9))
            ctx.setLineWidth(max(1.5, r * 0.12))
            ctx.strokeEllipse(in: circle.insetBy(dx: r * 0.06, dy: r * 0.06))
            let textColor: NSColor = a.color.r + a.color.g + a.color.b > 2.4 ? .black : .white
            var font = NSFont.systemFont(ofSize: r * 1.15, weight: .heavy)
            let w = (label as NSString).size(withAttributes: [.font: font]).width
            if w > r * 1.5 { font = NSFont.systemFont(ofSize: r * 1.15 * r * 1.5 / w, weight: .heavy) }
            drawText(label, attributes: [.font: font, .foregroundColor: textColor], centeredIn: circle, ctx: ctx)

        case .text:
            let blur = a.fontSize * 0.25
            ctx.setShadow(offset: CGSize(width: 0, height: -blur * 0.2 * k), blur: blur * k,
                          color: CGColor(gray: 0, alpha: 0.35))
            drawText(a.text, attributes: a.textAttributes, at: a.p1, ctx: ctx)
        }
    }

    static func softShadow(_ ctx: CGContext, k: CGFloat, width: CGFloat) {
        ctx.setShadow(offset: CGSize(width: 0, height: -width * 0.4 * k), blur: width * 1.5 * k,
                      color: CGColor(gray: 0, alpha: 0.3))
    }

    static func drawArrow(from p1: CGPoint, to p2: CGPoint, width: CGFloat, color: CGColor, ctx: CGContext) {
        let dx = p2.x - p1.x, dy = p2.y - p1.y
        let len = hypot(dx, dy)
        guard len > 0.5 else { return }
        let ux = dx / len, uy = dy / len
        let headLen = min(max(width * 4.5, 14), len * 0.9)
        let headHalf = headLen * 0.55
        let base = CGPoint(x: p2.x - ux * headLen, y: p2.y - uy * headLen)

        ctx.beginTransparencyLayer(auxiliaryInfo: nil)
        // Tapered shaft: thin at the tail, full width at the head.
        let tailHalf = width * 0.25, headShaftHalf = width * 0.5
        let nx = -uy, ny = ux
        let shaftEnd = CGPoint(x: base.x + ux * headLen * 0.3, y: base.y + uy * headLen * 0.3)
        let path = CGMutablePath()
        path.move(to: CGPoint(x: p1.x + nx * tailHalf, y: p1.y + ny * tailHalf))
        path.addLine(to: CGPoint(x: shaftEnd.x + nx * headShaftHalf, y: shaftEnd.y + ny * headShaftHalf))
        path.addLine(to: CGPoint(x: shaftEnd.x - nx * headShaftHalf, y: shaftEnd.y - ny * headShaftHalf))
        path.addLine(to: CGPoint(x: p1.x - nx * tailHalf, y: p1.y - ny * tailHalf))
        path.closeSubpath()

        // Fill each piece on its own: in a single path, opposite windings cancel out where
        // they overlap (nonzero rule) and leave a hole where the shaft meets the head.
        ctx.setFillColor(color)
        ctx.addPath(path)
        ctx.fillPath()
        ctx.fillEllipse(in: CGRect(x: p1.x - tailHalf, y: p1.y - tailHalf, width: tailHalf * 2, height: tailHalf * 2))

        // Head
        ctx.move(to: p2)
        ctx.addLine(to: CGPoint(x: base.x + nx * headHalf, y: base.y + ny * headHalf))
        ctx.addLine(to: CGPoint(x: base.x - nx * headHalf, y: base.y - ny * headHalf))
        ctx.closePath()
        ctx.fillPath()
        // Round the head's corners slightly.
        ctx.move(to: p2)
        ctx.addLine(to: CGPoint(x: base.x + nx * headHalf, y: base.y + ny * headHalf))
        ctx.addLine(to: CGPoint(x: base.x - nx * headHalf, y: base.y - ny * headHalf))
        ctx.closePath()
        ctx.setStrokeColor(color)
        ctx.setLineWidth(max(1, width * 0.35))
        ctx.strokePath()
        ctx.endTransparencyLayer()
    }

    static func drawText(_ text: String, attributes: [NSAttributedString.Key: Any], at p: CGPoint, ctx: CGContext) {
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: true)
        let s = text as NSString
        let size = s.boundingRect(with: CGSize(width: 100_000, height: 100_000),
                                  options: [.usesLineFragmentOrigin], attributes: attributes).size
        s.draw(with: CGRect(origin: p, size: CGSize(width: ceil(size.width) + 2, height: ceil(size.height) + 2)),
               options: [.usesLineFragmentOrigin], attributes: attributes)
        NSGraphicsContext.restoreGraphicsState()
    }

    static func drawText(_ text: String, attributes: [NSAttributedString.Key: Any], centeredIn rect: CGRect, ctx: CGContext) {
        let size = (text as NSString).size(withAttributes: attributes)
        drawText(text, attributes: attributes,
                 at: CGPoint(x: rect.midX - size.width / 2, y: rect.midY - size.height / 2), ctx: ctx)
    }
}
