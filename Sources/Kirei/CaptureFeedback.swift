import AppKit
import SwiftUI

/// Kirei's own sounds, synthesized once as WAV data so they need no resource files.
enum Sounds {
    private static let rate = 44_100.0

    static func playCapture() { play(capture) }
    static func playCopy() { play(copy) }
    static func playTick() { play(tick) }

    private static func play(_ sound: NSSound?) {
        guard AppSettings.shared.playSound, let sound else { return }
        sound.stop()
        sound.play()
    }

    /// A soft two-part shutter: a filtered noise click followed by a lighter one.
    private static let capture: NSSound? = {
        var rng = SystemRandomNumberGenerator()
        var lp = 0.0
        let samples = synth(duration: 0.16) { t in
            let n = Double.random(in: -1...1, using: &rng)
            lp += (n - lp) * 0.35
            let first = exp(-t * 140) * 0.55
            let second = t > 0.065 ? exp(-(t - 0.065) * 170) * 0.32 : 0
            let body = sin(2 * .pi * 1_900 * t) * exp(-t * 90) * 0.12
            return lp * (first + second) + body
        }
        return sound(samples, volume: 0.55)
    }()

    /// Two short glassy notes, a fifth apart.
    private static let copy: NSSound? = {
        let samples = synth(duration: 0.32) { t in
            func note(_ f: Double, _ start: Double) -> Double {
                guard t >= start else { return 0 }
                let u = t - start
                let attack = min(1, u / 0.004)
                return (sin(2 * .pi * f * u) + 0.25 * sin(2 * .pi * f * 2 * u)) * attack * exp(-u * 18)
            }
            return 0.32 * (note(1_318.5, 0) + note(1_975.5, 0.07))
        }
        return sound(samples, volume: 0.45)
    }()

    private static let tick: NSSound? = {
        let samples = synth(duration: 0.05) { t in sin(2 * .pi * 2_400 * t) * exp(-t * 120) * 0.5 }
        return sound(samples, volume: 0.35)
    }()

    private static func synth(duration: Double, _ f: (Double) -> Double) -> [Int16] {
        let n = Int(duration * rate)
        return (0..<n).map { i in
            let t = Double(i) / rate
            let fadeOut = min(1, Double(n - i) / (rate * 0.01))
            return Int16(max(-1, min(1, f(t) * fadeOut)) * Double(Int16.max))
        }
    }

    private static func sound(_ samples: [Int16], volume: Float) -> NSSound? {
        var d = Data()
        func u32(_ v: UInt32) { withUnsafeBytes(of: v.littleEndian) { d.append(contentsOf: $0) } }
        func u16(_ v: UInt16) { withUnsafeBytes(of: v.littleEndian) { d.append(contentsOf: $0) } }
        let bytes = UInt32(samples.count * 2)
        d.append(contentsOf: Array("RIFF".utf8)); u32(36 + bytes)
        d.append(contentsOf: Array("WAVEfmt ".utf8)); u32(16); u16(1); u16(1)
        u32(UInt32(rate)); u32(UInt32(rate) * 2); u16(2); u16(16)
        d.append(contentsOf: Array("data".utf8)); u32(bytes)
        samples.forEach { u16(UInt16(bitPattern: $0)) }
        let s = NSSound(data: d)
        s?.volume = volume
        return s
    }
}

enum Haptics {
    static func align() {
        NSHapticFeedbackManager.defaultPerformer.perform(.alignment, performanceTime: .now)
    }
}

/// Visual feedback at the moment of capture.
enum CaptureFeedback {
    /// A brief white flash over the captured region, like a camera shutter.
    static func flash(_ rect: CGRect) {
        let w = overlayWindow(rect)
        w.backgroundColor = NSColor.white
        w.alphaValue = 0.55
        w.orderFrontRegardless()
        NSAnimationContext.runAnimationGroup({ ctx in
            ctx.duration = 0.28
            ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
            w.animator().alphaValue = 0
        }) { w.orderOut(nil) }
    }

    /// Flies the captured image from where it was on screen into `target`, with a little overshoot.
    static func fly(_ image: CGImage, from rect: CGRect, to target: CGRect, done: @escaping () -> Void) {
        let w = overlayWindow(rect)
        w.backgroundColor = .clear
        w.hasShadow = true
        let view = NSImageView(frame: NSRect(origin: .zero, size: rect.size))
        view.image = NSImage(cgImage: image, size: rect.size)
        view.imageScaling = .scaleProportionallyUpOrDown
        view.autoresizingMask = [.width, .height]
        view.wantsLayer = true
        view.layer?.cornerRadius = 10
        view.layer?.cornerCurve = .continuous
        view.layer?.masksToBounds = true
        w.contentView = view
        w.orderFrontRegardless()
        // Keep the image's proportions inside the thumbnail slot.
        let s = min(target.width / rect.width, target.height / rect.height)
        let end = CGRect(x: target.midX - rect.width * s / 2, y: target.midY - rect.height * s / 2,
                         width: rect.width * s, height: rect.height * s)
        NSAnimationContext.runAnimationGroup({ ctx in
            ctx.duration = 0.42
            ctx.timingFunction = CAMediaTimingFunction(controlPoints: 0.3, 1.25, 0.5, 1)
            w.animator().setFrame(end, display: true)
        }) {
            done()
            NSAnimationContext.runAnimationGroup({ ctx in
                ctx.duration = 0.18
                w.animator().alphaValue = 0
            }) { w.orderOut(nil) }
        }
    }

    private static func overlayWindow(_ rect: CGRect) -> NSWindow {
        let w = NSWindow(contentRect: rect, styleMask: [.borderless], backing: .buffered, defer: false)
        w.isOpaque = false
        w.hasShadow = false
        w.ignoresMouseEvents = true
        w.level = .screenSaver
        w.isReleasedWhenClosed = false
        w.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        return w
    }
}

/// Self-timer: a large count in the middle of the screen. Clicking it cancels.
final class Countdown {
    private static var current: Countdown?
    private let panel: NSPanel
    private let state = CountdownState()
    private var timer: Timer?
    private let done: () -> Void

    static func start(seconds: Int, done: @escaping () -> Void) {
        current?.cancel()
        current = Countdown(seconds: seconds, done: done)
    }

    private init(seconds: Int, done: @escaping () -> Void) {
        self.done = done
        state.value = seconds
        let size = NSSize(width: 168, height: 168)
        panel = NSPanel(contentRect: NSRect(origin: .zero, size: size),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.contentView = NSHostingView(rootView: CountdownView(state: state) { [weak self] in self?.cancel() })
        let screen = NSScreen.screens.first { NSMouseInRect(NSEvent.mouseLocation, $0.frame, false) } ?? NSScreen.main
        if let f = screen?.frame { panel.setFrameOrigin(NSPoint(x: f.midX - size.width / 2, y: f.midY - size.height / 2)) }
        panel.orderFrontRegardless()
        Sounds.playTick()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.tick() }
    }

    private func tick() {
        guard state.value > 1 else { return finish() }
        withAnimation(.spring(response: 0.3, dampingFraction: 0.6)) { state.value -= 1 }
        Sounds.playTick()
    }

    private func finish() {
        timer?.invalidate()
        panel.orderOut(nil)
        Countdown.current = nil
        // Give the window server a frame to remove the panel before capturing.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.12, execute: done)
    }

    private func cancel() {
        timer?.invalidate()
        panel.orderOut(nil)
        Countdown.current = nil
    }
}

private final class CountdownState: ObservableObject {
    @Published var value = 0
}

private struct CountdownView: View {
    @ObservedObject var state: CountdownState
    let onCancel: () -> Void

    var body: some View {
        VStack(spacing: 2) {
            Text("\(state.value)")
                .font(.system(size: 64, weight: .semibold, design: .rounded))
                .monospacedDigit()
                .contentTransition(.numericText(countsDown: true))
            Text(L("Click to cancel"))
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(width: 150, height: 150)
        .kireiGlass(in: Circle())
        .contentShape(Circle())
        .onTapGesture(perform: onCancel)
        .frame(width: 168, height: 168)
    }
}
