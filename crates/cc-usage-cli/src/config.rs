//! Platform-neutral configuration for the CLI.
//!
//! Precedence (lowest to highest): built-in defaults < `~/.config/cc-usage/config.toml`
//! < command-line flags. This deliberately does NOT use COSMIC's `cosmic_config` — the
//! CLI must run on any OS, so it owns a simple, portable config story. The defaults match
//! the COSMIC applet's defaults so behavior is consistent across frontends.

use serde::Deserialize;
use std::path::PathBuf;

pub const DEFAULT_CREDS_PATH: &str = "~/.claude/.credentials.json";
pub const DEFAULT_DAILY_BUDGET: f64 = 20.0;
pub const DEFAULT_WORK_DAYS: u8 = 5;
pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 300;

/// Resolved configuration used by the CLI run.
#[derive(Debug, Clone)]
pub struct Config {
    pub creds_path: String,
    pub daily_budget: f64,
    pub work_days: u8,
    pub poll_interval_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            creds_path: DEFAULT_CREDS_PATH.to_string(),
            daily_budget: DEFAULT_DAILY_BUDGET,
            work_days: DEFAULT_WORK_DAYS,
            poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS,
        }
    }
}

/// Optional fields read from the TOML config file. Anything omitted falls back to defaults.
#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    creds_path: Option<String>,
    daily_budget: Option<f64>,
    work_days: Option<u8>,
    poll_interval_secs: Option<u64>,
}

impl Config {
    /// Build the effective config: defaults, overlaid by the config file (if present),
    /// overlaid by any explicitly-provided CLI flags.
    pub fn resolve(
        creds_path: Option<String>,
        daily_budget: Option<f64>,
        work_days: Option<u8>,
    ) -> Self {
        let mut cfg = Config::default();

        // Layer 1: config file (best-effort; ignored if missing or malformed).
        if let Some(file) = load_file_config() {
            if let Some(v) = file.creds_path {
                cfg.creds_path = v;
            }
            if let Some(v) = file.daily_budget {
                cfg.daily_budget = v;
            }
            if let Some(v) = file.work_days {
                cfg.work_days = v;
            }
            if let Some(v) = file.poll_interval_secs {
                cfg.poll_interval_secs = v;
            }
        }

        // Layer 2: explicit CLI flags win.
        if let Some(v) = creds_path {
            cfg.creds_path = v;
        }
        if let Some(v) = daily_budget {
            cfg.daily_budget = v;
        }
        if let Some(v) = work_days {
            cfg.work_days = v;
        }

        cfg.validated()
    }

    /// Clamp values to valid ranges, mirroring the applet's `Config::validated`.
    fn validated(mut self) -> Self {
        self.work_days = self.work_days.clamp(1, 7);
        if self.poll_interval_secs < 30 {
            self.poll_interval_secs = 30;
        }
        self
    }

    /// The credentials path with `~` expanded to `$HOME`.
    pub fn resolved_creds_path(&self) -> PathBuf {
        expand_tilde(&self.creds_path)
    }
}

/// Path to the optional TOML config file: `$HOME/.config/cc-usage/config.toml`.
fn config_file_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/cc-usage/config.toml"))
}

fn load_file_config() -> Option<FileConfig> {
    let path = config_file_path()?;
    let contents = std::fs::read_to_string(path).ok()?;
    toml::from_str(&contents).ok()
}

/// Expand a leading `~/` (or bare `~`) to `$HOME`. Other paths are returned unchanged.
/// Mirrors the applet's `config::expand_tilde`.
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
    fn defaults_match_applet() {
        let c = Config::default();
        assert_eq!(c.creds_path, "~/.claude/.credentials.json");
        assert_eq!(c.daily_budget, 20.0);
        assert_eq!(c.work_days, 5);
        assert_eq!(c.poll_interval_secs, 300);
    }

    #[test]
    fn flags_override_defaults_and_validate() {
        let c = Config::resolve(Some("/tmp/creds.json".into()), Some(33.3), Some(9));
        assert_eq!(c.creds_path, "/tmp/creds.json");
        assert_eq!(c.daily_budget, 33.3);
        assert_eq!(c.work_days, 7, "work_days clamps to 7");
    }

    #[test]
    fn expand_tilde_absolute_unchanged() {
        assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
    }
}
