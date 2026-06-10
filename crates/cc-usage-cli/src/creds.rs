//! Credential resolution for the CLI.
//!
//! Order: read the credentials **file** (the Linux location and the explicit `--creds-path`).
//! On macOS, where Claude Code stores its OAuth token in the **login Keychain** instead of a
//! file, fall back to the Keychain when no explicit `--creds-path` was given.

use cc_usage_budget::{read_token, BudgetError};

use crate::config::Config;

/// Inputs that influence credential resolution.
pub struct CredsOptions {
    /// True when the user passed `--creds-path` explicitly (disables the Keychain fallback).
    pub creds_path_explicit: bool,
    /// macOS Keychain generic-password service name to look up.
    pub keychain_service: String,
    /// When true, never consult the Keychain.
    pub no_keychain: bool,
}

/// Resolve the OAuth token from the best available source.
pub fn resolve_token(cfg: &Config, opts: &CredsOptions) -> Result<String, BudgetError> {
    let file_result = read_token(&cfg.resolved_creds_path());

    // If the file worked, or the user explicitly chose a file, we're done (success or its error).
    if file_result.is_ok() || opts.creds_path_explicit {
        return file_result;
    }

    // No usable file and no explicit path: try the macOS Keychain.
    #[cfg(target_os = "macos")]
    {
        if !opts.no_keychain {
            match read_token_from_keychain(&opts.keychain_service) {
                Ok(token) => return Ok(token),
                Err(kc_err) => {
                    // Surface both failures so the user knows we tried both sources.
                    let file_err = file_result.unwrap_err();
                    return Err(BudgetError::CredentialsRead(format!(
                        "no credentials file ({file_err}); Keychain fallback failed ({kc_err})"
                    )));
                }
            }
        }
    }

    file_result
}

/// Read the credentials blob from the macOS login Keychain via the `security` tool and
/// extract the token. The stored value is the same JSON Claude Code writes to disk on Linux;
/// if a future format stores the bare token, we accept that too.
#[cfg(target_os = "macos")]
pub fn read_token_from_keychain(service: &str) -> Result<String, BudgetError> {
    use std::process::Command;

    let output = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", service, "-w"])
        .output()
        .map_err(|e| BudgetError::CredentialsRead(format!("could not run security: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(BudgetError::CredentialsRead(format!(
            "Keychain item '{service}' not found: {}",
            stderr.trim()
        )));
    }

    let blob = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if blob.is_empty() {
        return Err(BudgetError::CredentialsMissingToken);
    }

    if blob.starts_with('{') {
        cc_usage_budget::parse_token(&blob)
    } else {
        // Bare token stored directly.
        Ok(blob)
    }
}
