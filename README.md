# claude-code-usage-indicator

Monitor your [Claude Code](https://docs.anthropic.com/en/docs/claude-code) usage budget at a
glance. Ships a **Linux COSMIC panel applet**, a **macOS menu bar app**, and a
cross-platform **CLI**, all sharing one Rust core.

## Showcase

| macOS menu bar | Linux (COSMIC panel applet) |
| --- | --- |
| ![macOS menu bar](docs/assets/showcase-macos.png) | ![Linux (COSMIC panel applet)](docs/assets/showcase.png) |

## Features

- **At-a-glance indicator** — color-coded weekly and session usage percentages, shown
  directly in the panel (Linux) or menu bar (macOS).
- **Dashboard popup** — weekly budget, session window, daily pace, and reset timers.
- **Pace-based coloring** — green / yellow / red based on whether you're on track for the
  cycle, not just raw usage.

## Project structure

A Cargo workspace plus a Swift package, split so it's clear which code targets which OS:

```
crates/
  cc-usage-budget/         Core logic — credentials, usage API, pace/color math.
                           Platform-agnostic, no GUI deps. Shared by everything.
  cc-usage-cli/            `cc-usage` binary — emits JSON (or human status).
                           Cross-platform; powers the macOS app and is handy in a terminal.
  cosmic-applet-cc-usage/  Linux / Pop!_OS COSMIC panel applet (libcosmic, Wayland).
macos/
  CcUsageMenuBar/          macOS menu bar app (Swift): AppKit NSStatusItem for the colored
                           bar item + SwiftUI popover dashboard. Runs the bundled CLI.
```

The root `Cargo.toml` is a virtual workspace; `cosmic-applet-cc-usage` is excluded from
default builds (it only compiles on Linux/Wayland), so `cargo build` / `cargo test` work on
macOS out of the box.

## Build & install

End users don't need [`just`](https://github.com/casey/just) — install via Homebrew (macOS,
below) or the raw `cargo`/`swift` commands. `just` is only a developer convenience that wraps
those commands and auto-dispatches per OS:

```sh
just build      # build the CLI + this platform's GUI
just install    # install this platform's GUI
just test       # test the cross-platform crates
```

### Linux (COSMIC applet)

Requires the [COSMIC desktop](https://github.com/pop-os/cosmic-epoch) and a Rust toolchain.

```sh
just install               # user-local install (no sudo); then add the applet to your panel
just install-system-linux  # system-wide (requires sudo)
```

`just install` auto-dispatches to the Linux recipe (`install-linux`); there are matching
`uninstall-linux` / `uninstall-system-linux` recipes.

**Starting with the desktop:** add the applet to your COSMIC panel (Panel settings → add
applet). The panel launches and manages it as part of your desktop session — there is no
separate login item to configure. This is unlike macOS (see below), where a menu bar app is
its own process that must opt into launching at login.

### macOS (menu bar app)

Install from a Homebrew tap (no `just` required — it builds the CLI and app from source, so
no Apple notarization is involved either):

```sh
brew install fuabioo/tap/cc-usage-menubar
open -a CcUsageMenuBar          # first launch adds the menu bar item
```

Requires the Rust toolchain (pulled in by the formula) and the Swift toolchain from the Xcode
Command Line Tools (`xcode-select --install`). A formula template lives at
[`packaging/homebrew/cc-usage-menubar.rb`](packaging/homebrew/cc-usage-menubar.rb).

<details>
<summary>Build from source without Homebrew</summary>

With `just`: `just run-macos` (build + bundle + launch) or `just install-macos` (copy the
`.app` into `~/Applications`). Without `just`, run the same steps directly:

```sh
cargo build --release -p cc-usage-cli
swift build -c release --package-path macos/CcUsageMenuBar
# then assemble CcUsageMenuBar.app (Swift binary + macos/CcUsageMenuBar/Resources/Info.plist
# + the cc-usage binary in Contents/Resources) and `codesign --sign -` it.
```

</details>

The result is a self-contained `CcUsageMenuBar.app` (ad-hoc signed) with the `cc-usage` CLI
bundled inside it.

#### Launching at login

macOS menu bar apps don't auto-start on their own. The go-to way is the app's own toggle:
**right-click the menu bar item → "Launch at Login"** (uses `SMAppService`; appears under
System Settings → General → Login Items). Alternatively, if you installed via the formula,
`brew services start cc-usage-menubar` registers a login agent. (On COSMIC there's no
equivalent step — the panel starts the applet for you.)

> **macOS credentials note:** Claude Code on macOS stores its OAuth token in the **login
> Keychain**, not in `~/.claude/.credentials.json` (the Linux location). The CLI/app read it
> automatically: if no credentials file is found, they fall back to the Keychain item
> `Claude Code-credentials`. The **first run shows a Keychain prompt** — click *Always Allow*.
> Override the item name with `--keychain-service NAME`, or disable the fallback with
> `--no-keychain`.

## CLI

`cc-usage` works on both platforms:

```sh
cc-usage --json          # machine-readable snapshot (the macOS app consumes this)
cc-usage --status        # human-readable report
cc-usage --creds-path /path/to/.credentials.json
cc-usage --daily-budget 20 --work-days 5
```

Other flags: `--timeout SECS` (HTTP request timeout, default 30), `--keychain-service NAME`
(macOS Keychain item, default `Claude Code-credentials`), and `--no-keychain`.

During development you can run the CLI through `just`: `just cli --status` (note: pass the
flags directly, with **no** leading `--`).

On failure it still prints a valid JSON document with an `error` object and exits non-zero.
Optional config file: `~/.config/cc-usage/config.toml` (`creds_path`, `daily_budget`,
`work_days`, `poll_interval_secs`); CLI flags override it. Defaults: `creds_path`
`~/.claude/.credentials.json`, `daily_budget` 20.0, `work_days` 5 (clamped 1–7),
`poll_interval_secs` 300 (minimum 30).

## Configuration (COSMIC applet)

Config is stored at `~/.config/cosmic/dev.fuabioo.CosmicAppletCcUsage/v1/` in RON format,
picked up automatically via hot-reload.

### Custom colors

Override any pace color by creating a file named for the color field:

```sh
echo 'Some((r:0.3,g:0.85,b:0.4,a:1.0))' > ~/.config/cosmic/dev.fuabioo.CosmicAppletCcUsage/v1/color_on_track
```

Available fields: `color_on_track`, `color_warning`, `color_over_budget`. To revert to theme
defaults, delete the file or set its contents to `None`.

On **macOS** the pace colors are not configurable; the app derives appearance-adaptive
colors (a darker, contrast-correct variant in Light mode and a brighter one in Dark mode) and
follows the system Light/Dark/Auto setting automatically — analogous to how the COSMIC applet
inherits its theme.

## Development

### Verifying macOS pace-color contrast

The macOS app can render its dashboard to PNGs in both appearances so you can eyeball the
foreground/background contrast of the green/amber/red pace colors without opening the popover:

```sh
just swatches            # writes target/macos/dashboard-{light,dark}.png
just swatches /tmp       # or choose an output directory
```

This runs the app's headless `--render-swatches DIR` mode, which exits before creating any
menu bar item. (`ProgressView` bars don't snapshot correctly under the offscreen renderer —
that's a rendering-only artifact, not how the live popover looks.)

### Releasing

Pushing a `v*` tag runs [`.github/workflows/release.yml`](.github/workflows/release.yml),
which uses [GoReleaser](https://goreleaser.com) (config in
[`.goreleaser.yaml`](.goreleaser.yaml)) to cross-compile the `cc-usage` CLI for Linux and
macOS (via `cargo-zigbuild`), publish a GitHub release with archives + checksums, and update
the `cc-usage` Homebrew cask in the tap.

- Validate the config locally with `goreleaser check`.
- Requires a `HOMEBREW_TAP_GITHUB_TOKEN` repo secret — a PAT with write access to the tap repo
  (the built-in `GITHUB_TOKEN` cannot push to another repository).
- Only the **CLI** is released this way. The macOS **menu bar app** is intentionally not
  shipped prebuilt (no Apple Developer ID / notarization); it stays a build-from-source
  Homebrew formula — see [`packaging/homebrew/cc-usage-menubar.rb`](packaging/homebrew/cc-usage-menubar.rb).

## License

MIT
