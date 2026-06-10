import Foundation
import SwiftUI

/// Pace color contract shared with `cc-usage --json` ("green"/"yellow"/"red").
enum PaceColor: String, Codable {
    case green, yellow, red

    /// Color for the SwiftUI dashboard.
    var swiftUIColor: Color {
        switch self {
        case .green: return .green
        case .yellow: return .yellow
        case .red: return .red
        }
    }

    /// Color for the AppKit menu bar drawing.
    var nsColor: NSColor {
        switch self {
        case .green: return .systemGreen
        case .yellow: return .systemYellow
        case .red: return .systemRed
        }
    }
}

// MARK: - JSON DTOs (mirror crates/cc-usage-cli/src/output.rs)

struct UsageSnapshot: Codable {
    let fetchedAt: Date
    let config: ConfigDTO
    let weekly: WindowDTO?
    let session: WindowDTO?
    let dailyPace: DailyPaceDTO?
    let error: ErrorDTO?

    var isError: Bool { error != nil }
}

struct ConfigDTO: Codable {
    let dailyBudget: Double
    let workDays: Int
    let pollIntervalSecs: Int
}

struct WindowDTO: Codable {
    let utilization: Double
    let resetsAt: Date
    let resetsInSecs: Int
    let paceColor: PaceColor
}

struct DailyPaceDTO: Codable {
    let workDayIndex: Int
    let ceiling: Double
    let remaining: Double
    let resetDayLocal: String
}

struct ErrorDTO: Codable {
    let kind: String
    let message: String
}

// MARK: - Decoding

extension JSONDecoder {
    /// Decoder configured for the CLI's snake_case keys and RFC3339 timestamps
    /// (with or without fractional seconds — chrono emits either).
    static func ccUsage() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase

        let withFraction = ISO8601DateFormatter()
        withFraction.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let plain = ISO8601DateFormatter()
        plain.formatOptions = [.withInternetDateTime]

        decoder.dateDecodingStrategy = .custom { d in
            let container = try d.singleValueContainer()
            let s = try container.decode(String.self)
            if let date = withFraction.date(from: s) ?? plain.date(from: s) {
                return date
            }
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Unrecognized date format: \(s)"
            )
        }
        return decoder
    }
}

// MARK: - Duration formatting (mirrors cc_usage_budget::format_duration)

/// Format a number of seconds as "3d 12h" / "2h 45m" / "45m" (minimum "1m").
func formatDuration(seconds: Int) -> String {
    if seconds <= 0 { return "0m" }
    let days = seconds / 86_400
    let hours = (seconds % 86_400) / 3_600
    let minutes = (seconds % 3_600) / 60
    switch (days, hours, minutes) {
    case let (d, h, _) where d > 0 && h > 0: return "\(d)d \(h)h"
    case let (d, _, _) where d > 0: return "\(d)d"
    case let (_, h, m) where h > 0 && m > 0: return "\(h)h \(m)m"
    case let (_, h, _) where h > 0: return "\(h)h"
    case let (_, _, m): return "\(max(m, 1))m"
    }
}
