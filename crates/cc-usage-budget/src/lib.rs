//! Reusable Claude Code usage-budget logic.
//!
//! This crate is UI- and platform-agnostic: it reads the Claude credentials
//! file, fetches usage from the Anthropic OAuth usage API, and computes
//! pace-based color indicators. It is shared by the COSMIC panel applet and
//! can back a macOS menu bar frontend (or any other) without modification.

pub mod api;
pub mod creds;
pub mod pace;
pub mod types;

// Re-export all public items
pub use api::fetch_usage;
pub use creds::read_token;
pub use pace::{compute_hourly_color, compute_weekly_pace_color, days_into_cycle, reset_day_name};
pub use types::{
    format_duration, BudgetError, BudgetState, PaceColor, WindowState,
};
