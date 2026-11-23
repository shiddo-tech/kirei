import AppKit
import SwiftUI
import UniformTypeIdentifiers

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private var statusItem: NSStatusItem!
    private var editors: [EditorWindowController] = []
    private var settingsWindow: NSWindow?
    private var onboarding: OnboardingWindowController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        buildMainMenu()
        buildStatusItem()
        HotKeyManager.shared.handler = { [weak self] action in self?.perform(action) }
        HotKeyManager.shared.registerAll()
        Diagnostics.start()

        if !UserDefaults.standard.bool(forKey: "didOnboard") {
            UserDefaults.standard.set(true, forKey: "didOnboard")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.showOnboarding(step: .welcome) }
        }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag { openSettings() }
        return true
    }

    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls { if let img = CaptureService.load(url: url) { openEditor(img) } }
    }

    // MARK: Status item

    private func buildStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        statusItem.button?.image = KireiLogo.menuBarImage()
        let menu = NSMenu()
        menu.delegate = self
        statusItem.menu = menu
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        guard menu === statusItem.menu else { return }
        menu.removeAllItems()
        for action in CaptureAction.allCases {
            let item = NSMenuItem(title: action.title, action: #selector(menuCapture(_:)), keyEquivalent: "")
            item.target = self
            item.representedObject = action.rawValue
            item.image = NSImage(systemSymbolName: action.symbol, accessibilityDescription: nil)
            if let hk = AppSettings.shared.hotKeys[action], hk.keyCode != 0xFFFF {
                item.title = action.title
                let attr = NSMutableAttributedString(string: action.title + "   ")
                attr.append(NSAttributedString(string: hk.display, attributes: [
                    .foregroundColor: NSColor.secondaryLabelColor,
                    .font: NSFont.menuFont(ofSize: 0),
                ]))
                item.attributedTitle = attr
            }
            menu.addItem(item)
            if action == .text { menu.addItem(.separator()) }
        }
        menu.addItem(withTitle: L("Open Image…"), action: #selector(openImage(_:)), keyEquivalent: "").target = self
        menu.addItem(withTitle: L("Capture History…"), action: #selector(openHistory(_:)), keyEquivalent: "").target = self
        if PinnedImage.count > 0 {
            menu.addItem(withTitle: L("Close Pinned Images"), action: #selector(closePins(_:)), keyEquivalent: "").target = self
        }
        menu.addItem(.separator())
        menu.addItem(withTitle: L("Settings…"), action: #selector(openSettingsAction(_:)), keyEquivalent: ",").target = self
        menu.addItem(withTitle: L("Quit Kirei"), action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
    }

    @objc private func menuCapture(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let action = CaptureAction(rawValue: raw) else { return }
        // Let the menu close before the capture UI appears.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { self.perform(action) }
    }

    // MARK: Capture flow

    func perform(_ action: CaptureAction, timed: Bool = false) {
        if action != .clipboard, !CaptureService.hasScreenPermission, !requestPermission(retry: action) {
            return
        }
        let delay = AppSettings.shared.captureDelay
        if delay > 0, [.area, .window, .fullscreen].contains(action), !timed {
            Countdown.start(seconds: delay) { [weak self] in self?.perform(action, timed: true) }
            return
        }
        CaptureService.capture(action) { [weak self] result in
            guard let self else { return }
            let captured: CapturedImage
            switch result {
            case .success(let c)?:
                captured = c
            case .failure(.permission)?:
                // Allowed in Settings but refused by the system: the grant needs a relaunch.
                if CaptureService.hasScreenPermission {
                    _ = self.askToRestartForPermission()
                } else {
                    _ = self.requestPermission(retry: action)
                }
                return
            case .failure(let f)?:
                NSLog("Kirei capture error: \(f)")
                HUD.show(L("Couldn't capture the screen"), symbol: "exclamationmark.triangle.fill")
                return
            case nil:
                if action == .clipboard { HUD.show(L("No image on the clipboard"), symbol: "doc.on.clipboard") }
                return
            }
            self.handle(captured, from: action)
        }
    }

    private func handle(_ captured: CapturedImage, from action: CaptureAction) {
        if action != .clipboard {
            if action != .text { CaptureHistory.shared.add(captured) }
            Sounds.playCapture()
            if let r = captured.screenRect { CaptureFeedback.flash(r) }
        }
        if action == .text {
            ocr(captured)
        } else if AppSettings.shared.afterCapture == .copyStyled && action != .clipboard {
            quickCopy(captured)
        } else if AppSettings.shared.afterCapture == .preview && action != .clipboard {
            FloatingPreview.show(captured) { [weak self] c in self?.openEditor(c) }
        } else {
            openEditor(captured)
        }
    }

    private func ocr(_ captured: CapturedImage) {
        HUD.show(L("Recognizing text…"), symbol: "text.viewfinder")
        DispatchQueue.global(qos: .userInitiated).async {
            let text = ImageAnalysis.text(in: captured.image)
            DispatchQueue.main.async {
                if text.isEmpty {
                    HUD.show(L("No text found"), symbol: "text.viewfinder")
                } else {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                    HUD.show(L("Text copied"), symbol: "text.viewfinder")
                }
            }
        }
    }

    private func quickCopy(_ captured: CapturedImage) {
        let model = EditorModel(image: captured.image, pixelScale: captured.scale)
        let finish = {
            if let img = model.renderFinal() {
                Exporter.copy(img, scale: captured.scale)
                Sounds.playCopy()
                HUD.show(L("Styled image copied"))
            }
        }
        if AppSettings.shared.autoRedact { model.autoRedact { _ in finish() } } else { finish() }
    }

    func openEditor(_ captured: CapturedImage) {
        let c = EditorWindowController(capture: captured)
        c.onClose = { [weak self] ctrl in
            self?.editors.removeAll { $0 === ctrl }
            self?.updateActivationPolicy()
        }
        editors.append(c)
        NSApp.setActivationPolicy(.regular)
        c.showWindow(nil)
        NSApp.activate(ignoringOtherApps: true)
        c.window?.makeKeyAndOrderFront(nil)
    }

    func updateActivationPolicy() {
        let hasWindows = !editors.isEmpty || (settingsWindow?.isVisible ?? false) || onboarding != nil
            || HistoryWindow.isVisible
        if !hasWindows { NSApp.setActivationPolicy(.accessory) }
    }

    /// Shows the permission step. Returns false: the capture continues only via "Try Anyway",
    /// since macOS may report a new grant only after the app restarts.
    private func requestPermission(retry: CaptureAction? = nil) -> Bool {
        CGRequestScreenCaptureAccess()
        showOnboarding(step: .permission, tryAnyway: retry.map { a in { [weak self] in self?.forceCapture(a) } })
        return false
    }

    private func forceCapture(_ action: CaptureAction) {
        CaptureService.capture(action) { [weak self] result in
            if case .success(let c)? = result { self?.handle(c, from: action) }
            else if case .failure? = result { HUD.show(L("Couldn't capture the screen"), symbol: "exclamationmark.triangle.fill") }
        }
    }

    /// macOS may apply a Screen Recording grant only after the app relaunches.
    /// Returns true when the user chose to continue without restarting.
    private func askToRestartForPermission() -> Bool {
        NSApp.activate(ignoringOtherApps: true)
        let alert = NSAlert()
        alert.messageText = L("Reopen Kirei to finish enabling capture")
        alert.informativeText = L("macOS applies the Screen Recording permission only after the app reopens. Until then, captures show just the desktop wallpaper.")
        alert.addButton(withTitle: L("Restart Now"))
        alert.addButton(withTitle: L("Try Anyway"))
        if alert.runModal() == .alertFirstButtonReturn {
            AppDelegate.relaunch()
            return false
        }
        return true
    }

    static func relaunch() {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/bin/sh")
        task.arguments = ["-c", "sleep 0.6; /usr/bin/open \"$0\"", Bundle.main.bundlePath]
        try? task.run()
        NSApp.terminate(nil)
    }

    func showOnboarding(step: OnboardingWindowController.Step, tryAnyway: (() -> Void)? = nil) {
        if let onboarding {
            onboarding.go(to: step, tryAnyway: tryAnyway)
        } else {
            let c = OnboardingWindowController(step: step, tryAnyway: tryAnyway)
            c.onClose = { [weak self] in
                self?.onboarding = nil
                DispatchQueue.main.async { self?.updateActivationPolicy() }
            }
            onboarding = c
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        onboarding?.showWindow(nil)
        onboarding?.window?.makeKeyAndOrderFront(nil)
    }

    // MARK: Menu actions

    @objc func openImage(_ sender: Any?) {
        NSApp.activate(ignoringOtherApps: true)
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.image]
        panel.allowsMultipleSelection = true
        guard panel.runModal() == .OK else { return }
        for url in panel.urls { if let img = CaptureService.load(url: url) { openEditor(img) } }
    }

    @objc func pasteImage(_ sender: Any?) { perform(.clipboard) }

    @objc func openHistory(_ sender: Any?) { HistoryWindow.show() }

    @objc private func closePins(_ sender: Any?) { PinnedImage.closeAll() }

    @objc func openSettingsAction(_ sender: Any?) { openSettings() }

    func openSettings() {
        if settingsWindow == nil {
            let w = SettingsWindow.make()
            NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: w, queue: .main) { [weak self] _ in
                DispatchQueue.main.async { self?.updateActivationPolicy() }
            }
            settingsWindow = w
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        settingsWindow?.makeKeyAndOrderFront(nil)
    }

    @objc private func captureFromMenu(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let a = CaptureAction(rawValue: raw) else { return }
        perform(a)
    }

    private func buildMainMenu() {
        let main = NSMenu()

        let appItem = NSMenuItem()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: L("About Kirei"), action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: L("Settings…"), action: #selector(openSettingsAction(_:)), keyEquivalent: ",").target = self
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: L("Hide Kirei"), action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(withTitle: L("Quit Kirei"), action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appItem.submenu = appMenu
        main.addItem(appItem)

        let fileItem = NSMenuItem()
        let file = NSMenu(title: L("File"))
        for action in CaptureAction.allCases where action != .clipboard {
            let i = file.addItem(withTitle: action.title, action: #selector(captureFromMenu(_:)), keyEquivalent: "")
            i.target = self
            i.representedObject = action.rawValue
        }
        file.addItem(.separator())
        file.addItem(withTitle: L("Open Image…"), action: #selector(openImage(_:)), keyEquivalent: "o").target = self
        let paste = file.addItem(withTitle: L("New from Clipboard"), action: #selector(pasteImage(_:)), keyEquivalent: "n")
        paste.target = self
        file.addItem(withTitle: L("Capture History…"), action: #selector(openHistory(_:)), keyEquivalent: "y").target = self
        file.addItem(withTitle: L("Pin to Screen"), action: #selector(EditorWindowController.kireiPin(_:)), keyEquivalent: "p")
        file.addItem(.separator())
        file.addItem(withTitle: L("Save"), action: #selector(EditorWindowController.kireiSave(_:)), keyEquivalent: "s")
        file.addItem(withTitle: L("Save As…"), action: #selector(EditorWindowController.kireiSaveAs(_:)), keyEquivalent: "S")
        file.addItem(.separator())
        file.addItem(withTitle: L("Close"), action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        fileItem.submenu = file
        main.addItem(fileItem)

        let editItem = NSMenuItem()
        let edit = NSMenu(title: L("Edit"))
        edit.addItem(withTitle: L("Undo"), action: Selector(("undo:")), keyEquivalent: "z")
        edit.addItem(withTitle: L("Redo"), action: Selector(("redo:")), keyEquivalent: "Z")
        edit.addItem(.separator())
        edit.addItem(withTitle: L("Cut"), action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: L("Copy"), action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: L("Paste"), action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: L("Select All"), action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        edit.addItem(.separator())
        edit.addItem(withTitle: L("Duplicate"), action: #selector(EditorWindowController.kireiDuplicate(_:)), keyEquivalent: "d")
        edit.addItem(withTitle: L("Bring to Front"), action: #selector(EditorWindowController.kireiBringToFront(_:)), keyEquivalent: "]")
        edit.addItem(withTitle: L("Send to Back"), action: #selector(EditorWindowController.kireiSendToBack(_:)), keyEquivalent: "[")
        edit.addItem(.separator())
        edit.addItem(withTitle: L("Copy Text (OCR)"), action: #selector(EditorWindowController.kireiCopyText(_:)), keyEquivalent: "t")
            .keyEquivalentModifierMask = [.command, .shift]
        edit.addItem(withTitle: L("Hide Sensitive Data"), action: #selector(EditorWindowController.kireiRedact(_:)), keyEquivalent: "r")
            .keyEquivalentModifierMask = [.command, .shift]
        editItem.submenu = edit
        main.addItem(editItem)

        let viewItem = NSMenuItem()
        let viewMenu = NSMenu(title: L("View"))
        viewMenu.addItem(withTitle: L("Zoom In"), action: #selector(EditorWindowController.kireiZoomIn(_:)), keyEquivalent: "+")
        viewMenu.addItem(withTitle: L("Zoom In"), action: #selector(EditorWindowController.kireiZoomIn(_:)), keyEquivalent: "=")
            .isHidden = true // ⌘= also zooms in, like other Mac apps
        viewMenu.addItem(withTitle: L("Zoom Out"), action: #selector(EditorWindowController.kireiZoomOut(_:)), keyEquivalent: "-")
        viewMenu.addItem(withTitle: L("Actual Size"), action: #selector(EditorWindowController.kireiActualSize(_:)), keyEquivalent: "0")
        viewMenu.addItem(withTitle: L("Fit in Window"), action: #selector(EditorWindowController.kireiZoomToFit(_:)), keyEquivalent: "9")
        viewItem.submenu = viewMenu
        main.addItem(viewItem)

        let windowItem = NSMenuItem()
        let windowMenu = NSMenu(title: L("Window"))
        windowMenu.addItem(withTitle: L("Minimize"), action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        windowItem.submenu = windowMenu
        main.addItem(windowItem)
        NSApp.windowsMenu = windowMenu

        NSApp.mainMenu = main
    }
}
