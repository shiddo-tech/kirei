// Makes Resources/kirei-voice.wav, the spoken "kirei" capture sound:
//   say -v Kyoko -r 190 -o /tmp/kyoko.aiff "きれい"
//   swiftc -O scripts/make-voice.swift -o /tmp/make-voice && /tmp/make-voice /tmp/kyoko.aiff Resources/kirei-voice.wav vocoder
// Modes: vocoder (the one in use), chord, ring, raw.
import AVFoundation
let a = CommandLine.arguments
let inURL = URL(fileURLWithPath: a[1]), outURL = URL(fileURLWithPath: a[2]), mode = a[3]
let f = try! AVAudioFile(forReading: inURL)
let fmt = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: f.processingFormat.sampleRate, channels: 1, interleaved: false)!
let buf = AVAudioPCMBuffer(pcmFormat: f.processingFormat, frameCapacity: AVAudioFrameCount(f.length))!
try! f.read(into: buf)
let sr = f.processingFormat.sampleRate
var x = Array(UnsafeBufferPointer(start: buf.floatChannelData![0], count: Int(buf.frameLength)))
// trim silence
let th: Float = 0.01
let s0 = max(0, (x.firstIndex { abs($0) > th } ?? 0) - Int(sr * 0.005))
let s1 = min(x.count, (x.lastIndex { abs($0) > th } ?? x.count) + Int(sr * 0.03))
x = Array(x[s0..<s1])

struct Biquad { var b0, b1, b2, a1, a2: Float; var z1: Float = 0, z2: Float = 0
    static func bp(_ fc: Double, _ q: Double, _ sr: Double) -> Biquad {
        let w = 2 * Double.pi * fc / sr, al = sin(w) / (2 * q), a0 = 1 + al
        return Biquad(b0: Float(al / a0), b1: 0, b2: Float(-al / a0), a1: Float(-2 * cos(w) / a0), a2: Float((1 - al) / a0)) }
    static func lp(_ fc: Double, _ sr: Double) -> Biquad {
        let w = 2 * Double.pi * fc / sr, al = sin(w) / (2 * 0.707), a0 = 1 + al, c = cos(w)
        return Biquad(b0: Float((1 - c) / 2 / a0), b1: Float((1 - c) / a0), b2: Float((1 - c) / 2 / a0), a1: Float(-2 * c / a0), a2: Float((1 - al) / a0)) }
    mutating func run(_ v: Float) -> Float { let y = b0 * v + z1; z1 = b1 * v - a1 * y + z2; z2 = b2 * v - a2 * y; return y }
}
func vocode(_ x: [Float], carrier: (Int) -> Float, dry: Float) -> [Float] {
    let bands = 22; var out = [Float](repeating: 0, count: x.count)
    for b in 0..<bands {
        let fc = 120 * pow(5000.0 / 120, Double(b) / Double(bands - 1))
        var m = Biquad.bp(fc, 6, sr), c = Biquad.bp(fc, 6, sr); var env: Float = 0
        for i in 0..<x.count {
            let e = abs(m.run(x[i])); env += (e - env) * (e > env ? 0.02 : 0.004)
            out[i] += c.run(carrier(i)) * env * 6
        }
    }
    return zip(out, x).map { $0 + $1 * dry }
}
func saw(_ hz: Double) -> (Int) -> Float { { i in Float(2 * (Double(i) * hz / sr).truncatingRemainder(dividingBy: 1) - 1) } }
var y: [Float]
switch mode {
case "vocoder":
    let c = saw(150); y = vocode(x, carrier: c, dry: 0.12)
case "chord":
    let c1 = saw(196), c2 = saw(293.66), c3 = saw(392)
    y = vocode(x, carrier: { i in (c1(i) + c2(i) * 0.7 + c3(i) * 0.4) * 0.6 }, dry: 0.08)
    // soft glassy tail
    let tail = Int(sr * 0.5); let start = y.count; y += [Float](repeating: 0, count: tail)
    for i in 0..<(start + tail) where i > start - Int(sr * 0.12) {
        let t = Double(i - (start - Int(sr * 0.12))) / sr
        y[i] += Float(sin(2 * .pi * 1567.98 * t) * exp(-t * 9) * 0.06 + sin(2 * .pi * 2349.3 * t) * exp(-t * 12) * 0.03)
    }
case "ring":
    y = x.enumerated().map { i, v in v * 0.5 + v * Float(sin(2 * .pi * 55 * Double(i) / sr)) * 0.7 }
    var d = [Float](repeating: 0, count: Int(sr * 0.006))
    var k = 0
    y = y.map { v in let o = v + d[k] * 0.45; d[k] = o; k = (k + 1) % d.count; return o }
    y = y.map { (($0 * 512).rounded() / 512) }
default:
    y = x
}
// gentle lowpass to take the edge off, fades, normalize
var lp = Biquad.lp(mode == "raw" ? 9000 : 6500, sr); y = y.map { lp.run($0) }
let fadeIn = Int(sr * 0.004), fadeOut = Int(sr * 0.06)
for i in 0..<min(fadeIn, y.count) { y[i] *= Float(i) / Float(fadeIn) }
for i in 0..<min(fadeOut, y.count) { y[y.count - 1 - i] *= Float(i) / Float(fadeOut) }
let peak = y.map { abs($0) }.max() ?? 1; y = y.map { $0 / peak * 0.5 }
let ob = AVAudioPCMBuffer(pcmFormat: fmt, frameCapacity: AVAudioFrameCount(y.count))!
ob.frameLength = AVAudioFrameCount(y.count)
y.withUnsafeBufferPointer { ob.floatChannelData![0].update(from: $0.baseAddress!, count: y.count) }
let of = try! AVAudioFile(forWriting: outURL, settings: [AVFormatIDKey: kAudioFormatLinearPCM, AVSampleRateKey: sr, AVNumberOfChannelsKey: 1, AVLinearPCMBitDepthKey: 16, AVLinearPCMIsFloatKey: false])
try! of.write(from: ob)
print(mode, String(format: "%.2fs", Double(y.count) / sr))
