use reqwest::header::HeaderMap;

/// Cap on Retry-After header values jr will honor before aborting retry.
///
/// Atlassian's typical Retry-After values are 1425-3089s (24-50 minutes) per
/// Atlassian community forum reports; documented ceiling is 3600s. Foreground
/// 30-min sleep is poor UX for an interactive CLI. RFC 9110 §10.2.3 confirms
/// the client MAY abort instead of honoring Retry-After. Users running batch
/// operations should wrap jr in a shell-level retry/cron job.
///
/// Source: .factory/research/S-3.07-wave3-verification.md (Part A claim verified)
pub const MAX_RETRY_AFTER_SECS: u64 = 60;

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
    // NFR-SCA-1: Retry-After integer-only parsing is deliberate. Atlassian sends
    // seconds-as-integer in practice; HTTP-date format ("Mon, 04 May 2026 00:00:00 GMT")
    // is not observed but would silently fall through to DEFAULT_RETRY_SECS=1. If HTTP-date
    // variants surface in production, add chrono parsing here. Coordinated with
    // NFR-R-NEW-1 cap delivered in S-3.07.
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
