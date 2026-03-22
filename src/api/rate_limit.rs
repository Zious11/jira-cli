use reqwest::header::HeaderMap;

/// Rate limit information parsed from Jira API response headers.
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// Number of seconds to wait before retrying (from Retry-After header).
    pub retry_after_secs: Option<u64>,
    /// Number of remaining requests in the current window (from X-RateLimit-Remaining header).
    pub remaining: Option<u64>,
}

impl RateLimitInfo {
    /// Parse rate limit information from HTTP response headers.
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let retry_after_secs = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());

        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());

        Self {
            retry_after_secs,
            remaining,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_parse_retry_after_and_remaining() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("5"));
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("42"));

        let info = RateLimitInfo::from_headers(&headers);
        assert_eq!(info.retry_after_secs, Some(5));
        assert_eq!(info.remaining, Some(42));
    }

    #[test]
    fn test_missing_headers_returns_none() {
        let headers = HeaderMap::new();
        let info = RateLimitInfo::from_headers(&headers);
        assert_eq!(info.retry_after_secs, None);
        assert_eq!(info.remaining, None);
    }
}
