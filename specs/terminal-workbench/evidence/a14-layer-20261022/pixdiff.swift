// Pixel diff of two PNGs: total pixels, pixels whose max channel delta > 0 and > 32.
import AppKit
func rgba(_ p: String) -> (Int, Int, [UInt8]) {
    let rep = NSBitmapImageRep(data: try! Data(contentsOf: URL(fileURLWithPath: p)))!
    let w = rep.pixelsWide, h = rep.pixelsHigh
    var buf = [UInt8](repeating: 0, count: w * h * 4)
    let ctx = CGContext(data: &buf, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.draw(rep.cgImage!, in: CGRect(x: 0, y: 0, width: w, height: h))
    return (w, h, buf)
}
let a = rgba(CommandLine.arguments[1]), b = rgba(CommandLine.arguments[2])
guard a.0 == b.0 && a.1 == b.1 else { print("{\"size_mismatch\":true}"); exit(1) }
var any = 0, big = 0
for i in stride(from: 0, to: a.2.count, by: 4) {
    let d = (0..<3).map { abs(Int(a.2[i + $0]) - Int(b.2[i + $0])) }.max()!
    if d > 0 { any += 1 }; if d > 32 { big += 1 }
}
print("{\"pixels\":\(a.0 * a.1),\"changed\":\(any),\"changed_gt32\":\(big)}")
