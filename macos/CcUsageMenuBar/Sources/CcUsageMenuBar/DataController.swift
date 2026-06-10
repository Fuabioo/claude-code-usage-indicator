import Foundation
import Combine

/// A spawn/IO/decoding failure surfaced by running the CLI (distinct from a CLI-reported
/// `error` JSON document, which decodes successfully into `UsageSnapshot`).
struct CLIError: Error {
    let message: String
}

/// Polls the bundled `cc-usage --json` binary on a timer and publishes the decoded snapshot.
/// Keeps the last successful snapshot when a refresh fails (stale-is-better-than-nothing,
/// mirroring the COSMIC applet's behavior).
final class DataController: ObservableObject {
    /// Last decoded snapshot (success OR a structured error document from the CLI).
    @Published private(set) var snapshot: UsageSnapshot?
    /// Last successful snapshot, retained across transient failures.
    @Published private(set) var lastGood: UsageSnapshot?
    /// A spawn/IO/decoding problem that isn't a CLI-reported error (e.g. binary missing).
    @Published private(set) var runtimeError: String?
    @Published private(set) var lastUpdated: Date?

    private var timer: Timer?
    private let decoder = JSONDecoder.ccUsage()

    /// Effective poll interval: from the latest config, else 300s.
    private var pollInterval: TimeInterval {
        TimeInterval(snapshot?.config.pollIntervalSecs ?? 300)
    }

    func start() {
        refresh()
        scheduleTimer()
    }

    private func scheduleTimer() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: pollInterval, repeats: true) { [weak self] _ in
            self?.refresh()
        }
    }

    /// Run the CLI off the main thread and publish results back on the main thread.
    func refresh() {
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self else { return }
            let result = self.runCLI()
            DispatchQueue.main.async {
                self.apply(result)
            }
        }
    }

    private func apply(_ result: Result<UsageSnapshot, CLIError>) {
        switch result {
        case .success(let snap):
            self.snapshot = snap
            self.runtimeError = nil
            if !snap.isError { self.lastGood = snap }
            self.lastUpdated = Date()
            // Re-arm the timer if the configured interval changed.
            if let t = timer, t.timeInterval != pollInterval { scheduleTimer() }
        case .failure(let error):
            self.runtimeError = error.message
            self.lastUpdated = Date()
        }
    }

    /// Locate the `cc-usage` binary and run it with `--json`.
    private func runCLI() -> Result<UsageSnapshot, CLIError> {
        let launch = Self.resolveLaunch()

        let process = Process()
        process.executableURL = URL(fileURLWithPath: launch.executable)
        process.arguments = launch.leadingArgs + ["--json"]

        let stdout = Pipe()
        process.standardOutput = stdout
        process.standardError = Pipe()

        do {
            try process.run()
        } catch {
            return .failure(CLIError(message: "failed to launch cc-usage: \(error.localizedDescription)"))
        }

        let data = stdout.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        // The CLI prints a valid JSON document even on failure (exit 1), so decode regardless.
        do {
            let snap = try decoder.decode(UsageSnapshot.self, from: data)
            return .success(snap)
        } catch {
            let raw = String(data: data, encoding: .utf8) ?? "<non-utf8>"
            return .failure(CLIError(message: "could not decode cc-usage output: \(error.localizedDescription)\n\(raw)"))
        }
    }

    /// How to launch the CLI: an explicit absolute path (`leadingArgs` empty), or
    /// `/usr/bin/env cc-usage …` as a PATH-based last resort.
    struct Launch {
        let executable: String
        let leadingArgs: [String]
    }

    /// Resolution order: $CC_USAGE_BIN → bundled Resources → next to the executable → PATH lookup.
    static func resolveLaunch() -> Launch {
        let fm = FileManager.default

        if let env = ProcessInfo.processInfo.environment["CC_USAGE_BIN"], fm.isExecutableFile(atPath: env) {
            return Launch(executable: env, leadingArgs: [])
        }
        if let res = Bundle.main.resourceURL?.appendingPathComponent("cc-usage").path,
           fm.isExecutableFile(atPath: res) {
            return Launch(executable: res, leadingArgs: [])
        }
        let exeDir = Bundle.main.bundleURL.deletingLastPathComponent()
        let sibling = exeDir.appendingPathComponent("cc-usage").path
        if fm.isExecutableFile(atPath: sibling) {
            return Launch(executable: sibling, leadingArgs: [])
        }
        // Last resort: resolve `cc-usage` from PATH.
        return Launch(executable: "/usr/bin/env", leadingArgs: ["cc-usage"])
    }
}
