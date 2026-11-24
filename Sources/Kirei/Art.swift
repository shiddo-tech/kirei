import CoreGraphics
import Foundation

enum ArtKind: String, Codable, CaseIterable {
    case mesh, linear, radial
}

/// Procedural background. Mesh reads `palette` as [base, blobs…]; linear and radial as OKLab ramps.
struct Art: Codable, Hashable {
    var kind: ArtKind
    var palette: [RGBA]
    var seed: Int

    init(_ kind: ArtKind, _ hexes: [String], seed: Int) {
        self.kind = kind
        palette = hexes.map { RGBA(hex: $0) }
        self.seed = seed
    }

    /// Same palette, different shapes.
    func reseeded() -> Art {
        var a = self
        repeat { a.seed = Int.random(in: 1...9999) } while a.seed == seed
        return a
    }

    func isVariant(of other: Art) -> Bool { kind == other.kind && palette == other.palette }

    static let presets: [Art] = [
        Art(.mesh, ["#1A0B2E", "#FF6B8B", "#FFB36B", "#7C4DFF", "#FFD9C2"], seed: 3),
        Art(.mesh, ["#04122E", "#1F5BFF", "#00D1C1", "#5B3BFF", "#7FE7FF"], seed: 8),
        Art(.mesh, ["#E9E4FF", "#B9A6FF", "#FFB8D9", "#9CC8FF", "#FFE6D1"], seed: 5),
        Art(.mesh, ["#07060F", "#4B1FFF", "#E13CFF", "#0E5BFF", "#FF4D8D"], seed: 12),
        Art(.mesh, ["#062820", "#1DB98A", "#3AD1FF", "#C4F27A", "#0E7C6B"], seed: 4),
        Art(.mesh, ["#120606", "#FF4A1C", "#FF9A3C", "#B3123A", "#5A1150"], seed: 9),
        Art(.mesh, ["#1B1D24", "#4A4F63", "#8A90A8", "#D9D4C7", "#2E3242"], seed: 6),
        Art(.mesh, ["#DCEBFF", "#8FD3FF", "#B9A4FF", "#FFC0E0", "#FFF1C9"], seed: 2),
        Art(.mesh, ["#FFE9EC", "#FF9EB5", "#FFC9A8", "#E58CFF", "#FFF4E8"], seed: 7),
        Art(.mesh, ["#0B0A12", "#2A2160", "#C88A3B", "#6A3BB5", "#F2C46D"], seed: 11),
        Art(.mesh, ["#EFFFF7", "#7FE8C4", "#7CC8FF", "#D7F7A6", "#B6F0E6"], seed: 14),
        Art(.mesh, ["#F4EDE2", "#E2C8A8", "#C9A38A", "#F7D9C4", "#B8C2D6"], seed: 10),
        Art(.linear, ["#2B1B5A", "#A43E8C", "#FF7A6B", "#FFC38A"], seed: 3),
        Art(.linear, ["#050816", "#102A6B", "#2F6BFF", "#8FD0FF"], seed: 5),
        Art(.linear, ["#FFE3D1", "#FFB5A0", "#F08BA8"], seed: 2),
        Art(.linear, ["#04161A", "#0E5C55", "#3FD1A0", "#D6FF9E"], seed: 8),
        Art(.linear, ["#F3EEFF", "#C8B6FF", "#8F7BFF"], seed: 4),
        Art(.linear, ["#0D0E12", "#23252E", "#4A4E5C"], seed: 6),
        Art(.linear, ["#FFF6D6", "#FFD36E", "#FF8A4C"], seed: 9),
        Art(.linear, ["#F2F8FF", "#BFD9FF", "#7FA6E8", "#3A5BA8"], seed: 1),
        Art(.linear, ["#12040B", "#4A0E2C", "#A8325E", "#F08A7E"], seed: 7),
        Art(.radial, ["#C9B8FF", "#6B4BFF", "#22145C", "#07060F"], seed: 3),
        Art(.radial, ["#FFE2B0", "#FF9A4A", "#7A2A1A", "#120806"], seed: 6),
        Art(.radial, ["#CDEBFF", "#3E8BFF", "#0C2766", "#03081A"], seed: 8),
        Art(.radial, ["#FFFFFF", "#EEF0F6", "#CDD3E1"], seed: 2),
        Art(.radial, ["#CFFFE6", "#2BD49A", "#0B4A3A", "#021512"], seed: 5),
        Art(.radial, ["#FFE0EC", "#FF7FAE", "#7A1E5B", "#1A0614"], seed: 9),
    ]
}

/// Renders art per pixel and caches it. Fields are smooth, so a capped size upscales without visible loss.
final class ArtCache {
    static let shared = ArtCache()
    static let thumbnails = ArtCache(limit: 96)
    private let limit: Int
    private var cache: [Key: CGImage] = [:]
    private var order: [Key] = []
    private let lock = NSLock()

    init(limit: Int = 8) { self.limit = limit }

    private struct Key: Hashable { let art: Art; let w: Int; let h: Int }

    func image(_ art: Art, for size: CGSize) -> CGImage? {
        guard size.width >= 1, size.height >= 1 else { return nil }
        // Aspect snapped to 0.05 steps so dragging padding reuses the same render.
        let aspect = max(0.3, min(4, (size.width / size.height * 20).rounded() / 20))
        let needed = max(size.width, size.height)
        let long: CGFloat = [256, 512, 1024].first { $0 >= needed } ?? 1600
        let w = Int(aspect >= 1 ? long : long * aspect), h = Int(aspect >= 1 ? long / aspect : long)
        let key = Key(art: art, w: max(1, w), h: max(1, h))
        lock.lock()
        if let c = cache[key] { lock.unlock(); return c }
        lock.unlock()
        guard let img = ArtField.render(art, width: key.w, height: key.h) else { return nil }
        lock.lock()
        cache[key] = img
        order.append(key)
        if order.count > limit { cache[order.removeFirst()] = nil }
        lock.unlock()
        return img
    }
}

private enum ArtField {
    typealias V3 = SIMD3<Double>
    /// `px` is one pixel in v units, for detail that must not alias at small sizes.
    typealias Field = (_ u: Double, _ v: Double, _ aspect: Double, _ px: Double) -> V3

    static func render(_ art: Art, width w: Int, height h: Int) -> CGImage? {
        guard !art.palette.isEmpty, let ctx = Renderer.makeContext(width: w, height: h), let data = ctx.data else { return nil }
        let f = field(art)
        let buf = data.bindMemory(to: UInt8.self, capacity: ctx.bytesPerRow * h)
        let stride = ctx.bytesPerRow
        let aspect = Double(w) / Double(h)
        let px = 1 / Double(max(1, h - 1))
        DispatchQueue.concurrentPerform(iterations: h) { y in
            let v = h > 1 ? Double(y) / Double(h - 1) : 0
            for x in 0..<w {
                let c = f(w > 1 ? Double(x) / Double(w - 1) : 0, v, aspect, px)
                let o = y * stride + x * 4
                buf[o] = q(c.x); buf[o + 1] = q(c.y); buf[o + 2] = q(c.z); buf[o + 3] = 255
            }
        }
        return ctx.makeImage()
    }

    private static func q(_ d: Double) -> UInt8 { UInt8(max(0, min(255, d * 255 + 0.5))) }

    private static func field(_ art: Art) -> Field {
        let p = art.palette.map { V3($0.r, $0.g, $0.b) }
        let s = art.seed
        switch art.kind {
        case .mesh: return mesh(p, s)
        case .linear: return linear(p, s)
        case .radial: return radial(p, s)
        }
    }

    // MARK: Noise

    private static func hash(_ x: Int, _ y: Int, _ s: Int) -> Double {
        var h = UInt32(truncatingIfNeeded: x &* 374761393 &+ y &* 668265263 &+ s &* 982451653)
        h = (h ^ (h >> 13)) &* 1274126177
        h ^= h >> 16
        return Double(h) / Double(UInt32.max)
    }

    private static func noise(_ x: Double, _ y: Double, _ s: Int) -> Double {
        let fx = floor(x), fy = floor(y)
        let xi = Int(fx), yi = Int(fy)
        let xf = x - fx, yf = y - fy
        let u = xf * xf * xf * (xf * (xf * 6 - 15) + 10), v = yf * yf * yf * (yf * (yf * 6 - 15) + 10)
        let a = hash(xi, yi, s), b = hash(xi + 1, yi, s), c = hash(xi, yi + 1, s), d = hash(xi + 1, yi + 1, s)
        let top = a + (b - a) * u
        return top + (c + (d - c) * u - top) * v
    }

    private static func fbm(_ x: Double, _ y: Double, _ s: Int, _ octaves: Int = 3) -> Double {
        var t = 0.0, amp = 0.5, freq = 1.0
        for i in 0..<octaves {
            t += amp * noise(x * freq, y * freq, s + i * 17)
            freq *= 2.03
            amp *= 0.5
        }
        return t / (1 - pow(0.5, Double(octaves)))
    }

    private static func sstep(_ e0: Double, _ e1: Double, _ x: Double) -> Double {
        let t = max(0, min(1, (x - e0) / (e1 - e0)))
        return t * t * (3 - 2 * t)
    }

    private static func mix(_ a: V3, _ b: V3, _ t: Double) -> V3 { a + (b - a) * t }

    // MARK: Fields

    private static func toLinear(_ c: Double) -> Double { c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4) }
    private static func toSRGB(_ c: Double) -> Double { c <= 0.0031308 ? c * 12.92 : 1.055 * pow(max(0, c), 1 / 2.4) - 0.055 }

    private static func oklab(_ c: V3) -> V3 {
        let r = toLinear(c.x), g = toLinear(c.y), b = toLinear(c.z)
        let l = cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
        let m = cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
        let s = cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
        return V3(0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
                  1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
                  0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s)
    }

    private static func fromOklab(_ c: V3) -> V3 {
        let l = pow(c.x + 0.3963377774 * c.y + 0.2158037573 * c.z, 3)
        let m = pow(c.x - 0.1055613458 * c.y - 0.0638541728 * c.z, 3)
        let s = pow(c.x - 0.0894841775 * c.y - 1.2914855480 * c.z, 3)
        return V3(toSRGB(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
                  toSRGB(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
                  toSRGB(-0.0041960771 * l - 0.7034226162 * m + 1.7076147010 * s))
    }

    private static func oklabRamp(_ labs: [V3], _ t: Double) -> V3 {
        guard labs.count > 1 else { return fromOklab(labs[0]) }
        let t = max(0, min(0.9999, t)) * Double(labs.count - 1)
        let i = Int(t)
        return fromOklab(mix(labs[i], labs[i + 1], sstep(0, 1, t - Double(i))))
    }

    /// Multi-stop gradient blended in OKLab, bent slightly by low-frequency noise so the bands
    /// don't read as a flat CSS ramp.
    private static func linear(_ p: [V3], _ s: Int) -> Field {
        let labs = p.map(oklab)
        let ang = (0.15 + hash(s, 3, 21) * 0.5) * .pi
        let dx = cos(ang), dy = sin(ang)
        return { u, v, a, _ in
            let x = (u - 0.5) * a, y = v - 0.5
            let span = (abs(dx) * a + abs(dy)) / 2
            let bend = (fbm(u * 0.9, v * 0.9, s + 60, 2) - 0.5) * 0.22
            return oklabRamp(labs, (x * dx + y * dy) / (2 * span) + 0.5 + bend)
        }
    }

    /// Light pooling from an off-canvas point: an OKLab ramp over distance, slightly elliptical.
    private static func radial(_ p: [V3], _ s: Int) -> Field {
        let labs = p.map(oklab)
        let cx = hash(s, 1, 31) < 0.5 ? -0.1 + hash(s, 2, 31) * 0.4 : 0.7 + hash(s, 2, 31) * 0.4
        let cy = -0.15 + hash(s, 4, 31) * 0.35
        return { u, v, a, _ in
            let dx = (u - cx) * a, dy = (v - cy) * 1.25
            let d = sqrt(dx * dx + dy * dy) / (0.95 * max(1, a))
            let bend = (fbm(u * 1.1, v * 1.1, s + 61, 2) - 0.5) * 0.12
            return oklabRamp(labs, d + bend)
        }
    }

    /// Mesh gradient: soft color blobs over a base, blended in OKLab so mixes stay clean instead of
    /// going muddy, with a gentle warp so the blobs read as flowing light rather than circles.
    private static func mesh(_ p: [V3], _ s: Int) -> Field {
        let labs = p.map(oklab)
        let base = labs[0], blobs = labs.count > 1 ? Array(labs.dropFirst()) : [labs[0]]
        let centers = blobs.indices.map { i in
            SIMD2(0.05 + 0.9 * hash(s, i, 41), 0.05 + 0.9 * hash(s, i, 42))
        }
        let radii = blobs.indices.map { i in 0.38 + 0.22 * hash(s, i, 43) }
        return { u, v, a, _ in
            let wx = (fbm(u * 1.2, v * 1.2, s + 70) - 0.5) * 0.4
            let wy = (fbm(u * 1.2 + 3.7, v * 1.2 + 1.9, s + 71) - 0.5) * 0.4
            let x = u + wx, y = v + wy
            var sum = base * 0.18, total = 0.18
            for i in blobs.indices {
                let dx = (x - centers[i].x) * a, dy = y - centers[i].y
                let w = exp(-(dx * dx + dy * dy) / (radii[i] * radii[i]) * 1.6)
                sum += blobs[i] * w
                total += w
            }
            return fromOklab(sum / total)
        }
    }


}
