// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Kirei",
    platforms: [.macOS(.v14)],
    targets: [
        .executableTarget(
            name: "Kirei",
            path: "Sources/Kirei",
            linkerSettings: [
                .linkedFramework("Carbon"),
                .linkedFramework("Vision"),
                .linkedFramework("ServiceManagement"),
                .linkedFramework("MetricKit"),
            ]
        )
    ]
)
