use cc_usage_budget::{
    compute_hourly_color, compute_weekly_pace_color, fetch_usage, read_token, BudgetError,
    BudgetState, WindowState,
};
use crate::config::{self, Config};
use chrono::Utc;
use cosmic::app::Core;
use cosmic::cosmic_config::CosmicConfigEntry;
use cosmic::iced::Subscription;
use cosmic::iced::window;
use cosmic::{Application, Element};
use std::time::{Duration, Instant};

const APP_ID: &str = "dev.fuabioo.CosmicAppletCcUsage";

/// Initial backoff delay for rate-limit errors (seconds).
const INITIAL_BACKOFF_SECS: u64 = 30;
/// Maximum backoff delay (seconds). Aligns with the default poll_interval (300s)
/// so at worst the retry cadence matches normal polling.
const MAX_BACKOFF_SECS: u64 = 300;
/// Fixed retry delay for non-rate-limit transient errors (seconds).
const NON_RATELIMIT_RETRY_SECS: u64 = 30;

/// Main application model.
pub struct AppModel {
    pub core: Core,
    pub popup: Option<window::Id>,
    pub config: Config,
    pub budget: Option<BudgetState>,
    pub error: Option<BudgetError>,
    pub client: reqwest::Client,
    pub last_attempted: Option<Instant>,
    /// Incremented on success, Tick, and config change.
    /// Stale retry futures check this to avoid compounding.
    pub retry_generation: u64,
    /// Current rate-limit backoff in seconds (0 = not rate-limited).
    pub rate_limit_backoff: u64,
    /// When any error cooldown expires. Tick is suppressed until this time.
    pub error_backoff_until: Option<Instant>,
}

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    PopupClosed(window::Id),
    Tick,
    BudgetUpdate(Result<BudgetState, BudgetError>),
    RetryBudgetUpdate {
        result: Result<BudgetState, BudgetError>,
        generation: u64,
    },
    CosmicConfigUpdate(Config),
    Surface(cosmic::surface::Action),
}

impl Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, cosmic::app::Task<Self::Message>) {
        let config = match cosmic::cosmic_config::Config::new(APP_ID, config::CONFIG_VERSION) {
            Ok(config_helper) => match Config::get_entry(&config_helper) {
                Ok(c) => c.validated(),
                Err((_errs, c)) => c.validated(),
            },
            Err(_) => Config::default(),
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        eprintln!(
            "[cc-usage] starting with poll_interval={}s",
            config.poll_interval_secs
        );

        // Spawn immediate first fetch
        let first_fetch = {
            let client = client.clone();
            let creds_path = config.creds_path.clone();
            let daily_budget = config.daily_budget;
            let work_days = config.work_days;
            cosmic::task::future(async move {
                Message::BudgetUpdate(
                    fetch_and_compute(&creds_path, daily_budget, work_days, &client).await,
                )
            })
        };

        let app = AppModel {
            core,
            popup: None,
            config,
            budget: None,
            error: None,
            client,
            last_attempted: Some(Instant::now()),
            retry_generation: 0,
            rate_limit_backoff: 0,
            error_backoff_until: None,
        };

        (app, first_fetch)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.core
            .applet
            .autosize_window(crate::views::panel::render(self))
            .into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Self::Message> {
        crate::views::popup::render(self, id)
    }

    fn update(&mut self, message: Self::Message) -> cosmic::app::Task<Self::Message> {
        match message {
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }

            Message::Tick => {
                // Suppress polling while in error backoff
                if let Some(until) = self.error_backoff_until {
                    if Instant::now() < until {
                        eprintln!("[cc-usage] tick suppressed: error backoff active");
                        return cosmic::app::Task::none();
                    }
                    self.error_backoff_until = None;
                }
                // Tick is a fresh polling attempt — increment generation to
                // invalidate any in-flight retry chains from previous errors
                self.retry_generation += 1;
                self.last_attempted = Some(Instant::now());
                eprintln!("[cc-usage] polling API...");
                let client = self.client.clone();
                let creds_path = self.config.creds_path.clone();
                let daily_budget = self.config.daily_budget;
                let work_days = self.config.work_days;
                return cosmic::task::future(async move {
                    Message::BudgetUpdate(
                        fetch_and_compute(&creds_path, daily_budget, work_days, &client).await,
                    )
                });
            }

            Message::BudgetUpdate(result) => {
                return self.handle_budget_result(result);
            }

            Message::RetryBudgetUpdate { result, generation } => {
                if generation != self.retry_generation {
                    eprintln!(
                        "[cc-usage] stale retry discarded (gen {} vs current {})",
                        generation, self.retry_generation
                    );
                    return cosmic::app::Task::none();
                }
                return self.handle_budget_result(result);
            }

            Message::CosmicConfigUpdate(new_config) => {
                self.config = new_config;
                // Config changes (especially creds_path) may resolve the error condition
                self.retry_generation += 1;
                self.rate_limit_backoff = 0;
                self.error_backoff_until = None;
            }

            Message::Surface(action) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(action),
                ));
            }
        }

        cosmic::app::Task::none()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch([
            cosmic::iced::time::every(Duration::from_secs(self.config.poll_interval_secs))
                .map(|_| Message::Tick),
            self.core()
                .watch_config::<Config>(APP_ID)
                .map(|update| Message::CosmicConfigUpdate(update.config)),
        ])
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Self::Message> {
        Some(Message::PopupClosed(id))
    }
}

impl AppModel {
    /// Shared handler for budget fetch results (used by both BudgetUpdate and RetryBudgetUpdate).
    fn handle_budget_result(
        &mut self,
        result: Result<BudgetState, BudgetError>,
    ) -> cosmic::app::Task<Message> {
        match result {
            Ok(state) => {
                eprintln!(
                    "[cc-usage] poll OK: weekly={:.0}% session={:.0}%",
                    state.weekly.utilization, state.hourly.utilization
                );
                self.budget = Some(state);
                self.error = None;
                self.rate_limit_backoff = 0;
                self.error_backoff_until = None;
                self.retry_generation += 1; // invalidates all in-flight retries
                cosmic::app::Task::none()
            }
            Err(err) => {
                eprintln!("[cc-usage] poll error: {err}");
                self.error = Some(err.clone());
                // Keep previous budget data if available (stale data is better than no data)

                // Don't retry Unauthorized — tokens don't un-expire
                if matches!(err, BudgetError::Unauthorized) {
                    eprintln!("[cc-usage] not retrying: token needs manual refresh");
                    return cosmic::app::Task::none();
                }

                let retry_delay = match &err {
                    BudgetError::RateLimited(server_secs) => {
                        self.rate_limit_backoff =
                            next_rate_limit_backoff(self.rate_limit_backoff, *server_secs);
                        eprintln!(
                            "[cc-usage] rate limited, backing off {}s",
                            self.rate_limit_backoff
                        );
                        self.rate_limit_backoff
                    }
                    _ => {
                        eprintln!("[cc-usage] retrying in {NON_RATELIMIT_RETRY_SECS}s...");
                        NON_RATELIMIT_RETRY_SECS
                    }
                };

                self.error_backoff_until =
                    Some(Instant::now() + Duration::from_secs(retry_delay));
                let gen = self.retry_generation;
                let client = self.client.clone();
                let creds_path = self.config.creds_path.clone();
                let daily_budget = self.config.daily_budget;
                let work_days = self.config.work_days;
                cosmic::task::future(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay)).await;
                    Message::RetryBudgetUpdate {
                        result: fetch_and_compute(&creds_path, daily_budget, work_days, &client)
                            .await,
                        generation: gen,
                    }
                })
            }
        }
    }
}

/// Compute next backoff delay for rate-limit errors.
/// Returns seconds to wait before next retry, capped at MAX_BACKOFF_SECS.
fn next_rate_limit_backoff(current: u64, server_retry_after: u64) -> u64 {
    let next = if current == 0 {
        INITIAL_BACKOFF_SECS.max(server_retry_after)
    } else {
        (current * 2).max(server_retry_after)
    };
    next.min(MAX_BACKOFF_SECS)
}

/// Fetch usage data and compute pace colors.
async fn fetch_and_compute(
    creds_path: &str,
    daily_budget: f64,
    work_days: u8,
    client: &reqwest::Client,
) -> Result<BudgetState, BudgetError> {
    let expanded_path = config::expand_tilde(creds_path);
    let token = read_token(&expanded_path)?;
    let usage = fetch_usage(&token, client).await?;

    let weekly_color = compute_weekly_pace_color(
        usage.seven_day.utilization,
        daily_budget,
        work_days,
        usage.seven_day.resets_at,
        Utc::now(),
    );
    let hourly_color = compute_hourly_color(usage.five_hour.utilization);

    Ok(BudgetState {
        weekly: WindowState {
            utilization: usage.seven_day.utilization,
            resets_at: usage.seven_day.resets_at,
            pace_color: weekly_color,
        },
        hourly: WindowState {
            utilization: usage.five_hour.utilization,
            resets_at: usage.five_hour.resets_at,
            pace_color: hourly_color,
        },
        last_updated: Instant::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_initial_no_server_hint() {
        assert_eq!(next_rate_limit_backoff(0, 0), INITIAL_BACKOFF_SECS);
    }

    #[test]
    fn test_backoff_initial_server_floor_wins() {
        assert_eq!(next_rate_limit_backoff(0, 60), 60);
    }

    #[test]
    fn test_backoff_initial_server_capped() {
        assert_eq!(next_rate_limit_backoff(0, 999), MAX_BACKOFF_SECS);
    }

    #[test]
    fn test_backoff_doubling() {
        assert_eq!(next_rate_limit_backoff(30, 0), 60);
        assert_eq!(next_rate_limit_backoff(60, 0), 120);
        assert_eq!(next_rate_limit_backoff(120, 0), 240);
    }

    #[test]
    fn test_backoff_cap() {
        assert_eq!(next_rate_limit_backoff(240, 0), MAX_BACKOFF_SECS);
        assert_eq!(next_rate_limit_backoff(300, 0), MAX_BACKOFF_SECS);
    }

    #[test]
    fn test_backoff_server_floor_over_double() {
        assert_eq!(next_rate_limit_backoff(30, 200), 200);
    }

    #[test]
    fn test_backoff_full_sequence() {
        let mut backoff = 0u64;
        let expected = [30, 60, 120, 240, 300, 300];
        for &exp in &expected {
            backoff = next_rate_limit_backoff(backoff, 0);
            assert_eq!(backoff, exp);
        }
    }
}
