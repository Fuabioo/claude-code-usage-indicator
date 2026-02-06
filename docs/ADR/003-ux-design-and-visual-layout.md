# ADR-003: UX Design and Visual Layout

| Field       | Value                                                    |
|-------------|----------------------------------------------------------|
| Status      | Proposed                                                 |
| Date        | 2025-02-05                                               |
| Depends on  | ADR-001, ADR-002                                         |
| Authors     | fuabioo                                                  |

## Context

The applet has two visual surfaces:

1. **Panel Icon View** -- always visible in the COSMIC panel bar, must be compact
2. **Popup Detail View** -- shown on click, provides full budget dashboard

The UX must replicate the information density of the existing terminal status line while conforming to COSMIC panel conventions and leveraging the GUI's capabilities (color fills, icons, progress bars, tooltips).

## Decision

### Panel Icon View (Compact)

The panel icon shows the most critical metric at a glance: **weekly usage percentage with pace-aware coloring**.

#### Layout

```
+-------------------------------------------+
|  [icon]  42%  |  15%                      |
|  weekly       |  hourly                   |
+-------------------------------------------+
         ^              ^
    pace-colored    fixed-threshold colored
```

**Implementation**: Two colored text segments separated by a thin divider, flanking an optional symbolic icon.

#### Panel States

| State              | Icon                | Text              | Color           | Tooltip                                        |
|--------------------|---------------------|-------------------|-----------------|-------------------------------------------------|
| Normal (green)     | `fuel-symbolic`     | `42%  | 15%`     | Green on both   | "Weekly: 42% (resets in 3d 12h) | Hourly: 15% (resets in 2h 45m)" |
| Caution (yellow)   | `fuel-symbolic`     | `68%  | 55%`     | Yellow on one+  | Same format                                    |
| Critical (red)     | `fuel-symbolic`     | `92%  | 85%`     | Red on one+     | Same format                                    |
| Over budget        | `fuel-symbolic`     | `110% | 95%`     | Red + bold      | "OVER BUDGET -- Weekly: 110% ..."              |
| Error              | `dialog-warning`    | `--`              | Dim/gray        | "Unable to fetch usage data: <reason>"         |
| No credentials     | `dialog-question`   | `?`               | Dim/gray        | "Claude credentials not found"                 |

#### Color Mapping to COSMIC Theme

The applet should use COSMIC's semantic color tokens where possible, falling back to explicit colors for the pace-based system:

| PaceColor | COSMIC Semantic Color                  | Fallback Hex |
|-----------|----------------------------------------|--------------|
| Green     | `cosmic_theme::palette::success`       | `#4CAF50`    |
| Yellow    | `cosmic_theme::palette::warning`       | `#FFC107`    |
| Red       | `cosmic_theme::palette::destructive`   | `#F44336`    |
| Dim/Gray  | `cosmic_theme::palette::text_disabled` | `#9E9E9E`    |

#### Icon Selection

Use freedesktop symbolic icon names that convey "resource/fuel/quota":

**Primary candidates** (check availability on COSMIC icon theme):

| Priority | Icon Name              | Rationale                                |
|----------|------------------------|------------------------------------------|
| 1        | `battery-level-*`      | Universal "remaining resource" metaphor  |
| 2        | `speedometer-symbolic`  | Gauge/meter metaphor                     |
| 3        | `timer-symbolic`       | Time-based resource                      |
| 4        | Custom SVG             | Branded icon if none fit                 |

**Fallback strategy**: If no suitable symbolic icon exists, render a small colored circle (dot) that changes color based on pace -- this is the simplest possible "traffic light" indicator.

### Popup Detail View (Expanded)

Activated by clicking the panel icon. Provides the full budget dashboard.

#### Popup Layout

```
+------------------------------------------+
|  Claude Code Usage                       |
+------------------------------------------+
|                                          |
|  Weekly Budget                           |
|  [=========>          ] 42%              |
|  Resets in 3d 12h          Pace: On Track|
|                                          |
|  --------------------                    |
|                                          |
|  Hourly Session                          |
|  [====>               ] 15%              |
|  Resets in 2h 45m                        |
|                                          |
|  --------------------                    |
|                                          |
|  Daily Budget Pace                       |
|  Today's ceiling: 60% (Wed, day 3/5)    |
|  Consumed: 42% of 60% ceiling           |
|  Remaining today: 18%                   |
|                                          |
|  --------------------                    |
|                                          |
|  Last updated: 2 min ago                 |
|                                          |
+------------------------------------------+
```

#### Popup Sections

##### Section 1: Weekly Budget

| Element          | Widget                     | Details                                             |
|------------------|----------------------------|-----------------------------------------------------|
| Label            | `widget::text`             | "Weekly Budget"                                     |
| Progress bar     | `widget::progress_bar`     | 0-100%, pace-colored fill                           |
| Percentage       | `widget::text`             | "42%" -- pace-colored, bold                         |
| Reset timer      | `widget::text`             | "Resets in 3d 12h" -- secondary text color          |
| Pace indicator   | `widget::text`             | "On Track" / "Caution" / "Over Budget" -- colored   |

##### Section 2: Hourly Session

| Element          | Widget                     | Details                                             |
|------------------|----------------------------|-----------------------------------------------------|
| Label            | `widget::text`             | "Hourly Session"                                    |
| Progress bar     | `widget::progress_bar`     | 0-100%, threshold-colored fill                      |
| Percentage       | `widget::text`             | "15%" -- threshold-colored                          |
| Reset timer      | `widget::text`             | "Resets in 2h 45m" -- secondary text color          |

##### Section 3: Daily Budget Pace (the "am I cooked?" section)

This is the unique value-add over a raw percentage. It contextualizes usage against the work-day schedule.

| Element             | Widget             | Details                                              |
|---------------------|--------------------|------------------------------------------------------|
| Label               | `widget::text`     | "Daily Budget Pace"                                  |
| Today's ceiling     | `widget::text`     | "Today's ceiling: 60% (Wed, day 3/5)"               |
| Consumed vs ceiling | `widget::text`     | "Consumed: 42% of 60% ceiling"                       |
| Remaining           | `widget::text`     | "Remaining today: 18%"                               |

**Color logic for "Remaining"**:
- Remaining > 25% of ceiling: Green
- Remaining 0-25% of ceiling: Yellow
- Remaining < 0 (over ceiling): Red, text changes to "Over by X%"

##### Section 4: Footer

| Element          | Widget             | Details                                              |
|------------------|--------------------|------------------------------------------------------|
| Last updated     | `widget::text`     | "Last updated: 2 min ago" -- dim text                |
| Staleness warning| `widget::text`     | (conditional) "Data may be stale" if error flag set  |

#### Popup Dimensions

Following COSMIC conventions (see `cosmic-applet-battery`, `cosmic-applet-audio`):

| Property    | Value   | Notes                                    |
|-------------|---------|------------------------------------------|
| Min width   | 300px   | Ensures content doesn't wrap awkwardly   |
| Max width   | 372px   | Standard COSMIC popup max                |
| Min height  | 200px   | Enough for all sections                  |
| Max height  | 600px   | Scrollable if needed                     |

#### Widget Mapping to libcosmic

| UI Element        | libcosmic Widget                              |
|-------------------|-----------------------------------------------|
| Popup container   | `self.core.applet.popup_container(content)`   |
| Section title     | `widget::text::heading("Weekly Budget")`      |
| Progress bar      | `widget::progress_bar(0.0..=100.0, value)`    |
| Info rows         | `widget::settings::item(label, value)`        |
| Section divider   | `widget::divider::horizontal::default()`      |
| Percentage text   | `widget::text(format!("{}%", val))`           |
| Panel icon button | `self.core.applet.icon_button("icon-name")`   |
| Tooltip           | `self.core.applet.applet_tooltip(w, text, p)` |
| Scrollable body   | `widget::scrollable(content)`                 |

### Interaction Design

| User Action              | Behavior                                                   |
|--------------------------|------------------------------------------------------------|
| Hover over panel icon    | Show tooltip with full text summary                        |
| Click panel icon         | Toggle popup open/close                                    |
| Click outside popup      | Close popup                                                |
| Escape key               | Close popup                                                |
| Panel resize             | Applet auto-resizes text (via `self.core.applet.text()`)   |
| Theme change (dark/light)| Colors adapt automatically via COSMIC theme tokens         |

### Responsive Behavior

The panel icon adapts to panel size:

| Panel Size | Panel Icon Content                                          |
|------------|-------------------------------------------------------------|
| Small      | Colored dot only (no text)                                  |
| Medium     | `42%` (weekly only, colored)                                |
| Large      | `42% | 15%` (weekly + hourly, colored)                      |
| XL         | `42% 3d 12h | 15% 2h 45m` (full, like the terminal widget) |

Detection via `COSMIC_PANEL_SIZE` environment variable.

## Information Hierarchy

Ordered by importance (what the user needs to see most urgently):

1. **Am I over budget?** -- Red color on panel icon answers instantly
2. **How much weekly budget remains?** -- Weekly % in panel
3. **Am I on pace for the day?** -- Popup "Daily Budget Pace" section
4. **When does the window reset?** -- Tooltip and popup reset timers
5. **Hourly session state** -- Panel secondary indicator and popup section

## Accessibility Considerations

| Concern                    | Approach                                                       |
|----------------------------|----------------------------------------------------------------|
| Color blindness            | Never rely on color alone -- text labels ("On Track", etc.)    |
| Screen readers             | All widgets have semantic text content                         |
| High contrast              | COSMIC theme handles this; semantic colors adapt automatically |
| Small panel sizes          | Degrade gracefully (see Responsive Behavior table)             |

## Alternatives Considered

### 1. Icon-only panel indicator (no text)

Rejected as default. A colored dot is ambiguous -- users need to click to see any data. The percentage text provides immediate actionable information.

### 2. Separate popup tabs for weekly vs hourly

Rejected. Both fit in a single scrollable view. Tabs add unnecessary interaction cost for a small amount of data.

### 3. Notification-based alerts instead of panel indicator

Considered as a **future enhancement** (not MVP). Notifications are disruptive and the user wants passive, glanceable monitoring. Can be added later with configurable thresholds.

### 4. Chart/graph of usage over time

Deferred. Requires historical data storage (SQLite or similar). Out of scope for MVP. Can be added later as a popup tab.

## References

- COSMIC applet popup sizing: `cosmic-applet-battery`, `cosmic-applet-audio` source code
- Freedesktop icon naming spec: https://specifications.freedesktop.org/icon-naming-spec/latest/
- COSMIC theme palette: `cosmic_theme::palette` module in libcosmic
- Existing terminal widget layout: `my-cc-status-line/settings.example.yml`
