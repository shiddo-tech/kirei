import AppKit
import ScreenCaptureKit
import SwiftUI

/// Scrolling capture: pick an area, scroll down, and the frames are stitched into one tall image.
/// Frames are matched by exact row hashes, which works because scrolled pixels are identical.
/// Fixed bands at the top or bottom of the area (sticky headers, footers) appear only once.
final class ScrollingCapture {
    private static var current: ScrollingCapture?

    private let filter: SCContentFilter
    private let config: SCStreamConfiguration
    private let rect: CGRect
    private let scale: CGFloat
    private let done: (CapturedImage) -> Void
    private var timer: Timer?
    private var busy = false
    private var border: NSWindow?
    private var controls: NSPanel?
    private let state = ScrollState()

    private var stitcher = Stitcher()

    struct Frame {
        let image: CGImage
        let hashes: [UInt64]
    }

    static func start(done: @escaping (CapturedImage) -> Void) {
        guard current == nil, !SelectionOverlay.isActive, !ScreenRecorder.isRecording else { return }
        SelectionOverlay.begin(windowMode: false) { selection in
            guard let selection, case .area(_, let screen) = selection else { return }
            CaptureService.recordingFilter(for: selection) { result in
                guard let (filter, config, rect) = result else {
                    HUD.show(L("Couldn't capture the screen"), symbol: "exclamationmark.triangle.fill")
                    return
                }
                config.showsCursor = false
                current = ScrollingCapture(filter: filter, config: config, rect: rect,
                                           scale: screen.backingScaleFactor, done: done)
            }
        }
    }

    private init(filter: SCContentFilter, config: SCStreamConfiguration, rect: CGRect, scale: CGFloat,
                 done: @escaping (CapturedImage) -> Void) {
        self.filter = filter
        self.config = config
        self.rect = rect
        self.scale = scale
        self.done = done
        showChrome()
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in self?.grab() }
        grab()
    }

    private func grab() {
        guard !busy else { return }
        busy = true
        SCScreenshotManager.captureImage(contentFilter: filter, configuration: config) { [weak self] image, _ in
            guard let self else { return }
            let frame = image.map { Frame(image: $0, hashes: Self.rowHashes($0)) }
            DispatchQueue.main.async {
                self.busy = false
                if let frame { self.add(frame) }
            }
        }
    }

    private func add(_ frame: Frame) {
        guard ScrollingCapture.current === self else { return }
        if stitcher.add(frame) { state.height = Int(CGFloat(stitcher.height) / scale) }
    }

    /// Finds how far the content moved up between two frames.
    /// Returns the shift in rows plus the fixed bands at the top and bottom.
    static func match(_ a: [UInt64], _ b: [UInt64]) -> (Int, Int, Int)? {
        let h = min(a.count, b.count)
        guard h > 40 else { return nil }
        var top = 0
        while top < h && a[top] == b[top] { top += 1 }
        if top == h { return nil } // nothing changed
        var bottom = 0
        while bottom < h - top && a[h - 1 - bottom] == b[h - 1 - bottom] { bottom += 1 }
        let region = top..<(h - bottom)
        guard region.count > 30 else { return nil }
        // Only rows that differ from the row above say anything about position: runs of identical
        // rows (blank space, solid bars) match at many shifts.
        let edges = region.dropFirst().filter { b[$0] != b[$0 - 1] }
        var best: (shift: Int, score: Double)?
        for shift in 1..<(region.count - 8) {
            var hits = 0, total = 0
            for j in edges where j + shift < region.upperBound {
                total += 1
                if a[j + shift] == b[j] { hits += 1 }
            }
            guard total >= 4 else { break }
            let score = Double(hits) / Double(total)
            if score > (best?.score ?? 0) + 1e-9 { best = (shift, score) }
        }
        guard let best, best.score >= 0.9 else { return nil }
        let shift = best.shift
        return (shift, top, bottom)
    }

    static func rowHashes(_ image: CGImage) -> [UInt64] {
        let w = image.width, h = image.height
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: Renderer.colorSpace, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let data = ctx.data else { return [] }
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        let bytes = data.bindMemory(to: UInt8.self, capacity: w * h * 4)
        // Skip the right edge, where overlay scrollbars fade in and out.
        let usable = max(1, w - Int(Double(w) * 0.03) - 16) * 4
        var out = [UInt64](repeating: 0, count: h)
        for y in 0..<h {
            let row = bytes + y * w * 4
            var hash: UInt64 = 0xcbf2_9ce4_8422_2325
            var x = 0
            while x < usable {
                hash = (hash ^ UInt64(row[x])) &* 0x100_0000_01b3
                x += 1
            }
            out[y] = hash
        }
        return out
    }

    // MARK: Finish

    private func finish(cancelled: Bool) {
        timer?.invalidate()
        hideChrome()
        ScrollingCapture.current = nil
        guard !cancelled, let img = stitcher.result() else { return }
        done(CapturedImage(image: img, scale: scale))
    }

    // MARK: Chrome

    private func showChrome() {
        let b = NSWindow(contentRect: rect.insetBy(dx: -3, dy: -3), styleMask: [.borderless], backing: .buffered, defer: false)
        b.isOpaque = false
        b.backgroundColor = .clear
        b.ignoresMouseEvents = true
        b.level = .statusBar
        b.isReleasedWhenClosed = false
        b.contentView = NSHostingView(rootView:
            DashedFrame(color: .accentColor)
        )
        b.orderFrontRegardless()
        border = b

        let size = NSSize(width: 340, height: 48)
        let screen = NSScreen.screens.first { $0.frame.intersects(rect) } ?? NSScreen.main
        let vf = screen?.visibleFrame ?? rect
        var origin = NSPoint(x: rect.midX - size.width / 2, y: rect.minY - size.height - 12)
        if origin.y < vf.minY + 8 { origin.y = vf.minY + 24 }
        origin.x = max(vf.minX + 8, min(origin.x, vf.maxX - size.width - 8))
        let p = PreviewPanel(contentRect: NSRect(origin: origin, size: size),
                             styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        p.isOpaque = false
        p.backgroundColor = .clear
        p.level = .statusBar
        p.isMovableByWindowBackground = true
        p.contentView = NSHostingView(rootView: ScrollControls(
            state: state,
            onDone: { [weak self] in self?.finish(cancelled: false) },
            onCancel: { [weak self] in self?.finish(cancelled: true) }))
        p.orderFrontRegardless()
        controls = p
    }

    private func hideChrome() {
        border?.orderOut(nil)
        controls?.orderOut(nil)
    }
}

/// Accumulates frames into one tall image.
/// Each frame is placed at its scroll offset and later frames paint over earlier ones, so a fixed
/// footer is covered by the next frame's content and only the last one survives. Fixed-band sizes
/// can be overestimated when blank rows happen to line up; the margin keeps that from losing rows.
struct Stitcher {
    private var first: CGImage?
    private var previous: [UInt64] = []
    private var slices: [(image: CGImage, at: Int)] = []
    private var offset = 0
    private var frameHeight = 0
    private static let maxHeight = 30_000
    private static let margin = 80

    var height: Int { offset + frameHeight }

    /// Returns true when the frame moved the content.
    mutating func add(_ frame: ScrollingCapture.Frame) -> Bool {
        let h = frame.hashes.count
        guard first != nil else {
            first = frame.image
            previous = frame.hashes
            frameHeight = h
            return true
        }
        guard h == frameHeight, let (shift, top, bottom) = ScrollingCapture.match(previous, frame.hashes),
              shift > 0, offset + shift + h < Self.maxHeight else { return false }
        offset += shift
        let start = max(top, h - bottom - shift - Self.margin)
        guard let slice = Self.copyRows(frame.image, start..<h) else { return false }
        slices.append((slice, offset + start))
        previous = frame.hashes
        return true
    }

    func result() -> CGImage? {
        guard let first else { return nil }
        let w = first.width, total = height
        guard let ctx = Renderer.makeContext(width: w, height: total) else { return nil }
        ctx.draw(first, in: CGRect(x: 0, y: total - first.height, width: w, height: first.height))
        for s in slices {
            ctx.draw(s.image, in: CGRect(x: 0, y: total - s.at - s.image.height, width: w, height: s.image.height))
        }
        return ctx.makeImage()
    }

    /// A real copy, so the captured frame's buffer isn't kept alive by a crop.
    private static func copyRows(_ image: CGImage, _ rows: Range<Int>) -> CGImage? {
        guard !rows.isEmpty, let crop = image.cropping(to: CGRect(x: 0, y: rows.lowerBound, width: image.width, height: rows.count)),
              let ctx = Renderer.makeContext(width: crop.width, height: crop.height) else { return nil }
        ctx.draw(crop, in: CGRect(x: 0, y: 0, width: crop.width, height: crop.height))
        return ctx.makeImage()
    }
}

private final class ScrollState: ObservableObject {
    @Published var height = 0
}

private struct ScrollControls: View {
    @ObservedObject var state: ScrollState
    let onDone: () -> Void
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "arrow.down").font(.system(size: 13, weight: .bold)).foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 0) {
                Text(L("Scroll down slowly")).font(.callout.weight(.semibold))
                Text(String(format: L("%d pt captured"), state.height))
                    .font(.caption).foregroundStyle(.secondary).monospacedDigit()
                    .contentTransition(.numericText())
            }
            Spacer(minLength: 0)
            Button(L("Cancel"), action: onCancel).buttonStyle(.plain).foregroundStyle(.secondary).font(.callout)
            Button(action: onDone) {
                Text(L("Done")).font(.callout.weight(.semibold))
                    .padding(.horizontal, 12).padding(.vertical, 5)
                    .background(Color.accentColor, in: Capsule())
                    .foregroundStyle(.white)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 14)
        .frame(width: 336, height: 44)
        .kireiGlass(in: Capsule())
        .frame(width: 340, height: 48)
    }
}
