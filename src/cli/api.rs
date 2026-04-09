//! `jr api` — raw API passthrough command.
//!
//! Provides an escape hatch for calling the Jira REST API directly with
//! stored credentials, modeled on `gh api`. Supports method override,
//! request body (inline / file / stdin), and custom headers.

use crate::api::client::{JiraClient, extract_error_message};
use crate::error::JrError;
use anyhow::Result;
use clap::ValueEnum;
use reqwest::Method;
use reqwest::header::{CONTENT_TYPE, HeaderName, HeaderValue};
use std::io::{Read, Write};

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
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
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
    path: String,
    method: HttpMethod,
    data: Option<String>,
    header: Vec<String>,
    client: &JiraClient,
) -> Result<()> {
    let normalized_path = normalize_path(&path)?;

    // Reads real stdin in production; resolve_body takes impl Read for testing.
    let body = resolve_body(data.as_deref(), std::io::stdin().lock())?;

    let custom_headers: Vec<(HeaderName, HeaderValue)> = header
        .iter()
        .map(|h| parse_header(h))
        .collect::<Result<Vec<_>>>()?;

    // Use .build() + headers_mut().insert() for replace semantics, so user
    // headers (applied last) override any defaults like Content-Type.
    // RequestBuilder::header() would append and produce duplicates.
    let mut req = client.request(method.into(), &normalized_path).build()?;

    if let Some(body_str) = body {
        *req.body_mut() = Some(body_str.into());
        req.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    }

    for (name, value) in custom_headers {
        req.headers_mut().insert(name, value);
    }

    let response = client.send_raw(req).await?;
    let status = response.status();
    let body_bytes = response.bytes().await?;

    // Print response body to stdout (raw bytes, no reformatting).
    // Matches gh api behavior: no trailing newline added — preserves
    // exact server bytes for file redirection.
    std::io::stdout().write_all(&body_bytes)?;

    if status.is_success() {
        Ok(())
    } else if status.as_u16() == 401 {
        Err(JrError::NotAuthenticated.into())
    } else {
        let message = extract_error_message(&body_bytes);
        Err(JrError::ApiError {
            status: status.as_u16(),
            message,
        }
        .into())
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

    #[test]
    fn test_normalize_path_rejects_uppercase_https() {
        // RFC 3986: URL schemes are case-insensitive.
        let err = normalize_path("HTTPS://site.atlassian.net/rest/api/3/myself").unwrap_err();
        assert!(err.to_string().contains("do not include the instance URL"));
    }

    #[test]
    fn test_normalize_path_rejects_mixed_case_http() {
        let err = normalize_path("Http://site.atlassian.net/foo").unwrap_err();
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

    #[test]
    fn test_parse_header_rejects_crlf_injection() {
        // HTTP header injection via CRLF is a well-known attack vector.
        // HeaderValue::from_str rejects control characters (visible ASCII only).
        let err = parse_header("X-Foo: bar\r\nInjected: evil").unwrap_err();
        assert!(err.to_string().contains("Invalid header value"));
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
