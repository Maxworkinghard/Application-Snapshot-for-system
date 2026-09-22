// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "snapshot-recorder",
    platforms: [
        .macOS(.v14)
    ],
    targets: [
        .executableTarget(
            name: "snapshot-recorder",
            path: "Sources/snapshot-recorder"
        )
    ],
    swiftLanguageModes: [.v5]
)
