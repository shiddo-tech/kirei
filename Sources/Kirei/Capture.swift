import AppKit
import ImageIO
import ScreenCaptureKit
import SwiftUI
import UniformTypeIdentifiers

struct CapturedImage {
    let image: CGImage
    /// Pixels per point (2 on Retina).
    let scale: CGFloat
}

enum CaptureService {
    enum Failure: Error { case permission, noDisplay, noWindow, failed }

    /// Runs a capture in-process with ScreenCaptureKit, so it always uses Kirei's own
    /// Screen Recording permission. `nil` means the user cancelled.
    static func capture(_ action: CaptureAction, completion: @escaping (Result<CapturedImage, Failure>?) -> Void) {
        switch action {
        case .clipboard:
            completion(fromPasteboard().map { .success($0) })
        case .fullscreen:
            let mouse = NSEvent.mouseLocation
            guard let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) ?? NSScreen.main else {
                return completion(.failure(.noDisplay))
            }
            captureArea(screen.frame, on: screen, completion: completion)
        case .area, .text, .window:
            guard !SelectionOverlay.isActive, !freezing else { return }
            freezing = true
            // Snapshot every display before the overlay appears: activating Kirei or clicking
            // closes open menus in other apps, so the selection is cropped from this frozen frame.
            freezeScreens { frozen in
                freezing = false
                SelectionOverlay.begin(windowMode: action == .window, frozen: frozen) { selection in
                    switch selection {
                    case .area(let rect, let screen)?:
                        if let frozenShot = frozen[displayID(of: screen)], let cropped = crop(frozenShot, to: rect, on: screen) {
                            completion(.success(cropped))
                        } else {
                            captureArea(rect, on: screen, completion: completion)
                        }
                    case .window(let id)?: captureWindow(id, completion: completion)
                    case nil: completion(nil)
                    }
                }
            }
        }
    }

    private static var freezing = false

    static func displayID(of screen: NSScreen) -> CGDirectDisplayID {
        screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID ?? 0
    }

    /// Full-resolution image of every display, keyed by display ID. Empty when capture isn't possible.
    private static func freezeScreens(_ done: @escaping ([CGDirectDisplayID: CapturedImage]) -> Void) {
        let screens = NSScreen.screens
        shareableContent { content, _ in
            guard let content else { return DispatchQueue.main.async { done([:]) } }
            let me = ProcessInfo.processInfo.processIdentifier
            let mine = content.applications.filter { $0.processID == me }
            let group = DispatchGroup()
            let lock = NSLock()
            var result: [CGDirectDisplayID: CapturedImage] = [:]
            for screen in screens {
                let id = displayID(of: screen)
                guard let display = content.displays.first(where: { $0.displayID == id }) else { continue }
                let scale = screen.backingScaleFactor
                let config = SCStreamConfiguration()
                config.width = max(1, Int((screen.frame.width * scale).rounded()))
                config.height = max(1, Int((screen.frame.height * scale).rounded()))
                config.showsCursor = false
                config.captureResolution = .best
                let filter = SCContentFilter(display: display, excludingApplications: mine, exceptingWindows: [])
                group.enter()
                SCScreenshotManager.captureImage(contentFilter: filter, configuration: config) { image, _ in
                    if let image {
                        lock.lock(); result[id] = CapturedImage(image: image, scale: scale); lock.unlock()
                    }
                    group.leave()
                }
            }
            group.notify(queue: .main) { done(result) }
        }
    }

    /// `rect` is in Cocoa global coordinates, snapped the same way as `captureArea`.
    private static func crop(_ shot: CapturedImage, to rect: CGRect, on screen: NSScreen) -> CapturedImage? {
        let s = shot.scale
        let x0 = (rect.minX - screen.frame.minX).rounded(), x1 = (rect.maxX - screen.frame.minX).rounded()
        let y0 = (screen.frame.maxY - rect.maxY).rounded(), y1 = (screen.frame.maxY - rect.minY).rounded()
        let px = CGRect(x: x0 * s, y: y0 * s, width: max(1, x1 - x0) * s, height: max(1, y1 - y0) * s)
            .integral.intersection(CGRect(x: 0, y: 0, width: shot.image.width, height: shot.image.height))
        guard !px.isEmpty, let img = shot.image.cropping(to: px) else { return nil }
        return CapturedImage(image: img, scale: s)
    }

    private static func shareableContent(_ done: @escaping (SCShareableContent?, Failure?) -> Void) {
        SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: true) { content, error in
            if let content { return done(content, nil) }
            let declined = (error as NSError?)?.code == SCStreamError.userDeclined.rawValue
            done(nil, declined || !hasScreenPermission ? .permission : .failed)
        }
    }

    /// `rect` is in Cocoa global coordinates.
    private static func captureArea(_ rect: CGRect, on screen: NSScreen,
                                    completion: @escaping (Result<CapturedImage, Failure>?) -> Void) {
        let scale = screen.backingScaleFactor
        let displayID = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID
        shareableContent { content, failure in
            guard let content else { return finish(.failure(failure ?? .failed), completion) }
            guard let display = content.displays.first(where: { $0.displayID == displayID }) else {
                return finish(.failure(.noDisplay), completion)
            }
            let me = ProcessInfo.processInfo.processIdentifier
            let filter = SCContentFilter(display: display,
                                         excludingApplications: content.applications.filter { $0.processID == me },
                                         exceptingWindows: [])
            let config = SCStreamConfiguration()
            // Display-local points, top-left origin. Snapped to whole points: any fraction
            // (even a half point on Retina) makes ScreenCaptureKit resample and blur the image.
            let x0 = (rect.minX - screen.frame.minX).rounded(), x1 = (rect.maxX - screen.frame.minX).rounded()
            let y0 = (screen.frame.maxY - rect.maxY).rounded(), y1 = (screen.frame.maxY - rect.minY).rounded()
            let local = CGRect(x: x0, y: y0, width: max(1, x1 - x0), height: max(1, y1 - y0))
            config.sourceRect = local
            config.width = max(1, Int((local.width * scale).rounded()))
            config.height = max(1, Int((local.height * scale).rounded()))
            config.showsCursor = false
            config.captureResolution = .best
            shoot(filter, config, scale: scale, completion: completion)
        }
    }

    private static func captureWindow(_ id: CGWindowID, completion: @escaping (Result<CapturedImage, Failure>?) -> Void) {
        shareableContent { content, failure in
            guard let content else { return finish(.failure(failure ?? .failed), completion) }
            guard let window = content.windows.first(where: { $0.windowID == id }) else {
                return finish(.failure(.noWindow), completion)
            }
            let filter = SCContentFilter(desktopIndependentWindow: window)
            let scale = CGFloat(filter.pointPixelScale)
            let config = SCStreamConfiguration()
            // Window frames are whole points, so this is a 1:1 copy.
            config.width = max(1, Int((window.frame.width * scale).rounded()))
            config.height = max(1, Int((window.frame.height * scale).rounded()))
            config.showsCursor = false
            config.ignoreShadowsSingleWindow = true
            config.shouldBeOpaque = false
            config.captureResolution = .best
            shoot(filter, config, scale: scale, completion: completion)
        }
    }

    private static func shoot(_ filter: SCContentFilter, _ config: SCStreamConfiguration, scale: CGFloat,
                              completion: @escaping (Result<CapturedImage, Failure>?) -> Void) {
        SCScreenshotManager.captureImage(contentFilter: filter, configuration: config) { image, error in
            if let image {
                finish(.success(CapturedImage(image: image, scale: scale)), completion)
            } else {
                NSLog("Kirei capture failed: \(String(describing: error))")
                let declined = (error as NSError?)?.code == SCStreamError.userDeclined.rawValue
                finish(.failure(declined ? .permission : .failed), completion)
            }
        }
    }

    private static func finish(_ r: Result<CapturedImage, Failure>, _ completion: @escaping (Result<CapturedImage, Failure>?) -> Void) {
        DispatchQueue.main.async { completion(r) }
    }

    static func load(url: URL) -> CapturedImage? {
        guard let src = CGImageSourceCreateWithURL(url as CFURL, nil),
              let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { return nil }
        let props = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any]
        let dpi = (props?[kCGImagePropertyDPIWidth] as? Double) ?? 72
        let scale = max(1, CGFloat((dpi / 72).rounded()))
        return CapturedImage(image: img, scale: scale)
    }

    static func fromPasteboard() -> CapturedImage? {
        let pb = NSPasteboard.general
        if let urls = pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL],
           let first = urls.first, let img = load(url: first) {
            return img
        }
        for type in [NSPasteboard.PasteboardType.png, .tiff] {
            if let data = pb.data(forType: type), let src = CGImageSourceCreateWithData(data as CFData, nil),
               let img = CGImageSourceCreateImageAtIndex(src, 0, nil) {
                return CapturedImage(image: img, scale: NSScreen.main?.backingScaleFactor ?? 2)
            }
        }
        if let ns = NSImage(pasteboard: pb), let cg = ns.cgImage(forProposedRect: nil, context: nil, hints: nil) {
            return CapturedImage(image: cg, scale: max(1, CGFloat(cg.width) / ns.size.width))
        }
        return nil
    }

    static var hasScreenPermission: Bool { CGPreflightScreenCaptureAccess() }
}

// MARK: - Export

enum Exporter {
    static func data(_ image: CGImage, format: AppSettings.Format, scale: CGFloat) -> Data? {
        let data = NSMutableData()
        let type = format == .png ? UTType.png : UTType.jpeg
        guard let dest = CGImageDestinationCreateWithData(data, type.identifier as CFString, 1, nil) else { return nil }
        var props: [CFString: Any] = [
            kCGImagePropertyDPIWidth: 72 * scale,
            kCGImagePropertyDPIHeight: 72 * scale,
        ]
        var img = image
        if format == .jpeg {
            props[kCGImageDestinationLossyCompressionQuality] = 0.92
            img = flattened(image) ?? image
        }
        CGImageDestinationAddImage(dest, img, props as CFDictionary)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return data as Data
    }

    /// JPEG has no alpha: composite on white.
    static func flattened(_ image: CGImage) -> CGImage? {
        guard let ctx = Renderer.makeContext(width: image.width, height: image.height) else { return nil }
        let r = CGRect(x: 0, y: 0, width: image.width, height: image.height)
        ctx.setFillColor(.white)
        ctx.fill(r)
        ctx.draw(image, in: r)
        return ctx.makeImage()
    }

    static func copy(_ image: CGImage, scale: CGFloat) {
        let pb = NSPasteboard.general
        pb.clearContents()
        let item = NSPasteboardItem()
        if let png = data(image, format: .png, scale: scale) { item.setData(png, forType: .png) }
        let ns = NSImage(cgImage: image, size: NSSize(width: CGFloat(image.width) / scale,
                                                      height: CGFloat(image.height) / scale))
        if let tiff = ns.tiffRepresentation { item.setData(tiff, forType: .tiff) }
        pb.writeObjects([item])
    }

    static func fileName(ext: String) -> String {
        let f = DateFormatter()
        f.dateFormat = L("yyyy-MM-dd 'at' HH.mm.ss")
        return "Kirei \(f.string(from: Date())).\(ext)"
    }

    static func ext(_ format: AppSettings.Format) -> String { format == .png ? "png" : "jpg" }

    @discardableResult
    static func save(_ image: CGImage, scale: CGFloat, to url: URL, format: AppSettings.Format) -> Bool {
        guard let d = data(image, format: format, scale: scale) else { return false }
        do { try d.write(to: url); return true } catch { return false }
    }

    static func saveToDefaultFolder(_ image: CGImage, scale: CGFloat) -> URL? {
        let s = AppSettings.shared
        let url = s.saveFolder.appendingPathComponent(fileName(ext: ext(s.format)))
        return save(image, scale: scale, to: url, format: s.format) ? url : nil
    }

    static func temporaryFile(_ image: CGImage, scale: CGFloat) -> URL? {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("KireiExports", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent(fileName(ext: "png"))
        return save(image, scale: scale, to: url, format: .png) ? url : nil
    }
}

// MARK: - HUD toast

enum HUD {
    private static var panel: NSPanel?
    private static var hideWork: DispatchWorkItem?

    static func show(_ message: String, symbol: String = "checkmark.circle.fill") {
        panel?.orderOut(nil)
        let view = NSHostingView(rootView:
            HStack(spacing: 10) {
                Image(systemName: symbol).font(.system(size: 18, weight: .semibold))
                Text(message).font(.system(size: 14, weight: .medium))
            }
            .foregroundStyle(.primary)
            .padding(.horizontal, 18).padding(.vertical, 12)
            .background(.regularMaterial, in: Capsule())
            .overlay(Capsule().strokeBorder(.primary.opacity(0.08)))
            .fixedSize()
        )
        let size = view.fittingSize
        let p = NSPanel(contentRect: NSRect(origin: .zero, size: size),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = true
        p.level = .statusBar
        p.ignoresMouseEvents = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.contentView = view
        let screen = NSScreen.screens.first { NSMouseInRect(NSEvent.mouseLocation, $0.frame, false) } ?? NSScreen.main
        if let f = screen?.visibleFrame {
            p.setFrameOrigin(NSPoint(x: f.midX - size.width / 2, y: f.minY + 80))
        }
        p.alphaValue = 0
        p.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { $0.duration = 0.15; p.animator().alphaValue = 1 }
        panel = p
        hideWork?.cancel()
        let work = DispatchWorkItem {
            NSAnimationContext.runAnimationGroup({ $0.duration = 0.3; p.animator().alphaValue = 0 }) {
                p.orderOut(nil)
            }
        }
        hideWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.6, execute: work)
    }
}
