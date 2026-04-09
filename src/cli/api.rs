//! `jr api` — raw API passthrough command.
//!
//! Provides an escape hatch for calling the Jira REST API directly with
//! stored credentials, modeled on `gh api`. Supports method override,
//! request body (inline / file / stdin), and custom headers.

use crate::api::client::JiraClient;
use crate::error::JrError;
use anyhow::Result;
use clap::ValueEnum;
use reqwest::Method;
use reqwest::header::{HeaderName, HeaderValue};
use std::io::Read;

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

/// Resolve the `--data` argument into an actual request body.
/// - `None` → `None`
/// - `Some("@-")` → read from `stdin` parameter
/// - `Some("@filename")` → read from file
/// - `Some(inline)` → use as-is
///
/// Validates that the resulting body is valid JSON.
pub fn resolve_body<R: Read>(arg: Option<&str>, mut stdin: R) -> Result<Option<String>> {
    let body = match arg {
        None => return Ok(None),
        Some("@-") => {
            let mut buf = String::new();
            stdin.read_to_string(&mut buf)?;
            buf
        }
        Some(s) if s.starts_with('@') => {
            let path = &s[1..];
            std::fs::read_to_string(path)?
        }
        Some(s) => s.to_string(),
    };

    // Validate JSON — Jira REST API always uses JSON, catch typos before network
    serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| JrError::UserError(format!("Request body is not valid JSON: {e}")))?;

    Ok(Some(body))
}

/// Main entry point for `jr api`.
///
/// Takes the parsed CLI arguments, performs validation, builds an HTTP request,
/// sends it via `JiraClient::send_raw`, and prints the response body to stdout.
pub async fn handle_api(
    _path: String,
    _method: HttpMethod,
    _data: Option<String>,
    _header: Vec<String>,
    _client: &JiraClient,
) -> Result<()> {
    // Implemented in Task 7
    Ok(())
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

    use std::io::Cursor;

    #[test]
    fn test_resolve_body_none() {
        let stdin: Cursor<&[u8]> = Cursor::new(b"");
        let result = resolve_body(None, stdin).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_body_inline_json() {
        let stdin: Cursor<&[u8]> = Cursor::new(b"");
        let result = resolve_body(Some(r#"{"a":1}"#), stdin).unwrap();
        assert_eq!(result, Some(r#"{"a":1}"#.to_string()));
    }

    #[test]
    fn test_resolve_body_invalid_json_errors() {
        let stdin: Cursor<&[u8]> = Cursor::new(b"");
        let err = resolve_body(Some("not json"), stdin).unwrap_err();
        assert!(err.to_string().contains("Request body is not valid JSON"));
    }

    #[test]
    fn test_resolve_body_at_file_reads_contents() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), r#"{"from":"file"}"#).unwrap();
        let arg = format!("@{}", tmp.path().display());

        let stdin: Cursor<&[u8]> = Cursor::new(b"");
        let result = resolve_body(Some(&arg), stdin).unwrap();
        assert_eq!(result, Some(r#"{"from":"file"}"#.to_string()));
    }

    #[test]
    fn test_resolve_body_at_file_not_found() {
        let stdin: Cursor<&[u8]> = Cursor::new(b"");
        let err = resolve_body(Some("@/nonexistent/path/to/file.json"), stdin).unwrap_err();
        // Propagated std::io::Error
        assert!(err.to_string().to_lowercase().contains("no such file"));
    }

    #[test]
    fn test_resolve_body_at_dash_reads_stdin() {
        let stdin_content = br#"{"from":"stdin"}"#;
        let stdin = Cursor::new(&stdin_content[..]);
        let result = resolve_body(Some("@-"), stdin).unwrap();
        assert_eq!(result, Some(r#"{"from":"stdin"}"#.to_string()));
    }
}
