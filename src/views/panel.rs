use crate::app::{AppModel, Message};
use cc_usage_budget::{format_duration, BudgetState};
use crate::config::Config;
use crate::fl;
use cosmic::iced::{window, Rectangle};
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{container, text};
use cosmic::{widget, Element};

/// Render the panel icon view (what appears in the COSMIC panel bar).
pub fn render(app: &AppModel) -> Element<'_, Message> {
    let content = match (&app.budget, &app.error) {
        (Some(budget), _) => render_budget_view(budget, &app.config),
        (None, Some(_)) => render_error_view(),
        (None, None) => render_loading_view(),
    };

    // Popup creation logic inline using on_press_with_rectangle
    let have_popup = app.popup;
    let btn = widget::button::custom(content)
        .class(cosmic::theme::Button::AppletIcon)
        .on_press_with_rectangle(move |offset, bounds| {
            if let Some(id) = have_popup {
                Message::Surface(destroy_popup(id))
            } else {
                Message::Surface(app_popup::<AppModel>(
                    move |state: &mut AppModel| {
                        let new_id = window::Id::unique();
                        state.popup = Some(new_id);
                        let mut popup_settings = state.core.applet.get_popup_settings(
                            state.core.main_window_id().unwrap(),
                            new_id,
                            None,
                            None,
                            None,
                        );
                        popup_settings.positioner.anchor_rect = Rectangle {
                            x: (bounds.x - offset.x) as i32,
                            y: (bounds.y - offset.y) as i32,
                            width: bounds.width as i32,
                            height: bounds.height as i32,
                        };
                        popup_settings
                    },
                    None, // Use view_window for popup content
                ))
            }
        });

    let popup_open = app.popup.is_some();
    Element::from(
        app.core.applet.applet_tooltip::<Message>(
            btn,
            fl!("app-name"),
            popup_open,
            |a| Message::Surface(a),
            None,
        ),
    )
}

/// Render the budget display based on panel size.
fn render_budget_view<'a>(budget: &'a BudgetState, config: &'a Config) -> Element<'a, Message> {
    let panel_size = detect_panel_size();

    let weekly_pct = format!("{:.0}%", budget.weekly.utilization);
    let hourly_pct = format!("{:.0}%", budget.hourly.utilization);

    let weekly_color = config.resolve_pace_color(&budget.weekly.pace_color);
    let hourly_color = config.resolve_pace_color(&budget.hourly.pace_color);

    let weekly_resets = format_duration(
        budget
            .weekly
            .resets_at
            .signed_duration_since(chrono::Utc::now()),
    );
    let hourly_resets = format_duration(
        budget
            .hourly
            .resets_at
            .signed_duration_since(chrono::Utc::now()),
    );

    match panel_size {
        PanelSize::Small => {
            // Just a colored dot
            container(text("●").class(cosmic::theme::Text::Color(weekly_color)))
                .padding(4)
                .into()
        }
        PanelSize::Medium => {
            // Weekly percentage only
            container(text(weekly_pct.clone()).class(cosmic::theme::Text::Color(weekly_color)))
                .padding([0, 8])
                .into()
        }
        PanelSize::Large => {
            // Weekly % | Session %
            widget::row::with_children(vec![
                text(weekly_pct.clone())
                    .class(cosmic::theme::Text::Color(weekly_color))
                    .into(),
                text(" | ").class(cosmic::theme::Text::Default).into(),
                text(hourly_pct.clone())
                    .class(cosmic::theme::Text::Color(hourly_color))
                    .into(),
            ])
            .padding([0, 8])
            .spacing(4)
            .into()
        }
        PanelSize::XL => {
            // Full: "42% 3d12h | 15% 2h45m"
            widget::row::with_children(vec![
                text(weekly_pct)
                    .class(cosmic::theme::Text::Color(weekly_color))
                    .into(),
                text(format!(" {}", weekly_resets))
                    .class(cosmic::theme::Text::Default)
                    .into(),
                text(" | ").class(cosmic::theme::Text::Default).into(),
                text(hourly_pct)
                    .class(cosmic::theme::Text::Color(hourly_color))
                    .into(),
                text(format!(" {}", hourly_resets))
                    .class(cosmic::theme::Text::Default)
                    .into(),
            ])
            .padding([0, 8])
            .spacing(4)
            .into()
        }
    }
}

/// Render the error state (no credentials or API failure).
fn render_error_view() -> Element<'static, Message> {
    container(text("--").class(cosmic::theme::Text::Default))
        .padding([0, 8])
        .into()
}

/// Render the loading/initial state (before first budget fetch).
fn render_loading_view() -> Element<'static, Message> {
    container(text("?").class(cosmic::theme::Text::Default))
        .padding([0, 8])
        .into()
}

/// Detect panel size from environment variable.
///
/// COSMIC_PANEL_SIZE is set by the panel compositor.
fn detect_panel_size() -> PanelSize {
    match std::env::var("COSMIC_PANEL_SIZE").as_deref() {
        Ok("small") => PanelSize::Small,
        Ok("medium") => PanelSize::Medium,
        Ok("large") => PanelSize::Large,
        Ok("xl") => PanelSize::XL,
        _ => PanelSize::Large, // Default
    }
}

enum PanelSize {
    Small,
    Medium,
    Large,
    XL,
}
