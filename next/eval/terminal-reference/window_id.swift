// Prints "<windowID> <x> <y> <w> <h>" for the largest on-screen window owned by a pid.
import CoreGraphics
import Foundation

let pid = Int32(CommandLine.arguments[1])!
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
let best = list
    .filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
    .max { a, b in
        let ra = a[kCGWindowBounds as String] as! [String: CGFloat], rb = b[kCGWindowBounds as String] as! [String: CGFloat]
        return ra["Width"]! * ra["Height"]! < rb["Width"]! * rb["Height"]!
    }
guard let w = best, let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { exit(1) }
print("\(w[kCGWindowNumber as String]!) \(Int(b["X"]!)) \(Int(b["Y"]!)) \(Int(b["Width"]!)) \(Int(b["Height"]!))")
