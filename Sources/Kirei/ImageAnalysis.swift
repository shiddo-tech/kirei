import AppKit
import Vision

/// RGBA8 pixel access for analysis.
struct PixelBuffer {
    let width: Int
    let height: Int
    let data: [UInt8]

    init?(_ image: CGImage) {
        let width = image.width, height = image.height
        self.width = width; self.height = height
        var buf = [UInt8](repeating: 0, count: width * height * 4)
        let ok = buf.withUnsafeMutableBytes { ptr -> Bool in
            guard let ctx = CGContext(data: ptr.baseAddress, width: width, height: height, bitsPerComponent: 8,
                                      bytesPerRow: width * 4, space: Renderer.colorSpace,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            ctx.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard ok else { return nil }
        data = buf
    }

    /// (x, y) with y measured from the top.
    @inline(__always) func px(_ x: Int, _ y: Int) -> (Int, Int, Int, Int) {
        let i = (y * width + x) * 4
        return (Int(data[i]), Int(data[i + 1]), Int(data[i + 2]), Int(data[i + 3]))
    }
}

enum ImageAnalysis {
    /// Finds the region inside a uniform-colored border so the content can be re-centered.
    static func balancedRect(_ image: CGImage) -> CGRect {
        let full = CGRect(x: 0, y: 0, width: image.width, height: image.height)
        guard image.width > 20, image.height > 20, let pb = PixelBuffer(image) else { return full }
        let ref = pb.px(0, 0)
        let tol = 8
        func same(_ x: Int, _ y: Int) -> Bool {
            let p = pb.px(x, y)
            return abs(p.0 - ref.0) <= tol && abs(p.1 - ref.1) <= tol && abs(p.2 - ref.2) <= tol && abs(p.3 - ref.3) <= tol
        }
        let w = pb.width, h = pb.height
        var top = 0
        rows: while top < h - 1 {
            for x in 0..<w where !same(x, top) { break rows }
            top += 1
        }
        var bottom = h - 1
        rows2: while bottom > top {
            for x in 0..<w where !same(x, bottom) { break rows2 }
            bottom -= 1
        }
        var left = 0
        cols: while left < w - 1 {
            for y in top...bottom where !same(left, y) { break cols }
            left += 1
        }
        var right = w - 1
        cols2: while right > left {
            for y in top...bottom where !same(right, y) { break cols2 }
            right -= 1
        }
        let r = CGRect(x: left, y: top, width: right - left + 1, height: bottom - top + 1)
        // Ignore degenerate results (blank image or nothing to trim).
        if r.width < 16 || r.height < 16 { return full }
        return r
    }

    /// Dominant opaque color along the crop's edges, used to fill the inset.
    static func edgeColor(_ image: CGImage, crop: CGRect) -> RGBA {
        guard let pb = PixelBuffer(image) else { return .white }
        let c = crop.integral.intersection(CGRect(x: 0, y: 0, width: pb.width, height: pb.height))
        guard !c.isEmpty else { return .white }
        var counts: [Int: Int] = [:]
        func sample(_ x: Int, _ y: Int) {
            let p = pb.px(x, y)
            guard p.3 > 250 else { return }
            let key = (p.0 >> 3) << 10 | (p.1 >> 3) << 5 | (p.2 >> 3)
            counts[key, default: 0] += 1
        }
        let x0 = Int(c.minX), x1 = Int(c.maxX) - 1, y0 = Int(c.minY), y1 = Int(c.maxY) - 1
        let inset = min(3, (x1 - x0) / 4, (y1 - y0) / 4)
        let stepX = max(1, (x1 - x0) / 200), stepY = max(1, (y1 - y0) / 200)
        for x in stride(from: x0, through: x1, by: stepX) { sample(x, y0 + inset); sample(x, y1 - inset) }
        for y in stride(from: y0, through: y1, by: stepY) { sample(x0 + inset, y); sample(x1 - inset, y) }
        guard let best = counts.max(by: { $0.value < $1.value })?.key else { return .white }
        return RGBA(r: Double((best >> 10) & 31) / 31, g: Double((best >> 5) & 31) / 31, b: Double(best & 31) / 31)
    }

    // MARK: - OCR

    static func recognizeText(_ image: CGImage) -> [VNRecognizedTextObservation] {
        let req = VNRecognizeTextRequest()
        req.recognitionLevel = .accurate
        req.usesLanguageCorrection = true
        req.recognitionLanguages = ["pt-BR", "en-US", "es-ES"]
        if #available(macOS 13.0, *) { req.automaticallyDetectsLanguage = true }
        let handler = VNImageRequestHandler(cgImage: image, options: [:])
        try? handler.perform([req])
        return req.results ?? []
    }

    static func text(in image: CGImage) -> String {
        let obs = recognizeText(image)
        // Group observations into lines by vertical position.
        let sorted = obs.sorted { a, b in
            let ay = a.boundingBox.midY, by = b.boundingBox.midY
            if abs(ay - by) > min(a.boundingBox.height, b.boundingBox.height) * 0.5 { return ay > by }
            return a.boundingBox.minX < b.boundingBox.minX
        }
        var lines: [String] = []
        var current = ""
        var lastY: CGFloat?
        var lastH: CGFloat = 0
        for o in sorted {
            guard let s = o.topCandidates(1).first?.string else { continue }
            let y = o.boundingBox.midY
            if let ly = lastY, abs(ly - y) <= min(lastH, o.boundingBox.height) * 0.5 {
                current += "  " + s
            } else {
                if lastY != nil { lines.append(current) }
                current = s
            }
            lastY = y; lastH = o.boundingBox.height
        }
        if lastY != nil { lines.append(current) }
        return lines.joined(separator: "\n")
    }

    // MARK: - Sensitive data

    private static let patterns: [NSRegularExpression] = [
        // E-mail
        #"[A-Z0-9._%+\-]+@[A-Z0-9.\-]+\.[A-Z]{2,}"#,
        // IPv4
        #"\b(?:\d{1,3}\.){3}\d{1,3}\b"#,
        // CPF / CNPJ
        #"\b\d{3}\.?\d{3}\.?\d{3}-?\d{2}\b"#,
        #"\b\d{2}\.?\d{3}\.?\d{3}/?\d{4}-?\d{2}\b"#,
        // Common API keys / tokens
        #"\b(?:sk|pk|rk)[-_](?:live_|test_|proj-|ant-)?[A-Za-z0-9_\-]{16,}"#,
        #"\bAKIA[0-9A-Z]{16}\b"#,
        #"\bgh[pousr]_[A-Za-z0-9]{20,}\b"#,
        #"\bxox[abprs]-[A-Za-z0-9\-]{10,}"#,
        #"\bAIza[0-9A-Za-z_\-]{30,}"#,
        #"\beyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{4,}"#,
        // Long opaque secrets (letters + digits, 32+ chars)
        #"\b(?=[A-Za-z0-9_\-]*\d)(?=[A-Za-z0-9_\-]*[A-Za-z])[A-Za-z0-9_\-]{32,}\b"#,
    ].map { try! NSRegularExpression(pattern: $0, options: [.caseInsensitive]) }

    private static let cardPattern = try! NSRegularExpression(pattern: #"\b(?:\d[ \-]?){12,18}\d\b"#)
    private static let detector = try! NSDataDetector(types: NSTextCheckingResult.CheckingType.phoneNumber.rawValue)

    private static func luhn(_ s: String) -> Bool {
        let digits = s.compactMap(\.wholeNumberValue)
        guard (13...19).contains(digits.count) else { return false }
        var sum = 0
        for (i, d) in digits.reversed().enumerated() {
            if i % 2 == 1 { let x = d * 2; sum += x > 9 ? x - 9 : x } else { sum += d }
        }
        return sum % 10 == 0
    }

    /// Returns rects (source pixels, y-down) covering sensitive text.
    static func sensitiveRects(_ image: CGImage) -> [CGRect] {
        let W = CGFloat(image.width), H = CGFloat(image.height)
        var rects: [CGRect] = []
        for obs in recognizeText(image) {
            guard let cand = obs.topCandidates(1).first else { continue }
            let s = cand.string
            let ns = s as NSString
            let whole = NSRange(location: 0, length: ns.length)
            var ranges: [NSRange] = []
            for re in patterns { ranges += re.matches(in: s, range: whole).map(\.range) }
            ranges += cardPattern.matches(in: s, range: whole).map(\.range).filter { luhn(ns.substring(with: $0)) }
            ranges += detector.matches(in: s, range: whole).map(\.range).filter { r in
                ns.substring(with: r).filter(\.isNumber).count >= 8
            }
            for r in ranges {
                guard let range = Range(r, in: s),
                      let box = try? cand.boundingBox(for: range)?.boundingBox else { continue }
                var rect = CGRect(x: box.minX * W, y: (1 - box.maxY) * H, width: box.width * W, height: box.height * H)
                let pad = rect.height * 0.2
                rect = rect.insetBy(dx: -pad, dy: -pad)
                rects.append(rect)
            }
        }
        return rects
    }
}
