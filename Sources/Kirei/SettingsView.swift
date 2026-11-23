import SwiftUI
import AppKit

/// Preferences window with toolbar-style tabs, like the system apps.
enum SettingsWindow {
    static func make() -> NSWindow {
        let tabs = NSTabViewController()
        tabs.tabStyle = .toolbar
        tabs.transitionOptions = [.crossfade, .allowUserInteraction]
        tabs.addTabViewItem(item(GeneralSettingsView(), L("General"), "gearshape"))
        tabs.addTabViewItem(item(ShortcutsSettingsView(), L("Shortcuts"), "command"))
        tabs.addTabViewItem(item(AboutView(), L("About"), "info.circle"))
        let w = NSWindow(contentViewController: tabs)
        w.styleMask = [.titled, .closable]
        w.toolbarStyle = .preference
        w.title = L("General")
        w.isReleasedWhenClosed = false
        w.setContentSize(NSSize(width: 540, height: 480))
        w.center()
        return w
    }

    private static func item<V: View>(_ view: V, _ title: String, _ symbol: String) -> NSTabViewItem {
        let host = NSHostingController(rootView: view.frame(width: 540, height: 480))
        host.title = title
        let item = NSTabViewItem(viewController: host)
        item.label = title
        item.image = NSImage(systemSymbolName: symbol, accessibilityDescription: title)
        return item
    }
}

struct GeneralSettingsView: View {
    @ObservedObject var settings = AppSettings.shared
    @State private var permission = CaptureService.hasScreenPermission
    @State private var language = AppLanguage.current
    private let launchLanguage = AppLanguage.current

    var body: some View {
        Form {
            Section(L("After Capture")) {
                Picker(L("Action"), selection: $settings.afterCapture) {
                    ForEach(AppSettings.AfterCapture.allCases) { Text($0.title).tag($0) }
                }
                Toggle(L("Hide sensitive data automatically"), isOn: $settings.autoRedact)
                Toggle(L("Play sound when copying"), isOn: $settings.playSound)
            }

            Section(L("Saving")) {
                HStack {
                    Text(L("Folder"))
                    Spacer()
                    Text(settings.saveFolder.path.replacingOccurrences(of: NSHomeDirectory(), with: "~"))
                        .foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.middle)
                    Button(L("Change…"), action: chooseFolder)
                }
                Picker(L("Format"), selection: $settings.format) {
                    Text("PNG").tag(AppSettings.Format.png)
                    Text("JPEG").tag(AppSettings.Format.jpeg)
                }
            }

            Section(L("System")) {
                Picker(L("Language"), selection: $language) {
                    ForEach(AppLanguage.allCases) { Text($0.title).tag($0) }
                }
                .onChange(of: language) { _, lang in AppLanguage.current = lang }
                if language != launchLanguage {
                    HStack {
                        Text(L("Restart Kirei to apply the new language."))
                            .foregroundStyle(.secondary)
                        Spacer()
                        Button(L("Restart Now")) { AppDelegate.relaunch() }
                    }
                }
                Toggle(L("Open at login"), isOn: Binding(
                    get: { settings.launchAtLogin },
                    set: { settings.launchAtLogin = $0 }
                ))
                HStack {
                    Text(L("Screen Recording"))
                    Spacer()
                    if permission {
                        Label(L("Allowed"), systemImage: "checkmark.circle.fill").foregroundStyle(.green)
                    } else {
                        Button(L("Grant Permission…")) {
                            (NSApp.delegate as? AppDelegate)?.showOnboarding(step: .permission)
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .onAppear { permission = CaptureService.hasScreenPermission }
    }

    private func chooseFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.directoryURL = settings.saveFolder
        if panel.runModal() == .OK, let url = panel.url { settings.saveFolder = url }
    }
}

struct ShortcutsSettingsView: View {
    var body: some View {
        Form {
            Section {
                ForEach(CaptureAction.allCases) { action in
                    HStack {
                        Label(action.title, systemImage: action.symbol)
                        Spacer()
                        HotKeyRecorder(action: action)
                    }
                }
            } header: {
                Text(L("Global Shortcuts"))
            } footer: {
                Text(L("Click a shortcut and press the new keys. Esc cancels."))
                    .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
    }
}

struct AboutView: View {
    private var version: String {
        let info = Bundle.main.infoDictionary
        let v = info?["CFBundleShortVersionString"] as? String ?? "–"
        let b = info?["CFBundleVersion"] as? String ?? "–"
        return String(format: L("Version %@ (%@)"), v, b)
    }

    var body: some View {
        VStack(spacing: 12) {
            Spacer()
            if let icon = NSApp.applicationIconImage {
                Image(nsImage: icon).resizable().frame(width: 112, height: 112)
                    .shadow(color: .black.opacity(0.18), radius: 14, y: 6)
            }
            Text("Kirei").font(.system(size: 30, weight: .bold))
            Text(version).foregroundStyle(.secondary).textSelection(.enabled)
            Text(L("Beautiful, share-ready screenshots in one shortcut."))
                .foregroundStyle(.secondary)
            HStack(spacing: 10) {
                Button(L("Show Welcome Tour")) { (NSApp.delegate as? AppDelegate)?.showOnboarding(step: .welcome) }
                Button(L("Diagnostics Folder")) { NSWorkspace.shared.open(Diagnostics.folder) }
            }
            .padding(.top, 8)
            Spacer()
            Text("© 2026 Shiddo").font(.caption).foregroundStyle(.tertiary).padding(.bottom, 18)
        }
        .frame(maxWidth: .infinity)
    }
}

struct HotKeyRecorder: View {
    let action: CaptureAction
    @ObservedObject var settings = AppSettings.shared
    @State private var recording = false
    @State private var monitor: Any?

    private var current: HotKey? {
        guard let hk = settings.hotKeys[action], hk.keyCode != 0xFFFF else { return nil }
        return hk
    }

    var body: some View {
        HStack(spacing: 4) {
            Button(action: toggle) {
                Text(recording ? L("Press the shortcut…") : (current?.display ?? L("None")))
                    .frame(minWidth: 120)
                    .foregroundStyle(recording ? Color.accentColor : .primary)
            }
            if current != nil && !recording {
                Button {
                    settings.hotKeys[action] = HotKey(keyCode: 0xFFFF, modifiers: 0, display: "")
                } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.secondary) }
                    .buttonStyle(.plain)
                    .help(L("Remove shortcut"))
            }
        }
        .onDisappear(perform: stop)
    }

    private func toggle() { recording ? stop() : start() }

    private func start() {
        recording = true
        HotKeyManager.shared.suspended = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { e in
            if e.keyCode == 53 { stop(); return nil } // esc cancels
            let mods = e.modifierFlags.intersection([.command, .option, .control, .shift])
            guard !mods.intersection([.command, .option, .control]).isEmpty else { NSSound.beep(); return nil }
            settings.hotKeys[action] = HotKey(keyCode: UInt32(e.keyCode),
                                              modifiers: HotKey.carbonModifiers(mods),
                                              display: HotKey.displayString(e))
            stop()
            return nil
        }
    }

    private func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        if recording {
            recording = false
            HotKeyManager.shared.suspended = false
        }
    }
}
