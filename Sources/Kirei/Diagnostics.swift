import Foundation
import MetricKit

/// Keeps MetricKit crash and hang reports on disk so users can send them with a bug report.
final class Diagnostics: NSObject, MXMetricManagerSubscriber {
    private static let shared = Diagnostics()

    static var folder: URL {
        let url = FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Logs/Kirei", isDirectory: true)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    static func start() {
        MXMetricManager.shared.add(shared)
    }

    func didReceive(_ payloads: [MXDiagnosticPayload]) {
        for p in payloads { write(p.jsonRepresentation(), prefix: "diagnostic") }
    }

    func didReceive(_ payloads: [MXMetricPayload]) {
        for p in payloads { write(p.jsonRepresentation(), prefix: "metrics") }
    }

    private func write(_ data: Data, prefix: String) {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd'T'HH-mm-ss"
        let url = Diagnostics.folder.appendingPathComponent("\(prefix)-\(f.string(from: Date())).json")
        try? data.write(to: url)
    }
}
