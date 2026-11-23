// Draws the Kirei app icon (concept A · Moldura) at 1024px and writes a PNG.
import AppKit

let S: CGFloat = 1024
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(S), pixelsHigh: Int(S), bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                           bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
let gc = NSGraphicsContext(bitmapImageRep: rep)!
NSGraphicsContext.current = gc
let ctx = gc.cgContext

// Standard macOS icon body: 824pt square with the usual margin.
let body = CGRect(x: 100, y: 100, width: 824, height: 824)
let bodyPath = CGPath(roundedRect: body, cornerWidth: 185, cornerHeight: 185, transform: nil)

let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: a)
}

// Body: deep green gradient with a soft drop shadow.
ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: -14), blur: 34, color: CGColor(gray: 0, alpha: 0.35))
ctx.addPath(bodyPath)
ctx.setFillColor(rgb(0x3E8C63))
ctx.fillPath()
ctx.restoreGState()

ctx.saveGState()
ctx.addPath(bodyPath)
ctx.clip()
let bodyGradient = CGGradient(colorsSpace: srgb, colors: [rgb(0x5CC08A), rgb(0x3E8C63), rgb(0x235C40)] as CFArray,
                              locations: [0, 0.5, 1])!
ctx.drawLinearGradient(bodyGradient, start: CGPoint(x: body.minX, y: body.maxY), end: CGPoint(x: body.maxX, y: body.minY), options: [])
// Light bloom from the top-left and a darker floor.
let bloom = CGGradient(colorsSpace: srgb, colors: [CGColor(gray: 1, alpha: 0.28), CGColor(gray: 1, alpha: 0)] as CFArray,
                       locations: [0, 1])!
ctx.drawRadialGradient(bloom, startCenter: CGPoint(x: body.minX + 230, y: body.maxY - 170), startRadius: 0,
                       endCenter: CGPoint(x: body.minX + 230, y: body.maxY - 170), endRadius: 620, options: [])
let floor = CGGradient(colorsSpace: srgb, colors: [CGColor(gray: 0, alpha: 0), CGColor(gray: 0, alpha: 0.22)] as CFArray,
                       locations: [0, 1])!
ctx.drawLinearGradient(floor, start: CGPoint(x: 0, y: body.midY), end: CGPoint(x: 0, y: body.minY), options: [])
ctx.restoreGState()

// Inner rim: bright at the top, fading out, like a glass edge.
ctx.saveGState()
ctx.addPath(bodyPath)
ctx.clip()
ctx.addPath(CGPath(roundedRect: body.insetBy(dx: 3, dy: 3), cornerWidth: 182, cornerHeight: 182, transform: nil))
ctx.setLineWidth(6)
ctx.replacePathWithStrokedPath()
ctx.clip()
let rim = CGGradient(colorsSpace: srgb, colors: [CGColor(gray: 1, alpha: 0.45), CGColor(gray: 1, alpha: 0.05), CGColor(gray: 0, alpha: 0.15)] as CFArray,
                     locations: [0, 0.5, 1])!
ctx.drawLinearGradient(rim, start: CGPoint(x: 0, y: body.maxY), end: CGPoint(x: 0, y: body.minY), options: [])
ctx.restoreGState()

// The mark: two frame corners on the Shiddo diagonal, in a 100×100 y-down box.
func corner(_ t: (CGFloat, CGFloat) -> CGPoint) -> CGPath {
    let p = CGMutablePath()
    p.move(to: t(8, 60))
    p.addLine(to: t(8, 36))
    p.addCurve(to: t(36, 8), control1: t(8, 20), control2: t(20, 8))
    p.addLine(to: t(60, 8))
    p.addLine(to: t(54, 24))
    p.addLine(to: t(40, 24))
    p.addCurve(to: t(24, 40), control1: t(31, 24), control2: t(24, 31))
    p.addLine(to: t(24, 54))
    p.closeSubpath()
    return p
}
let markSize: CGFloat = 520
let origin = CGPoint(x: (S - markSize) / 2, y: (S - markSize) / 2)
let k = markSize / 100
// y-down mark space → y-up bitmap space
let map: (CGFloat, CGFloat) -> CGPoint = { x, y in CGPoint(x: origin.x + x * k, y: origin.y + (100 - y) * k) }
let mapRotated: (CGFloat, CGFloat) -> CGPoint = { x, y in map(100 - x, 100 - y) }
let markPath = CGMutablePath()
markPath.addPath(corner(map))
markPath.addPath(corner(mapRotated))

// Mark: raised white frame corners with a soft green-tinted shadow and a subtle top-to-bottom sheen.
ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: -16), blur: 30, color: rgb(0x0E3322, 0.55))
ctx.addPath(markPath)
ctx.setFillColor(.white)
ctx.fillPath()
ctx.restoreGState()

ctx.saveGState()
ctx.addPath(markPath)
ctx.clip()
let sheen = CGGradient(colorsSpace: srgb, colors: [rgb(0xFFFFFF), rgb(0xFFFFFF), rgb(0xD9F0E3)] as CFArray,
                       locations: [0, 0.45, 1])!
ctx.drawLinearGradient(sheen, start: CGPoint(x: 0, y: origin.y + markSize), end: CGPoint(x: 0, y: origin.y), options: [])
ctx.restoreGState()

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
