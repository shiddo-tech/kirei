import AppKit

// Developer self-test: `Kirei --render-test <in.png> <out.png> [style.json]`
let args = CommandLine.arguments
if args.count == 5, args[1] == "--preview-test", let scale = Double(args[4]) {
    // Renders the base layer exactly like the editor preview does.
    guard let cap = CaptureService.load(url: URL(fileURLWithPath: args[2])) else { exit(1) }
    let model = EditorModel(image: cap.image, pixelScale: cap.scale, style: PresetStore.shared.lastStyle)
    let img = Renderer.render(source: cap.image, style: model.style, crop: model.displayCrop, edgeColor: model.edgeColor,
                              annotations: [], effects: nil, scale: CGFloat(scale))!
    Exporter.save(img, scale: 1, to: URL(fileURLWithPath: args[3]), format: .png)
    print("style:", model.style.background, "crop:", model.displayCrop, "out:", img.width, "x", img.height)
    exit(0)
}
if args.count >= 4, args[1] == "--render-test" {
    guard let cap = CaptureService.load(url: URL(fileURLWithPath: args[2])) else { exit(1) }
    let style = args.count >= 5
        ? (try? JSONDecoder().decode(Style.self, from: Data(contentsOf: URL(fileURLWithPath: args[4])))) ?? .default
        : .default
    let model = EditorModel(image: cap.image, pixelScale: cap.scale, style: style)
    let rects = ImageAnalysis.sensitiveRects(cap.image)
    print("balanced:", model.balancedRect, "full:", model.fullRect, "sensitive:", rects.count)
    print("ocr:", ImageAnalysis.text(in: cap.image).replacingOccurrences(of: "\n", with: " | "))
    let u = model.unit
    var list = rects.map { Annotation(kind: .pixelate, p1: $0.origin, p2: CGPoint(x: $0.maxX, y: $0.maxY), color: .black, width: 0) }
    let c = model.crop
    list.append(Annotation(kind: .arrow, p1: CGPoint(x: c.minX + 40, y: c.maxY - 40), p2: CGPoint(x: c.midX, y: c.midY), color: Palette.annotation[0], width: u * 2))
    list.append(Annotation(kind: .rect, p1: CGPoint(x: c.minX + 20, y: c.minY + 20), p2: CGPoint(x: c.midX, y: c.minY + 120), color: Palette.annotation[4], width: u * 2))
    list.append(Annotation(kind: .counter, p1: CGPoint(x: c.maxX - 60, y: c.minY + 60), p2: .zero, color: Palette.annotation[5], width: u * 2))
    list.append(Annotation(kind: .text, p1: CGPoint(x: c.midX, y: c.maxY - 120), p2: .zero, color: Palette.annotation[1], width: u * 2, text: "Olá Kirei", fontSize: u * 13))
    list.append(Annotation(kind: .highlight, p1: CGPoint(x: c.minX + 30, y: c.midY), p2: CGPoint(x: c.midX, y: c.midY + 40), color: Palette.annotation[2], width: u))
    let img = Renderer.render(source: cap.image, style: model.style, crop: model.crop, edgeColor: model.edgeColor,
                              annotations: list, effects: EffectImages(source: cap.image))!
    Exporter.save(img, scale: cap.scale, to: URL(fileURLWithPath: args[3]), format: .png)
    print("out:", img.width, "x", img.height)
    exit(0)
}

// Developer UI check: `Kirei --ui-test <in.png> <snapshot.png>` opens the editor, reports which views
// receive clicks in key spots and writes a snapshot of the window.
if args.count == 4, args[1] == "--ui-test" {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    guard let cap = CaptureService.load(url: URL(fileURLWithPath: args[2])) else { exit(1) }
    let editor = EditorWindowController(capture: cap)
    editor.showWindow(nil)
    DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
        guard let w = editor.window, let frameView = w.contentView?.superview else { exit(2) }
        let H = frameView.bounds.height, W = frameView.bounds.width
        let bar = H - w.contentLayoutRect.height
        print("window:", W, "x", H, "bar:", bar)
        for (name, p) in [("first tool", CGPoint(x: 99, y: H - bar / 2)),
                          ("wand area", CGPoint(x: W - 284 - 30, y: H - bar / 2)),
                          ("inspector header", CGPoint(x: W - 40, y: H - bar / 2)),
                          ("empty top row", CGPoint(x: W - 284 - 200, y: H - bar / 2)),
                          ("canvas", CGPoint(x: 400, y: H / 2)),
                          ("inspector save card", CGPoint(x: W - 230, y: H - bar - 60)),
                          ("inspector slider", CGPoint(x: W - 120, y: 60)),
                          ("inspector keyboard button", CGPoint(x: W - 26, y: H - bar / 2))] {
            let v = frameView.hitTest(p)
            var chain: [String] = []
            var cur = v
            while let c = cur, chain.count < 7 { chain.append(String(describing: type(of: c))); cur = c.superview }
            print("\(name):", chain.joined(separator: " < "))
        }
        let panel = PreviewPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        print("preview panel canBecomeKey:", panel.canBecomeKey)
        // cacheDisplay draws in subview order, not SwiftUI layer order; good enough for layout checks.
        if let rep = frameView.bitmapImageRepForCachingDisplay(in: frameView.bounds) {
            frameView.cacheDisplay(in: frameView.bounds, to: rep)
            try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: args[3]))
        }
        exit(0)
    }
    app.run()
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
