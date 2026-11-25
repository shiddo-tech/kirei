import AppKit
let w = 1000, h = 420
let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setFillColor(.white); ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)
let attrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 30), .foregroundColor: NSColor.black]
for (i, s) in ["Contato: maria.silva@exemplo.com.br", "CPF 123.456.789-09 e telefone (11) 98765-4321", "Relatório mensal de vendas"].enumerated() {
    (s as NSString).draw(at: NSPoint(x: 60, y: h - 100 - i * 90), withAttributes: attrs)
}
let img = ctx.makeImage()!
let rep = NSBitmapImageRep(cgImage: img)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
