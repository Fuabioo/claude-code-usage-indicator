// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "CcUsageMenuBar",
    platforms: [.macOS(.v13)], // MenuBarExtra/Gauge era; we use AppKit NSStatusItem + SwiftUI
    targets: [
        .executableTarget(
            name: "CcUsageMenuBar",
            path: "Sources/CcUsageMenuBar"
        )
    ]
)
