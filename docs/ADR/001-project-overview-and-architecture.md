# ADR-001: Project Overview and Architecture

| Field       | Value                                                    |
|-------------|----------------------------------------------------------|
| Status      | Proposed                                                 |
| Date        | 2025-02-05                                               |
| Supersedes  | N/A                                                      |
| Authors     | fuabioo                                                  |

## Context

For several months, a Go-based status line tool (`my-cc-status-line`) has been displaying Claude Code usage metrics inside the Claude Code terminal status bar. The core widgets are:

```
<weekly-session-usage-%> <time-till-weekly-reset> | <hourly-session-usage-%> <time-till-hourly-reset>
```

With pace-based color coding (green/yellow/red) relative to daily budget thresholds.

**Problem**: These metrics are only visible when the Claude Code terminal is active. Since Claude Code usage budget is a system-wide concern (it affects all concurrent sessions and planning for the work week), the data should be available at all times from the OS desktop panel -- not buried inside one terminal instance.

**Target platform**: Pop!_OS with the COSMIC desktop environment.

## Decision

Build a native **COSMIC panel applet** in Rust that surfaces Claude Code usage metrics directly in the OS panel (system tray area). The applet will:

1. Display a compact usage indicator icon/text in the panel bar
2. Provide a popup window with detailed budget breakdown on click
3. Poll the Anthropic OAuth usage API on a configurable interval
4. Apply pace-based color coding identical to the existing Go tool

## Architecture

### High-Level Component Diagram

```
+-------------------------------+
|     COSMIC Panel (Wayland)    |
|  +-------------------------+  |
|  |  Panel Icon View        |  |  <- Compact: colored % or icon + %
|  |  (always visible)       |  |
|  +----------||--------------+  |
|             ||                 |
|  +----------vv--------------+  |
|  |  Popup Detail View       |  |  <- Expanded: full budget dashboard
|  |  (on click/hover)        |  |
|  +-------------------------+  |
+-------------------------------+
        |                |
        v                v
+---------------+  +------------------+
|  Budget       |  |  Config          |
|  Service      |  |  (cosmic-config) |
|  (async)      |  |  ~/.config/      |
+-------|-------+  +------------------+
        |
        v
+------------------+
|  Anthropic OAuth |
|  Usage API       |
|  (HTTPS)         |
+------------------+
```

### Component Responsibilities

| Component            | Responsibility                                                   |
|----------------------|------------------------------------------------------------------|
| **Panel Icon View**  | Renders compact usage indicator in the panel bar                 |
| **Popup View**       | Renders detailed budget dashboard when panel icon is activated   |
| **Budget Service**   | Async polling of Anthropic API, caching, pace computation        |
| **Config**           | Persistent user preferences via `cosmic-config`                  |
| **Credential Reader**| Reads OAuth token from `~/.claude/.credentials.json`             |

### Technology Stack

| Layer           | Choice                                              | Rationale                                                                                              |
|-----------------|-----------------------------------------------------|--------------------------------------------------------------------------------------------------------|
| Language        | Rust                                                | Required by COSMIC; also aligns with performance/safety goals                                          |
| GUI Framework   | libcosmic (iced-based)                              | Required for COSMIC panel integration                                                                  |
| Async Runtime   | tokio (via libcosmic `tokio` feature)               | Standard async runtime, bundled with libcosmic applet feature                                          |
| HTTP Client     | reqwest                                             | Mature, tokio-native, TLS support                                                                      |
| Serialization   | serde + serde_json                                  | De-facto Rust standard for JSON handling                                                               |
| Time            | chrono + chrono-tz                                  | Timezone-aware date arithmetic for work-day budget calculations                                        |
| Configuration   | cosmic-config (CosmicConfigEntry derive)            | Native COSMIC settings integration, auto file-watch, schema versioning                                 |
| Credentials     | Direct file read of `~/.claude/.credentials.json`   | Same source as the Claude CLI itself                                                                   |

## Alternatives Considered

### 1. Keep it in the terminal status line (status quo)

**Rejected** because the usage budget is a system-wide, always-relevant concern. Metrics are invisible when switching to other applications or when Claude Code is not in the foreground.

### 2. Generic system tray application (e.g., GTK StatusNotifierItem)

**Rejected** because COSMIC has its own panel applet protocol. A generic tray icon would lack native COSMIC theming, popup integration, and panel-aware sizing. It would also be a second-class citizen in the COSMIC desktop.

### 3. Port the existing Go tool as a panel applet

**Rejected** because COSMIC applets must be Rust binaries using libcosmic. Go cannot link against the iced/libcosmic framework. A clean Rust rewrite of the budget logic (which is the only piece we need) is straightforward.

### 4. GNOME Shell extension or Cinnamon applet

**Rejected** because the target environment is COSMIC on Pop!_OS, not GNOME.

## Consequences

### Positive

- Usage metrics visible at all times regardless of active application
- Native COSMIC look-and-feel with theme integration
- Independent process -- crash isolation from Claude Code itself
- Popup provides more screen real estate for detailed budget views
- Can be extended later with notifications (e.g., "you've hit 80% of today's budget")

### Negative

- New codebase to maintain (Rust instead of Go)
- Tied to COSMIC desktop -- won't work on GNOME/KDE (acceptable since this is a personal tool)
- libcosmic is pre-1.0 and API may change between COSMIC releases
- Requires manual installation (build, `just install`, add via Settings > Panel)

### Risks

| Risk                                    | Mitigation                                                          |
|-----------------------------------------|---------------------------------------------------------------------|
| libcosmic API breaks on COSMIC updates  | Pin libcosmic git rev in Cargo.toml; update deliberately            |
| Anthropic API changes response format   | Isolate API types behind a trait; version the response parser       |
| Credential file format changes          | Read defensively with serde; log clear errors on parse failure      |
| Panel applet not discovered after install | Validate .desktop file; document manual `cosmic-panel` restart      |

## References

- [COSMIC Toolkit Book -- Panel Applets](https://pop-os.github.io/libcosmic-book/panel-applets.html)
- [pop-os/cosmic-applet-template](https://github.com/pop-os/cosmic-applet-template)
- [pop-os/cosmic-applets](https://github.com/pop-os/cosmic-applets) (battery, time, power)
- [cosmic-utils/minimon-applet](https://github.com/cosmic-utils/minimon-applet) (community monitoring applet)
- Existing `my-cc-status-line` Go project for budget calculation reference
