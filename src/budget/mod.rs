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
