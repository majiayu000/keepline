import CoreGraphics
import Foundation
// Sweeps the pointer over a rectangle in a zigzag: hover enter/leave on every row.
let a = CommandLine.arguments.dropFirst().compactMap { Double($0) }
let (x0, y0, x1, y1) = (a[0], a[1], a[2], a[3])
var y = y0
while y <= y1 {
    var x = x0
    while x <= x1 {
        CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: CGPoint(x: x, y: y), mouseButton: .left)?.post(tap: .cghidEventTap)
        usleep(12000)
        x += 40
    }
    y += 37
}
