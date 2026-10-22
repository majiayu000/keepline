// Isolated-app activation is user-authorized. No refocus within a sample.
// Retry only invalid foreground samples; preserve every receipt and metric file.
import AppKit
import CoreGraphics
let args = CommandLine.arguments
let pid = Int32(args[1])!
let prefix = args[2]
_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
var attempts: [[String:Any]] = []
func idleSeconds() -> Double {
    [CGEventType.keyDown, .leftMouseDown, .rightMouseDown, .scrollWheel, .mouseMoved]
        .map { CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: $0) }.min() ?? 0
}
let gateStart=Date()
while idleSeconds() < 20 && Date().timeIntervalSince(gateStart) < 180 {
    RunLoop.current.run(until: Date(timeIntervalSinceNow:1))
}
let gate:[String:Any] = ["wait_seconds":Date().timeIntervalSince(gateStart),"input_idle_seconds":idleSeconds(),"eligible":idleSeconds()>=20]
try JSONSerialization.data(withJSONObject:gate,options:[.prettyPrinted,.sortedKeys]).write(to:URL(fileURLWithPath:"\(prefix)-idle-gate.json"))
if idleSeconds() < 20 { exit(2) }

// One complete silent attempt per invocation; caller caps each case at three.
for attempt in 1...1 {
    let activated = NSRunningApplication(processIdentifier: pid)?.activate(options: [.activateAllWindows]) ?? false
    RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.5))
    let task = Process()
    task.executableURL = URL(fileURLWithPath: args[3])
    task.arguments = Array(args.dropFirst(4)).map { $0.replacingOccurrences(of: "{attempt}", with: String(attempt)) }
    task.environment = ["PATH":"/usr/bin:/bin:/usr/sbin:/sbin", "HOME":NSHomeDirectory(), "LANG":"en_US.UTF-8"]
    let start = Date()
    var observations: [[String:Any]] = []
    func sample(_ source: String) {
        let app = NSWorkspace.shared.frontmostApplication
        observations.append(["seconds":Date().timeIntervalSince(start), "source":source,
            "pid":app?.processIdentifier ?? 0, "bundle":app?.bundleIdentifier ?? "",
            "name":app?.localizedName ?? "", "utc":ISO8601DateFormatter().string(from:Date()), "input_idle_seconds":idleSeconds()])
    }
    let token = NSWorkspace.shared.notificationCenter.addObserver(forName:NSWorkspace.didActivateApplicationNotification, object:nil, queue:.main) { _ in sample("activation") }
    sample("before")
    try task.run()
    while task.isRunning {
        RunLoop.current.run(until: Date(timeIntervalSinceNow:0.1))
        sample("poll")
    }
    sample("after")
    NSWorkspace.shared.notificationCenter.removeObserver(token)
    let valid = activated && task.terminationStatus == 0 && observations.allSatisfy { ($0["pid"] as? Int32) == pid }
    let result: [String:Any] = ["target":pid,"attempt":attempt,"activated":activated,"continuous_frontmost":valid,"exit":task.terminationStatus,"samples":observations]
    try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:URL(fileURLWithPath:"\(prefix)-focus-\(attempt).json"))
    attempts.append(["attempt":attempt,"valid":valid])
    if valid { break }
}
try JSONSerialization.data(withJSONObject:attempts,options:[.prettyPrinted,.sortedKeys]).write(to:URL(fileURLWithPath:"\(prefix)-attempts.json"))
