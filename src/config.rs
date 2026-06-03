// NOTE: `use cosmic::cosmic_config` is required — the CosmicConfigEntry derive macro
// generates code referencing `cosmic_config::` as a bare crate path.
use cosmic::cosmic_config;
use cosmic::cosmic_config::{CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use cosmic::iced::Color;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CONFIG_VERSION: u64 = 1;

/// Optional RGBA color override (f32 components, 0.0..1.0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ConfigColor {
    pub fn to_iced_color(&self) -> Color {
        Color::from_rgba(self.r, self.g, self.b, self.a)
    }
}

/// Persistent configuration for the applet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    /// Number of work days per week
    pub work_days: u8,
    /// Expected usage % per work day
    pub daily_budget: f64,
    /// Path to Claude credentials file (supports ~ expansion)
    pub creds_path: String,
    /// Custom color for on-track pace (overrides theme success color)
    pub color_on_track: Option<ConfigColor>,
    /// Custom color for warning pace (overrides theme warning color)
    pub color_warning: Option<ConfigColor>,
    /// Custom color for over-budget pace (overrides theme destructive color)
    pub color_over_budget: Option<ConfigColor>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_secs: 300, // 5 minutes
            work_days: 5,            // Budget days per cycle (must be >= 1)
            daily_budget: 20.0,      // 20% per day for 5-day week = 100% total
            creds_path: "~/.claude/.credentials.json".to_string(),
            color_on_track: None,
            color_warning: None,
            color_over_budget: None,
        }
    }
}

impl Config {
    /// Ensures config values are within valid ranges.
    /// Called when loading config from persistent storage.
    pub fn validated(mut self) -> Self {
        self.work_days = self.work_days.clamp(1, 7);
        if self.poll_interval_secs < 30 {
            self.poll_interval_secs = 30; // Minimum 30 seconds
        }
        self
    }

    /// Resolve the on-track/green color: custom override or COSMIC theme success.
    pub fn resolve_on_track_color(&self) -> Color {
        resolve_color(&self.color_on_track, || {
            cosmic::theme::active().cosmic().success_color().into()
        })
    }

    /// Resolve the warning/yellow color: custom override or COSMIC theme warning.
    pub fn resolve_warning_color(&self) -> Color {
        resolve_color(&self.color_warning, || {
            cosmic::theme::active().cosmic().warning_color().into()
        })
    }

    /// Resolve the over-budget/red color: custom override or COSMIC theme destructive.
    pub fn resolve_over_budget_color(&self) -> Color {
        resolve_color(&self.color_over_budget, || {
            cosmic::theme::active().cosmic().destructive_color().into()
        })
    }

    /// Resolve a PaceColor enum to an iced Color using config overrides or theme defaults.
    pub fn resolve_pace_color(&self, color: &crate::budget::PaceColor) -> Color {
        match color {
            crate::budget::PaceColor::Green => self.resolve_on_track_color(),
            crate::budget::PaceColor::Yellow => self.resolve_warning_color(),
            crate::budget::PaceColor::Red => self.resolve_over_budget_color(),
        }
    }
}

/// Resolve a color override or fall back to a theme default.
fn resolve_color(override_color: &Option<ConfigColor>, theme_fallback: impl FnOnce() -> Color) -> Color {
    match override_color {
        Some(c) => c.to_iced_color(),
        None => theme_fallback(),
    }
}

/// Expands a tilde-prefixed path to an absolute path.
///
/// # Examples
///
/// - `~/.claude/file.json` -> `/home/user/.claude/file.json`
/// - `/absolute/path` -> `/absolute/path` (unchanged)
/// - `~otheruser/path` -> `~otheruser/path` (unchanged, user-specific paths not supported)
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        if path == "~" {
            return PathBuf::from(home);
        }
        if let Some(suffix) = path.strip_prefix("~/") {
            return PathBuf::from(home).join(suffix);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.poll_interval_secs, 300);
        assert_eq!(config.work_days, 5);
        assert_eq!(config.daily_budget, 20.0);
        assert_eq!(config.creds_path, "~/.claude/.credentials.json");
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test/path");
        assert!(expanded.to_string_lossy().ends_with("test/path"));
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }

    #[test]
    fn test_expand_absolute_path_unchanged() {
        let path = "/absolute/path/file.json";
        let expanded = expand_tilde(path);
        assert_eq!(expanded, PathBuf::from(path));
    }

    #[test]
    fn test_expand_tilde_user_path_unchanged() {
        let path = "~otheruser/test/path";
        let expanded = expand_tilde(path);
        assert_eq!(expanded, PathBuf::from(path));
    }

    #[test]
    fn test_expand_bare_tilde() {
        let expanded = expand_tilde("~");
        assert!(!expanded.to_string_lossy().starts_with('~'));
        assert!(!expanded.to_string_lossy().is_empty());
    }
}
