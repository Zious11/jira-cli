//! `jr api` — raw API passthrough command.
//!
//! Provides an escape hatch for calling the Jira REST API directly with
//! stored credentials, modeled on `gh api`. Supports method override,
//! request body (inline / file / stdin), and custom headers.

use crate::error::JrError;
use anyhow::Result;
use clap::ValueEnum;
use reqwest::Method;
use reqwest::header::{HeaderName, HeaderValue};

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl From<HttpMethod> for Method {
    fn from(method: HttpMethod) -> Self {
        match method {
            HttpMethod::Get => Method::GET,
            HttpMethod::Post => Method::POST,
            HttpMethod::Put => Method::PUT,
            HttpMethod::Patch => Method::PATCH,
            HttpMethod::Delete => Method::DELETE,
        }
    }
}

/// Normalize a user-provided API path:
/// - Accept absolute paths like `/rest/api/3/myself`
/// - Prepend `/` if missing (e.g. `rest/api/3/myself` → `/rest/api/3/myself`)
/// - Reject absolute URLs (starting with `http://` or `https://`)
pub fn normalize_path(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Err(JrError::UserError(
            "Use a path like /rest/api/3/... — do not include the instance URL".into(),
        )
        .into());
    }
    if trimmed.starts_with('/') {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("/{trimmed}"))
    }
}

/// Parse a user-supplied header string in `Key: Value` format.
/// Rejects `Authorization` (case-insensitive) to prevent credential override.
pub fn parse_header(raw: &str) -> Result<(HeaderName, HeaderValue)> {
    let (key, value) = raw.split_once(':').ok_or_else(|| {
        JrError::UserError(format!(
            "Header must be in 'Key: Value' format (got: {raw})"
        ))
    })?;

    let key = key.trim();
    let value = value.trim();

    if key.is_empty() {
        return Err(JrError::UserError("Header key cannot be empty".into()).into());
    }

    if key.eq_ignore_ascii_case("authorization") {
        return Err(JrError::UserError(
            "Cannot override the Authorization header — auth is managed by jr".into(),
        )
        .into());
    }

    let name = HeaderName::from_bytes(key.as_bytes())
        .map_err(|e| JrError::UserError(format!("Invalid header name '{key}': {e}")))?;
    let value = HeaderValue::from_str(value)
        .map_err(|e| JrError::UserError(format!("Invalid header value '{value}': {e}")))?;

    Ok((name, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_with_slash() {
        let result = normalize_path("/rest/api/3/myself").unwrap();
        assert_eq!(result, "/rest/api/3/myself");
    }

    #[test]
    fn test_normalize_path_without_slash() {
        let result = normalize_path("rest/api/3/myself").unwrap();
        assert_eq!(result, "/rest/api/3/myself");
    }

    #[test]
    fn test_normalize_path_trims_whitespace() {
        let result = normalize_path("  /rest/api/3/myself  ").unwrap();
        assert_eq!(result, "/rest/api/3/myself");
    }

    #[test]
    fn test_normalize_path_rejects_http_url() {
        let err = normalize_path("http://site.atlassian.net/rest/api/3/myself").unwrap_err();
        assert!(err.to_string().contains("do not include the instance URL"));
    }

    #[test]
    fn test_normalize_path_rejects_https_url() {
        let err = normalize_path("https://site.atlassian.net/rest/api/3/myself").unwrap_err();
        assert!(err.to_string().contains("do not include the instance URL"));
    }

    #[test]
    fn test_parse_header_valid() {
        let (name, value) = parse_header("X-Foo: bar").unwrap();
        assert_eq!(name.as_str(), "x-foo");
        assert_eq!(value.to_str().unwrap(), "bar");
    }

    #[test]
    fn test_parse_header_no_colon() {
        let err = parse_header("X-Foo bar").unwrap_err();
        assert!(err.to_string().contains("Key: Value"));
    }

    #[test]
    fn test_parse_header_empty_key() {
        let err = parse_header(": bar").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_parse_header_trims_whitespace() {
        let (name, value) = parse_header("  X-Foo  :   bar  ").unwrap();
        assert_eq!(name.as_str(), "x-foo");
        assert_eq!(value.to_str().unwrap(), "bar");
    }

    #[test]
    fn test_parse_header_value_with_colon() {
        // Value contains a colon — should split on FIRST colon only
        let (name, value) = parse_header("X-Request-Id: abc:def:ghi").unwrap();
        assert_eq!(name.as_str(), "x-request-id");
        assert_eq!(value.to_str().unwrap(), "abc:def:ghi");
    }

    #[test]
    fn test_parse_header_rejects_authorization() {
        let err = parse_header("Authorization: Bearer foo").unwrap_err();
        assert!(err.to_string().contains("Authorization"));
    }

    #[test]
    fn test_parse_header_rejects_authorization_case_insensitive() {
        let err = parse_header("authorization: Bearer foo").unwrap_err();
        assert!(err.to_string().contains("Authorization"));
        let err = parse_header("AUTHORIZATION: Bearer foo").unwrap_err();
        assert!(err.to_string().contains("Authorization"));
    }
}
