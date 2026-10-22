import CoreGraphics
import Foundation
let d = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
print("locked=\(d["CGSSessionScreenIsLocked"] ?? false) onConsole=\(d[kCGSessionOnConsoleKey as String] ?? "?")")
