import AppKit

/// AppKit bootstrap for an agent (menu-bar-only) app. `LSUIElement` in Info.plist keeps it
/// out of the Dock; `.accessory` activation policy is the runtime equivalent so it also
/// works when launched directly via `swift run` (no bundle).
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let dataController = DataController()
    private var statusController: StatusItemController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Headless verification mode (no status item): `CcUsageMenuBar --render-swatches DIR`.
        // Handled here (rather than before app launch) so it runs in this main-actor context,
        // which `ImageRenderer` requires; we exit before creating any menu bar item.
        if let i = CommandLine.arguments.firstIndex(of: "--render-swatches"), i + 1 < CommandLine.arguments.count {
            SwatchRenderer.run(outputDir: CommandLine.arguments[i + 1])
            exit(0)
        }

        statusController = StatusItemController(controller: dataController)
        dataController.start()
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
