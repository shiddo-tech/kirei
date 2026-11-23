import AppKit

/// The Kirei mark: two frame corners on the Shiddo diagonal (concept A · Moldura).
enum KireiLogo {
    /// Mark path in a 100×100 box, y-down.
    static let path: CGPath = {
        let p = CGMutablePath()
        for rotated in [false, true] {
            let t: (CGFloat, CGFloat) -> CGPoint = { x, y in rotated ? CGPoint(x: 100 - x, y: 100 - y) : CGPoint(x: x, y: y) }
            p.move(to: t(8, 60))
            p.addLine(to: t(8, 36))
            p.addCurve(to: t(36, 8), control1: t(8, 20), control2: t(20, 8))
            p.addLine(to: t(60, 8))
            p.addLine(to: t(54, 24))
            p.addLine(to: t(40, 24))
            p.addCurve(to: t(24, 40), control1: t(31, 24), control2: t(24, 31))
            p.addLine(to: t(24, 54))
            p.closeSubpath()
        }
        return p
    }()

    /// Template image for the menu bar; macOS tints it for light/dark mode.
    static func menuBarImage(size: CGFloat = 18) -> NSImage {
        let img = NSImage(size: NSSize(width: size, height: size), flipped: true) { rect in
            guard let ctx = NSGraphicsContext.current?.cgContext else { return false }
            let inset: CGFloat = 1
            let s = (rect.width - inset * 2) / 100
            ctx.translateBy(x: inset, y: inset)
            ctx.scaleBy(x: s, y: s)
            ctx.addPath(path)
            ctx.setFillColor(.black)
            ctx.fillPath()
            return true
        }
        img.isTemplate = true
        img.accessibilityDescription = "Kirei"
        return img
    }
}
