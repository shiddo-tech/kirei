import AppKit

/// What the user picked on the selection overlay.
enum Selection {
    /// Rect in Cocoa global coordinates, on the given screen.
    case area(CGRect, NSScreen)
    case window(CGWindowID)
}

/// An on-screen window that can be picked in window mode.
struct PickableWindow {
    let id: CGWindowID
    /// Cocoa global coordinates.
    let frame: CGRect
}

/// Full-screen overlay (one window per display) for picking an area or a window,
/// in the spirit of the system screenshot UI.
final class SelectionOverlay {
    private static var current: SelectionOverlay?

    private var windows: [OverlayWindow] = []
    private let completion: (Selection?) -> Void
    private(set) var windowMode: Bool
    private(set) var hovered: PickableWindow?
    private let pickable: [PickableWindow]
    let frozen: [CGDirectDisplayID: CapturedImage]

    static var isActive: Bool { current != nil }

    static func begin(windowMode: Bool, frozen: [CGDirectDisplayID: CapturedImage] = [:],
                      completion: @escaping (Selection?) -> Void) {
        guard current == nil else { return }
        let overlay = SelectionOverlay(windowMode: windowMode, frozen: frozen, completion: completion)
        current = overlay
        overlay.show()
    }

    private init(windowMode: Bool, frozen: [CGDirectDisplayID: CapturedImage], completion: @escaping (Selection?) -> Void) {
        self.windowMode = windowMode
        self.frozen = frozen
        self.completion = completion
        pickable = SelectionOverlay.onScreenWindows()
    }

    private func show() {
        let mouse = NSEvent.mouseLocation
        for screen in NSScreen.screens {
            let w = OverlayWindow(screen: screen, overlay: self)
            windows.append(w)
            w.orderFrontRegardless()
            w.display()
        }
        updateHover(at: mouse)
        // Activating Kirei closes menus open in other apps. Do it only after the frozen frame is
        // on screen, so the menu closes hidden under the overlay instead of flickering.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in
            guard let self, SelectionOverlay.current === self else { return }
            NSApp.activate(ignoringOtherApps: true)
            let key = self.windows.first { NSMouseInRect(NSEvent.mouseLocation, $0.frame, false) } ?? self.windows.first
            key?.makeKeyAndOrderFront(nil)
        }
    }

    // MARK: State

    func toggleMode() {
        windowMode.toggle()
        updateHover(at: NSEvent.mouseLocation)
        windows.forEach { $0.overlayView.modeChanged() }
    }

    func updateHover(at p: CGPoint) {
        hovered = windowMode ? pickable.first { $0.frame.contains(p) } : nil
        windows.forEach { $0.overlayView.needsDisplay = true }
    }

    func finish(_ selection: Selection?) {
        guard SelectionOverlay.current === self else { return }
        for w in windows {
            w.overlayView.detach()
            w.orderOut(nil)
            w.close()
        }
        windows.removeAll()
        SelectionOverlay.current = nil
        // Let the overlay leave the screen before anything is captured.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.12) { [completion] in completion(selection) }
    }

    // MARK: Windows under the cursor

    /// Normal app windows, front to back, excluding Kirei's own.
    private static func onScreenWindows() -> [PickableWindow] {
        guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
                as? [[String: Any]] else { return [] }
        let primaryHeight = NSScreen.screens.first?.frame.height ?? 0
        let me = ProcessInfo.processInfo.processIdentifier
        return list.compactMap { info in
            guard (info[kCGWindowLayer as String] as? Int) == 0,
                  (info[kCGWindowOwnerPID as String] as? Int32) != me,
                  (info[kCGWindowAlpha as String] as? Double ?? 1) > 0.01,
                  let id = info[kCGWindowNumber as String] as? CGWindowID,
                  let b = info[kCGWindowBounds as String] as? [String: CGFloat],
                  let x = b["X"], let y = b["Y"], let w = b["Width"], let h = b["Height"],
                  w > 40, h > 40 else { return nil }
            // CG global (top-left origin) → Cocoa global (bottom-left origin).
            return PickableWindow(id: id, frame: CGRect(x: x, y: primaryHeight - y - h, width: w, height: h))
        }
    }
}

private final class OverlayWindow: NSWindow {
    let overlayView: OverlayView

    init(screen: NSScreen, overlay: SelectionOverlay) {
        overlayView = OverlayView(overlay: overlay, screen: screen)
        super.init(contentRect: screen.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        setFrame(screen.frame, display: false)
        level = .screenSaver
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        animationBehavior = .none
        ignoresMouseEvents = false
        acceptsMouseMovedEvents = true
        isReleasedWhenClosed = false
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        contentView = overlayView
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
}

private final class OverlayView: NSView {
    /// Weak: AppKit may still draw or deliver an event after the overlay finished.
    private weak var overlay: SelectionOverlay?
    private let screen: NSScreen
    private var dragStart: CGPoint?
    private var dragCurrent: CGPoint?
    private var mouse: CGPoint?

    init(overlay: SelectionOverlay, screen: NSScreen) {
        self.overlay = overlay
        self.screen = screen
        super.init(frame: CGRect(origin: .zero, size: screen.frame.size))
        addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeAlways, .inVisibleRect, .cursorUpdate],
                                       owner: self, userInfo: nil))
    }

    required init?(coder: NSCoder) { fatalError() }

    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    func detach() {
        overlay = nil
        trackingAreas.forEach(removeTrackingArea)
    }

    func modeChanged() {
        dragStart = nil
        dragCurrent = nil
        window?.invalidateCursorRects(for: self)
        needsDisplay = true
    }

    override func resetCursorRects() {
        guard let overlay else { return }
        addCursorRect(bounds, cursor: overlay.windowMode ? .pointingHand : .crosshair)
    }

    override func cursorUpdate(with event: NSEvent) {
        guard let overlay else { return }
        (overlay.windowMode ? NSCursor.pointingHand : NSCursor.crosshair).set()
    }

    // MARK: Geometry

    private func toGlobal(_ p: CGPoint) -> CGPoint {
        CGPoint(x: p.x + screen.frame.minX, y: p.y + screen.frame.minY)
    }

    private func toLocal(_ r: CGRect) -> CGRect {
        r.offsetBy(dx: -screen.frame.minX, dy: -screen.frame.minY)
    }

    /// Snapped to whole points, the same grid the capture uses.
    private var selection: CGRect? {
        guard let a = dragStart, let b = dragCurrent else { return nil }
        let x0 = min(a.x, b.x).rounded(), x1 = max(a.x, b.x).rounded()
        let y0 = min(a.y, b.y).rounded(), y1 = max(a.y, b.y).rounded()
        return CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0).intersection(bounds)
    }

    private func clamped(_ p: CGPoint) -> CGPoint {
        CGPoint(x: min(max(p.x, 0), bounds.width), y: min(max(p.y, 0), bounds.height))
    }

    // MARK: Events

    override func mouseMoved(with event: NSEvent) {
        guard let overlay else { return }
        mouse = convert(event.locationInWindow, from: nil)
        overlay.updateHover(at: NSEvent.mouseLocation)
    }

    override func mouseDown(with event: NSEvent) {
        guard let overlay else { return }
        if overlay.windowMode {
            if let w = overlay.hovered { overlay.finish(.window(w.id)) }
            return
        }
        dragStart = clamped(convert(event.locationInWindow, from: nil))
        dragCurrent = dragStart
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        guard dragStart != nil else { return }
        dragCurrent = clamped(convert(event.locationInWindow, from: nil))
        mouse = dragCurrent
        needsDisplay = true
    }

    override func mouseUp(with event: NSEvent) {
        guard let overlay, let r = selection else { return }
        if r.width >= 4 && r.height >= 4 {
            overlay.finish(.area(r.offsetBy(dx: screen.frame.minX, dy: screen.frame.minY), screen))
        } else {
            // A plain click switches to window mode on the window under the cursor.
            dragStart = nil
            dragCurrent = nil
            overlay.toggleMode()
            if let w = overlay.hovered { overlay.finish(.window(w.id)) }
        }
    }

    override func rightMouseDown(with event: NSEvent) { overlay?.finish(nil) }

    override func keyDown(with event: NSEvent) {
        guard let overlay else { return }
        switch event.keyCode {
        case 53: overlay.finish(nil)         // esc
        case 49: overlay.toggleMode()        // space
        default: super.keyDown(with: event)
        }
    }

    // MARK: Drawing

    override func draw(_ dirtyRect: NSRect) {
        guard let overlay, let ctx = NSGraphicsContext.current?.cgContext else { return }
        let dim = CGColor(gray: 0, alpha: 0.32)
        let frozen = overlay.frozen[CaptureService.displayID(of: screen)]?.image
        if let frozen { ctx.draw(frozen, in: bounds) }
        func reveal(_ r: CGRect) {
            if let frozen {
                ctx.saveGState()
                ctx.clip(to: r)
                ctx.draw(frozen, in: bounds)
                ctx.restoreGState()
            } else {
                ctx.setBlendMode(.clear)
                ctx.fill(r)
                ctx.setBlendMode(.normal)
            }
        }

        if overlay.windowMode {
            ctx.setFillColor(dim)
            ctx.fill(bounds)
            if let w = overlay.hovered {
                let r = toLocal(w.frame)
                reveal(r)
                ctx.setFillColor(NSColor.controlAccentColor.withAlphaComponent(0.18).cgColor)
                ctx.fill(r)
                ctx.setStrokeColor(NSColor.controlAccentColor.cgColor)
                ctx.setLineWidth(3)
                ctx.stroke(r.insetBy(dx: 1.5, dy: 1.5))
            }
            drawHint(L("Click a window to capture it · Space: select area · Esc: cancel"))
            return
        }

        ctx.setFillColor(dim)
        ctx.fill(bounds)

        if let r = selection {
            reveal(r)
            ctx.setStrokeColor(CGColor(gray: 1, alpha: 0.95))
            ctx.setLineWidth(1)
            ctx.stroke(r.insetBy(dx: -0.5, dy: -0.5))
            drawSizeLabel(for: r)
            if let m = mouse, let frozen { drawLoupe(at: m, image: frozen, ctx: ctx) }
        } else if let m = mouse, bounds.contains(m) {
            // Crosshair guides.
            ctx.setStrokeColor(CGColor(gray: 1, alpha: 0.55))
            ctx.setLineWidth(1)
            ctx.move(to: CGPoint(x: 0, y: m.y.rounded() + 0.5)); ctx.addLine(to: CGPoint(x: bounds.width, y: m.y.rounded() + 0.5))
            ctx.move(to: CGPoint(x: m.x.rounded() + 0.5, y: 0)); ctx.addLine(to: CGPoint(x: m.x.rounded() + 0.5, y: bounds.height))
            ctx.strokePath()
            if let frozen { drawLoupe(at: m, image: frozen, ctx: ctx) }
            drawHint(L("Drag to select an area · Space or click: pick a window · Esc: cancel"))
        } else {
            drawHint(L("Drag to select an area · Space or click: pick a window · Esc: cancel"))
        }
    }

    /// Magnified pixels around the cursor with a pixel grid, the center pixel and the coordinates.
    private func drawLoupe(at m: CGPoint, image: CGImage, ctx: CGContext) {
        let cells = 15, cell: CGFloat = 8
        let side = CGFloat(cells) * cell
        let scale = CGFloat(image.width) / bounds.width
        // View points (bottom-left origin) → image pixels (top-left origin).
        let px = Int((m.x * scale).rounded(.down)), py = Int(((bounds.height - m.y) * scale).rounded(.down))
        let half = cells / 2
        let src = CGRect(x: px - half, y: py - half, width: cells, height: cells)
        let imageBounds = CGRect(x: 0, y: 0, width: image.width, height: image.height)

        var origin = CGPoint(x: m.x + 22, y: m.y - side - 22)
        if origin.x + side > bounds.width - 8 { origin.x = m.x - side - 22 }
        if origin.y < 8 { origin.y = m.y + 22 }
        let box = CGRect(origin: origin, size: CGSize(width: side, height: side))
        let shape = CGPath(roundedRect: box, cornerWidth: 14, cornerHeight: 14, transform: nil)

        ctx.saveGState()
        ctx.setShadow(offset: CGSize(width: 0, height: -3), blur: 12, color: CGColor(gray: 0, alpha: 0.45))
        ctx.addPath(shape)
        ctx.setFillColor(CGColor(gray: 0.1, alpha: 1))
        ctx.fillPath()
        ctx.restoreGState()

        ctx.saveGState()
        ctx.addPath(shape)
        ctx.clip()
        let visible = src.intersection(imageBounds)
        if !visible.isEmpty, let piece = image.cropping(to: visible) {
            let dx = (visible.minX - src.minX) * cell
            let dyTop = (visible.minY - src.minY) * cell
            let dest = CGRect(x: box.minX + dx, y: box.maxY - dyTop - visible.height * cell,
                              width: visible.width * cell, height: visible.height * cell)
            ctx.interpolationQuality = .none
            ctx.draw(piece, in: dest)
        }
        ctx.setStrokeColor(CGColor(gray: 0, alpha: 0.14))
        ctx.setLineWidth(0.5)
        for i in 1..<cells {
            let o = CGFloat(i) * cell
            ctx.move(to: CGPoint(x: box.minX + o, y: box.minY)); ctx.addLine(to: CGPoint(x: box.minX + o, y: box.maxY))
            ctx.move(to: CGPoint(x: box.minX, y: box.minY + o)); ctx.addLine(to: CGPoint(x: box.maxX, y: box.minY + o))
        }
        ctx.strokePath()
        let center = CGRect(x: box.minX + CGFloat(half) * cell, y: box.minY + CGFloat(half) * cell, width: cell, height: cell)
        ctx.setStrokeColor(.white)
        ctx.setLineWidth(1.5)
        ctx.stroke(center)
        ctx.setStrokeColor(CGColor(gray: 0, alpha: 0.6))
        ctx.setLineWidth(0.5)
        ctx.stroke(center.insetBy(dx: -1, dy: -1))
        ctx.restoreGState()

        ctx.addPath(shape)
        ctx.setStrokeColor(CGColor(gray: 1, alpha: 0.85))
        ctx.setLineWidth(1.5)
        ctx.strokePath()

        let label = "\(Int(m.x)), \(Int(bounds.height - m.y))" as NSString
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .medium),
            .foregroundColor: NSColor.white,
        ]
        let ls = label.size(withAttributes: attrs)
        let lbox = CGRect(x: box.midX - ls.width / 2 - 7, y: box.minY - ls.height - 10, width: ls.width + 14, height: ls.height + 5)
        NSColor.black.withAlphaComponent(0.72).setFill()
        NSBezierPath(roundedRect: lbox, xRadius: lbox.height / 2, yRadius: lbox.height / 2).fill()
        label.draw(at: CGPoint(x: lbox.minX + 7, y: lbox.minY + 2.5), withAttributes: attrs)
    }

    private func drawSizeLabel(for r: CGRect) {
        let text = "\(Int(r.width)) × \(Int(r.height))" as NSString
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium),
            .foregroundColor: NSColor.white,
        ]
        let size = text.size(withAttributes: attrs)
        var box = CGRect(x: r.maxX - size.width - 12, y: r.minY - size.height - 12, width: size.width + 12, height: size.height + 6)
        if box.minY < 4 { box.origin.y = r.minY + 6 }
        if box.minX < 4 { box.origin.x = r.minX }
        NSColor.black.withAlphaComponent(0.7).setFill()
        NSBezierPath(roundedRect: box, xRadius: 5, yRadius: 5).fill()
        text.draw(at: CGPoint(x: box.minX + 6, y: box.minY + 3), withAttributes: attrs)
    }

    private func drawHint(_ hint: String) {
        // Only on the screen with the cursor.
        guard NSMouseInRect(NSEvent.mouseLocation, screen.frame, false) else { return }
        let text = hint as NSString
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 13, weight: .medium),
            .foregroundColor: NSColor.white,
        ]
        let size = text.size(withAttributes: attrs)
        let box = CGRect(x: (bounds.width - size.width) / 2 - 14, y: bounds.height - 90, width: size.width + 28, height: size.height + 14)
        NSColor.black.withAlphaComponent(0.65).setFill()
        NSBezierPath(roundedRect: box, xRadius: box.height / 2, yRadius: box.height / 2).fill()
        text.draw(at: CGPoint(x: box.minX + 14, y: box.minY + 7), withAttributes: attrs)
    }
}
