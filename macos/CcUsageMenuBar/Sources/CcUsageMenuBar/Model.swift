import AppKit
import Foundation
import SwiftUI

/// Pace color contract shared with `cc-usage --json` ("green"/"yellow"/"red").
///
/// We deliberately do NOT use the stock `.systemGreen`/`.systemYellow` colors: those are
/// tuned as UI element fills and lose contrast as text on the light popover background
/// (yellow especially). COSMIC gets contrast for free from its theme's success/warning/
/// destructive roles; on macOS we mirror that by resolving each pace to an appearance-aware
/// color — a darker, saturated variant in Light mode and a brighter one in Dark mode — so
/// text stays legible whether the app is following the system Light, Dark, or Auto theme.
enum PaceColor: String, Codable {
    case green, yellow, red

    /// Appearance-adaptive AppKit color (used for the menu bar drawing and, via
    /// `swiftUIColor`, the dashboard). Resolves per the active Light/Dark appearance.
    var nsColor: NSColor {
        switch self {
        // light variant: contrast-correct on a light background; dark variant: brighter.
        case .green:
            return .paceAdaptive(light: (0.082, 0.502, 0.239), dark: (0.290, 0.871, 0.502))
        case .yellow:
            return .paceAdaptive(light: (0.706, 0.325, 0.035), dark: (0.984, 0.749, 0.141))
        case .red:
            return .paceAdaptive(light: (0.776, 0.157, 0.157), dark: (0.973, 0.443, 0.443))
        }
    }

    /// Color for the SwiftUI dashboard, derived from the adaptive AppKit color so the
    /// dashboard and the menu bar always agree and both follow the system appearance.
    var swiftUIColor: Color {
        Color(nsColor: nsColor)
    }
}

private extension NSColor {
    /// A dynamic color that returns `light`/`dark` sRGB components based on the appearance it
    /// is drawn in, so it tracks the system Light/Dark/Auto setting automatically.
    static func paceAdaptive(
        light: (r: CGFloat, g: CGFloat, b: CGFloat),
        dark: (r: CGFloat, g: CGFloat, b: CGFloat)
    ) -> NSColor {
        NSColor(name: nil) { appearance in
            let isDark = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            let c = isDark ? dark : light
            return NSColor(srgbRed: c.r, green: c.g, blue: c.b, alpha: 1.0)
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
