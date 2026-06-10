import AppKit
import Combine
import SwiftUI

/// Owns the menu bar item. Renders the at-a-glance status directly into the bar button
/// as a colored, non-template NSImage (weekly% · session% tinted by pace), and toggles a
/// SwiftUI popover dashboard on click.
final class StatusItemController {
    private static let popoverWidth: CGFloat = 280

    private let statusItem: NSStatusItem
    private let popover = NSPopover()
    private let hostingController: NSHostingController<DashboardView>
    private let controller: DataController
    private var cancellables = Set<AnyCancellable>()

    init(controller: DataController) {
        self.controller = controller
        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        self.hostingController = NSHostingController(rootView: DashboardView(controller: controller))
        // Let the hosting controller drive the popover's content size as SwiftUI relayouts,
        // so the popover never anchors using a stale/oversized fitting size.
        hostingController.sizingOptions = [.preferredContentSize]

        popover.behavior = .transient
        popover.contentViewController = hostingController
        popover.contentSize = NSSize(width: Self.popoverWidth, height: 200)

        if let button = statusItem.button {
            button.image = Self.placeholderImage()
            button.imagePosition = .imageOnly
            button.target = self
            button.action = #selector(handleClick(_:))
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }

        // Redraw the bar whenever data changes.
        controller.$snapshot
            .receive(on: RunLoop.main)
            .sink { [weak self] _ in self?.updateBar() }
            .store(in: &cancellables)
        controller.$runtimeError
            .receive(on: RunLoop.main)
            .sink { [weak self] _ in self?.updateBar() }
            .store(in: &cancellables)
    }

    // MARK: - Click handling

    @objc private func handleClick(_ sender: NSStatusBarButton) {
        let event = NSApp.currentEvent
        if event?.type == .rightMouseUp {
            showMenu()
        } else {
            togglePopover(sender)
        }
    }

    private func togglePopover(_ sender: NSStatusBarButton) {
        if popover.isShown {
            popover.performClose(sender)
        } else {
            // Force a SwiftUI layout pass and pin the popover to the resulting fitting size
            // BEFORE showing, so it anchors at the correct height (no post-show shrink/float).
            hostingController.view.layoutSubtreeIfNeeded()
            var fitting = hostingController.view.fittingSize
            if fitting.width <= 0 { fitting.width = Self.popoverWidth }
            if fitting.height <= 0 { fitting.height = 200 }
            popover.contentSize = fitting

            popover.show(relativeTo: sender.bounds, of: sender, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
        }
    }

    private func showMenu() {
        let menu = NSMenu()
        menu.addItem(withTitle: "Refresh", action: #selector(refresh), keyEquivalent: "r").target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit", action: #selector(quit), keyEquivalent: "q").target = self

        statusItem.menu = menu
        statusItem.button?.performClick(nil)
        statusItem.menu = nil // detach so left-click returns to popover behavior
    }

    @objc private func refresh() { controller.refresh() }
    @objc private func quit() { NSApp.terminate(nil) }

    // MARK: - Bar rendering

    private func updateBar() {
        guard let button = statusItem.button else { return }

        // Prefer live success data; fall back to last good; else show an error glyph.
        let snap = (controller.snapshot?.isError == false) ? controller.snapshot : controller.lastGood

        if let snap, let weekly = snap.weekly, let session = snap.session {
            button.image = Self.barImage(weekly: weekly, session: session)
        } else {
            button.image = Self.errorImage()
        }
    }

    // MARK: - Image drawing

    /// Draw "W% · S%" with each percentage tinted by its pace color.
    private static func barImage(weekly: WindowDTO, session: WindowDTO) -> NSImage {
        let font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium)
        let sep = NSColor.secondaryLabelColor

        let attributed = NSMutableAttributedString()
        attributed.append(segment(String(format: "%.0f%%", weekly.utilization),
                                   color: weekly.paceColor.nsColor, font: font))
        attributed.append(segment(" · ", color: sep, font: font))
        attributed.append(segment(String(format: "%.0f%%", session.utilization),
                                   color: session.paceColor.nsColor, font: font))
        return render(attributed)
    }

    private static func errorImage() -> NSImage {
        let font = NSFont.systemFont(ofSize: 13, weight: .medium)
        let attributed = segment("⚠ CC", color: .systemRed, font: font)
        return render(attributed)
    }

    private static func placeholderImage() -> NSImage {
        let font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium)
        return render(segment("… CC", color: .secondaryLabelColor, font: font))
    }

    private static func segment(_ s: String, color: NSColor, font: NSFont) -> NSAttributedString {
        NSAttributedString(string: s, attributes: [.foregroundColor: color, .font: font])
    }

    /// Rasterize an attributed string into a non-template (colored) menu bar image.
    private static func render(_ attributed: NSAttributedString) -> NSImage {
        let size = attributed.size()
        let height: CGFloat = 18 // fits the ~22pt menu bar with padding
        let imageSize = NSSize(width: ceil(size.width), height: height)

        let image = NSImage(size: imageSize)
        image.lockFocus()
        let y = (height - size.height) / 2
        attributed.draw(at: NSPoint(x: 0, y: y))
        image.unlockFocus()
        image.isTemplate = false // keep our colors; do not let the system tint it
        return image
    }
}
