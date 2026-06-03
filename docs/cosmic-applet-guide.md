# Building a COSMIC Panel Applet (libcosmic) — A Practical Guide

This guide walks through building a **COSMIC panel applet** in Rust + libcosmic, end to
end, grounded in a complete working reference: **cosmic-applet-cc-usage** (this repo), an
applet that monitors Claude Code usage budget. Every API signature, path, and feature
name below was verified against the actual source in this repo and the pinned libcosmic
checkout. Where something is uncertain it is flagged explicitly.

The goal is that you can build a *new* applet from scratch — for example a "codex usage"
monitor or a "better clock" — by copying the patterns here. A copy-and-rename checklist
is at the end (§12).

All file references are relative to the repo root unless noted. The pinned libcosmic
source lives at:

```
~/.cargo/git/checkouts/libcosmic-41009aea1d72760b/59bebbd/
```

References to it below are written as `libcosmic:src/...`.

---

## 1. Overview

A **COSMIC panel applet** is a standalone executable that the COSMIC panel
(`cosmic-panel`) launches as a *separate process*. The panel hosts each applet by
embedding its Wayland surface into the panel bar. This means:

- An applet is a normal binary on `$PATH` (e.g. `~/.local/bin/cosmic-applet-cc-usage`),
  discovered by the panel via a `.desktop` file with `X-CosmicApplet=true`.
- Crashes are isolated to the applet process — but the panel will *respawn* a crashing
  applet, which can produce a crash loop (see §11).
- Rendering is via **libcosmic**, which wraps **iced** and the COSMIC theme. iced uses
  the **Elm architecture**:

  - **Model** — your `AppModel` struct holds all state.
  - **Message** — an enum of every event that can mutate state.
  - **`update(&mut self, Message) -> Task<Message>`** — the only place state changes;
    returns async follow-up work as a `Task`.
  - **`view(&self) -> Element<Message>`** — a pure function from state to widgets. Widgets
    emit `Message`s. Never mutate state in `view`.

The panel surface is small (the bar icon). Clicking it opens a **popup** — a second
Wayland surface, anchored to the panel button, rendered by `view_window`.

libcosmic at the pinned rev requires **edition 2024 / rustc ≥ 1.93** (verified:
`libcosmic:Cargo.toml` has `rust-version = "1.93"`). This repo builds fine on rustc 1.95.

---

## 2. Project scaffolding

### `Cargo.toml`

The reference `Cargo.toml` pins libcosmic to a specific git rev and selects a feature
set. The pin is **load-bearing** — see §11 for why you must keep it contemporaneous with
your installed COSMIC.

```toml
[package]
name = "cosmic-applet-cc-usage"
version = "0.1.0"
edition = "2021"          # your crate can be 2021; libcosmic itself is 2024
license = "MIT"

[dependencies]
libcosmic = { git = "https://github.com/pop-os/libcosmic.git", rev = "59bebbdffeb8ed4abd3f2577c099745b9a2f2c37", features = [
    "applet",        # applet helpers: Context, autosize_window, popup_container, etc.
    "applet-token",  # token/auth helpers for applets
    "tokio",         # tokio executor for async Tasks
    "wayland",       # Wayland surface support (popups, subsurfaces)
    "winit",         # winit windowing backend
] }

# Async runtime (used by reqwest + cosmic::task::future)
tokio = { version = "1", features = ["time"] }

# Config persistence — REQUIRED as a direct dep: the CosmicConfigEntry derive macro
# generates code that emits RON. Without `ron` in scope the derive won't compile.
ron = "0.12"

# i18n (Fluent)
i18n-embed = { version = "0.16", features = ["fluent-system", "desktop-requester"] }
i18n-embed-fl = "0.10"
rust-embed = "8"

# Whatever your data source needs (this applet polls an HTTP API):
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
```

**Feature notes** (what each gives you):

- `applet` — the `cosmic::applet::Context` and its methods: `autosize_window`,
  `popup_container`, `get_popup_settings`, `applet_tooltip`, plus `cosmic::applet::run`
  and `cosmic::applet::style`.
- `applet-token` — applet token/auth integration. Enabled by the reference applet; keep
  it unless you know you don't need it.
- `tokio` — provides `cosmic::executor::Default` backed by tokio, required for
  `cosmic::task::future`/`cosmic::task::message`.
- `wayland` — gates the surface/popup machinery (`cosmic::surface::action::app_popup`,
  `destroy_popup`, etc. are `#[cfg(all(feature = "wayland", target_os = "linux"))]`).
- `winit` — windowing backend; some surface helpers additionally require `winit`
  (verified: `app_popup` is gated on `wayland` + `target_os = "linux"` + `winit` in
  `libcosmic:src/surface/action.rs`).

### Minimal `src/main.rs`

The entry point is a single call. `cosmic::applet::run::<App>` takes **one argument**
(your `Application::Flags`):

```rust
mod app;
mod config;
mod i18n;
mod views;

use app::AppModel;

fn main() -> cosmic::iced::Result {
    // Initialize i18n before launching (optional but recommended).
    if let Err(e) = i18n::init() {
        eprintln!("[my-applet] failed to init i18n: {e}");
    }
    cosmic::applet::run::<AppModel>(())
}
```

Verified signature (`libcosmic:src/applet/mod.rs:535`):

```rust
pub fn run<App: Application>(flags: App::Flags) -> iced::Result
```

The reference `src/main.rs` also adds a `--status` diagnostic subcommand that fetches data
and prints the computed numbers without launching the GUI — a useful pattern for auditing
your data layer headlessly.

---

## 3. The `Application` trait

Your model implements `cosmic::Application`. The reference impl is `src/app.rs`. Below is
each method with the real signatures and the reference behavior.

### The model and message enum

```rust
use cosmic::app::Core;
use cosmic::iced::window;
use cosmic::{Application, Element};

pub struct AppModel {
    pub core: Core,                  // REQUIRED — libcosmic plumbing
    pub popup: Option<window::Id>,   // the currently-open popup surface, if any
    pub config: Config,
    pub budget: Option<BudgetState>, // your data
    pub error: Option<BudgetError>,
    pub client: reqwest::Client,     // reused across polls (don't build per-poll)
    // ... backoff/retry bookkeeping ...
}

#[derive(Debug, Clone)]
pub enum Message {
    PopupClosed(window::Id),
    Tick,                                          // poll timer fired
    BudgetUpdate(Result<BudgetState, BudgetError>),// async fetch landed
    CosmicConfigUpdate(Config),                    // hot-reload from cosmic_config
    Surface(cosmic::surface::Action),              // MUST be forwarded — see below
}
```

The `Core` is mandatory state libcosmic uses for theming, panel anchor, window IDs, etc.
You expose it via `core()`/`core_mut()`.

### `init`

```rust
const APP_ID: &str = "dev.fuabioo.CosmicAppletCcUsage";

impl Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core { &self.core }
    fn core_mut(&mut self) -> &mut Core { &mut self.core }

    fn init(core: Core, _flags: Self::Flags) -> (Self, cosmic::app::Task<Self::Message>) {
        // Load persisted config (see §6)
        let config = match cosmic::cosmic_config::Config::new(APP_ID, config::CONFIG_VERSION) {
            Ok(helper) => match Config::get_entry(&helper) {
                Ok(c) => c.validated(),
                Err((_errs, c)) => c.validated(), // partial config: take what loaded
            },
            Err(_) => Config::default(),
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        // Kick off an immediate first fetch as the initial Task.
        let first_fetch = cosmic::task::future(async move {
            Message::BudgetUpdate(/* ...fetch... */)
        });

        let app = AppModel { core, popup: None, config, /* ... */ };
        (app, first_fetch)
    }
```

`init` returns `(Self, Task)`. The `Task` is async work run after construction — here, the
first data fetch so the panel doesn't sit empty until the first timer tick.

### `view` — the panel surface

```rust
fn view(&self) -> Element<'_, Self::Message> {
    self.core
        .applet
        .autosize_window(crate::views::panel::render(self))
        .into()
}
```

`autosize_window` wraps your panel content so the surface sizes itself to fit (verified
`libcosmic:src/applet/mod.rs:458`):

```rust
pub fn autosize_window<'a, Message: 'static>(
    &self,
    content: impl Into<Element<'a, Message>>,
) -> Autosize<'a, Message, crate::Theme, crate::Renderer>
```

### `view_window` — popup surfaces (called per surface id)

```rust
fn view_window(&self, id: window::Id) -> Element<'_, Self::Message> {
    crate::views::popup::render(self, id)
}
```

`view_window` is invoked **for each non-main surface id**. Your popup, the tooltip
surface, etc. all flow through here. The reference passes the `id` to the popup renderer
(`src/views/popup.rs`); a robust applet should check whether `id == self.popup` and render
accordingly if it ever creates multiple surfaces. (The reference currently renders the
popup content for any id — fine because it only ever has one popup — but note this if you
add more surfaces.)

### `update`

`update` is the only mutation point. It returns a `cosmic::app::Task<Message>`
(`Task::none()` for "nothing further"). The reference dispatches async fetches via
`cosmic::task::future(async move { Message::... })` and timer-driven polls on `Tick`.

```rust
fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
    match message {
        Message::PopupClosed(id) => {
            if self.popup.as_ref() == Some(&id) { self.popup = None; }
        }
        Message::Tick => {
            // build async fetch and return it as a Task
            return cosmic::task::future(async move { Message::BudgetUpdate(/* ... */) });
        }
        Message::BudgetUpdate(result) => { /* store data or error */ }
        Message::CosmicConfigUpdate(new_config) => { self.config = new_config; }

        // The critical one: forward surface actions to the runtime.
        Message::Surface(action) => {
            return cosmic::task::message(cosmic::Action::Cosmic(
                cosmic::app::Action::Surface(action),
            ));
        }
    }
    cosmic::app::Task::none()
}
```

**Why `Surface` must be forwarded.** Popup creation/teardown is expressed as a
`cosmic::surface::Action` produced inside a widget callback (the panel button — see §5).
A widget can only emit *your* `Message`, so the action is wrapped as `Message::Surface(a)`.
But the *runtime* is what actually creates/destroys the Wayland surface — so in `update`
you must hand it back to the runtime by re-wrapping:

```rust
cosmic::task::message(cosmic::Action::Cosmic(cosmic::app::Action::Surface(action)))
```

If you drop it (e.g. `Message::Surface(_) => {}`), nothing happens — popups silently never
open. This was an actual regression during this project's development.

### `subscription` — timers + config hot-reload

```rust
fn subscription(&self) -> Subscription<Self::Message> {
    Subscription::batch([
        cosmic::iced::time::every(Duration::from_secs(self.config.poll_interval_secs))
            .map(|_| Message::Tick),
        self.core()
            .watch_config::<Config>(APP_ID)
            .map(|update| Message::CosmicConfigUpdate(update.config)),
    ])
}
```

Two subscriptions, batched:

- **`cosmic::iced::time::every(Duration)`** — a recurring timer. Mapping its emission to a
  `Message` gives you a poll/refresh loop. (A clock applet would map this to a "redraw"
  message every second — see §12.)
- **`core().watch_config::<Config>(APP_ID)`** — emits a `cosmic_config::Update<Config>`
  whenever the persisted config changes on disk (see §6). Note `Update<T>` does **not**
  impl `Clone`, so extract `update.config` *before* wrapping it in a `Message`.

Because the timer's interval comes from `self.config.poll_interval_secs`, the subscription
identity changes when config changes, and iced re-creates the timer with the new
interval — live reconfiguration for free.

### `style`

```rust
fn style(&self) -> Option<cosmic::iced::theme::Style> {
    Some(cosmic::applet::style())
}
```

`cosmic::applet::style()` (verified `libcosmic:src/applet/mod.rs:594`) returns the correct
transparent-background style so the applet blends into the panel. Use it verbatim.

### `on_close_requested`

```rust
fn on_close_requested(&self, id: window::Id) -> Option<Self::Message> {
    Some(Message::PopupClosed(id))
}
```

The compositor calls this when a surface (e.g. the popup) is dismissed — clicking away,
pressing Escape, etc. Return a message so your `update` can clear `self.popup`. Without
this your model thinks the popup is still open and the next button press tries to
*destroy* an already-gone popup instead of opening a new one.

---

## 4. The panel button + tooltip

The panel content (`src/views/panel.rs`) is a `button::custom` styled as an applet icon,
wrapped in `applet_tooltip`.

```rust
use cosmic::{widget, Element};

let btn = widget::button::custom(content)            // content = your Element (text/icon)
    .class(cosmic::theme::Button::AppletIcon)        // panel-appropriate styling
    .on_press_with_rectangle(move |offset, bounds| { /* see §5 */ });

Element::from(
    app.core.applet.applet_tooltip::<Message>(
        btn,                         // the wrapped content
        fl!("app-name"),             // tooltip text
        app.popup.is_some(),         // has_popup: suppress tooltip while popup is open
        |a| Message::Surface(a),     // forward the tooltip's own surface actions
        None,                        // parent window id (None = default)
    ),
)
```

Verified signature (`libcosmic:src/applet/mod.rs:293`):

```rust
pub fn applet_tooltip<'a, Message: 'static>(
    &self,
    content: impl Into<Element<'a, Message>>,
    tooltip: impl Into<Cow<'static, str>>,
    has_popup: bool,
    on_surface_action: impl Fn(crate::surface::Action) -> Message + 'static,
    parent_id: Option<window::Id>,
) -> Tooltip<'a, Message, Message>
```

Notes:

- **`has_popup`** — when `true`, the tooltip suppresses itself. Pass `app.popup.is_some()`
  so the hover tooltip doesn't fight the open popup. (Internally the tooltip only installs
  its hover-popup callback when `!has_popup`.)
- **`on_surface_action`** — the tooltip is itself a Wayland subsurface and emits surface
  actions (e.g. destroy-on-leave). You must map these to `Message::Surface(a)` so they get
  forwarded in `update` (§3). The tooltip *will not work* if you swallow them.
- **The wrapper is a compositor layout contract.** Wrapping the button in `applet_tooltip`
  is what gives the compositor the geometry it needs; a regression in this project removed
  the wrapper and broke hover behavior. Keep the button inside the tooltip.
- `cosmic::theme::Button::AppletIcon` is the styling that makes the button look like a
  native panel icon (correct padding/hover). Use `button::custom(...).class(...)`, **not**
  a plain `button(...)`.

---

## 5. Popups — the full lifecycle

The popup is created **inline in `view`**, from the button's
`on_press_with_rectangle` callback, by emitting a surface action. It is **not** created in
`update`. Here is the complete reference flow from `src/views/panel.rs`:

```rust
use cosmic::iced::{window, Rectangle};
use cosmic::surface::action::{app_popup, destroy_popup};

let have_popup = app.popup;     // window::Id is Copy
let btn = widget::button::custom(content)
    .class(cosmic::theme::Button::AppletIcon)
    .on_press_with_rectangle(move |offset, bounds| {
        if let Some(id) = have_popup {
            // Popup already open → toggle it closed.
            Message::Surface(destroy_popup(id))
        } else {
            // Open a new popup.
            Message::Surface(app_popup::<AppModel>(
                move |state: &mut AppModel| {
                    let new_id = window::Id::unique();
                    state.popup = Some(new_id);

                    let mut popup_settings = state.core.applet.get_popup_settings(
                        state.core.main_window_id().unwrap(),
                        new_id,
                        None, None, None,
                    );
                    // Anchor the popup to the button's on-screen rectangle.
                    popup_settings.positioner.anchor_rect = Rectangle {
                        x: (bounds.x - offset.x) as i32,
                        y: (bounds.y - offset.y) as i32,
                        width: bounds.width as i32,
                        height: bounds.height as i32,
                    };
                    popup_settings
                },
                None, // None → popup content comes from view_window (recommended path)
            ))
        }
    });
```

Key pieces:

- **`on_press_with_rectangle(|offset, bounds| -> Message)`** gives you the button's
  on-screen geometry, which you use to anchor the popup. (`offset` is the surface origin;
  subtract it from `bounds` to get panel-relative coordinates.)

- **`app_popup::<App>(settings_fn, view)`** (verified `libcosmic:src/surface/action.rs:98`):

  ```rust
  pub fn app_popup<App: Application>(
      settings: impl Fn(&mut App) -> SctkPopupSettings + Send + Sync + 'static,
      view: Option<Box<dyn for<'a> Fn(&'a App) -> Element<'a, cosmic::Action<App::Message>> + ...>>,
  ) -> Action
  ```

  - The `settings` closure runs with `&mut App`, so this is where you mint a fresh
    `window::Id::unique()` and store it in `state.popup`. (Mutating model state inside the
    settings closure is the intended pattern here.)
  - Passing `view = None` tells the runtime to render the popup via your `view_window`.
    This is the well-trodden path. There is an inline alternative
    (`Some(Box::new(...))`) but it is a less-tested code path — prefer `None` +
    `view_window`.

- **`get_popup_settings`** (verified `libcosmic:src/applet/mod.rs:405`):

  ```rust
  pub fn get_popup_settings(
      &self,
      parent: window::Id,
      id: window::Id,
      size: Option<(u32, u32)>,
      width_padding: Option<i32>,
      height_padding: Option<i32>,
  ) -> SctkPopupSettings
  ```

  It fills in sensible anchor/gravity/size-limits based on the panel anchor (left/right/
  top/bottom). You then override `positioner.anchor_rect` to pin the popup to the button.

- **`destroy_popup(id) -> Action`** (verified `libcosmic:src/surface/action.rs:15`) tears
  the popup down. Emitting it on press while a popup is open gives toggle behavior.

- **`main_window_id().unwrap()`** — `Option`. The reference uses `.unwrap()` here; it has
  been a flagged panic risk. The main window id is essentially always present when a press
  callback fires, but if you want to be defensive, match on it and fall back to
  `Task::none()` instead of unwrapping.

### Rendering popup content with `popup_container`

In `view_window` → `src/views/popup.rs`:

```rust
pub fn render(app: &AppModel, _id: window::Id) -> Element<'_, Message> {
    let content = /* a widget::column of your sections */;
    app.core
        .applet
        .popup_container(content)
        .max_width(372.0)
        .min_width(300.0)
        .into()
}
```

`popup_container` (verified `libcosmic:src/applet/mod.rs:359`) wraps your content in the
themed COSMIC popup chrome (background, border, corner radius) and autosizes it. Returns an
`Autosize` you can constrain with `.max_width`/`.min_width`.

### Closing

The popup closes through `on_close_requested` → `Message::PopupClosed(id)` → clears
`self.popup` (§3). Clicking the button again while open emits `destroy_popup` directly.
Both paths converge on `self.popup = None`.

---

## 6. Configuration & hot-reload (`cosmic_config`)

Config lives in `src/config.rs`. It uses the `CosmicConfigEntry` derive so it can be
loaded/saved/watched via `cosmic_config`.

```rust
// THIS IMPORT IS LOAD-BEARING. See the warning below.
use cosmic::cosmic_config;
use cosmic::cosmic_config::{CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    pub poll_interval_secs: u64,
    pub work_days: u8,
    pub daily_budget: f64,
    pub creds_path: String,
    // Optional theme overrides, etc.
    pub color_on_track: Option<ConfigColor>,
}

impl Default for Config {
    fn default() -> Self {
        Self { poll_interval_secs: 300, work_days: 5, daily_budget: 20.0,
                creds_path: "~/.claude/.credentials.json".into(), color_on_track: None }
    }
}

impl Config {
    /// Clamp loaded values into valid ranges. Always run on load.
    pub fn validated(mut self) -> Self {
        self.work_days = self.work_days.clamp(1, 7);
        if self.poll_interval_secs < 30 { self.poll_interval_secs = 30; }
        self
    }
}
```

### Loading

```rust
let config = match cosmic::cosmic_config::Config::new(APP_ID, CONFIG_VERSION) {
    Ok(helper) => match Config::get_entry(&helper) {
        Ok(c) => c.validated(),
        Err((_errs, c)) => c.validated(),  // partial load: keep the fields that parsed
    },
    Err(_) => Config::default(),
};
```

`Config::get_entry` returns `Result<Config, (Vec<Error>, Config)>` — even on error you get
a best-effort `Config` back, so you can always proceed. Always run `.validated()` to clamp
untrusted on-disk values.

### Hot-reload

The `watch_config::<Config>(APP_ID)` subscription (§3) emits whenever the config files
change, so users editing config (or a future settings UI) get live updates without a
restart. Remember to extract `.config` from the `Update` (it isn't `Clone`).

### Storage location

`cosmic_config` stores each field as a separate RON file under:

```
~/.config/cosmic/<APP_ID>/v<CONFIG_VERSION>/
```

i.e. for this applet: `~/.config/cosmic/dev.fuabioo.CosmicAppletCcUsage/v1/`. To override
a single setting by hand, write its value (RON) into a file named for the field, e.g.
`v1/poll_interval_secs` containing `60`.

### CRITICAL gotcha — keep `use cosmic::cosmic_config;`

The `CosmicConfigEntry` derive macro **generates code that references `cosmic_config::` as
a bare path**. So `src/config.rs` must contain:

```rust
use cosmic::cosmic_config;
```

This import *looks* unused to linters, rustfmt's import pruning, and "simplifier" agents —
they will try to delete it. **Do not let them.** Deleting it breaks the build with errors
pointing at the derive expansion (not at the import), which is confusing. The reference
file keeps a comment on the import explaining exactly this. Also keep `ron` as a direct
dependency (§2) — the derive emits RON serialization code.

---

## 7. Theming & colors

### Text colors

Text styling uses `.class(cosmic::theme::Text::...)` (**not** `.style()`). The full enum
(verified `libcosmic:src/theme/style/iced.rs:1318`) is:

```rust
pub enum Text {
    Accent,
    Default,                              // <- the #[default]
    Color(Color),
    Custom(fn(&Theme) -> text::Style),    // fn pointer, not a closure (must be Copy)
}
```

Usage from `src/views/popup.rs`:

```rust
use cosmic::widget::text;

text("On Track").class(cosmic::theme::Text::Default);
text("42%").class(cosmic::theme::Text::Color(my_color)).size(20).font(cosmic::font::semibold());
```

`cosmic::font::semibold()` (verified `libcosmic:src/font.rs:23`) returns a `Font` — note
it's the function `semibold()`, **not** a `FONT_SEMIBOLD` constant.

### Semantic theme colors

Pull colors from the active COSMIC theme so the applet respects the user's light/dark/
accent settings. The reference resolves to semantic colors with optional user overrides
(`src/config.rs`):

```rust
use cosmic::iced::Color;

// success / warning / destructive → Srgba → Color via .into()
fn on_track() -> Color  { cosmic::theme::active().cosmic().success_color().into() }
fn warning() -> Color   { cosmic::theme::active().cosmic().warning_color().into() }
fn over_budget() -> Color { cosmic::theme::active().cosmic().destructive_color().into() }
```

`cosmic::theme::active().cosmic()` yields the theme's color palette; `success_color()`,
`warning_color()`, `destructive_color()` return `Srgba` which converts to iced `Color`
with `.into()`. The reference layers user RON overrides on top (`resolve_pace_color`),
falling back to these theme colors when no override is set.

### The progress-bar styling limitation (important)

The COSMIC linear progress widget is created with
`cosmic::widget::progress_bar::determinate_linear(fraction)` (used in
`src/views/popup.rs`):

```rust
cosmic::widget::progress_bar::determinate_linear((pct as f32 / 100.0).clamp(0.0, 1.0))
    .width(Length::Fill)
    .into()
```

**Limitation:** at this libcosmic rev, the progress bar's `StyleSheet` defines
`type Style = ()` (verified `libcosmic:src/widget/progress_bar/style.rs:41` and `:61`).
That means the widget takes **no per-instance style** — its bar color is computed entirely
from the theme (`accent_color()`/`accent_text_color()`), and you **cannot** color a single
bar (e.g. red for over-budget) through the COSMIC widget. The reference therefore conveys
pace state via the colored *text* next to the bar, leaving the bar at the theme accent
color.

If you need per-bar coloring, the workarounds are:

1. **Raw iced progress bar.** Use `iced::widget::progress_bar`, which exposes a `.style()`
   closure where you can return a `Style` with your own `bar.background`. You lose the
   COSMIC look-and-feel but gain color control. (Verify the exact iced `progress_bar`
   `.style` signature against the iced version libcosmic re-exports before relying on it —
   I did not exhaustively check the raw-iced path in this repo.)
2. **Manual bar.** Compose a fixed-size `container` (the track) holding an inner
   `container` whose width is `Length::FillPortion`/a fraction of the track, each styled
   with its own background `Color`. Fully under your control, more code.

Pick (1) for least code if you can accept non-COSMIC styling; (2) when you want exact
control and theme integration.

---

## 8. Internationalization (Fluent)

### `i18n.toml`

At the repo root:

```toml
fallback_language = "en"

[fluent]
assets_dir = "i18n"
```

The `[fluent]` subsection with `assets_dir` is required. `fallback_language` is the locale
used when the system locale has no translation.

### `.ftl` files

Translations live under `i18n/<lang>/<crate_name>.ftl`, e.g.
`i18n/en/cosmic_applet_cc_usage.ftl`:

```fluent
app-name = Claude Code Usage
weekly-budget = Weekly
resets-in = Resets in {$time}
todays-ceiling-detail = {$label}: {$ceiling}% (day {$index}/{$total})
```

Note Fluent's interpolation syntax `{$arg}` and that you can nest a looked-up message as an
argument value.

### The `fl!` macro

The reference defines a thin `fl!` wrapper in `src/i18n.rs` over `i18n_embed_fl::fl!`,
bound to a process-wide `FluentLanguageLoader`:

```rust
#[macro_export]
macro_rules! fl {
    ($id:literal) => {{ i18n_embed_fl::fl!($crate::i18n::loader(), $id) }};
    ($id:literal, $($args:tt)*) => {{ i18n_embed_fl::fl!($crate::i18n::loader(), $id, $($args)*) }};
}
```

Initialization (`src/i18n.rs`) embeds the `i18n/` dir with `rust_embed` and selects the
locale from the desktop environment:

```rust
#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let langs = DesktopLanguageRequester::requested_languages();
    i18n_embed::select(loader(), &Localizations, &langs)?;
    Ok(())
}
```

Call `i18n::init()` once in `main` before `run`. Then use `fl!("key")` /
`fl!("key", arg = value)` anywhere in views. `arg` values are passed by name and must match
the `{$arg}` placeholders; pass strings as `&str` (the reference uses
`time = resets_text.as_str()`).

---

## 9. Desktop integration & packaging

### APP_ID naming

Use **reverse-DNS** for the app id: `dev.fuabioo.CosmicAppletCcUsage`. This same string is:

- the `const APP_ID` in your code and `Application::APP_ID`,
- the `cosmic_config` namespace (config path),
- the `.desktop` / `.metainfo.xml` / icon file basenames.

For a new applet, pick e.g. `dev.fuabioo.CosmicAppletCodexUsage` and use it everywhere.

### `.desktop` file

`resources/<APP_ID>.desktop` — this is how the panel discovers the applet:

```ini
[Desktop Entry]
Name=Claude Code Usage
Comment=Monitor Claude Code weekly and session usage budget
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

The applet-specific keys:

- **`X-CosmicApplet=true`** — marks this as a panel applet so COSMIC Settings lists it.
- **`X-CosmicHoverPopup=Auto`** — hover-popup behavior. (Reference uses `Auto`.)
- **`Categories=COSMIC;...`** — include `COSMIC` so it groups correctly.
- **`NoDisplay=true`** — hide it from the normal app launcher (it's a panel applet, not a
  launchable app).
- **`Exec`** must be the installed binary name on `$PATH`.
- **`Icon`** matches the installed icon basename (the `<APP_ID>.svg`).

### AppStream metainfo (optional but recommended)

`resources/<APP_ID>.metainfo.xml` declares it as a COSMIC panel addon:

```xml
<component type="addon">
  <id>dev.fuabioo.CosmicAppletCcUsage</id>
  <extends>com.system76.CosmicPanel</extends>
  <name>Claude Code Usage</name>
  ...
</component>
```

`<extends>com.system76.CosmicPanel</extends>` is what marks it as a panel extension.

### Icon

`resources/icons/<APP_ID>.svg` — a scalable icon installed into the hicolor theme.

### `justfile` install recipes

The reference `justfile` installs to **user-local** dirs (no sudo) by default:

```just
name := 'cosmic-applet-cc-usage'
appid := 'dev.fuabioo.CosmicAppletCcUsage'

user-bindir   := env('HOME') / '.local' / 'bin'
user-appdir   := env('HOME') / '.local' / 'share' / 'applications'
user-iconsdir := env('HOME') / '.local' / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'
user-metainfodir := env('HOME') / '.local' / 'share' / 'metainfo'

install: build-release
    install -Dm0755 target/release/{{name}} {{user-bindir}}/{{name}}
    install -Dm0644 resources/{{appid}}.desktop {{user-appdir}}/{{appid}}.desktop
    install -Dm0644 resources/{{appid}}.metainfo.xml {{user-metainfodir}}/{{appid}}.metainfo.xml
    install -Dm0644 resources/icons/{{appid}}.svg {{user-iconsdir}}/{{appid}}.svg

install-system: build-release    # /usr/bin, /usr/share/... — needs sudo
    install -Dm0755 target/release/{{name}} /usr/bin/{{name}}
    # ...desktop / metainfo / icon into /usr/share/...
```

Prefer `just install` (writes to `~/.local`, requires `~/.local/bin` on `$PATH`). Use
`just install-system` (with `sudo just install-system`) only for a machine-wide install.
There are matching `uninstall` / `uninstall-system` recipes.

> Project convention: install the applet via `just install`, not `cargo install --path .`.

---

## 10. Running, adding to the panel & debugging

### Build & install

```sh
just install        # cargo build --release + copy to ~/.local/bin and ~/.local/share
```

Ensure `~/.local/bin` is on your `PATH` so the panel can `Exec=cosmic-applet-cc-usage`.

### Add it to the panel

Open **COSMIC Settings → Desktop → Panel** (or **Dock**), choose the applets/config for a
panel, and add your applet from the list (it shows up because of `X-CosmicApplet=true`).
After install you may need to log out/in or restart `cosmic-panel` for a newly installed
applet to appear.

### Debugging / logs

The panel spawns applets as children, so applet stdout/stderr surfaces under the panel's
journal unit. Tail it and filter to your tag:

```sh
journalctl -t cosmic-panel -f
# or filter by your eprintln prefix:
journalctl -t cosmic-panel -f | grep cc-usage
```

The reference logs with a consistent prefix, e.g.:

```rust
eprintln!("[cc-usage] polling API...");
eprintln!("[cc-usage] poll OK: weekly={:.0}% session={:.0}%", w, s);
```

Adopt an `eprintln!("[my-applet] ...")` convention; those lines show up in the panel
journal and make filtering trivial. The `--status` subcommand pattern (§2) is also great
for debugging the data layer without the GUI.

---

## 11. Gotchas / troubleshooting

### libcosmic rev vs. compositor compatibility (the big one)

Your applet **pins a libcosmic git rev** (§2). libcosmic implements the *client* half of
the Wayland surface lifecycle; `cosmic-comp`/`cosmic-panel` implement the *compositor*
half. These must agree on the protocol contract.

When COSMIC updates (a new `cosmic-comp`/`cosmic-panel`) and starts enforcing newer
surface-lifecycle rules, an **old client** pinned to a stale libcosmic rev can violate
them and crash. A characteristic failure is:

```
xdg_surface error 3: must ack the initial configure before attaching buffer
```

This often triggers on **hover or popup open** (the moment a new surface is created), and
because the panel respawns crashed applets, you get a **crash-respawn loop** — the applet
flickers in and out of the panel.

**Fix:** bump the libcosmic `rev` in `Cargo.toml` to one contemporaneous with your
installed COSMIC release, then rebuild and reinstall:

```sh
# edit Cargo.toml: rev = "<newer-rev>"
cargo update -p libcosmic
just install
```

**Diagnosis — is your applet stale relative to the compositor?** Compare the build epoch:

```sh
# COSMIC build epoch is embedded in the dpkg version string (the big number):
dpkg -l | grep cosmic-comp
#   ii  cosmic-comp  0.1~1779893019~24.04~22fe419  ...
#                        ^^^^^^^^^^ unix epoch of the COSMIC build

# Compare binary mtimes as a rough sanity check:
ls -l /usr/bin/cosmic-comp
ls -l ~/.local/bin/cosmic-applet-cc-usage
```

If your applet binary is much older than `/usr/bin/cosmic-comp`, and you see surface
errors in `journalctl -t cosmic-panel`, a libcosmic bump + rebuild is the likely fix. Keep
the rev reasonably current — don't pin and forget.

### rustc MSRV moves with libcosmic

libcosmic at this rev requires **rustc ≥ 1.93** and is **edition 2024** internally
(verified `libcosmic:Cargo.toml`: `edition = "2024"`, `rust-version = "1.93"`). When you
bump the libcosmic rev, the MSRV may rise too — keep your toolchain current
(`rustup update`). Your *own* crate can remain `edition = "2021"`.

### Other traps observed in this project

- **Dropping `Message::Surface`** → popups never open. Always forward it (§3).
- **Removing `use cosmic::cosmic_config;`** → config derive fails to compile (§6).
- **Building a new `reqwest::Client` per poll** → wasteful; build once in `init` and clone
  the handle (cloning a `Client` is cheap and shares the connection pool).
- **`Update<Config>` is not `Clone`** → extract `.config` before wrapping in a `Message`.
- **Static subscription id** → if your poll interval comes from config, derive the timer
  from `self.config.*` so the subscription identity changes and iced rebuilds it on config
  change.
- **`main_window_id().unwrap()`** in the popup closure is a (small) panic risk; consider
  matching instead of unwrapping.

---

## 12. Checklist for a new applet

Copy the reference and rename. Concretely:

1. **Copy the skeleton.** `Cargo.toml`, `src/main.rs`, `src/app.rs`, `src/config.rs`,
   `src/i18n.rs`, `src/views/{mod,panel,popup}.rs`, `i18n.toml`, `i18n/en/*.ftl`,
   `justfile`, and `resources/` (`.desktop`, `.metainfo.xml`, `icons/*.svg`).
2. **Pick a new APP_ID** (reverse-DNS, e.g. `dev.fuabioo.CosmicAppletCodexUsage`) and
   change it **everywhere**: `const APP_ID`, the `justfile` `name`/`appid`, and all
   `resources/` filenames + their internal `id`/`Icon`/`Exec` fields.
3. **Rename the crate** in `Cargo.toml` (`name`) and the binary references in `justfile`
   and the `.desktop` `Exec`. Rename the `.ftl` file to `<crate_name>.ftl`.
4. **Swap the data source.** Replace the `budget/` module with your own. Keep the shape:
   an async function returning `Result<YourData, YourError>`, called from a
   `cosmic::task::future(...)` in `init` and on `Tick`.
5. **Adjust state.** Edit `AppModel` fields and the `Message` enum, but **keep**
   `PopupClosed`, `Surface(cosmic::surface::Action)`, `CosmicConfigUpdate`, and the timer
   message.
6. **Rewrite the views.** `panel::render` (icon/text + the popup toggle button) and
   `popup::render` (your dashboard). Keep the `applet_tooltip` wrapper and the
   `on_press_with_rectangle` → `app_popup`/`destroy_popup` pattern verbatim.
7. **Update config fields** in `src/config.rs` (keep the `CosmicConfigEntry` derive, the
   `use cosmic::cosmic_config;` import, `CONFIG_VERSION`, and `validated()`).
8. **Translate.** Edit the `.ftl` keys; add more `i18n/<lang>/` dirs as needed.
9. **Build & install:** `just install`, then add it via COSMIC Settings, and tail
   `journalctl -t cosmic-panel -f | grep <your-prefix>`.

### Worked idea A — "Codex usage" monitor

Reuse the polling pattern almost verbatim:

- Keep `subscription`'s `time::every(poll_interval)` → `Message::Tick` loop.
- Replace `budget/api.rs::fetch_usage` with a fetch against Codex's usage endpoint, and
  `budget/creds.rs::read_token` with wherever Codex stores its token.
- Keep the `BudgetState`/`WindowState` shape (utilization %, reset time, a pace color), or
  define your own equivalent.
- Panel shows a colored percentage; popup shows progress bars + reset times.
- New APP_ID: `dev.fuabioo.CosmicAppletCodexUsage`.

### Worked idea B — a "better clock"

- **Subscription:** `cosmic::iced::time::every(Duration::from_secs(1))` mapped to a
  `Message::Tick` that just stores `chrono::Local::now()` (or triggers a redraw). No async
  fetch, no `reqwest`/`tokio` HTTP needed — you can drop those deps.
- **Panel `view`:** render the formatted time string with `text(...)`; use
  `autosize_window` so the panel width tracks the string.
- **Popup `view_window`:** a calendar/world-clock dashboard via `popup_container`.
- **Config:** time format string, 12/24h, timezone list — all via the same
  `CosmicConfigEntry` + `watch_config` pattern, so users get live updates.
- Everything else (button + tooltip + popup lifecycle + desktop integration) is identical
  to the reference.

---

## Appendix — verified reference map

| Concept | Reference file | libcosmic signature source |
|---|---|---|
| Entry point | `src/main.rs` | `libcosmic:src/applet/mod.rs:535` (`run`) |
| Application impl | `src/app.rs` | — |
| Panel button + tooltip | `src/views/panel.rs` | `libcosmic:src/applet/mod.rs:293` (`applet_tooltip`) |
| Popup create/destroy | `src/views/panel.rs` | `libcosmic:src/surface/action.rs:98,15` (`app_popup`, `destroy_popup`) |
| Popup settings | `src/views/panel.rs` | `libcosmic:src/applet/mod.rs:405` (`get_popup_settings`) |
| Popup content | `src/views/popup.rs` | `libcosmic:src/applet/mod.rs:359` (`popup_container`) |
| Autosize panel | `src/app.rs` | `libcosmic:src/applet/mod.rs:458` (`autosize_window`) |
| Applet style | `src/app.rs` | `libcosmic:src/applet/mod.rs:594` (`style`) |
| Config + hot-reload | `src/config.rs`, `src/app.rs` | — |
| Text colors | `src/views/{panel,popup}.rs` | `libcosmic:src/theme/style/iced.rs:1318` (`Text`) |
| Semibold font | `src/views/popup.rs` | `libcosmic:src/font.rs:23` (`semibold`) |
| Progress bar limitation | `src/views/popup.rs` | `libcosmic:src/widget/progress_bar/style.rs:41,61` (`type Style = ()`) |
| i18n / `fl!` | `src/i18n.rs`, `i18n.toml`, `i18n/en/*.ftl` | — |
| Desktop integration | `resources/*.desktop`, `*.metainfo.xml`, `justfile` | — |
