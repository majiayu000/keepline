// Measurement guard only: activation is authorized for these isolated test apps.
// Does not change measure.py, load, process accounting, or re-activate after focus is lost.
import AppKit
let args = CommandLine.arguments
let pid = Int32(args[1])!
let receipt = args[2]
_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let activated = NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateAllWindows, .activateIgnoringOtherApps]) ?? false
RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.5))
let task = Process()
task.executableURL = URL(fileURLWithPath: args[3])
task.arguments = Array(args.dropFirst(4))
task.environment = ["PATH":"/usr/bin:/bin:/usr/sbin:/sbin", "HOME":NSHomeDirectory(), "LANG":"en_US.UTF-8"]
var observations: [[String:Any]] = []
let start = Date()
func sample() {
    observations.append(["seconds":Date().timeIntervalSince(start), "frontmost":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0])
}
sample()
try task.run()
while task.isRunning {
    RunLoop.current.run(until: Date(timeIntervalSinceNow:0.5))
    sample()
}
sample()
let valid = activated && observations.allSatisfy { ($0["frontmost"] as? Int32) == pid }
let result: [String:Any] = ["target":pid,"activated":activated,"continuous_frontmost":valid,"exit":task.terminationStatus,"samples":observations]
try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:URL(fileURLWithPath:receipt))
print("continuous_frontmost=\(valid), samples=\(observations.count), child_exit=\(task.terminationStatus)")
