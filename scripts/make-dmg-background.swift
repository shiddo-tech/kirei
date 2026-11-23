// Draws the DMG window background (660×400 pt) at 2x. Icons sit at (170, 190) and (490, 190).
import AppKit

let W: CGFloat = 660, H: CGFloat = 400, scale: CGFloat = 2
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(W * scale), pixelsHigh: Int(H * scale),
                           bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                           colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
rep.size = NSSize(width: W, height: H)
NSGraphicsContext.saveGraphicsState()
let gc = NSGraphicsContext(bitmapImageRep: rep)!
NSGraphicsContext.current = gc
let ctx = gc.cgContext // already 2x: rep.size is in points
let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: a)
}

// Base: warm off-white with two soft green blooms, echoing the icon.
ctx.setFillColor(rgb(0xF7F6F2))
ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
for (center, radius, color) in [(CGPoint(x: 120, y: 360), CGFloat(380), rgb(0x5CC08A, 0.22)),
                                (CGPoint(x: 600, y: 20), CGFloat(340), rgb(0x3E8C63, 0.16))] {
    let g = CGGradient(colorsSpace: srgb, colors: [color, rgb(0x5CC08A, 0)] as CFArray, locations: [0, 1])!
    ctx.drawRadialGradient(g, startCenter: center, startRadius: 0, endCenter: center, endRadius: radius, options: [])
}

// Arrow between the two icons (y-up: icons at y = H - 190).
let y = H - 190
let arrow = CGMutablePath()
arrow.move(to: CGPoint(x: 262, y: y + 6))
arrow.addCurve(to: CGPoint(x: 392, y: y + 6), control1: CGPoint(x: 300, y: y + 40), control2: CGPoint(x: 354, y: y + 40))
ctx.saveGState()
ctx.addPath(arrow)
ctx.setStrokeColor(rgb(0x2E6B4B, 0.55))
ctx.setLineWidth(3)
ctx.setLineCap(.round)
ctx.setLineDash(phase: 0, lengths: [0.1, 9])
ctx.strokePath()
ctx.restoreGState()
// Head aligned with the curve's end tangent.
let tip = CGPoint(x: 394, y: y + 4)
let back = atan2(40.0 - 6.0, 354.0 - 392.0)
let head = CGMutablePath()
head.move(to: CGPoint(x: tip.x + 14 * cos(back + 0.6), y: tip.y + 14 * sin(back + 0.6)))
head.addLine(to: tip)
head.addLine(to: CGPoint(x: tip.x + 14 * cos(back - 0.6), y: tip.y + 14 * sin(back - 0.6)))
ctx.addPath(head)
ctx.setStrokeColor(rgb(0x2E6B4B, 0.6))
ctx.setLineWidth(3)
ctx.setLineCap(.round)
ctx.setLineJoin(.round)
ctx.strokePath()

func text(_ s: String, size: CGFloat, weight: NSFont.Weight, color: NSColor, y: CGFloat) {
    let attrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: color]
    let str = NSAttributedString(string: s, attributes: attrs)
    let w = str.size().width
    str.draw(at: NSPoint(x: (W - w) / 2, y: y))
}
text("Arraste o Kirei para Aplicativos", size: 15, weight: .semibold, color: NSColor(srgbRed: 0.14, green: 0.24, blue: 0.19, alpha: 0.9), y: 72)
text("Drag Kirei to Applications · Arrastra Kirei a Aplicaciones", size: 11, weight: .regular,
     color: NSColor(srgbRed: 0.14, green: 0.24, blue: 0.19, alpha: 0.55), y: 52)

NSGraphicsContext.restoreGraphicsState()
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
