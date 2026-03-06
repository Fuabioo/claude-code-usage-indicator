use crate::budget::{
    compute_hourly_color, compute_weekly_pace_color, fetch_usage, read_token, BudgetError,
    BudgetState, WindowState,
};
use crate::config::{self, Config};
use cosmic::cosmic_config::CosmicConfigEntry;
use chrono::Utc;
use cosmic::app::Core;
use cosmic::iced::futures::StreamExt;
use cosmic::iced::Subscription;
use cosmic::iced::window;
use cosmic::{Application, Element};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

const APP_ID: &str = "dev.fuabioo.CosmicAppletCcUsage";

/// Main application model.
pub struct AppModel {
    pub core: Core,
    pub popup: Option<window::Id>,
    pub config: Config,
    pub budget: Option<BudgetState>,
    pub error: Option<BudgetError>,
}

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    PopupClosed(window::Id),
    BudgetUpdate(Result<BudgetState, BudgetError>),
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

        let app = AppModel {
            core,
            popup: None,
            config,
            budget: None,
            error: None,
        };

        (app, cosmic::app::Task::none())
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
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

            Message::BudgetUpdate(result) => match result {
                Ok(state) => {
                    self.budget = Some(state);
                    self.error = None;
                }
                Err(err) => {
                    self.error = Some(err);
                    // Keep previous budget data if available (stale data is better than no data)
                }
            },

            Message::CosmicConfigUpdate(new_config) => {
                self.config = new_config;
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
            budget_poller(
                self.config.poll_interval_secs,
                self.config.creds_path.clone(),
                self.config.daily_budget,
                self.config.work_days,
            ),
            self.core()
                .watch_config::<Config>(APP_ID)
                .map(|update| Message::CosmicConfigUpdate(update.config)),
        ])
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Self::Message> {
        Some(Message::PopupClosed(id))
    }
}

/// Budget polling subscription.
///
/// Continuously polls the Anthropic API for usage data at the configured interval.
fn budget_poller(
    poll_interval_secs: u64,
    creds_path: String,
    daily_budget: f64,
    work_days: u8,
) -> Subscription<Message> {
    // Hash config values into subscription ID to restart subscription on config changes
    let mut hasher = DefaultHasher::new();
    poll_interval_secs.hash(&mut hasher);
    creds_path.hash(&mut hasher);
    daily_budget.to_bits().hash(&mut hasher);
    work_days.hash(&mut hasher);
    let subscription_id = format!("budget-poll-{:x}", hasher.finish());

    Subscription::run_with_id(
        subscription_id,
        futures_util::stream::unfold(
            (0u64, None::<reqwest::Client>),
            move |(sleep_secs, client)| {
                let creds_path = creds_path.clone();
                async move {
                    // Sleep between polls (skip on first iteration for immediate data)
                    if sleep_secs > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_secs(sleep_secs)).await;
                    }

                    // Create or reuse HTTP client with timeout
                    let client = client.unwrap_or_else(|| {
                        reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(30))
                            .build()
                            .unwrap_or_default()
                    });

                    let result =
                        fetch_and_compute(&creds_path, daily_budget, work_days, &client).await;
                    let next_sleep = match &result {
                        Ok(_) => poll_interval_secs,
                        Err(_) => 30, // Retry faster on error
                    };

                    Some((result, (next_sleep, Some(client))))
                }
            },
        )
        .map(Message::BudgetUpdate),
    )
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
