pub mod api;
pub mod creds;
pub mod pace;
pub mod types;

// Re-export all public items
pub use api::fetch_usage;
pub use creds::read_token;
pub use pace::{compute_hourly_color, compute_weekly_pace_color, weekday_to_work_index};
pub use types::{
    format_duration, BudgetError, BudgetState, PaceColor, WindowState,
};
