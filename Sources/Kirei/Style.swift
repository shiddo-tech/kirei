import AppKit
import SwiftUI

struct RGBA: Codable, Hashable {
    var r: Double
    var g: Double
    var b: Double
    var a: Double = 1

    init(r: Double, g: Double, b: Double, a: Double = 1) {
        self.r = r; self.g = g; self.b = b; self.a = a
    }

    init(hex: String, alpha: Double = 1) {
        var s = hex.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("#") { s.removeFirst() }
        let v = UInt64(s, radix: 16) ?? 0
        r = Double((v >> 16) & 0xFF) / 255
        g = Double((v >> 8) & 0xFF) / 255
        b = Double(v & 0xFF) / 255
        a = alpha
    }

    init(nsColor: NSColor) {
        let c = nsColor.usingColorSpace(.sRGB) ?? .black
        r = Double(c.redComponent); g = Double(c.greenComponent)
        b = Double(c.blueComponent); a = Double(c.alphaComponent)
    }

    var cgColor: CGColor { CGColor(srgbRed: r, green: g, blue: b, alpha: a) }
    var nsColor: NSColor { NSColor(srgbRed: r, green: g, blue: b, alpha: a) }
    var color: Color { Color(.sRGB, red: r, green: g, blue: b, opacity: a) }

    func with(alpha: Double) -> RGBA { RGBA(r: r, g: g, b: b, a: alpha) }

    static let white = RGBA(r: 1, g: 1, b: 1)
    static let black = RGBA(r: 0, g: 0, b: 0)
}

enum Background: Codable, Hashable {
    case none
    case solid(RGBA)
    case gradient([RGBA], angle: Double)
    case image(path: String)
    /// Base color plus up to three soft color blooms. No longer offered; kept so saved presets still load.
    case mesh([RGBA])
    case art(Art)

    /// Smooth color ramps band in 8-bit exports unless a little noise dithers them.
    var isSmooth: Bool {
        switch self {
        case .gradient, .mesh, .art: return true
        default: return false
        }
    }
}

enum WindowFrame: String, Codable, CaseIterable, Identifiable {
    case none, macLight, macDark, browserLight, browserDark

    var id: String { rawValue }

    var title: String {
        switch self {
        case .none: return L("None")
        case .macLight: return L("macOS · Light")
        case .macDark: return L("macOS · Dark")
        case .browserLight: return L("Browser · Light")
        case .browserDark: return L("Browser · Dark")
        }
    }

    var isDark: Bool { self == .macDark || self == .browserDark }
    var isBrowser: Bool { self == .browserLight || self == .browserDark }
}

enum AspectRatio: String, Codable, CaseIterable, Identifiable {
    case auto, square, r4x3, r3x2, r16x9, r16x10, r21x9, r9x16, r4x5

    var id: String { rawValue }

    var label: String {
        switch self {
        case .auto: return L("Automatic")
        case .square: return L("1:1 · Square")
        case .r4x3: return "4:3"
        case .r3x2: return "3:2"
        case .r16x9: return "16:9 · X / YouTube"
        case .r16x10: return "16:10"
        case .r21x9: return "21:9 · Ultrawide"
        case .r9x16: return L("9:16 · Stories")
        case .r4x5: return "4:5 · Instagram"
        }
    }

    var value: CGFloat? {
        switch self {
        case .auto: return nil
        case .square: return 1
        case .r4x3: return 4.0 / 3
        case .r3x2: return 3.0 / 2
        case .r16x9: return 16.0 / 9
        case .r16x10: return 16.0 / 10
        case .r21x9: return 21.0 / 9
        case .r9x16: return 9.0 / 16
        case .r4x5: return 4.0 / 5
        }
    }
}

struct Style: Codable, Hashable {
    var background: Background = .solid(RGBA(hex: "#F2F2F7"))
    /// Fractions of the screenshot's larger side.
    var padding: Double = 0.08
    var inset: Double = 0
    var cornerRadius: Double = 0.012
    /// 0...1
    var shadow: Double = 0.5
    var aspect: AspectRatio = .auto
    /// 0...8, row-major 3x3 grid. 4 = center.
    var alignment: Int = 4
    var autoBalance: Bool = true
    var frame: WindowFrame = .none
    /// 0...1 film grain over the background.
    var grain: Double = 0
    var watermark: String = ""
    /// 0...1 blur for image backgrounds.
    var backgroundBlur: Double = 0
    /// Thin inner edge around the screenshot.
    var hairline: Bool = true

    static let `default` = Style()

    init() {}

    private enum CodingKeys: String, CodingKey {
        case background, padding, inset, cornerRadius, shadow, aspect, alignment, autoBalance, frame, grain, watermark, backgroundBlur, hairline
    }

    // Presets saved by older versions lack the newer keys; synthesized decoding would drop them.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let d = Style()
        background = (try? c.decodeIfPresent(Background.self, forKey: .background)) ?? d.background
        padding = try c.decodeIfPresent(Double.self, forKey: .padding) ?? d.padding
        inset = try c.decodeIfPresent(Double.self, forKey: .inset) ?? d.inset
        cornerRadius = try c.decodeIfPresent(Double.self, forKey: .cornerRadius) ?? d.cornerRadius
        shadow = try c.decodeIfPresent(Double.self, forKey: .shadow) ?? d.shadow
        aspect = (try? c.decodeIfPresent(AspectRatio.self, forKey: .aspect)) ?? d.aspect
        alignment = try c.decodeIfPresent(Int.self, forKey: .alignment) ?? d.alignment
        autoBalance = try c.decodeIfPresent(Bool.self, forKey: .autoBalance) ?? d.autoBalance
        frame = (try? c.decodeIfPresent(WindowFrame.self, forKey: .frame)) ?? d.frame
        grain = try c.decodeIfPresent(Double.self, forKey: .grain) ?? d.grain
        watermark = try c.decodeIfPresent(String.self, forKey: .watermark) ?? d.watermark
        backgroundBlur = try c.decodeIfPresent(Double.self, forKey: .backgroundBlur) ?? d.backgroundBlur
        hairline = try c.decodeIfPresent(Bool.self, forKey: .hairline) ?? d.hairline
    }
}

enum Gradients {
    static let presets: [[RGBA]] = [
        ["#FF9A8B", "#FF6A88", "#FF99AC"],
        ["#4158D0", "#C850C0", "#FFCC70"],
        ["#0093E9", "#80D0C7"],
        ["#8EC5FC", "#E0C3FC"],
        ["#C9D6FF", "#E2E2E2"],
        ["#FBAB7E", "#F7CE68"],
        ["#F6D5F7", "#FBE9D7"],
        ["#21D4FD", "#B721FF"],
        ["#08AEEA", "#2AF598"],
        ["#FEE140", "#FA709A"],
        ["#134E5E", "#71B280"],
        ["#FF3CAC", "#784BA0", "#2B86C5"],
        ["#667EEA", "#764BA2"],
        ["#F5F7FA", "#C3CFE2"],
        ["#0F2027", "#203A43", "#2C5364"],
        ["#232526", "#414345"],
    ].map { $0.map { RGBA(hex: $0) } }

    static let meshes: [[RGBA]] = [
        ["#1B1035", "#7B2FF7", "#F107A3", "#FF8A3D"],
        ["#0B1D3A", "#1E6FFF", "#00C2A8", "#7DE2FC"],
        ["#FFF4E6", "#FFB38A", "#FF6F91", "#FFD56B"],
        ["#0F2A1D", "#2E8B57", "#B8E986", "#3E8C63"],
        ["#F3F0FF", "#B9A7FF", "#8EC5FC", "#FBC2EB"],
        ["#101014", "#3A3A55", "#5A4FCF", "#1F8EF1"],
        ["#2B0F0F", "#E94560", "#F2A365", "#7A1F3D"],
        ["#E9F7F2", "#7ED6B9", "#4FB3D9", "#C9F2D9"],
    ].map { $0.map { RGBA(hex: $0) } }

    static let solids: [RGBA] = [
        "#FFFFFF", "#F2F2F7", "#D1D1D6", "#1C1C1E", "#000000",
        "#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#00C7BE",
        "#007AFF", "#5856D6", "#AF52DE", "#FF2D55",
    ].map { RGBA(hex: $0) }
}

/// Colors offered for annotations.
enum Palette {
    static let annotation: [RGBA] = [
        "#FF3B30", "#FF9500", "#FFCC00", "#34C759", "#007AFF",
        "#AF52DE", "#FFFFFF", "#000000",
    ].map { RGBA(hex: $0) }
}
