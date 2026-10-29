// Puts a square crop of `src` into Apple's macOS icon grid: an 824×824 rounded square
// centred on a transparent 1024×1024 canvas, so the system does not frame it.
import CoreGraphics
import ImageIO
import Foundation
let a = CommandLine.arguments
let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: a[1]) as CFURL, nil)!
let img = CGImageSourceCreateImageAtIndex(src, 0, nil)!
let crop = img.cropping(to: CGRect(x: Int(a[2])!, y: Int(a[3])!, width: Int(a[4])!, height: Int(a[4])!))!
let ctx = CGContext(data: nil, width: 1024, height: 1024, bitsPerComponent: 8, bytesPerRow: 0,
                    space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
let tile = CGRect(x: 100, y: 100, width: 824, height: 824)
ctx.addPath(CGPath(roundedRect: tile, cornerWidth: 185, cornerHeight: 185, transform: nil))
ctx.clip()
ctx.interpolationQuality = .high
ctx.draw(crop, in: tile)
let out = ctx.makeImage()!
let dst = CGImageDestinationCreateWithURL(URL(fileURLWithPath: a[5]) as CFURL, "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(dst, out, nil); CGImageDestinationFinalize(dst)
