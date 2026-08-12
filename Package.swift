// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "WindowSnap",
    platforms: [
        .macOS(.v14)
    ],
    targets: [
        .executableTarget(
            name: "WindowSnap",
            path: "Sources/WindowSnap"
        )
    ],
    swiftLanguageModes: [.v5]
)
