import AppKit
import SwiftUI
import UniformTypeIdentifiers

final class EditorWindowController: NSWindowController, NSWindowDelegate {
    let model: EditorModel
    private let undo = UndoManager()
    private var keyMonitor: Any?
    var onClose: ((EditorWindowController) -> Void)?

    init(capture: CapturedImage) {
        model = EditorModel(image: capture.image, pixelScale: capture.scale)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1280, height: 820),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                              backing: .buffered, defer: false)
        window.minSize = NSSize(width: 980, height: 620)
        window.isReleasedWhenClosed = false
        window.title = "Kirei"
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.titlebarSeparatorStyle = .none
        // An empty compact toolbar sets the title bar height; the top row matches it exactly so the
        // traffic lights sit centered in it and no content slides under the title bar.
        window.toolbar = NSToolbar(identifier: "KireiEditor")
        window.toolbarStyle = .unifiedCompact
        window.tabbingMode = .disallowed
        super.init(window: window)
        window.delegate = self
        model.undoManager = undo

        let actions = EditorActions(
            copy: { [weak self] in self?.copyImage() },
            save: { [weak self] in self?.saveQuick() },
            saveAs: { [weak self] in self?.saveAs() },
            share: { [weak self] v in self?.share(from: v) },
            copyText: { [weak self] in self?.copyText() },
            autoRedact: { [weak self] in self?.autoRedact() },
            pin: { [weak self] in self?.pin() },
            dragFile: { [weak self] in self?.tempFile() }
        )
        window.setContentSize(NSSize(width: 1280, height: 820))
        let barHeight = max(38, window.frame.height - window.contentLayoutRect.height)
        let host = NSHostingController(rootView: EditorView(model: model, actions: actions, barHeight: barHeight))
        host.sizingOptions = []
        // The layout reserves the title bar row itself; with the default safe area, the inspector's
        // scroll view extends under that row and its edge-effect view swallows the header's clicks.
        host.safeAreaRegions = []
        window.contentViewController = host
        window.setContentSize(NSSize(width: 1280, height: 820))
        window.center()
        installKeyMonitor()

        if AppSettings.shared.autoRedact { model.autoRedact() }
    }

    required init?(coder: NSCoder) { fatalError() }

    // MARK: Keyboard

    private func installKeyMonitor() {
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] e in
            guard let self, e.window === self.window else { return e }
            // Leave typing alone.
            if self.window?.firstResponder is NSText { return e }
            // Handled here instead of the Edit menu, whose copy: can be taken by focused SwiftUI controls.
            if e.modifierFlags.intersection([.command, .option, .control, .shift]) == .command,
               e.charactersIgnoringModifiers?.lowercased() == "c" {
                self.copyImage(); return nil
            }
            let mods = e.modifierFlags.intersection([.command, .option, .control])
            guard mods.isEmpty else { return e }
            switch Int(e.keyCode) {
            case 51, 117: // delete, forward delete
                self.model.deleteSelected(); return nil
            case 53: // escape
                if !self.model.selection.isEmpty { self.model.selection = [] } else { self.model.tool = .select }
                return nil
            case 123, 124, 125, 126: // arrows nudge the selection
                guard !self.model.selection.isEmpty else { return e }
                let step: CGFloat = e.modifierFlags.contains(.shift) ? 10 : 1
                let d: CGSize = [123: CGSize(width: -step, height: 0), 124: CGSize(width: step, height: 0),
                                 125: CGSize(width: 0, height: step), 126: CGSize(width: 0, height: -step)][Int(e.keyCode)]!
                self.model.nudgeSelection(by: CGSize(width: d.width * self.model.pixelScale,
                                                     height: d.height * self.model.pixelScale))
                return nil
            default:
                if let c = e.charactersIgnoringModifiers?.lowercased().first,
                   let t = Tool.allCases.first(where: { $0.shortcut == c }) {
                    self.model.tool = t
                    return nil
                }
                return e
            }
        }
    }

    // MARK: Responder-chain actions (from the main menu)

    @objc func copy(_ sender: Any?) { copyImage() }
    @objc func delete(_ sender: Any?) { model.deleteSelected() }
    override func selectAll(_ sender: Any?) { model.tool = .select; model.selectAll() }
    @objc func kireiDuplicate(_ sender: Any?) { model.duplicate(model.selection) }
    @objc func kireiBringToFront(_ sender: Any?) { model.bringToFront(model.selection) }
    @objc func kireiSendToBack(_ sender: Any?) { model.sendToBack(model.selection) }
    @objc func kireiSave(_ sender: Any?) { saveQuick() }
    @objc func kireiSaveAs(_ sender: Any?) { saveAs() }
    @objc func kireiCopyText(_ sender: Any?) { copyText() }
    @objc func kireiRedact(_ sender: Any?) { autoRedact() }
    @objc func kireiPin(_ sender: Any?) { pin() }
    @objc func kireiZoomIn(_ sender: Any?) { model.zoomIn() }
    @objc func kireiZoomOut(_ sender: Any?) { model.zoomOut() }
    @objc func kireiActualSize(_ sender: Any?) { model.zoomActualSize() }
    @objc func kireiZoomToFit(_ sender: Any?) { model.zoomToFit() }

    func windowWillReturnUndoManager(_ window: NSWindow) -> UndoManager? { undo }

    func windowWillClose(_ notification: Notification) {
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }
        keyMonitor = nil
        onClose?(self)
    }

    // MARK: Export

    private func finalImage() -> CGImage? {
        model.editingTextID = nil
        return model.renderFinal()
    }

    func pin() {
        guard let img = finalImage() else { return }
        PinnedImage.pin(CapturedImage(image: img, scale: model.pixelScale))
    }

    func copyImage() {
        guard let img = finalImage() else { return }
        Exporter.copy(img, scale: model.pixelScale)
        HUD.show(L("Image copied"))
        Sounds.playCopy()
    }

    func saveQuick() {
        guard let img = finalImage() else { return }
        if let url = Exporter.saveToDefaultFolder(img, scale: model.pixelScale) {
            HUD.show(String(format: L("Saved to %@"), url.deletingLastPathComponent().lastPathComponent), symbol: "folder.fill")
        } else {
            HUD.show(L("Couldn't save"), symbol: "exclamationmark.triangle.fill")
        }
    }

    func saveAs() {
        guard let window, let img = finalImage() else { return }
        let panel = NSSavePanel()
        let settings = AppSettings.shared
        panel.nameFieldStringValue = Exporter.fileName(ext: Exporter.ext(settings.format))
        panel.directoryURL = settings.saveFolder
        panel.allowedContentTypes = AppSettings.Format.allCases.map(\.type)
        panel.isExtensionHidden = false

        let picker = NSPopUpButton(frame: .zero, pullsDown: false)
        picker.addItems(withTitles: AppSettings.Format.allCases.map(\.title))
        picker.selectItem(at: AppSettings.Format.allCases.firstIndex(of: settings.format) ?? 0)
        let label = NSTextField(labelWithString: L("Format:"))
        let stack = NSStackView(views: [label, picker])
        stack.edgeInsets = NSEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        panel.accessoryView = stack

        panel.beginSheetModal(for: window) { [weak self] resp in
            guard resp == .OK, var url = panel.url, let self else { return }
            let format = AppSettings.Format.allCases[max(0, picker.indexOfSelectedItem)]
            url = url.deletingPathExtension().appendingPathExtension(Exporter.ext(format))
            if Exporter.save(img, scale: self.model.pixelScale, to: url, format: format) {
                HUD.show(L("Image saved"), symbol: "folder.fill")
            }
        }
    }

    func tempFile() -> URL? {
        guard let img = finalImage() else { return nil }
        return Exporter.temporaryFile(img, scale: model.pixelScale)
    }

    func share(from view: NSView) {
        guard let url = tempFile() else { return }
        NSSharingServicePicker(items: [url]).show(relativeTo: view.bounds, of: view, preferredEdge: .minY)
    }

    func copyText() {
        model.copyText { text in
            guard !text.isEmpty else {
                HUD.show(L("No text found"), symbol: "text.viewfinder")
                return
            }
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            let lines = text.split(separator: "\n").count
            HUD.show(lines == 1 ? L("Text copied (1 line)") : String(format: L("Text copied (%d lines)"), lines),
                     symbol: "text.viewfinder")
        }
    }

    func autoRedact() {
        model.autoRedact { n in
            HUD.show(n == 0 ? L("No sensitive data found") : n == 1 ? L("1 item redacted") : String(format: L("%d items redacted"), n),
                     symbol: "eye.slash.fill")
        }
    }
}
