import AppKit
import SwiftUI

/// Headless verification utility: renders the real `DashboardView` (with mock data) to PNGs
/// in both Light and Dark appearances, so the adaptive pace-color contrast can be inspected
/// without manually opening the popover. Invoked via `CcUsageMenuBar --render-swatches DIR`.
///
/// It exits before any `NSStatusItem` is created, so it never registers a menu bar item.
enum SwatchRenderer {
    static func run(outputDir: String) {
        _ = NSApplication.shared // initialize AppKit for rendering

        let mock = mockSnapshot()
        let cases: [(label: String, scheme: ColorScheme, appearance: NSAppearance.Name, bg: Color)] = [
            ("light", .light, .aqua, .white),
            ("dark", .dark, .darkAqua, Color(white: 0.13)),
        ]

        for c in cases {
            let view = DashboardView(controller: DataController(previewSnapshot: mock))
                .environment(\.colorScheme, c.scheme)
                .background(c.bg)

            guard let appearance = NSAppearance(named: c.appearance) else { continue }
            appearance.performAsCurrentDrawingAppearance {
                let renderer = ImageRenderer(content: view)
                renderer.scale = 2
                guard let image = renderer.nsImage,
                      let tiff = image.tiffRepresentation,
                      let rep = NSBitmapImageRep(data: tiff),
                      let png = rep.representation(using: .png, properties: [:])
                else {
                    FileHandle.standardError.write(Data("render failed for \(c.label)\n".utf8))
                    return
                }
                let path = "\(outputDir)/dashboard-\(c.label).png"
                try? png.write(to: URL(fileURLWithPath: path))
                FileHandle.standardError.write(Data("wrote \(path)\n".utf8))
            }
        }
    }

    /// Mock snapshot exercising all three pace colors AND the "Remaining today" label:
    /// red (weekly), amber (session), green (daily pace under budget).
    private static func mockSnapshot() -> UsageSnapshot {
        UsageSnapshot(
            fetchedAt: Date(timeIntervalSince1970: 1_760_000_000),
            config: ConfigDTO(dailyBudget: 20, workDays: 5, pollIntervalSecs: 300),
            weekly: WindowDTO(utilization: 92, resetsAt: Date(timeIntervalSince1970: 1_760_400_000),
                              resetsInSecs: 5 * 86_400 + 10 * 3_600, paceColor: .red),
            session: WindowDTO(utilization: 68, resetsAt: Date(timeIntervalSince1970: 1_760_010_000),
                               resetsInSecs: 3 * 3_600 + 21 * 60, paceColor: .yellow),
            dailyPace: DailyPaceDTO(workDayIndex: 3, ceiling: 60, remaining: 14,
                                    resetDayLocal: "Mon Jun 15, 8:00 PM"),
            error: nil
        )
    }
}
