import SwiftUI

/// The popover dashboard — reproduces the COSMIC popup: weekly, session, daily-pace, footer.
struct DashboardView: View {
    @ObservedObject var controller: DataController

    /// Prefer the freshest document; if the latest is an error, fall back to last good data.
    private var displaySnapshot: UsageSnapshot? {
        if let s = controller.snapshot, !s.isError { return s }
        return controller.lastGood
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let snap = displaySnapshot {
                if let weekly = snap.weekly {
                    WindowSection(title: "Weekly Budget", window: weekly, showPaceLabel: true)
                }
                Divider()
                if let session = snap.session {
                    WindowSection(title: "Session (5h)", window: session, showPaceLabel: false)
                }
                Divider()
                if let pace = snap.dailyPace {
                    DailyPaceSection(pace: pace)
                }
            } else {
                Text("No data yet…")
                    .foregroundStyle(.secondary)
            }

            Divider()
            FooterView(controller: controller)
        }
        .padding(14)
        .frame(width: 280)
    }
}

private struct WindowSection: View {
    let title: String
    let window: WindowDTO
    let showPaceLabel: Bool

    private var paceLabel: String {
        switch window.paceColor {
        case .green: return "On track"
        case .yellow: return "Caution"
        case .red: return "Over budget"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.headline)

            ProgressView(value: min(window.utilization, 100) / 100.0)
                .tint(window.paceColor.swiftUIColor)

            HStack(spacing: 6) {
                Text(String(format: "%.0f%%", window.utilization))
                    .font(.title3).bold()
                    .foregroundStyle(window.paceColor.swiftUIColor)
                if showPaceLabel {
                    Text("· \(paceLabel)")
                        .foregroundStyle(window.paceColor.swiftUIColor)
                }
            }

            Text("Resets in \(formatDuration(seconds: window.resetsInSecs))")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}

private struct DailyPaceSection: View {
    let pace: DailyPaceDTO

    private var isOver: Bool { pace.remaining < 0 }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Daily Pace").font(.headline)

            if isOver {
                Text(String(format: "Over by %.0f%%", abs(pace.remaining)))
                    .foregroundStyle(PaceColor.red.swiftUIColor).bold()
            } else {
                Text(String(format: "Remaining today: %.0f%%", pace.remaining))
                    .foregroundStyle(PaceColor.green.swiftUIColor).bold()
            }

            Text("Work day \(pace.workDayIndex) · ceiling \(String(format: "%.0f%%", pace.ceiling))")
                .font(.caption)
                .foregroundStyle(.secondary)
            Text("Resets \(pace.resetDayLocal)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }
}

private struct FooterView: View {
    @ObservedObject var controller: DataController

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let err = controller.snapshot?.error {
                Text("Error: \(err.message)")
                    .font(.caption)
                    .foregroundStyle(PaceColor.red.swiftUIColor)
            } else if let runtime = controller.runtimeError {
                Text("Error: \(runtime)")
                    .font(.caption)
                    .foregroundStyle(PaceColor.red.swiftUIColor)
                    .lineLimit(3)
            }

            HStack {
                if let updated = controller.lastUpdated {
                    Text("Updated \(updated.formatted(date: .omitted, time: .standard))")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Refresh") { controller.refresh() }
                    .buttonStyle(.borderless)
                    .font(.caption)
            }
        }
    }
}
