import AppKit
import AVFoundation
import ScreenCaptureKit
import SwiftUI
import UniformTypeIdentifiers

/// Screen recording of an area or a window to MP4 (H.264), with GIF export afterwards.
final class ScreenRecorder: NSObject, SCStreamOutput, SCStreamDelegate {
    static private(set) var current: ScreenRecorder?
    static var isRecording: Bool { current != nil }
    static var onChange: (() -> Void)?

    private var stream: SCStream?
    private var writer: AVAssetWriter?
    private var input: AVAssetWriterInput?
    private var started = false
    private let url: URL
    private let queue = DispatchQueue(label: "kirei.recorder")
    private var border: NSWindow?
    private var controls: NSPanel?
    private let startDate = Date()

    // MARK: Start

    static func start() {
        guard current == nil, !SelectionOverlay.isActive else { return }
        SelectionOverlay.begin(windowMode: false) { selection in
            guard let selection else { return }
            CaptureService.recordingFilter(for: selection) { result in
                guard let (filter, config, rect) = result else {
                    HUD.show(L("Couldn't start recording"), symbol: "exclamationmark.triangle.fill")
                    return
                }
                let r = ScreenRecorder(rect: rect)
                current = r
                r.begin(filter: filter, config: config)
            }
        }
    }

    static func stop() { current?.finish() }

    private init(rect: CGRect) {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("KireiRecordings", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        url = dir.appendingPathComponent(Exporter.fileName(ext: "mp4"))
        super.init()
        showChrome(around: rect)
    }

    private func begin(filter: SCContentFilter, config: SCStreamConfiguration) {
        do {
            let w = try AVAssetWriter(outputURL: url, fileType: .mp4)
            // H.264 needs even dimensions.
            let width = config.width & ~1, height = config.height & ~1
            config.width = width
            config.height = height
            let settings: [String: Any] = [
                AVVideoCodecKey: AVVideoCodecType.h264,
                AVVideoWidthKey: width,
                AVVideoHeightKey: height,
                AVVideoCompressionPropertiesKey: [
                    AVVideoAverageBitRateKey: max(4_000_000, width * height * 6),
                    AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                    AVVideoExpectedSourceFrameRateKey: 60,
                ],
            ]
            let i = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
            i.expectsMediaDataInRealTime = true
            w.add(i)
            writer = w
            input = i

            let s = SCStream(filter: filter, configuration: config, delegate: self)
            try s.addStreamOutput(self, type: .screen, sampleHandlerQueue: queue)
            stream = s
            s.startCapture { [weak self] error in
                guard let error else { return }
                NSLog("Kirei recording failed to start: \(error)")
                DispatchQueue.main.async {
                    self?.teardown()
                    HUD.show(L("Couldn't start recording"), symbol: "exclamationmark.triangle.fill")
                }
            }
            Sounds.playTick()
            ScreenRecorder.onChange?()
        } catch {
            NSLog("Kirei recorder: \(error)")
            teardown()
            HUD.show(L("Couldn't start recording"), symbol: "exclamationmark.triangle.fill")
        }
    }

    // MARK: Frames

    func stream(_ stream: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, sb.isValid, let writer, let input else { return }
        // Only complete frames carry new pixels; idle/blank ones would stall the timeline.
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let raw = attachments.first?[.status] as? Int,
              SCFrameStatus(rawValue: raw) == .complete else { return }
        if !started {
            guard writer.startWriting() else { return }
            writer.startSession(atSourceTime: sb.presentationTimeStamp)
            started = true
        }
        if input.isReadyForMoreMediaData { input.append(sb) }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        NSLog("Kirei recording stopped: \(error)")
        DispatchQueue.main.async { self.finish() }
    }

    // MARK: Stop

    private func finish() {
        guard ScreenRecorder.current === self else { return }
        let s = stream
        stream = nil
        s?.stopCapture { [weak self] _ in
            self?.queue.async { self?.closeFile() }
        }
        if s == nil { queue.async { self.closeFile() } }
        hideChrome()
    }

    private func closeFile() {
        guard let writer, started else {
            DispatchQueue.main.async {
                self.teardown()
                HUD.show(L("Nothing was recorded"), symbol: "record.circle")
            }
            return
        }
        input?.markAsFinished()
        writer.finishWriting { [weak self] in
            guard let self else { return }
            let ok = writer.status == .completed
            DispatchQueue.main.async {
                let url = self.url
                self.teardown()
                if ok {
                    Sounds.playCapture()
                    RecordingPreview.show(url)
                } else {
                    HUD.show(L("Couldn't save the recording"), symbol: "exclamationmark.triangle.fill")
                }
            }
        }
    }

    private func teardown() {
        hideChrome()
        if ScreenRecorder.current === self { ScreenRecorder.current = nil }
        ScreenRecorder.onChange?()
    }

    // MARK: On-screen chrome

    /// A dashed frame around the recorded area and a small stop control under it.
    /// Kirei's own windows are excluded from the capture, so neither shows in the video.
    private func showChrome(around rect: CGRect) {
        let b = NSWindow(contentRect: rect.insetBy(dx: -3, dy: -3), styleMask: [.borderless], backing: .buffered, defer: false)
        b.isOpaque = false
        b.backgroundColor = .clear
        b.ignoresMouseEvents = true
        b.level = .statusBar
        b.isReleasedWhenClosed = false
        b.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        b.contentView = NSHostingView(rootView:
            RoundedRectangle(cornerRadius: 4, style: .continuous)
                .strokeBorder(Color.red.opacity(0.85), style: StrokeStyle(lineWidth: 2, dash: [6, 4]))
        )
        b.orderFrontRegardless()
        border = b

        let size = NSSize(width: 176, height: 44)
        let screen = NSScreen.screens.first { $0.frame.intersects(rect) } ?? NSScreen.main
        let vf = screen?.visibleFrame ?? rect
        var origin = NSPoint(x: rect.midX - size.width / 2, y: rect.minY - size.height - 12)
        if origin.y < vf.minY + 8 { origin.y = min(rect.maxY + 12, vf.maxY - size.height - 8) }
        if rect.height > vf.height - 120 { origin.y = vf.minY + 24 }
        origin.x = max(vf.minX + 8, min(origin.x, vf.maxX - size.width - 8))
        let p = PreviewPanel(contentRect: NSRect(origin: origin, size: size),
                             styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.isOpaque = false
        p.backgroundColor = .clear
        p.level = .statusBar
        p.isMovableByWindowBackground = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.contentView = NSHostingView(rootView: RecordingControls(start: startDate) { ScreenRecorder.stop() })
        p.orderFrontRegardless()
        controls = p
    }

    private func hideChrome() {
        border?.orderOut(nil)
        controls?.orderOut(nil)
        border = nil
        controls = nil
    }
}

private struct RecordingControls: View {
    let start: Date
    let onStop: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(.red).frame(width: 9, height: 9)
                .phaseAnimator([1.0, 0.35]) { v, phase in v.opacity(phase) } animation: { _ in .easeInOut(duration: 0.8) }
            TimelineView(.periodic(from: start, by: 1)) { ctx in
                let s = Int(ctx.date.timeIntervalSince(start))
                Text(String(format: "%d:%02d", s / 60, s % 60))
                    .font(.system(size: 13, weight: .semibold, design: .rounded))
                    .monospacedDigit()
            }
            Spacer(minLength: 0)
            Button(action: onStop) {
                Label(L("Stop"), systemImage: "stop.fill")
                    .font(.callout.weight(.semibold))
                    .padding(.horizontal, 10).padding(.vertical, 5)
                    .background(.red, in: Capsule())
                    .foregroundStyle(.white)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 14)
        .frame(width: 172, height: 40)
        .kireiGlass(in: Capsule())
        .frame(width: 176, height: 44)
    }
}

// MARK: - After recording

enum RecordingPreview {
    private static var panel: NSPanel?

    static func show(_ url: URL) {
        panel?.orderOut(nil)
        let size = NSSize(width: 300, height: 220)
        let p = PreviewPanel(contentRect: NSRect(origin: .zero, size: size),
                             styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.isOpaque = false
        p.backgroundColor = .clear
        p.level = .statusBar
        p.becomesKeyOnlyIfNeeded = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        p.contentView = NSHostingView(rootView: RecordingPreviewView(url: url) { close() })
        let screen = NSScreen.screens.first { NSMouseInRect(NSEvent.mouseLocation, $0.frame, false) } ?? NSScreen.main
        let f = screen?.visibleFrame ?? .zero
        p.setFrameOrigin(NSPoint(x: f.maxX - size.width - 20, y: f.minY + 20))
        p.alphaValue = 0
        p.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup { $0.duration = 0.25; p.animator().alphaValue = 1 }
        panel = p
    }

    static func close() {
        guard let p = panel else { return }
        panel = nil
        NSAnimationContext.runAnimationGroup({ $0.duration = 0.2; p.animator().alphaValue = 0 }) { p.orderOut(nil) }
    }
}

private struct RecordingPreviewView: View {
    let url: URL
    let onClose: () -> Void
    @State private var poster: CGImage?
    @State private var working = false

    var body: some View {
        VStack(spacing: 10) {
            ZStack(alignment: .topLeading) {
                Group {
                    if let poster {
                        Image(decorative: poster, scale: 1).resizable().aspectRatio(contentMode: .fit)
                    } else {
                        Image(systemName: "film").font(.largeTitle).foregroundStyle(.secondary)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                .onDrag { NSItemProvider(contentsOf: url) ?? NSItemProvider() }
                .onTapGesture(count: 2) { NSWorkspace.shared.open(url) }
                Button(action: onClose) {
                    Image(systemName: "xmark").font(.system(size: 9, weight: .bold))
                        .frame(width: 20, height: 20).kireiGlass(in: Circle())
                }
                .buttonStyle(.plain)
                .padding(6)
            }
            HStack(spacing: 6) {
                button(L("Save MP4"), "square.and.arrow.down") { save(url, ext: "mp4") }
                button(L("Save GIF"), "photo.stack") { makeGIF() }
                button(L("Copy"), "doc.on.doc") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.writeObjects([url as NSURL])
                    Sounds.playCopy()
                    HUD.show(L("Video copied"))
                }
            }
            .disabled(working)
            .overlay { if working { ProgressView().controlSize(.small) } }
        }
        .padding(10)
        .frame(width: 292, height: 212)
        .kireiGlass(in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .frame(width: 300, height: 220)
        .task { poster = await Self.poster(url) }
    }

    private func button(_ title: String, _ symbol: String, _ run: @escaping () -> Void) -> some View {
        Button(action: run) {
            Label(title, systemImage: symbol).font(.caption.weight(.semibold)).lineLimit(1)
                .padding(.horizontal, 8).padding(.vertical, 5)
                .background(.primary.opacity(0.08), in: Capsule())
        }
        .buttonStyle(.plain)
    }

    private func save(_ file: URL, ext: String) {
        let dest = AppSettings.shared.saveFolder.appendingPathComponent(Exporter.fileName(ext: ext))
        do {
            try FileManager.default.copyItem(at: file, to: dest)
            HUD.show(String(format: L("Saved to %@"), dest.deletingLastPathComponent().lastPathComponent), symbol: "folder.fill")
            onClose()
        } catch {
            HUD.show(L("Couldn't save"), symbol: "exclamationmark.triangle.fill")
        }
    }

    private func makeGIF() {
        working = true
        Task {
            let gif = await GIFMaker.make(from: url)
            working = false
            if let gif { save(gif, ext: "gif") } else { HUD.show(L("Couldn't create the GIF"), symbol: "exclamationmark.triangle.fill") }
        }
    }

    private static func poster(_ url: URL) async -> CGImage? {
        let gen = AVAssetImageGenerator(asset: AVURLAsset(url: url))
        gen.appliesPreferredTrackTransform = true
        gen.maximumSize = CGSize(width: 600, height: 600)
        return try? await gen.image(at: .zero).image
    }
}

enum GIFMaker {
    /// 12 fps, at most 960 px wide; loops forever.
    static func make(from url: URL, fps: Double = 12, maxWidth: CGFloat = 960) async -> URL? {
        let asset = AVURLAsset(url: url)
        guard let duration = try? await asset.load(.duration) else { return nil }
        let seconds = CMTimeGetSeconds(duration)
        guard seconds > 0 else { return nil }
        let gen = AVAssetImageGenerator(asset: asset)
        gen.appliesPreferredTrackTransform = true
        gen.maximumSize = CGSize(width: maxWidth, height: maxWidth * 4)
        gen.requestedTimeToleranceBefore = .zero
        gen.requestedTimeToleranceAfter = .zero
        let count = max(1, Int(seconds * fps))
        let out = url.deletingPathExtension().appendingPathExtension("gif")
        guard let dest = CGImageDestinationCreateWithURL(out as CFURL, UTType.gif.identifier as CFString, count, nil) else { return nil }
        CGImageDestinationSetProperties(dest, [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFLoopCount: 0]] as CFDictionary)
        let frameProps = [kCGImagePropertyGIFDictionary: [kCGImagePropertyGIFDelayTime: 1 / fps]] as CFDictionary
        for i in 0..<count {
            let t = CMTime(seconds: Double(i) / fps, preferredTimescale: 600)
            if let img = try? await gen.image(at: t).image { CGImageDestinationAddImage(dest, img, frameProps) }
        }
        return CGImageDestinationFinalize(dest) ? out : nil
    }
}
