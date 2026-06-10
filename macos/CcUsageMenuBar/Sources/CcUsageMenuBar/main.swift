import AppKit

/// AppKit bootstrap for an agent (menu-bar-only) app. `LSUIElement` in Info.plist keeps it
/// out of the Dock; `.accessory` activation policy is the runtime equivalent so it also
/// works when launched directly via `swift run` (no bundle).
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let dataController = DataController()
    private var statusController: StatusItemController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusController = StatusItemController(controller: dataController)
        dataController.start()
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
