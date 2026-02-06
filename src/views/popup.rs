use crate::app::{AppModel, Message};
use crate::budget::{format_duration, weekday_to_work_index, BudgetError, BudgetState, PaceColor};
use crate::config::Config;
use crate::fl;
use chrono::{Datelike, Local};
use cosmic::iced::Length;
use cosmic::iced_core::Alignment;
use cosmic::widget::text;
use cosmic::{widget, Element};

/// Render the popup detail view (full budget dashboard).
pub fn render(app: &AppModel, _id: cosmic::iced_core::window::Id) -> Element<'_, Message> {
    let content = match (&app.budget, &app.error) {
        (Some(budget), _) => render_dashboard(budget, app),
        (None, Some(err)) => render_error_dashboard(err, &app.config),
        (None, None) => render_loading_dashboard(),
    };

    app.core
        .applet
        .popup_container(content)
        .max_width(372.0)
        .min_width(300.0)
        .into()
}

/// Render the full dashboard with all sections.
fn render_dashboard<'a>(budget: &'a BudgetState, app: &'a AppModel) -> Element<'a, Message> {
    let config = &app.config;
    let divider = || -> Element<'a, Message> { widget::divider::horizontal::default().into() };

    widget::column::with_children(vec![
        render_weekly_section(budget, config),
        divider(),
        render_hourly_section(budget, config),
        divider(),
        render_daily_pace_section(budget, config),
        divider(),
        render_footer(budget, app.error.as_ref(), config),
    ])
    .spacing(12)
    .padding(12)
    .width(Length::Fill)
    .into()
}

/// Section 1: Weekly Budget
fn render_weekly_section<'a>(budget: &'a BudgetState, config: &'a Config) -> Element<'a, Message> {
    let pct = budget.weekly.utilization;
    let color = &budget.weekly.pace_color;

    let resets_duration = budget
        .weekly
        .resets_at
        .signed_duration_since(chrono::Utc::now());
    let resets_text = format_duration(resets_duration);

    let pace_label = match color {
        PaceColor::Green => fl!("pace-on-track"),
        PaceColor::Yellow => fl!("pace-caution"),
        PaceColor::Red => fl!("pace-over-budget"),
    };

    let weekly_budget_text = fl!("weekly-budget");
    let resets_in_text = fl!("resets-in", time = resets_text.as_str());

    widget::column::with_children(vec![
        widget::text::heading(weekly_budget_text).into(),
        cosmic::widget::progress_bar(0.0..=100.0, pct as f32).class(pace_color_to_progress(*color)).into(),
        widget::row::with_children(vec![
            text(format!("{:.0}%", pct))
                .class(cosmic::theme::Text::Color(config.resolve_pace_color(color)))
                .size(20)
                .font(cosmic::font::semibold())
                .into(),
            text(format!(" - {}", pace_label)).class(cosmic::theme::Text::Color(config.resolve_pace_color(color))).into(),
        ])
        .spacing(4)
        .align_y(Alignment::Center)
        .into(),
        text(resets_in_text).class(cosmic::theme::Text::Default).into(),
    ])
    .spacing(8)
    .into()
}

/// Section 2: Session
fn render_hourly_section<'a>(budget: &'a BudgetState, config: &'a Config) -> Element<'a, Message> {
    let pct = budget.hourly.utilization;
    let color = &budget.hourly.pace_color;

    let resets_duration = budget
        .hourly
        .resets_at
        .signed_duration_since(chrono::Utc::now());
    let resets_text = format_duration(resets_duration);

    let hourly_session_text = fl!("session-budget");
    let resets_in_text = fl!("resets-in", time = resets_text.as_str());

    widget::column::with_children(vec![
        widget::text::heading(hourly_session_text).into(),
        cosmic::widget::progress_bar(0.0..=100.0, pct as f32).class(pace_color_to_progress(*color)).into(),
        widget::row::with_children(vec![
            text(format!("{:.0}%", pct))
                .class(cosmic::theme::Text::Color(config.resolve_pace_color(color)))
                .size(20)
                .font(cosmic::font::semibold())
                .into(),
        ])
        .spacing(4)
        .align_y(Alignment::Center)
        .into(),
        text(resets_in_text).class(cosmic::theme::Text::Default).into(),
    ])
    .spacing(8)
    .into()
}

/// Section 3: Daily Budget Pace (the "am I cooked?" section)
fn render_daily_pace_section<'a>(
    budget: &'a BudgetState,
    config: &'a Config,
) -> Element<'a, Message> {
    let now = Local::now();
    let work_day_index = weekday_to_work_index(now.weekday(), config.work_days);
    let ceiling = work_day_index as f64 * config.daily_budget;

    let weekday_name = format!("{:?}", now.weekday())
        .chars()
        .take(3)
        .collect::<String>();

    let consumed_pct = budget.weekly.utilization;
    let remaining = ceiling - consumed_pct;

    let (remaining_text, remaining_color) = if remaining < 0.0 {
        (
            format!("{}: {:.0}%", fl!("over-by"), remaining.abs()),
            config.resolve_over_budget_color(),
        )
    } else if remaining > ceiling * 0.25 {
        (
            format!("{}: {:.0}%", fl!("remaining-today"), remaining),
            config.resolve_on_track_color(),
        )
    } else {
        (
            format!("{}: {:.0}%", fl!("remaining-today"), remaining),
            config.resolve_warning_color(),
        )
    };

    let daily_budget_pace_text = fl!("daily-budget-pace");
    let todays_ceiling_text = fl!(
        "todays-ceiling-detail",
        label = fl!("todays-ceiling"),
        ceiling = format!("{:.0}", ceiling),
        weekday = weekday_name.as_str(),
        index = work_day_index.to_string(),
        total = config.work_days.to_string()
    );
    let consumed_text = fl!(
        "consumed-detail",
        label = fl!("consumed"),
        consumed = format!("{:.0}", consumed_pct),
        ceiling = format!("{:.0}", ceiling)
    );

    widget::column::with_children(vec![
        widget::text::heading(daily_budget_pace_text).into(),
        text(todays_ceiling_text)
        .class(cosmic::theme::Text::Default)
        .into(),
        text(consumed_text)
        .class(cosmic::theme::Text::Default)
        .into(),
        text(remaining_text).class(cosmic::theme::Text::Color(remaining_color)).into(),
    ])
    .spacing(8)
    .into()
}

/// Section 4: Footer
fn render_footer<'a>(budget: &'a BudgetState, error: Option<&'a BudgetError>, config: &'a Config) -> Element<'a, Message> {
    let elapsed = budget.last_updated.elapsed();
    let elapsed_secs = elapsed.as_secs();

    let elapsed_display = if elapsed_secs < 60 {
        format!("{}s", elapsed_secs)
    } else if elapsed_secs < 3600 {
        format!("{}m", elapsed_secs / 60)
    } else {
        format!("{}h", elapsed_secs / 3600)
    };
    let last_updated_text = fl!("last-updated", time = elapsed_display);

    let freshness_color = if elapsed_secs < 600 {
        config.resolve_on_track_color()
    } else {
        config.resolve_warning_color()
    };
    let freshness_indicator = text("●").class(cosmic::theme::Text::Color(freshness_color));

    let mut footer_items = vec![
        widget::row::with_children(vec![
            freshness_indicator.into(),
            text(last_updated_text).class(cosmic::theme::Text::Default).into(),
        ])
            .spacing(4)
            .into(),
    ];

    if error.is_some() {
        footer_items.push(
            text(fl!("data-may-be-stale"))
                .class(cosmic::theme::Text::Color(config.resolve_warning_color()))
                .into(),
        );
    }

    widget::column::with_children(footer_items).spacing(4).into()
}

/// Render error dashboard (when no budget data is available).
fn render_error_dashboard<'a>(err: &'a BudgetError, config: &'a Config) -> Element<'a, Message> {
    let error_text = match err {
        BudgetError::CredentialsMissingToken | BudgetError::CredentialsRead(_) => {
            fl!("error-credentials-not-found")
        }
        BudgetError::Unauthorized => fl!("error-unauthorized"),
        BudgetError::RateLimited => fl!("error-rate-limited"),
        BudgetError::Network(details) => fl!("error-network", details = details.as_str()),
        BudgetError::Parse(_) | BudgetError::CredentialsParse(_) => fl!("error-parse"),
        BudgetError::UnexpectedStatus(_) => fl!("error-unable-to-fetch"),
    };

    let app_name_text = fl!("app-name");

    widget::column::with_children(vec![
        widget::text::heading(app_name_text).into(),
        widget::divider::horizontal::default().into(),
        text(error_text).class(cosmic::theme::Text::Color(config.resolve_over_budget_color())).into(),
    ])
    .spacing(12)
    .padding(12)
    .into()
}

/// Render loading dashboard (initial state).
fn render_loading_dashboard() -> Element<'static, Message> {
    let app_name_text = fl!("app-name");
    let loading_text = fl!("loading-usage-data");

    widget::column::with_children(vec![
        widget::text::heading(app_name_text).into(),
        widget::divider::horizontal::default().into(),
        text(loading_text).class(cosmic::theme::Text::Default).into(),
    ])
    .spacing(12)
    .padding(12)
    .into()
}

/// Map PaceColor to COSMIC progress bar style.
fn pace_color_to_progress(color: PaceColor) -> cosmic::theme::ProgressBar {
    match color {
        PaceColor::Green => cosmic::theme::ProgressBar::Success,
        PaceColor::Yellow => cosmic::theme::ProgressBar::Primary,
        PaceColor::Red => cosmic::theme::ProgressBar::Danger,
    }
}
