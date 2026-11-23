import AppKit
import SwiftUI

final class OnboardingWindowController: NSWindowController, NSWindowDelegate {
    enum Step: Int { case welcome, permission, shortcuts }

    var onClose: (() -> Void)?
    private let state: OnboardingState

    init(step: Step, tryAnyway: (() -> Void)?) {
        state = OnboardingState(step: step, tryAnyway: tryAnyway)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 560, height: 460),
                              styleMask: [.titled, .closable, .fullSizeContentView],
                              backing: .buffered, defer: false)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true
        window.isReleasedWhenClosed = false
        window.title = L("Welcome to Kirei")
        super.init(window: window)
        window.delegate = self
        state.close = { [weak window] in window?.performClose(nil) }
        window.contentViewController = NSHostingController(rootView: OnboardingView(state: state))
        window.setContentSize(NSSize(width: 560, height: 460))
        window.center()
    }

    required init?(coder: NSCoder) { fatalError() }

    func go(to step: Step, tryAnyway: (() -> Void)?) {
        state.tryAnyway = tryAnyway
        withAnimation { state.step = step }
    }

    func windowWillClose(_ notification: Notification) {
        state.stopPolling()
        onClose?()
    }
}

final class OnboardingState: ObservableObject {
    @Published var step: OnboardingWindowController.Step { didSet { updatePolling() } }
    @Published var granted = CaptureService.hasScreenPermission
    var tryAnyway: (() -> Void)?
    var close: () -> Void = {}
    private var timer: Timer?

    init(step: OnboardingWindowController.Step, tryAnyway: (() -> Void)?) {
        self.step = step
        self.tryAnyway = tryAnyway
        updatePolling()
    }

    /// macOS often reports a new grant only after a relaunch, so polling is a bonus, not the main path.
    private func updatePolling() {
        guard step == .permission else { return stopPolling() }
        guard timer == nil else { return }
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            let g = CaptureService.hasScreenPermission
            if self?.granted != g { withAnimation { self?.granted = g } }
        }
    }

    func stopPolling() {
        timer?.invalidate()
        timer = nil
    }
}

private struct OnboardingView: View {
    @ObservedObject var state: OnboardingState

    var body: some View {
        VStack(spacing: 0) {
            ZStack {
                switch state.step {
                case .welcome: WelcomeStep()
                case .permission: PermissionStep(state: state)
                case .shortcuts: ShortcutsStep()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .transition(.asymmetric(insertion: .move(edge: .trailing).combined(with: .opacity),
                                    removal: .move(edge: .leading).combined(with: .opacity)))
            .id(state.step)

            HStack {
                HStack(spacing: 6) {
                    ForEach(0..<3) { i in
                        Capsule()
                            .fill(i == state.step.rawValue ? Color.accentColor : Color.primary.opacity(0.18))
                            .frame(width: i == state.step.rawValue ? 18 : 6, height: 6)
                    }
                }
                Spacer()
                if state.step != .welcome {
                    Button(L("Back")) { move(-1) }
                        .controlSize(.large)
                }
                Button(state.step == .shortcuts ? L("Get Started") : L("Continue")) {
                    if state.step == .shortcuts { state.close() } else { move(1) }
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 28)
            .padding(.bottom, 24)
        }
        .frame(width: 560, height: 460)
        .background(VisualEffect(material: .underWindowBackground))
        .animation(.spring(duration: 0.35), value: state.step)
    }

    private func move(_ d: Int) {
        if let s = OnboardingWindowController.Step(rawValue: state.step.rawValue + d) { state.step = s }
    }
}

private struct AppMark: View {
    var size: CGFloat = 96

    var body: some View {
        if let icon = NSApp.applicationIconImage {
            Image(nsImage: icon).resizable().frame(width: size, height: size)
        }
    }
}

private struct WelcomeStep: View {
    var body: some View {
        VStack(spacing: 14) {
            AppMark(size: 104)
                .shadow(color: .black.opacity(0.2), radius: 16, y: 8)
                .padding(.top, 36)
            Text(L("Welcome to Kirei")).font(.system(size: 28, weight: .bold))
            Text(L("Beautiful, share-ready screenshots in one shortcut."))
                .font(.title3)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            HStack(spacing: 22) {
                feature("camera.viewfinder", L("Capture"))
                feature("paintbrush.pointed", L("Style"))
                feature("eye.slash", L("Redact"))
                feature("paperplane", L("Share"))
            }
            .padding(.top, 18)
            Spacer()
        }
        .padding(.horizontal, 40)
    }

    private func feature(_ symbol: String, _ title: String) -> some View {
        VStack(spacing: 8) {
            Image(systemName: symbol)
                .font(.system(size: 20, weight: .medium))
                .foregroundStyle(Color.accentColor)
                .frame(width: 48, height: 48)
                .background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 12))
            Text(title).font(.callout.weight(.medium))
        }
    }
}

private struct PermissionStep: View {
    @ObservedObject var state: OnboardingState

    var body: some View {
        VStack(spacing: 16) {
            ZStack {
                RoundedRectangle(cornerRadius: 20)
                    .fill(state.granted ? Color.green.opacity(0.15) : Color.accentColor.opacity(0.12))
                    .frame(width: 84, height: 84)
                Image(systemName: state.granted ? "checkmark.shield.fill" : "rectangle.dashed.badge.record")
                    .font(.system(size: 36, weight: .medium))
                    .foregroundStyle(state.granted ? Color.green : Color.accentColor)
                    .contentTransition(.symbolEffect(.replace))
            }
            .padding(.top, 40)

            Text(state.granted ? L("Screen Recording is on") : L("Allow Screen Recording"))
                .font(.system(size: 24, weight: .bold))
            Text(state.granted
                 ? L("Kirei can capture your screen. You're all set.")
                 : L("Kirei needs this permission to capture the screen. Nothing leaves your Mac."))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 400)

            if !state.granted {
                VStack(alignment: .leading, spacing: 10) {
                    stepRow(1, L("Open System Settings › Privacy & Security › Screen & System Audio Recording."))
                    stepRow(2, L("Turn on Kirei. If it's already on, remove it with – and add it again."))
                    stepRow(3, L("Restart Kirei so macOS applies the permission."))
                }
                .padding(16)
                .background(Color.primary.opacity(0.04), in: RoundedRectangle(cornerRadius: 12))
                .frame(maxWidth: 440)

                HStack(spacing: 10) {
                    Button(L("Open Settings")) {
                        CGRequestScreenCaptureAccess()
                        NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")!)
                    }
                    Button(L("Restart Kirei")) { AppDelegate.relaunch() }
                    if let tryAnyway = state.tryAnyway {
                        Button(L("Try Anyway")) {
                            state.close()
                            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: tryAnyway)
                        }
                    }
                }
            }
            Spacer()
        }
        .padding(.horizontal, 40)
    }

    private func stepRow(_ n: Int, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text("\(n)")
                .font(.caption.weight(.bold))
                .foregroundStyle(.white)
                .frame(width: 18, height: 18)
                .background(Color.accentColor, in: Circle())
            Text(text).font(.callout).fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct ShortcutsStep: View {
    @ObservedObject var settings = AppSettings.shared

    var body: some View {
        VStack(spacing: 14) {
            Text(L("Your shortcuts")).font(.system(size: 24, weight: .bold)).padding(.top, 36)
            Text(L("Kirei lives in the menu bar. Use these from any app:"))
                .foregroundStyle(.secondary)
            VStack(spacing: 0) {
                ForEach(CaptureAction.allCases) { action in
                    HStack {
                        Label(action.title, systemImage: action.symbol)
                        Spacer()
                        Text(shortcut(action))
                            .font(.system(.callout, design: .rounded).weight(.semibold))
                            .padding(.horizontal, 8).padding(.vertical, 3)
                            .background(Color.primary.opacity(0.08), in: RoundedRectangle(cornerRadius: 6))
                    }
                    .padding(.vertical, 8)
                    .padding(.horizontal, 14)
                    if action != CaptureAction.allCases.last { Divider().padding(.leading, 14) }
                }
            }
            .background(Color.primary.opacity(0.04), in: RoundedRectangle(cornerRadius: 12))
            .frame(maxWidth: 440)
            Label(L("During an area capture, press Space to switch to window mode."), systemImage: "lightbulb")
                .font(.callout)
                .foregroundStyle(.secondary)
            Spacer()
        }
        .padding(.horizontal, 40)
    }

    private func shortcut(_ a: CaptureAction) -> String {
        guard let hk = settings.hotKeys[a], hk.keyCode != 0xFFFF else { return "—" }
        return hk.display
    }
}
