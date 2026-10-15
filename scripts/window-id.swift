// Find only the window belonging to the smoke process we launched.
import CoreGraphics
import Foundation
let pid = Int(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
if let window = windows.first(where: { ($0[kCGWindowOwnerPID as String] as? Int) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }) {
    print(window[kCGWindowNumber as String]!)
} else {
    fputs("No visible window for owned PID\n", stderr)
    exit(1)
}
