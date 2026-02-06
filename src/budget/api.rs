use super::types::{BudgetError, UsageResponse};
use reqwest::StatusCode;

const API_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// Fetches current usage data from the Anthropic OAuth usage API.
///
/// # Arguments
///
/// * `token` - OAuth bearer token (obtained from the Claude credentials file)
/// * `client` - Reusable HTTP client with timeout configuration
///
/// # Returns
///
/// The parsed UsageResponse on success, or a BudgetError on failure.
///
/// # Security
///
/// This function does NOT log the token value. The token is only passed as a Bearer header.
pub async fn fetch_usage(
    token: &str,
    client: &reqwest::Client,
) -> Result<UsageResponse, BudgetError> {
    let response = client
        .get(API_URL)
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| BudgetError::Network(e.to_string()))?;

    match response.status() {
        StatusCode::OK => {
            let body = response
                .json::<UsageResponse>()
                .await
                .map_err(|e| BudgetError::Parse(e.to_string()))?;
            Ok(body)
        }
        StatusCode::UNAUTHORIZED => Err(BudgetError::Unauthorized),
        StatusCode::TOO_MANY_REQUESTS => Err(BudgetError::RateLimited),
        status => Err(BudgetError::UnexpectedStatus(status.as_u16())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_url_is_correct() {
        assert_eq!(API_URL, "https://api.anthropic.com/api/oauth/usage");
    }

    #[test]
    fn test_budget_error_display() {
        let err = BudgetError::Network("connection refused".to_string());
        assert_eq!(err.to_string(), "network error: connection refused");

        let err = BudgetError::Unauthorized;
        assert_eq!(err.to_string(), "unauthorized -- token may be expired");

        let err = BudgetError::RateLimited;
        assert_eq!(err.to_string(), "rate limited by API");

        let err = BudgetError::UnexpectedStatus(500);
        assert_eq!(err.to_string(), "unexpected HTTP status: 500");
    }

    #[test]
    fn test_budget_error_is_clonable() {
        let err = BudgetError::Network("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }
}
