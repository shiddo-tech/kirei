import AppKit
import Carbon.HIToolbox
import ServiceManagement
import UniformTypeIdentifiers

struct Preset: Codable, Identifiable, Hashable {
    var id = UUID()
    var name: String
    var style: Style
}

final class PresetStore: ObservableObject {
    static let shared = PresetStore()
    private let defaults = UserDefaults.standard

    @Published var presets: [Preset] { didSet { save(presets, "presets") } }

    var lastStyle: Style {
        get { load("lastStyle") ?? .default }
        set { save(newValue, "lastStyle") }
    }

    private init() {
        presets = []
        presets = load("presets") ?? []
    }

    func add(name: String, style: Style) { presets.append(Preset(name: name, style: style)) }
    func remove(_ p: Preset) { presets.removeAll { $0.id == p.id } }

    func update(_ p: Preset, style: Style) {
        if let i = presets.firstIndex(where: { $0.id == p.id }) { presets[i].style = style }
    }

    private func save<T: Encodable>(_ v: T, _ key: String) {
        if let d = try? JSONEncoder().encode(v) { defaults.set(d, forKey: key) }
    }

    private func load<T: Decodable>(_ key: String) -> T? {
        defaults.data(forKey: key).flatMap { try? JSONDecoder().decode(T.self, from: $0) }
    }
}

// MARK: - Hotkeys

enum CaptureAction: String, CaseIterable, Codable, Identifiable {
    case area, window, fullscreen, text, clipboard
    var id: String { rawValue }

    var title: String {
        switch self {
        case .area: return L("Capture Area")
        case .window: return L("Capture Window")
        case .fullscreen: return L("Capture Full Screen")
        case .text: return L("Capture Text (OCR)")
        case .clipboard: return L("Edit Image from Clipboard")
        }
    }

    var symbol: String {
        switch self {
        case .area: return "rectangle.dashed"
        case .window: return "macwindow"
        case .fullscreen: return "display"
        case .text: return "text.viewfinder"
        case .clipboard: return "doc.on.clipboard"
        }
    }
}

struct HotKey: Codable, Equatable {
    var keyCode: UInt32
    var modifiers: UInt32   // Carbon modifier mask
    var display: String

    static let defaults: [CaptureAction: HotKey] = [
        .area: HotKey(keyCode: UInt32(kVK_ANSI_1), modifiers: UInt32(optionKey | shiftKey), display: "⌥⇧1"),
        .window: HotKey(keyCode: UInt32(kVK_ANSI_2), modifiers: UInt32(optionKey | shiftKey), display: "⌥⇧2"),
        .fullscreen: HotKey(keyCode: UInt32(kVK_ANSI_3), modifiers: UInt32(optionKey | shiftKey), display: "⌥⇧3"),
        .text: HotKey(keyCode: UInt32(kVK_ANSI_4), modifiers: UInt32(optionKey | shiftKey), display: "⌥⇧4"),
        .clipboard: HotKey(keyCode: UInt32(kVK_ANSI_5), modifiers: UInt32(optionKey | shiftKey), display: "⌥⇧5"),
    ]

    static func carbonModifiers(_ f: NSEvent.ModifierFlags) -> UInt32 {
        var m: UInt32 = 0
        if f.contains(.command) { m |= UInt32(cmdKey) }
        if f.contains(.option) { m |= UInt32(optionKey) }
        if f.contains(.control) { m |= UInt32(controlKey) }
        if f.contains(.shift) { m |= UInt32(shiftKey) }
        return m
    }

    static func displayString(_ event: NSEvent) -> String {
        let f = event.modifierFlags
        var s = ""
        if f.contains(.control) { s += "⌃" }
        if f.contains(.option) { s += "⌥" }
        if f.contains(.shift) { s += "⇧" }
        if f.contains(.command) { s += "⌘" }
        let special: [Int: String] = [
            kVK_Space: L("Space"), kVK_Return: "↩", kVK_Tab: "⇥", kVK_Delete: "⌫", kVK_Escape: "⎋",
            kVK_F1: "F1", kVK_F2: "F2", kVK_F3: "F3", kVK_F4: "F4", kVK_F5: "F5", kVK_F6: "F6",
            kVK_F7: "F7", kVK_F8: "F8", kVK_F9: "F9", kVK_F10: "F10", kVK_F11: "F11", kVK_F12: "F12",
            kVK_LeftArrow: "←", kVK_RightArrow: "→", kVK_UpArrow: "↑", kVK_DownArrow: "↓",
        ]
        if let sp = special[Int(event.keyCode)] { return s + sp }
        return s + (event.charactersIgnoringModifiers?.uppercased() ?? "?")
    }
}

final class AppSettings: ObservableObject {
    static let shared = AppSettings()
    private let d = UserDefaults.standard

    enum AfterCapture: String, CaseIterable, Identifiable {
        case preview, openEditor, copyStyled
        var id: String { rawValue }
        var title: String {
            switch self {
            case .preview: return L("Show floating preview")
            case .openEditor: return L("Open the editor")
            case .copyStyled: return L("Copy styled image directly")
            }
        }
    }

    enum Format: String, CaseIterable, Identifiable {
        case png, jpeg, heic
        var id: String { rawValue }
        var title: String { rawValue.uppercased() }
        var ext: String { self == .jpeg ? "jpg" : rawValue }
        var type: UTType {
            switch self {
            case .png: return .png
            case .jpeg: return .jpeg
            case .heic: return .heic
            }
        }
        var isLossy: Bool { self != .png }
    }

    @Published var hotKeys: [CaptureAction: HotKey] {
        didSet {
            if let data = try? JSONEncoder().encode(Dictionary(uniqueKeysWithValues: hotKeys.map { ($0.key.rawValue, $0.value) })) {
                d.set(data, forKey: "hotKeys")
            }
            HotKeyManager.shared.registerAll()
        }
    }
    @Published var afterCapture: AfterCapture { didSet { d.set(afterCapture.rawValue, forKey: "afterCapture") } }
    @Published var autoRedact: Bool { didSet { d.set(autoRedact, forKey: "autoRedact") } }
    @Published var format: Format { didSet { d.set(format.rawValue, forKey: "format") } }
    @Published var saveFolder: URL { didSet { d.set(saveFolder.path, forKey: "saveFolder") } }
    @Published var playSound: Bool { didSet { d.set(playSound, forKey: "playSound") } }
    /// false exports at 1 pixel per point (half size on Retina).
    @Published var exportRetina: Bool { didSet { d.set(exportRetina, forKey: "exportRetina") } }
    /// Seconds to wait before a capture; 0 = immediately.
    @Published var captureDelay: Int { didSet { d.set(captureDelay, forKey: "captureDelay") } }
    @Published var hideDesktopIcons: Bool { didSet { d.set(hideDesktopIcons, forKey: "hideDesktopIcons") } }
    @Published var keepHistory: Bool { didSet { d.set(keepHistory, forKey: "keepHistory") } }

    var launchAtLogin: Bool {
        get { SMAppService.mainApp.status == .enabled }
        set {
            objectWillChange.send()
            do {
                if newValue { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
            } catch {
                NSLog("Kirei launch at login: \(error)")
            }
        }
    }

    private init() {
        var keys = HotKey.defaults
        if let data = d.data(forKey: "hotKeys"),
           let saved = try? JSONDecoder().decode([String: HotKey].self, from: data) {
            for (k, v) in saved { if let a = CaptureAction(rawValue: k) { keys[a] = v } }
            // Keys explicitly cleared are stored as keyCode 0xFFFF.
        }
        hotKeys = keys
        afterCapture = AfterCapture(rawValue: d.string(forKey: "afterCapture") ?? "") ?? .preview
        autoRedact = d.bool(forKey: "autoRedact")
        format = Format(rawValue: d.string(forKey: "format") ?? "") ?? .png
        playSound = d.object(forKey: "playSound") as? Bool ?? true
        exportRetina = d.object(forKey: "exportRetina") as? Bool ?? true
        captureDelay = d.integer(forKey: "captureDelay")
        hideDesktopIcons = d.bool(forKey: "hideDesktopIcons")
        keepHistory = d.object(forKey: "keepHistory") as? Bool ?? true
        let desktop = FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask)[0]
        saveFolder = d.string(forKey: "saveFolder").map { URL(fileURLWithPath: $0) } ?? desktop
    }
}

// MARK: - Carbon global hotkeys

final class HotKeyManager {
    static let shared = HotKeyManager()
    var handler: ((CaptureAction) -> Void)?
    private var refs: [EventHotKeyRef] = []
    private var ids: [UInt32: CaptureAction] = [:]
    private var installed = false
    var suspended = false { didSet { registerAll() } }

    func registerAll() {
        for r in refs { UnregisterEventHotKey(r) }
        refs.removeAll(); ids.removeAll()
        installHandlerIfNeeded()
        guard !suspended else { return }
        for (i, action) in CaptureAction.allCases.enumerated() {
            guard let hk = AppSettings.shared.hotKeys[action], hk.keyCode != 0xFFFF else { continue }
            let id = UInt32(i + 1)
            var ref: EventHotKeyRef?
            let hkID = EventHotKeyID(signature: OSType(0x4B49_5245), id: id) // 'KIRE'
            if RegisterEventHotKey(hk.keyCode, hk.modifiers, hkID, GetApplicationEventTarget(), 0, &ref) == noErr,
               let ref {
                refs.append(ref)
                ids[id] = action
            }
        }
    }

    private func installHandlerIfNeeded() {
        guard !installed else { return }
        installed = true
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, event, _ -> OSStatus in
            var hkID = EventHotKeyID()
            GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID),
                              nil, MemoryLayout<EventHotKeyID>.size, nil, &hkID)
            let mgr = HotKeyManager.shared
            if let action = mgr.ids[hkID.id] {
                DispatchQueue.main.async { mgr.handler?(action) }
            }
            return noErr
        }, 1, &spec, nil, nil)
    }
}
