import AppKit
import SwiftUI

/// Post-capture thumbnail in the screen corner: drag it out, copy, save, or open the editor.
final class FloatingPreview {
    private static var current: FloatingPreview?

    private let panel: PreviewPanel
    private let captured: CapturedImage
    private let model: EditorModel
    private let onEdit: (CapturedImage) -> Void
    private var dismissWork: DispatchWorkItem?
    private var hovering = false

    static func show(_ captured: CapturedImage, onEdit: @escaping (CapturedImage) -> Void) {
        current?.close(animated: false)
        current = FloatingPreview(captured, onEdit: onEdit)
    }

    private init(_ captured: CapturedImage, onEdit: @escaping (CapturedImage) -> Void) {
        self.captured = captured
        self.onEdit = onEdit
        model = EditorModel(image: captured.image, pixelScale: captured.scale)
        let size = NSSize(width: 248, height: 176)
        panel = PreviewPanel(contentRect: NSRect(origin: .zero, size: size),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .statusBar
        panel.isMovableByWindowBackground = false
        panel.becomesKeyOnlyIfNeeded = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        let holder = PreviewImage()
        let reveal = holder
        holder.image = model.renderFinal()
        if AppSettings.shared.autoRedact {
            model.autoRedact { [weak model] _ in holder.image = model?.renderFinal() }
        }
        let view = FloatingPreviewView(
            holder: holder,
            onCopy: { [weak self] in self?.copy() },
            onSave: { [weak self] in self?.save() },
            onEdit: { [weak self] in self?.edit() },
            onClose: { [weak self] in self?.close(animated: true) },
            onHover: { [weak self] h in self?.hoverChanged(h) },
            dragFile: { [weak self] in
                guard let self, let img = self.model.renderFinal() else { return nil }
                return Exporter.temporaryFile(img, scale: captured.scale)
            }
        )
        panel.contentView = NSHostingView(rootView: view)

        let anchor = captured.screenRect.map { CGPoint(x: $0.midX, y: $0.midY) } ?? NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(anchor, $0.frame, false) } ?? NSScreen.main
        let f = screen?.visibleFrame ?? .zero
        let target = NSPoint(x: f.maxX - size.width - 20, y: f.minY + 20)
        panel.setFrameOrigin(target)
        panel.alphaValue = 0
        panel.orderFrontRegardless()
        let showPanel = { [panel] in
            panel.alphaValue = 1
            withAnimation(.spring(response: 0.42, dampingFraction: 0.68)) { reveal.shown = true }
        }
        if let from = captured.screenRect, from.width > 8, from.height > 8 {
            // The image glides from where it was captured into the thumbnail.
            let slot = CGRect(origin: target, size: size).insetBy(dx: 16, dy: 16)
            CaptureFeedback.fly(captured.image, from: from, to: slot, done: showPanel)
        } else {
            showPanel()
        }
        scheduleDismiss()
    }

    private func scheduleDismiss() {
        dismissWork?.cancel()
        guard !hovering else { return }
        let work = DispatchWorkItem { [weak self] in self?.close(animated: true) }
        dismissWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 7, execute: work)
    }

    private func hoverChanged(_ h: Bool) {
        hovering = h
        if h { dismissWork?.cancel() } else { scheduleDismiss() }
    }

    private func copy() {
        guard let img = model.renderFinal() else { return }
        Exporter.copy(img, scale: captured.scale)
        Sounds.playCopy()
        HUD.show(L("Image copied"))
        close(animated: true)
    }

    private func save() {
        guard let img = model.renderFinal() else { return }
        if let url = Exporter.saveToDefaultFolder(img, scale: captured.scale) {
            HUD.show(String(format: L("Saved to %@"), url.deletingLastPathComponent().lastPathComponent), symbol: "folder.fill")
        } else {
            HUD.show(L("Couldn't save"), symbol: "exclamationmark.triangle.fill")
        }
        close(animated: true)
    }

    private func edit() {
        close(animated: false)
        onEdit(captured)
    }

    private func close(animated: Bool) {
        dismissWork?.cancel()
        let p = panel
        if FloatingPreview.current === self { FloatingPreview.current = nil }
        guard animated else { p.orderOut(nil); return }
        NSAnimationContext.runAnimationGroup({ ctx in
            ctx.duration = 0.22
            p.animator().alphaValue = 0
            p.animator().setFrameOrigin(NSPoint(x: p.frame.minX + 40, y: p.frame.minY))
        }) { p.orderOut(nil) }
    }
}

/// Borderless panels refuse key status by default, which drops clicks and hover in a background app.
final class PreviewPanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

private final class PreviewImage: ObservableObject {
    @Published var image: CGImage?
    @Published var shown = false
}

private struct FloatingPreviewView: View {
    @ObservedObject var holder: PreviewImage
    let onCopy: () -> Void
    let onSave: () -> Void
    let onEdit: () -> Void
    let onClose: () -> Void
    let onHover: (Bool) -> Void
    let dragFile: () -> URL?
    @State private var hover = false

    var body: some View {
        ZStack {
            if let image = holder.image {
                Image(decorative: image, scale: 2)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .padding(8)
            }
            if hover {
                Color.black.opacity(0.35)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
                    .padding(4)
                VStack(spacing: 8) {
                    HStack(spacing: 8) {
                        action(L("Copy"), "doc.on.doc", onCopy)
                        action(L("Save"), "square.and.arrow.down", onSave)
                    }
                    action(L("Edit"), "slider.horizontal.3", onEdit)
                }
                .transition(.opacity)
                VStack {
                    HStack {
                        Button(action: onClose) {
                            Image(systemName: "xmark")
                                .font(.system(size: 9, weight: .bold))
                                .frame(width: 20, height: 20)
                                .kireiGlass(in: Circle())
                        }
                        .buttonStyle(.plain)
                        .help(L("Close"))
                        Spacer()
                    }
                    Spacer()
                }
                .padding(10)
            }
        }
        .frame(width: 240, height: 168)
        .kireiGlass(in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .scaleEffect(holder.shown ? 1 : 0.82, anchor: .bottomTrailing)
        .opacity(holder.shown ? 1 : 0)
        .padding(4)
        .contentShape(Rectangle())
        .onHover { h in
            withAnimation(.easeOut(duration: 0.15)) { hover = h }
            onHover(h)
        }
        .onTapGesture(count: 2, perform: onEdit)
        .onDrag {
            guard let url = dragFile() else { return NSItemProvider() }
            return NSItemProvider(contentsOf: url) ?? NSItemProvider()
        }
    }

    private func action(_ title: String, _ symbol: String, _ run: @escaping () -> Void) -> some View {
        Button(action: run) {
            Label(title, systemImage: symbol)
                .font(.callout.weight(.semibold))
                .foregroundStyle(.white)
                .padding(.horizontal, 12).padding(.vertical, 6)
                .background(.white.opacity(0.2), in: Capsule())
                .overlay(Capsule().strokeBorder(.white.opacity(0.3), lineWidth: 0.5))
        }
        .buttonStyle(.plain)
    }
}
