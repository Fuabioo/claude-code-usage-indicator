# ADR-002: Data Source and Budget Calculation

| Field       | Value                                                    |
|-------------|----------------------------------------------------------|
| Status      | Proposed                                                 |
| Date        | 2025-02-05                                               |
| Depends on  | ADR-001                                                  |
| Authors     | fuabioo                                                  |

## Context

The applet needs to display Claude Code usage metrics. The existing Go tool (`my-cc-status-line`) already solves this problem by querying the Anthropic OAuth usage API and applying pace-based coloring. This ADR documents how to replicate that data pipeline in Rust for the COSMIC applet.

## Decision

### Data Source: Anthropic OAuth Usage API

**Endpoint**: `GET https://api.anthropic.com/oauth/usage`

**Authentication**: Bearer token from `~/.claude/.credentials.json`

**Response Schema** (observed):

```json
{
  "seven_day": {
    "utilization": 42.5,
    "resets_at": "2025-02-10T00:00:00Z"
  },
  "five_hour": {
    "utilization": 15.0,
    "resets_at": "2025-02-05T14:00:00Z"
  }
}
```

| Field                     | Type    | Description                                       |
|---------------------------|---------|---------------------------------------------------|
| `seven_day.utilization`   | f64     | Weekly usage as percentage (0.0 - 100.0+)         |
| `seven_day.resets_at`     | ISO8601 | When the 7-day window resets                       |
| `five_hour.utilization`   | f64     | Rolling 5-hour usage as percentage (0.0 - 100.0+) |
| `five_hour.resets_at`     | ISO8601 | When the 5-hour window resets                      |

### Credential Reading

The applet reads the OAuth bearer token from the same file Claude Code uses:

```
~/.claude/.credentials.json
```

**Strategy**:
- Read file on startup and on each cache miss
- Parse with serde, extract the bearer token field
- If file is missing or unreadable, show an error state in the panel icon (not a crash)
- Never log or expose the token value

### Polling & Caching

| Parameter         | Default | Configurable | Notes                                           |
|-------------------|---------|--------------|--------------------------------------------------|
| Poll interval     | 5 min   | Yes          | Timer-based iced `Subscription`                  |
| Cache TTL         | 5 min   | Yes          | Matches poll interval; stale data shown on error |
| Retry on failure  | 30 sec  | No           | Exponential backoff up to poll interval          |

**Subscription flow**:

```
[Subscription::run_with_id]
    |
    +--> loop {
            tokio::time::sleep(poll_interval).await
            fetch_usage() -> Result<BudgetData, Error>
            channel.send(Message::BudgetUpdate(result)).await
         }
```

On `Message::BudgetUpdate(Ok(data))`:
- Store data in applet state
- Trigger re-render (automatic in iced)

On `Message::BudgetUpdate(Err(e))`:
- Keep previous cached data
- Set error flag for UI indication (e.g., stale indicator)
- Log error via `tracing`

### Budget Data Model

```rust
/// Raw API response
#[derive(Debug, Clone, Deserialize)]
struct UsageResponse {
    seven_day: UsageWindow,
    five_hour: UsageWindow,
}

#[derive(Debug, Clone, Deserialize)]
struct UsageWindow {
    utilization: f64,
    resets_at: DateTime<Utc>,
}

/// Processed budget state held in applet model
#[derive(Debug, Clone)]
struct BudgetState {
    weekly: WindowState,
    hourly: WindowState,
    last_updated: Instant,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct WindowState {
    utilization: f64,           // 0.0 - 100.0+
    resets_at: DateTime<Utc>,
    pace_color: PaceColor,      // Computed on update
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PaceColor {
    Green,
    Yellow,
    Red,
}
```

## Pace-Based Daily Budget Coloring Algorithm

This is the core logic ported from the Go tool. It answers: "Given my usage percentage and the current day of the work week, am I on track?"

### Configuration Inputs

| Parameter      | Type | Default | Description                                         |
|----------------|------|---------|-----------------------------------------------------|
| `work_days`    | u8   | 5       | Number of work days per week (Mon-Fri)              |
| `daily_budget` | f64  | 20.0    | Expected % consumption per work day (100/work_days) |

### Algorithm (weekly window only)

```
fn compute_pace_color(utilization: f64, daily_budget: f64) -> PaceColor:
    1. Determine current work day index (1-based):
       - Monday    = 1
       - Tuesday   = 2
       - Wednesday = 3
       - Thursday  = 4
       - Friday    = 5
       - Saturday  = 5  (inherits Friday's ceiling)
       - Sunday    = 5  (inherits Friday's ceiling)

    2. Compute daily ceiling:
       ceiling = work_day_index * daily_budget
       Example: Tuesday with 20% daily -> ceiling = 40%

    3. Compute pace ratio:
       ratio = utilization / ceiling

    4. Apply color thresholds:
       ratio < 0.75  -> Green   (well under budget)
       ratio < 1.00  -> Yellow  (approaching ceiling)
       ratio >= 1.00 -> Red     (over budget for today)
```

### Hourly Window Coloring

The hourly (5-hour) window uses **fixed thresholds** (no pace calculation, since it's too short-term for daily budgeting):

```
utilization <= 50%  -> Green
utilization <= 80%  -> Yellow
utilization > 80%   -> Red
```

### Visual Examples

**Monday morning, 5% weekly usage**:
- Ceiling: 1 * 20 = 20%
- Ratio: 5/20 = 0.25
- Color: **Green** (well under Monday's budget)

**Monday afternoon, 22% weekly usage**:
- Ceiling: 1 * 20 = 20%
- Ratio: 22/20 = 1.10
- Color: **Red** (already over Monday's budget)

**Wednesday, 55% weekly usage**:
- Ceiling: 3 * 20 = 60%
- Ratio: 55/60 = 0.92
- Color: **Yellow** (approaching Wednesday's ceiling)

**Friday, 95% weekly usage**:
- Ceiling: 5 * 20 = 100%
- Ratio: 95/100 = 0.95
- Color: **Yellow** (approaching full-week ceiling)

**Saturday, 80% weekly usage**:
- Ceiling: 5 * 20 = 100% (Friday's ceiling carries)
- Ratio: 80/100 = 0.80
- Color: **Yellow**

### Time-Until-Reset Formatting

```rust
fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;

    match (days, hours, minutes) {
        (d, h, _) if d > 0 && h > 0 => format!("{}d {}h", d, h),
        (d, _, _) if d > 0          => format!("{}d", d),
        (_, h, m) if h > 0 && m > 0 => format!("{}h {}m", h, m),
        (_, h, _) if h > 0          => format!("{}h", h),
        (_, _, m)                    => format!("{}m", m.max(1)),
    }
}
```

## Error Handling Strategy

Following the project's go-like error handling principle, all fallible operations propagate errors explicitly.

| Failure Mode                  | Behavior                                                         |
|-------------------------------|------------------------------------------------------------------|
| Credentials file missing      | Panel icon shows "?" with tooltip "Credentials not found"        |
| Credentials file unreadable   | Panel icon shows "?" with tooltip "Cannot read credentials"      |
| API request fails (network)   | Keep stale data, show staleness indicator, retry on next tick    |
| API returns unexpected JSON   | Log parse error, keep stale data, show staleness indicator       |
| API returns HTTP 401          | Show "!" icon with tooltip "Token expired -- re-auth needed"     |
| API returns HTTP 429          | Back off to 2x poll interval for next tick                       |
| utilization > 100%            | Display as-is (it's valid -- means over-budget)                  |

## Alternatives Considered

### 1. Read from the Go tool's SQLite database

Rejected. The Go tool tracks session costs, not budget data. Budget data comes directly from the API and is not persisted by the Go tool.

### 2. Shared IPC between Go tool and Rust applet

Rejected. Over-engineered. The API call is lightweight (one small JSON response) and the 5-minute cache means at most ~288 calls/day. Direct API access is simpler and eliminates a dependency on the Go tool running.

### 3. Use a longer cache TTL (e.g., 30 minutes)

Rejected as default. During active work, usage can spike quickly. 5 minutes provides good freshness. Users can increase TTL via config if desired.

## References

- `my-cc-status-line/internal/widget/budget/client.go` -- Go API client implementation
- `my-cc-status-line/internal/widget/budget/budget.go` -- Pace coloring algorithm
- `my-cc-status-line/internal/widget/budget/types.go` -- Budget data structures
