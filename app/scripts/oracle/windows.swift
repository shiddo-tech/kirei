import CoreGraphics
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in list where (w["kCGWindowOwnerName"] as? String) == CommandLine.arguments[1] {
    print(w["kCGWindowNumber"]!, w["kCGWindowLayer"]!, w["kCGWindowBounds"]!)
}
