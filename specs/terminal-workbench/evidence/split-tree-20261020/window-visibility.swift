// Read-only window metadata, restricted output to the fixture PID and overlap geometry.
import CoreGraphics
import Foundation
let pid = Int(CommandLine.arguments[1])!
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly,.excludeDesktopElements],kCGNullWindowID) as? [[String:Any]] ?? []
func rect(_ w:[String:Any])->CGRect {
    let b=w[kCGWindowBounds as String] as? [String:Double] ?? [:]
    return CGRect(x:b["X"] ?? 0,y:b["Y"] ?? 0,width:b["Width"] ?? 0,height:b["Height"] ?? 0)
}
guard let i=list.firstIndex(where:{ ($0[kCGWindowOwnerPID as String] as? Int)==pid && rect($0).width>500 && rect($0).height>500 }) else {exit(2)}
let target=rect(list[i])
let overlap=list[..<i].filter{($0[kCGWindowOwnerPID as String] as? Int) != pid && ($0[kCGWindowLayer as String] as? Int)==0}.map{target.intersection(rect($0))}.filter{!$0.isNull && $0.width*$0.height>1000}
let out:[String:Any]=["pid":pid,"frame":["x":target.minX,"y":target.minY,"width":target.width,"height":target.height],"overlapping_app_window_areas":overlap.map{$0.width*$0.height}]
print(String(data:try JSONSerialization.data(withJSONObject:out,options:.sortedKeys),encoding:.utf8)!)
