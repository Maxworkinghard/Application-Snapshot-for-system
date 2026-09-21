// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "snapshot-ocr",
    platforms: [
        .macOS(.v14)
    ],
    targets: [
        .executableTarget(
            name: "snapshot-ocr",
            path: "Sources/snapshot-ocr"
        )
    ],
    swiftLanguageModes: [.v5]
)
