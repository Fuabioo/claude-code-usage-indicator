use crate::budget::{
    compute_hourly_color, compute_weekly_pace_color, fetch_usage, read_token, BudgetError,
    BudgetState, WindowState,
};
use crate::config::{self, Config};
use cosmic::cosmic_config::CosmicConfigEntry;
use chrono::Utc;
use cosmic::app::Core;
use cosmic::iced::Subscription;
use cosmic::iced::window;
use cosmic::{Application, Element};
use std::time::{Duration, Instant};

const APP_ID: &str = "dev.fuabioo.CosmicAppletCcUsage";

/// Main application model.
pub struct AppModel {
    pub core: Core,
    pub popup: Option<window::Id>,
    pub config: Config,
    pub budget: Option<BudgetState>,
    pub error: Option<BudgetError>,
    pub client: reqwest::Client,
    pub last_attempted: Option<Instant>,
}

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    PopupClosed(window::Id),
    Tick,
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
        };

        (app, first_fetch)
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

            Message::Tick => {
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

            Message::BudgetUpdate(result) => match result {
                Ok(state) => {
                    eprintln!(
                        "[cc-usage] poll OK: weekly={:.0}% session={:.0}%",
                        state.weekly.utilization, state.hourly.utilization
                    );
                    self.budget = Some(state);
                    self.error = None;
                }
                Err(err) => {
                    eprintln!("[cc-usage] poll error: {err}");
                    self.error = Some(err);
                    // Keep previous budget data if available (stale data is better than no data)

                    // Spawn delayed retry (30s) for faster recovery on transient errors
                    eprintln!("[cc-usage] retrying in 30s...");
                    let client = self.client.clone();
                    let creds_path = self.config.creds_path.clone();
                    let daily_budget = self.config.daily_budget;
                    let work_days = self.config.work_days;
                    return cosmic::task::future(async move {
                        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                        Message::BudgetUpdate(
                            fetch_and_compute(&creds_path, daily_budget, work_days, &client).await,
                        )
                    });
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
