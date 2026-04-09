//! `jr api` — raw API passthrough command.
//!
//! Provides an escape hatch for calling the Jira REST API directly with
//! stored credentials, modeled on `gh api`. Supports method override,
//! request body (inline / file / stdin), and custom headers.

use crate::error::JrError;
use anyhow::Result;
use clap::ValueEnum;
use reqwest::Method;

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
}
