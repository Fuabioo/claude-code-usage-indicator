use super::types::BudgetError;
use std::fs;
use std::path::Path;

/// Reads the OAuth bearer token from the Claude credentials file.
///
/// # Arguments
///
/// * `path` - Path to the credentials JSON file (typically `~/.claude/.credentials.json`)
///
/// # Returns
///
/// The access token string on success, or a BudgetError on failure.
///
/// # Security
///
/// This function NEVER logs or prints the token value. Handle the returned token carefully.
pub fn read_token(path: &Path) -> Result<String, BudgetError> {
    let content = fs::read_to_string(path).map_err(|e| {
        BudgetError::CredentialsRead(format!("{}: {}", path.display(), e))
    })?;

    parse_token(&content)
}

/// Extract the OAuth access token from credentials JSON content.
///
/// Accepts the same `{"claudeAiOauth": {"accessToken": "..."}}` shape that Claude Code
/// writes, regardless of where the bytes came from (a file or, on macOS, the Keychain).
pub fn parse_token(content: &str) -> Result<String, BudgetError> {
    let parsed: serde_json::Value = serde_json::from_str(content).map_err(|e| {
        BudgetError::CredentialsParse(format!("invalid JSON: {}", e))
    })?;

    let token = parsed
        .get("claudeAiOauth")
        .and_then(|o| o.get("accessToken"))
        .and_then(|t| t.as_str())
        .ok_or(BudgetError::CredentialsMissingToken)?;

    Ok(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_token_success() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(
            file,
            r#"{{"claudeAiOauth": {{"accessToken": "test-token-12345"}}}}"#
        )
        .expect("Failed to write test data");

        let token = read_token(file.path()).expect("Should read token successfully");
        assert_eq!(token, "test-token-12345");
    }

    #[test]
    fn test_read_token_file_not_found() {
        let path = Path::new("/nonexistent/path/file.json");
        let result = read_token(path);

        assert!(result.is_err());
        match result {
            Err(BudgetError::CredentialsRead(msg)) => {
                assert!(msg.contains("nonexistent"));
            }
            _ => panic!("Expected CredentialsRead error"),
        }
    }

    #[test]
    fn test_read_token_invalid_json() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, "{{invalid json").expect("Failed to write test data");

        let result = read_token(file.path());

        assert!(result.is_err());
        match result {
            Err(BudgetError::CredentialsParse(msg)) => {
                assert!(msg.contains("invalid JSON"));
            }
            _ => panic!("Expected CredentialsParse error"),
        }
    }

    #[test]
    fn test_read_token_missing_oauth_field() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, r#"{{"otherField": "value"}}"#).expect("Failed to write test data");

        let result = read_token(file.path());

        assert!(result.is_err());
        match result {
            Err(BudgetError::CredentialsMissingToken) => {}
            _ => panic!("Expected CredentialsMissingToken error"),
        }
    }

    #[test]
    fn test_read_token_missing_access_token_field() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, r#"{{"claudeAiOauth": {{"otherField": "value"}}}}"#)
            .expect("Failed to write test data");

        let result = read_token(file.path());

        assert!(result.is_err());
        match result {
            Err(BudgetError::CredentialsMissingToken) => {}
            _ => panic!("Expected CredentialsMissingToken error"),
        }
    }

    #[test]
    fn test_read_token_null_access_token() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, r#"{{"claudeAiOauth": {{"accessToken": null}}}}"#)
            .expect("Failed to write test data");

        let result = read_token(file.path());

        assert!(result.is_err());
        match result {
            Err(BudgetError::CredentialsMissingToken) => {}
            _ => panic!("Expected CredentialsMissingToken error"),
        }
    }

    #[test]
    fn test_read_token_non_string_access_token() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, r#"{{"claudeAiOauth": {{"accessToken": 12345}}}}"#)
            .expect("Failed to write test data");

        let result = read_token(file.path());

        assert!(result.is_err());
        match result {
            Err(BudgetError::CredentialsMissingToken) => {}
            _ => panic!("Expected CredentialsMissingToken error"),
        }
    }
}
