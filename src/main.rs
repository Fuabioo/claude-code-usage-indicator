mod app;
mod budget;
mod config;
mod i18n;
mod views;

use app::AppModel;

fn main() -> cosmic::iced::Result {
    // Initialize i18n system
    if let Err(e) = i18n::init() {
        eprintln!("Warning: Failed to initialize i18n: {}", e);
    }

    // Launch the applet
    cosmic::applet::run::<AppModel>(())
}
