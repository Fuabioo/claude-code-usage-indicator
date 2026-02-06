# ADR-004: Technical Implementation Plan

| Field       | Value                                                    |
|-------------|----------------------------------------------------------|
| Status      | Proposed                                                 |
| Date        | 2025-02-05                                               |
| Depends on  | ADR-001, ADR-002, ADR-003                                |
| Authors     | fuabioo                                                  |

## Context

This ADR specifies the concrete Rust project structure, module decomposition, dependency manifest, build system, and installation procedure for the COSMIC panel applet.

## Decision

### Project Name and Identity

| Property     | Value                                            |
|--------------|--------------------------------------------------|
| Binary name  | `cosmic-applet-cc-usage`                         |
| App ID       | `dev.fuabioo.CosmicAppletCcUsage`                |
| Display name | "Claude Code Usage"                              |
| Icon         | `dev.fuabioo.CosmicAppletCcUsage`                |

### Project Structure

```
claude-code-usage-indicator/
+-- Cargo.toml
+-- Cargo.lock
+-- justfile
+-- i18n.toml
+-- docs/
|   +-- ADR/
|       +-- 001-project-overview-and-architecture.md
|       +-- 002-data-source-and-budget-calculation.md
|       +-- 003-ux-design-and-visual-layout.md
|       +-- 004-technical-implementation-plan.md
+-- i18n/
|   +-- en/
|       +-- cosmic_applet_cc_usage.ftl
+-- resources/
|   +-- dev.fuabioo.CosmicAppletCcUsage.desktop
|   +-- dev.fuabioo.CosmicAppletCcUsage.metainfo.xml
|   +-- icons/
|       +-- dev.fuabioo.CosmicAppletCcUsage.svg
+-- src/
    +-- main.rs           # Entry point: cosmic::applet::run
    +-- app.rs            # AppModel, Message enum, Application trait impl
    +-- config.rs         # CosmicConfigEntry for persistent settings
    +-- i18n.rs           # Localization bootstrap
    +-- budget/
    |   +-- mod.rs        # Re-exports
    |   +-- api.rs        # HTTP client for Anthropic OAuth usage API
    |   +-- types.rs      # UsageResponse, BudgetState, WindowState, PaceColor
    |   +-- pace.rs       # Pace-based coloring algorithm
    |   +-- credentials.rs # Credential file reader
    +-- views/
        +-- mod.rs        # Re-exports
        +-- panel.rs      # Panel icon view rendering
        +-- popup.rs      # Popup detail view rendering
```

### Cargo.toml

```toml
[package]
name = "cosmic-applet-cc-usage"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "COSMIC panel applet for monitoring Claude Code usage budget"

[dependencies]
# COSMIC / GUI
libcosmic = { git = "https://github.com/pop-os/libcosmic.git", features = [
    "applet",
    "applet-token",
    "tokio",
    "wayland",
    "winit",
] }

# Async
tokio = { version = "1", features = ["full"] }
futures-util = "0.3"

# HTTP
reqwest = { version = "0.12", default-features = false, features = [
    "rustls-tls",
    "json",
] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Logging
tracing = "0.1"

# i18n
i18n-embed = { version = "0.16", features = ["fluent-system", "desktop-requester"] }
i18n-embed-fl = "0.10"
rust-embed = "8"

# Error handling
thiserror = "2"
```

### Module Responsibilities

#### `src/main.rs`

Minimal entry point:

```rust
// Initialize i18n, launch applet
fn main() -> cosmic::iced::Result {
    // ...
    cosmic::applet::run::<app::AppModel>(())
}
```

#### `src/app.rs`

The core application model implementing `cosmic::Application`:

**State**:
```rust
struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    config: Config,
    budget: Option<BudgetState>,  // None until first fetch
    error: Option<BudgetError>,
}
```

**Messages**:
```rust
enum Message {
    TogglePopup,
    PopupClosed(Id),
    BudgetUpdate(Result<BudgetState, BudgetError>),
    ConfigChanged(Config),
}
```

**Key trait methods**:
- `view()` -- delegates to `views::panel::render()`
- `view_window()` -- delegates to `views::popup::render()`
- `update()` -- handles messages, updates state
- `subscription()` -- returns batched subscriptions for polling + config watch

#### `src/config.rs`

Persistent configuration via `cosmic-config`:

```rust
#[derive(Debug, Default, Clone, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct Config {
    /// Polling interval in seconds
    pub poll_interval_secs: u64,      // default: 300
    /// Number of work days per week
    pub work_days: u8,                // default: 5
    /// Expected usage % per work day
    pub daily_budget: f64,            // default: 20.0
    /// Path to Claude credentials file (supports ~ expansion)
    pub credentials_path: String,     // default: ~/.claude/.credentials.json
}
```

#### `src/budget/api.rs`

Async HTTP client:

```rust
pub async fn fetch_usage(token: &str) -> Result<UsageResponse, BudgetError> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/oauth/usage")
        .bearer_auth(token)
        .send()
        .await
        .map_err(BudgetError::Network)?;

    match resp.status() {
        StatusCode::OK => {
            let body = resp.json::<UsageResponse>().await
                .map_err(BudgetError::Parse)?;
            Ok(body)
        }
        StatusCode::UNAUTHORIZED => Err(BudgetError::Unauthorized),
        StatusCode::TOO_MANY_REQUESTS => Err(BudgetError::RateLimited),
        status => Err(BudgetError::UnexpectedStatus(status)),
    }
}
```

#### `src/budget/pace.rs`

Pure function, no side effects, easily testable:

```rust
pub fn compute_pace_color(
    utilization: f64,
    daily_budget: f64,
    work_days: u8,
    now: DateTime<Local>,
) -> PaceColor {
    let work_day_index = weekday_to_work_index(now.weekday(), work_days);
    let ceiling = work_day_index as f64 * daily_budget;

    if ceiling <= 0.0 {
        return PaceColor::Red;  // Defensive: avoid division by zero
    }

    let ratio = utilization / ceiling;

    if ratio < 0.75 {
        PaceColor::Green
    } else if ratio < 1.00 {
        PaceColor::Yellow
    } else {
        PaceColor::Red
    }
}

fn weekday_to_work_index(weekday: Weekday, work_days: u8) -> u8 {
    match weekday {
        Weekday::Mon => 1.min(work_days),
        Weekday::Tue => 2.min(work_days),
        Weekday::Wed => 3.min(work_days),
        Weekday::Thu => 4.min(work_days),
        Weekday::Fri => 5.min(work_days),
        Weekday::Sat | Weekday::Sun => work_days,  // Weekend uses last work day ceiling
    }
}
```

#### `src/budget/credentials.rs`

```rust
pub fn read_token(path: &Path) -> Result<String, BudgetError> {
    let content = fs::read_to_string(path)
        .map_err(|e| BudgetError::CredentialsRead(e))?;
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| BudgetError::CredentialsParse(e))?;
    parsed
        .get("claudeAiOauth")
        .and_then(|o| o.get("accessToken"))
        .and_then(|t| t.as_str())
        .map(String::from)
        .ok_or(BudgetError::CredentialsMissingToken)
}
```

#### `src/budget/types.rs`

All data structures and error types (see ADR-002 for full model).

#### `src/views/panel.rs`

Panel icon rendering logic:

```rust
pub fn render(app: &AppModel) -> Element<'_, Message> {
    // Determine panel size from core.applet
    // Render colored text based on budget state
    // Attach tooltip with full summary
    // Attach on_press -> Message::TogglePopup
}
```

#### `src/views/popup.rs`

Popup detail view rendering:

```rust
pub fn render(app: &AppModel, id: Id) -> Element<'_, Message> {
    // Build sections: Weekly, Hourly, Daily Pace, Footer
    // Use widget::list_column, widget::settings::item, widget::progress_bar
    // Wrap in self.core.applet.popup_container()
}
```

### Error Type

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum BudgetError {
    #[error("network error: {0}")]
    Network(String),  // Store Display string, not the original error (for Clone)
    #[error("failed to parse API response: {0}")]
    Parse(String),
    #[error("unauthorized -- token may be expired")]
    Unauthorized,
    #[error("rate limited by API")]
    RateLimited,
    #[error("unexpected HTTP status: {0}")]
    UnexpectedStatus(u16),
    #[error("cannot read credentials file: {0}")]
    CredentialsRead(String),
    #[error("cannot parse credentials file: {0}")]
    CredentialsParse(String),
    #[error("credentials file missing access token field")]
    CredentialsMissingToken,
}
```

### Subscription Architecture

```
subscription() returns Subscription::batch([
    |
    +-- [1] Budget Poller
    |       Subscription::run_with_id("budget-poll", channel(4, |tx| async {
    |           loop {
    |               let result = fetch_and_compute().await;
    |               tx.send(Message::BudgetUpdate(result)).await;
    |               tokio::time::sleep(poll_interval).await;
    |           }
    |       }))
    |
    +-- [2] Config Watcher
            self.core.watch_config::<Config>(APP_ID)
                .map(|u| Message::ConfigChanged(u.config))
])
```

### Desktop Entry File

`resources/dev.fuabioo.CosmicAppletCcUsage.desktop`:

```ini
[Desktop Entry]
Name=Claude Code Usage
Comment=Monitor Claude Code weekly and hourly usage budget
Type=Application
Icon=dev.fuabioo.CosmicAppletCcUsage
Exec=cosmic-applet-cc-usage
Terminal=false
StartupNotify=true
Categories=COSMIC;System;Monitor;
Keywords=claude;ai;usage;budget;
NoDisplay=true
X-CosmicApplet=true
X-CosmicHoverPopup=Auto
```

### Justfile (Build System)

```just
name := 'cosmic-applet-cc-usage'
appid := 'dev.fuabioo.CosmicAppletCcUsage'

prefix := '/usr'
bindir := prefix / 'bin'
appdir := prefix / 'share' / 'applications'
iconsdir := prefix / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'

default: build-release

build-debug:
    cargo build

build-release:
    cargo build --release

install:
    install -Dm0755 target/release/{{name}} {{bindir}}/{{name}}
    install -Dm0644 resources/{{appid}}.desktop {{appdir}}/{{appid}}.desktop
    install -Dm0644 resources/icons/{{appid}}.svg {{iconsdir}}/{{appid}}.svg

uninstall:
    rm -f {{bindir}}/{{name}}
    rm -f {{appdir}}/{{appid}}.desktop
    rm -f {{iconsdir}}/{{appid}}.svg

clean:
    cargo clean
```

### Testing Strategy

| Test Type         | Scope                              | Tool           |
|-------------------|------------------------------------|----------------|
| Unit tests        | `budget::pace`, `budget::types`    | `cargo test`   |
| Unit tests        | `budget::credentials` (with fixtures) | `cargo test`|
| Integration test  | `budget::api` (mock server)        | `cargo test` + wiremock |
| Manual testing    | Full applet in COSMIC panel        | `just install` + panel restart |

**Key unit test cases for `pace.rs`**:
- Monday 0% usage -> Green
- Monday 20%+ usage -> Red
- Wednesday 45% usage -> Yellow (ratio = 45/60 = 0.75)
- Friday 100% usage -> Yellow (ratio = 100/100 = 1.0 -> Red boundary)
- Saturday 80% usage -> Yellow (uses Friday ceiling)
- Edge: 0 daily_budget -> Red (defensive)
- Edge: utilization > 100% -> Red

### Implementation Phases

#### Phase 1: Skeleton + Data (MVP)

- [ ] Scaffold project from `cosmic-applet-template`
- [ ] Implement `budget::credentials` -- read OAuth token
- [ ] Implement `budget::api` -- fetch usage data
- [ ] Implement `budget::types` -- data model
- [ ] Implement `budget::pace` -- coloring algorithm with unit tests
- [ ] Wire up `app.rs` with polling subscription
- [ ] Implement `views::panel` -- colored percentage text in panel
- [ ] Basic tooltip

**Definition of done**: Panel shows colored `42% | 15%` text, updates every 5 minutes.

#### Phase 2: Popup Dashboard

- [ ] Implement `views::popup` -- full dashboard layout
- [ ] Weekly section with progress bar
- [ ] Hourly section with progress bar
- [ ] Daily budget pace section with ceiling calculation
- [ ] Footer with staleness indicator

**Definition of done**: Clicking panel icon opens popup with all sections from ADR-003.

#### Phase 3: Polish + Configuration

- [ ] Implement `config.rs` with cosmic-config
- [ ] Config watcher subscription
- [ ] Panel size responsiveness (Small/Medium/Large/XL)
- [ ] Error states (missing credentials, API failures)
- [ ] i18n strings
- [ ] Desktop entry and icon

**Definition of done**: Fully installable applet with settings persistence.

#### Phase 4: Future Enhancements (Post-MVP)

- [ ] Configurable notification thresholds (e.g., alert at 80%)
- [ ] Usage history chart (requires local storage)
- [ ] Multiple account support
- [ ] Custom icon based on usage level (depleting battery metaphor)

## Alternatives Considered

### 1. Single-file architecture (all in `app.rs`)

Rejected. The budget logic, API client, and view rendering are distinct concerns. Separate modules enable unit testing of the pace algorithm without GUI dependencies.

### 2. Use `ureq` instead of `reqwest` for HTTP

Considered. `ureq` is simpler and blocking-friendly, but since we're already in a tokio context (via libcosmic), `reqwest` integrates more naturally with async subscriptions and avoids blocking the UI thread.

### 3. Store usage history in SQLite

Deferred to Phase 4. MVP doesn't need historical data. If added later, can use `rusqlite` or even the existing Go tool's database.

### 4. Credential file watching via inotify

Deferred. The credential file changes rarely (only on re-auth). Reading on each poll cycle (every 5 min) is sufficient. Can add `notify` crate later if needed.

## References

- [cosmic-applet-template](https://github.com/pop-os/cosmic-applet-template) -- project structure reference
- [cosmic-applet-time](https://github.com/pop-os/cosmic-applets/tree/master/cosmic-applet-time) -- subscription patterns
- [cosmic-applet-battery](https://github.com/pop-os/cosmic-applets/tree/master/cosmic-applet-battery) -- progress bar and threshold UI patterns
- [minimon-applet](https://github.com/cosmic-utils/minimon-applet) -- community applet with polling
- [thiserror crate](https://docs.rs/thiserror) -- error type derivation
- [reqwest crate](https://docs.rs/reqwest) -- HTTP client
