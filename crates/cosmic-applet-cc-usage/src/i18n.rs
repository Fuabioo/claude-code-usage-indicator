use i18n_embed::{
    fluent::{fluent_language_loader, FluentLanguageLoader},
    DesktopLanguageRequester,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub fn loader() -> &'static FluentLanguageLoader {
    static LOADER: std::sync::OnceLock<FluentLanguageLoader> = std::sync::OnceLock::new();
    LOADER.get_or_init(|| fluent_language_loader!())
}

/// Initialize the i18n system.
///
/// Embeds the localization files and selects the appropriate language based on system settings.
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let requested_languages = DesktopLanguageRequester::requested_languages();
    i18n_embed::select(loader(), &Localizations, &requested_languages)?;
    Ok(())
}

/// Convenience macro for translation lookups.
///
/// # Example
///
/// ```
/// let label = fl!("app-name");
/// let message = fl!("resets-in", time = "3d 12h");
/// ```
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::loader(), $message_id)
    }};

    ($message_id:literal, $($args:tt)*) => {{
        i18n_embed_fl::fl!($crate::i18n::loader(), $message_id, $($args)*)
    }};
}
