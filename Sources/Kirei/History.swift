import AppKit
import SwiftUI
import ImageIO
import UniformTypeIdentifiers

struct HistoryItem: Codable, Identifiable, Hashable {
    var id = UUID()
    var date: Date
    var scale: CGFloat
    var width: Int
    var height: Int
}

/// Recent captures, kept as originals so they can be edited again.
/// Lives in Application Support; holds up to 60 captures from the last 14 days.
final class CaptureHistory: ObservableObject {
    static let shared = CaptureHistory()
    private static let limit = 60
    private static let maxAge: TimeInterval = 14 * 24 * 3600

    @Published private(set) var items: [HistoryItem] = []
    private let folder: URL
    private let queue = DispatchQueue(label: "kirei.history", qos: .utility)
    private var thumbs: [UUID: CGImage] = [:]

    private init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        folder = base.appendingPathComponent("Kirei/History", isDirectory: true)
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        if let d = try? Data(contentsOf: indexURL), let list = try? JSONDecoder().decode([HistoryItem].self, from: d) {
            items = list
        }
        prune()
    }

    private var indexURL: URL { folder.appendingPathComponent("index.json") }
    func fileURL(_ item: HistoryItem) -> URL { folder.appendingPathComponent("\(item.id.uuidString).png") }
    private func thumbURL(_ item: HistoryItem) -> URL { folder.appendingPathComponent("\(item.id.uuidString)-thumb.jpg") }

    func add(_ captured: CapturedImage) {
        guard AppSettings.shared.keepHistory else { return }
        let item = HistoryItem(date: Date(), scale: captured.scale, width: captured.image.width, height: captured.image.height)
        let img = captured.image
        queue.async {
            guard Self.write(img, to: self.fileURL(item), type: .png) else { return }
            if let t = Self.thumbnail(img, maxSide: 480) {
                _ = Self.write(t, to: self.thumbURL(item), type: .jpeg)
            }
            DispatchQueue.main.async {
                self.items.insert(item, at: 0)
                self.prune()
                self.saveIndex()
            }
        }
    }

    func load(_ item: HistoryItem) -> CapturedImage? {
        guard let src = CGImageSourceCreateWithURL(fileURL(item) as CFURL, nil),
              let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { return nil }
        return CapturedImage(image: img, scale: item.scale)
    }

    func thumbnail(_ item: HistoryItem) -> CGImage? {
        if let t = thumbs[item.id] { return t }
        guard let src = CGImageSourceCreateWithURL(thumbURL(item) as CFURL, nil),
              let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { return nil }
        thumbs[item.id] = img
        return img
    }

    func remove(_ item: HistoryItem) {
        items.removeAll { $0.id == item.id }
        deleteFiles(item)
        saveIndex()
    }

    func clear() {
        items.forEach(deleteFiles)
        items = []
        saveIndex()
    }

    private func prune() {
        let cutoff = Date().addingTimeInterval(-Self.maxAge)
        let keep = items.enumerated().filter { $0.offset < Self.limit && $0.element.date > cutoff }.map(\.element)
        let dropped = items.filter { i in !keep.contains { $0.id == i.id } }
        guard !dropped.isEmpty else { return }
        items = keep
        dropped.forEach(deleteFiles)
        saveIndex()
    }

    private func deleteFiles(_ item: HistoryItem) {
        thumbs[item.id] = nil
        try? FileManager.default.removeItem(at: fileURL(item))
        try? FileManager.default.removeItem(at: thumbURL(item))
    }

    private func saveIndex() {
        let list = items
        queue.async {
            if let d = try? JSONEncoder().encode(list) { try? d.write(to: self.indexURL, options: .atomic) }
        }
    }

    private static func write(_ img: CGImage, to url: URL, type: UTType) -> Bool {
        guard let dest = CGImageDestinationCreateWithURL(url as CFURL, type.identifier as CFString, 1, nil) else { return false }
        CGImageDestinationAddImage(dest, img, [kCGImageDestinationLossyCompressionQuality: 0.8] as CFDictionary)
        return CGImageDestinationFinalize(dest)
    }

    private static func thumbnail(_ img: CGImage, maxSide: CGFloat) -> CGImage? {
        let s = min(1, maxSide / CGFloat(max(img.width, img.height)))
        let w = max(1, Int(CGFloat(img.width) * s)), h = max(1, Int(CGFloat(img.height) * s))
        guard let ctx = Renderer.makeContext(width: w, height: h) else { return nil }
        ctx.setFillColor(.white)
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        ctx.interpolationQuality = .high
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
        return ctx.makeImage()
    }
}

// MARK: - Window

enum HistoryWindow {
    private static var window: NSWindow?

    static func show() {
        if window == nil {
            let host = NSHostingController(rootView: HistoryView())
            let w = NSWindow(contentViewController: host)
            w.title = L("Capture History")
            w.styleMask = [.titled, .closable, .resizable, .fullSizeContentView]
            w.titlebarAppearsTransparent = true
            w.isReleasedWhenClosed = false
            w.setContentSize(NSSize(width: 760, height: 560))
            w.minSize = NSSize(width: 480, height: 360)
            w.center()
            NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: w, queue: .main) { _ in
                DispatchQueue.main.async { (NSApp.delegate as? AppDelegate)?.updateActivationPolicy() }
            }
            window = w
        }
        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }

    static var isVisible: Bool { window?.isVisible ?? false }
}

private struct HistoryView: View {
    @ObservedObject var history = CaptureHistory.shared
    @ObservedObject var settings = AppSettings.shared
    private let columns = [GridItem(.adaptive(minimum: 200, maximum: 280), spacing: 18)]

    var body: some View {
        Group {
            if history.items.isEmpty {
                ContentUnavailableView {
                    Label(L("No captures yet"), systemImage: "photo.stack")
                } description: {
                    Text(settings.keepHistory
                         ? L("Your captures from the last 14 days show up here.")
                         : L("Turn on “Keep capture history” in Settings to see your captures here."))
                }
            } else {
                ScrollView {
                    LazyVGrid(columns: columns, spacing: 22) {
                        ForEach(history.items) { HistoryCell(item: $0) }
                    }
                    .padding(22)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .toolbar {
            ToolbarItem {
                Button(L("Clear History"), role: .destructive) { history.clear() }
                    .disabled(history.items.isEmpty)
            }
        }
    }
}

private struct HistoryCell: View {
    let item: HistoryItem
    @State private var hover = false

    private var relative: String {
        let f = RelativeDateTimeFormatter()
        f.unitsStyle = .short
        return f.localizedString(for: item.date, relativeTo: Date())
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ZStack {
                RoundedRectangle(cornerRadius: 12, style: .continuous).fill(.quaternary.opacity(0.5))
                if let t = CaptureHistory.shared.thumbnail(item) {
                    Image(decorative: t, scale: 1)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
                        .shadow(color: .black.opacity(0.18), radius: 6, y: 3)
                        .padding(12)
                }
            }
            .frame(height: 150)
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(hover ? Color.accentColor : .primary.opacity(0.08), lineWidth: hover ? 2 : 1))
            .scaleEffect(hover ? 1.02 : 1)
            HStack {
                Text(relative).font(.callout.weight(.medium))
                Spacer()
                Text("\(Int(CGFloat(item.width) / item.scale))×\(Int(CGFloat(item.height) / item.scale))")
                    .font(.caption).foregroundStyle(.secondary).monospacedDigit()
            }
        }
        .contentShape(Rectangle())
        .onHover { h in withAnimation(.spring(response: 0.25, dampingFraction: 0.8)) { hover = h } }
        .onTapGesture(count: 2) { edit() }
        .onDrag { NSItemProvider(contentsOf: CaptureHistory.shared.fileURL(item)) ?? NSItemProvider() }
        .contextMenu {
            Button(L("Edit")) { edit() }
            Button(L("Copy")) {
                if let c = CaptureHistory.shared.load(item) { Exporter.copy(c.image, scale: c.scale); Sounds.playCopy() }
            }
            Button(L("Pin to Screen")) {
                if let c = CaptureHistory.shared.load(item) { PinnedImage.pin(c) }
            }
            Button(L("Show in Finder")) {
                NSWorkspace.shared.activateFileViewerSelecting([CaptureHistory.shared.fileURL(item)])
            }
            Divider()
            Button(L("Delete"), role: .destructive) { CaptureHistory.shared.remove(item) }
        }
        .help(L("Double-click to edit"))
    }

    private func edit() {
        guard let c = CaptureHistory.shared.load(item) else { return }
        (NSApp.delegate as? AppDelegate)?.openEditor(c)
    }
}
