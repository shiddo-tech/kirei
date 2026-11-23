import AppKit
import SwiftUI

/// An image floating above every window, as a reference while you work.
/// Drag to move, scroll or pinch to resize, double-click to edit, right-click for options.
final class PinnedImage: NSObject, NSWindowDelegate {
    private static var open: [PinnedImage] = []

    private let window: PinWindow
    private let captured: CapturedImage
    private let aspect: CGFloat

    static func pin(_ captured: CapturedImage, near rect: CGRect? = nil) {
        open.append(PinnedImage(captured, near: rect))
    }

    private init(_ captured: CapturedImage, near rect: CGRect?) {
        self.captured = captured
        let img = captured.image
        let natural = NSSize(width: CGFloat(img.width) / captured.scale, height: CGFloat(img.height) / captured.scale)
        aspect = natural.width / max(1, natural.height)
        let screen = NSScreen.screens.first { NSMouseInRect(NSEvent.mouseLocation, $0.frame, false) } ?? NSScreen.main
        let vf = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let fit = min(1, vf.width * 0.5 / natural.width, vf.height * 0.5 / natural.height)
        let size = NSSize(width: natural.width * fit, height: natural.height * fit)
        let origin = rect.map { NSPoint(x: $0.minX, y: $0.maxY - size.height) }
            ?? NSPoint(x: vf.midX - size.width / 2, y: vf.midY - size.height / 2)

        window = PinWindow(contentRect: NSRect(origin: origin, size: size),
                           styleMask: [.borderless, .resizable], backing: .buffered, defer: false)
        super.init()
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = true
        window.level = .floating
        window.isMovableByWindowBackground = true
        window.isReleasedWhenClosed = false
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.contentAspectRatio = size
        window.minSize = NSSize(width: 60, height: 60 / aspect)
        window.delegate = self

        let view = PinView(image: img, onClose: { [weak self] in self?.close() },
                           onEdit: { [weak self] in self?.edit() })
        let host = NSHostingView(rootView: view)
        window.contentView = host
        window.menuProvider = { [weak self] in self?.menu() }
        window.onScroll = { [weak self] dy in self?.scale(by: 1 + dy * 0.01) }
        window.onMagnify = { [weak self] m in self?.scale(by: 1 + m) }

        window.alphaValue = 0
        window.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.2
            window.animator().alphaValue = 1
        }
    }

    private func scale(by factor: CGFloat) {
        var f = window.frame
        let w = max(window.minSize.width, min(f.width * factor, 4000))
        let h = w / aspect
        f.origin.x += (f.width - w) / 2
        f.origin.y += (f.height - h) / 2
        f.size = NSSize(width: w, height: h)
        window.setFrame(f, display: true)
    }

    private func menu() -> NSMenu {
        let m = NSMenu()
        func add(_ title: String, _ action: @escaping () -> Void) {
            let item = ClosureMenuItem(title: title, action: action)
            m.addItem(item)
        }
        add(L("Copy")) { [weak self] in
            guard let self else { return }
            Exporter.copy(self.captured.image, scale: self.captured.scale)
            Sounds.playCopy()
        }
        add(L("Edit")) { [weak self] in self?.edit() }
        add(L("Actual Size")) { [weak self] in
            guard let self else { return }
            let w = CGFloat(self.captured.image.width) / self.captured.scale
            self.scale(by: w / self.window.frame.width)
        }
        let opacity = NSMenuItem(title: L("Opacity"), action: nil, keyEquivalent: "")
        let sub = NSMenu()
        for p in [100, 75, 50, 25] {
            let item = ClosureMenuItem(title: "\(p)%") { [weak self] in self?.window.alphaValue = CGFloat(p) / 100 }
            item.state = abs(window.alphaValue * 100 - CGFloat(p)) < 1 ? .on : .off
            sub.addItem(item)
        }
        opacity.submenu = sub
        m.addItem(opacity)
        let through = ClosureMenuItem(title: L("Click Through")) { [weak self] in
            guard let self else { return }
            self.window.ignoresMouseEvents = true
            HUD.show(L("Click-through on. Close it from the menu bar."), symbol: "cursorarrow.motionlines")
        }
        m.addItem(through)
        m.addItem(.separator())
        add(L("Close")) { [weak self] in self?.close() }
        return m
    }

    private func edit() {
        (NSApp.delegate as? AppDelegate)?.openEditor(captured)
        close()
    }

    func windowWillClose(_ notification: Notification) {
        PinnedImage.open.removeAll { $0 === self }
    }

    func close() {
        let w = window
        NSAnimationContext.runAnimationGroup({ ctx in
            ctx.duration = 0.15
            w.animator().alphaValue = 0
        }) { w.orderOut(nil) }
        PinnedImage.open.removeAll { $0 === self }
    }

    static var count: Int { open.count }

    static func closeAll() { open.forEach { $0.close() } }
}

final class PinWindow: NSWindow {
    var menuProvider: (() -> NSMenu?)?
    var onScroll: ((CGFloat) -> Void)?
    var onMagnify: ((CGFloat) -> Void)?

    override var canBecomeKey: Bool { true }

    override func rightMouseDown(with event: NSEvent) {
        guard let menu = menuProvider?(), let view = contentView else { return }
        NSMenu.popUpContextMenu(menu, with: event, for: view)
    }

    override func scrollWheel(with event: NSEvent) {
        onScroll?(event.scrollingDeltaY * (event.hasPreciseScrollingDeltas ? 1 : 6))
    }

    override func magnify(with event: NSEvent) { onMagnify?(event.magnification) }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 { close() } else { super.keyDown(with: event) }
    }
}

final class ClosureMenuItem: NSMenuItem {
    private let run: () -> Void

    init(title: String, action: @escaping () -> Void) {
        run = action
        super.init(title: title, action: #selector(fire), keyEquivalent: "")
        target = self
    }

    required init(coder: NSCoder) { fatalError() }

    @objc private func fire() { run() }
}

private struct PinView: View {
    let image: CGImage
    let onClose: () -> Void
    let onEdit: () -> Void
    @State private var hover = false

    var body: some View {
        Image(decorative: image, scale: 1)
            .resizable()
            .interpolation(.high)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).strokeBorder(.white.opacity(hover ? 0.35 : 0.12), lineWidth: 1))
            .overlay(alignment: .topLeading) {
                if hover {
                    Button(action: onClose) {
                        Image(systemName: "xmark")
                            .font(.system(size: 9, weight: .bold))
                            .frame(width: 20, height: 20)
                            .kireiGlass(in: Circle())
                    }
                    .buttonStyle(.plain)
                    .padding(8)
                    .transition(.opacity)
                }
            }
            .onHover { h in withAnimation(.easeOut(duration: 0.15)) { hover = h } }
            .onTapGesture(count: 2, perform: onEdit)
    }
}
