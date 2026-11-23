import AppKit

enum Tool: String, CaseIterable, Identifiable {
    case select, arrow, rect, ellipse, highlight, text, counter, pixelate, blur, crop

    var id: String { rawValue }

    var symbol: String {
        switch self {
        case .select: return "cursorarrow"
        case .arrow: return "arrow.up.right"
        case .rect: return "rectangle"
        case .ellipse: return "circle"
        case .highlight: return "highlighter"
        case .text: return "textformat"
        case .counter: return "1.circle"
        case .pixelate: return "square.grid.3x3.fill"
        case .blur: return "drop.halffull"
        case .crop: return "crop"
        }
    }

    var title: String {
        switch self {
        case .select: return L("Select (V)")
        case .arrow: return L("Arrow (A)")
        case .rect: return L("Rectangle (R)")
        case .ellipse: return L("Ellipse (O)")
        case .highlight: return L("Highlighter (H)")
        case .text: return L("Text (T)")
        case .counter: return L("Counter (N)")
        case .pixelate: return L("Pixelate (P)")
        case .blur: return L("Blur (B)")
        case .crop: return L("Crop (C)")
        }
    }

    var shortcut: Character {
        switch self {
        case .select: return "v"
        case .arrow: return "a"
        case .rect: return "r"
        case .ellipse: return "o"
        case .highlight: return "h"
        case .text: return "t"
        case .counter: return "n"
        case .pixelate: return "p"
        case .blur: return "b"
        case .crop: return "c"
        }
    }

    var annotationKind: AnnotationKind? {
        switch self {
        case .arrow: return .arrow
        case .rect: return .rect
        case .ellipse: return .ellipse
        case .highlight: return .highlight
        case .text: return .text
        case .counter: return .counter
        case .pixelate: return .pixelate
        case .blur: return .blur
        case .select, .crop: return nil
        }
    }
}

enum AnnotationKind: String, Codable {
    case arrow, rect, ellipse, highlight, text, counter, pixelate, blur

    /// Kinds defined by a two-point box that can be resized by their corners.
    var isBox: Bool {
        switch self {
        case .rect, .ellipse, .highlight, .pixelate, .blur: return true
        default: return false
        }
    }
}

/// All geometry is in source-image pixel coordinates (origin top-left, y down).
struct Annotation: Identifiable, Codable, Equatable {
    var id = UUID()
    var kind: AnnotationKind
    var p1: CGPoint
    var p2: CGPoint
    var color: RGBA
    var width: CGFloat
    var text: String = ""
    var fontSize: CGFloat = 32

    var rect: CGRect {
        CGRect(x: min(p1.x, p2.x), y: min(p1.y, p2.y),
               width: abs(p1.x - p2.x), height: abs(p1.y - p2.y))
    }

    var counterRadius: CGFloat { max(width * 4.2, 14) }

    var textAttributes: [NSAttributedString.Key: Any] {
        [
            .font: NSFont.systemFont(ofSize: fontSize, weight: .bold),
            .foregroundColor: color.nsColor,
        ]
    }

    var textSize: CGSize {
        let s = text.isEmpty ? " " : text
        let r = (s as NSString).boundingRect(
            with: CGSize(width: 100_000, height: 100_000),
            options: [.usesLineFragmentOrigin], attributes: textAttributes)
        return CGSize(width: ceil(r.width), height: ceil(r.height))
    }

    /// Bounding box used for hit testing and selection outlines.
    var bounds: CGRect {
        switch kind {
        case .arrow:
            return rect.insetBy(dx: -width * 3, dy: -width * 3)
        case .text:
            return CGRect(origin: p1, size: textSize)
        case .counter:
            let r = counterRadius
            return CGRect(x: p1.x - r, y: p1.y - r, width: r * 2, height: r * 2)
        default:
            return rect
        }
    }

    func hitTest(_ p: CGPoint, tolerance: CGFloat) -> Bool {
        switch kind {
        case .arrow:
            return distance(from: p, toSegment: p1, p2) <= max(width * 2, tolerance)
        case .rect, .ellipse:
            // Outlined shapes: hit the stroke or anywhere inside (easier to grab).
            return rect.insetBy(dx: -tolerance, dy: -tolerance).contains(p)
        default:
            return bounds.insetBy(dx: -tolerance, dy: -tolerance).contains(p)
        }
    }

    /// Stricter hit used while a drawing tool is active: only outlines and arrow lines respond,
    /// so a new shape can still start on top of a label, a counter or a filled area.
    func grabTest(_ p: CGPoint, tolerance: CGFloat) -> Bool {
        switch kind {
        case .ellipse:
            let pad = max(width, tolerance)
            let rx = rect.width / 2, ry = rect.height / 2
            guard rx > 0, ry > 0 else { return false }
            let nx = (p.x - rect.midX) / rx, ny = (p.y - rect.midY) / ry
            return abs(sqrt(nx * nx + ny * ny) - 1) * min(rx, ry) <= pad
        case .rect:
            let pad = max(width, tolerance)
            return rect.insetBy(dx: -pad, dy: -pad).contains(p) && !rect.insetBy(dx: pad, dy: pad).contains(p)
        case .arrow:
            return hitTest(p, tolerance: tolerance)
        default:
            return false
        }
    }

    mutating func offset(by d: CGSize) {
        p1.x += d.width; p1.y += d.height
        p2.x += d.width; p2.y += d.height
    }
}

func distance(from p: CGPoint, toSegment a: CGPoint, _ b: CGPoint) -> CGFloat {
    let dx = b.x - a.x, dy = b.y - a.y
    let len2 = dx * dx + dy * dy
    if len2 == 0 { return hypot(p.x - a.x, p.y - a.y) }
    var t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2
    t = max(0, min(1, t))
    return hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy))
}
