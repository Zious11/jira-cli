//! `jr api` — raw API passthrough command.
//!
//! Provides an escape hatch for calling the Jira REST API directly with
//! stored credentials, modeled on `gh api`. Supports method override,
//! request body (inline / file / stdin), custom headers, and query
//! parameters (`-q`/`--query-param`).

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
    if trimmed.is_empty() {
        return Err(JrError::UserError("API path cannot be empty".into()).into());
    }
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

/// Merge percent-encoded `-q`/`--query-param NAME=VALUE` pairs onto an
/// already-`normalize_path`-normalized API path.
///
/// BC-X.16.001 Behavior 1-5 / Postconditions 1-5. Pure, side-effect-free
/// (Invariant 1): no I/O, no `JiraClient`.
///
/// Detection/merge considers only the part of `path` BEFORE its first `#`
/// (Behavior 1): no `?` in that pre-fragment part means a fresh leading `?`
/// introduces the assembled query; otherwise the query component is
/// everything after the FIRST `?`, and the new pairs are appended directly
/// (no separator) when that component is empty or already `&`-terminated,
/// or `&`-joined otherwise. The pre-existing query text and any `#fragment`
/// are passed through byte-for-byte. NAME and VALUE are each
/// `urlencoding::encode`d exactly once (Behavior 3) — never
/// `url::form_urlencoded::byte_serialize`, whose space -> `+` mapping is
/// the wrong semantics here (Invariant 3) — and joined by a literal `=`.
/// An empty pair list is the identity on `path` (Behavior 5 / Postcondition
/// 1).
pub(crate) fn append_query_params(path: &str, pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return path.to_string();
    }

    let (pre, frag) = match path.find('#') {
        Some(idx) => (&path[..idx], &path[idx..]),
        None => (path, ""),
    };

    let sep = match pre.find('?') {
        None => "?",
        Some(qpos) => {
            let query_component = &pre[qpos + 1..];
            if query_component.is_empty() || query_component.ends_with('&') {
                ""
            } else {
                "&"
            }
        }
    };

    let enc_pairs = pairs
        .iter()
        .map(|(name, value)| {
            format!(
                "{}={}",
                urlencoding::encode(name),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    format!("{pre}{sep}{enc_pairs}{frag}")
}

/// Parse a single `-q`/`--query-param` raw value into a `(NAME, VALUE)` pair,
/// splitting on the FIRST `=` only.
///
/// BC-X.16.002. Pure, side-effect-free: no I/O, no `JiraClient`. VALUE may
/// itself contain further `=` characters, preserved verbatim in the
/// split-off remainder (EC-X.16.001-2). Neither NAME nor VALUE is trimmed
/// of whitespace (matches BC-X.16.001 Behavior 3's no-trim design default —
/// this intentionally differs from `parse_header`'s trimming behavior).
/// Two client-side, pre-HTTP failures, both `JrError::UserError`/exit 64:
/// no `=` at all (M1) or an empty NAME (M2, literally nothing before the
/// first `=`). An empty VALUE (`k=`) is NOT an error (EC-X.16.001-1).
pub(crate) fn parse_query_param(raw: &str) -> Result<(String, String)> {
    match raw.split_once('=') {
        None => Err(JrError::UserError(format!(
            "--query-param must be in NAME=VALUE format (got: {raw})"
        ))
        .into()),
        Some((name, rest)) => {
            if name.is_empty() {
                Err(JrError::UserError(format!(
                    "--query-param NAME cannot be empty (got: {raw}) \u{2014} use NAME=VALUE, e.g. -q maxResults=50"
                ))
                .into())
            } else {
                Ok((name.to_string(), rest.to_string()))
            }
        }
    }
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
    query_param: Vec<String>,
    client: &JiraClient,
) -> Result<()> {
    let normalized_path = normalize_path(&path)?;

    // Every `-q`/`--query-param` value is parsed and merged here, right
    // after `normalize_path` and before `resolve_body`/`-H` parsing
    // (BC-X.16.002): `normalize_path`'s own path errors therefore still
    // surface before any `-q` validation, and `-q` validation in turn
    // completes before `resolve_body`'s blocking `-d @-` stdin read.
    //
    // This call is unconditional, including on zero `-q` flags:
    // `parse_query_param` over an empty `Vec` collects to an empty `Vec`
    // with no HTTP calls, and `append_query_params(p, &[]) == p` is an
    // identity (BC-X.16.001 Postcondition 1 / Behavior 5), so the zero-flag
    // case is behavior-preserving.
    let pairs: Vec<(String, String)> = query_param
        .iter()
        .map(|raw| parse_query_param(raw))
        .collect::<Result<Vec<_>>>()?;
    let normalized_path = append_query_params(&normalized_path, &pairs);

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
        Err(JrError::NotAuthenticated {
            hint: "Run \"jr auth login\" to connect.".to_string(),
        }
        .into())
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
    use proptest::prelude::*;
    use std::io::Cursor;

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
    fn test_normalize_path_rejects_empty() {
        let err = normalize_path("").unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_normalize_path_rejects_whitespace_only() {
        let err = normalize_path("   ").unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
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

    // ── BC-X.16.001/BC-X.16.002: `--query-param`/`-q` (issue #583) ─────────
    //
    // Direct-call cells for `append_query_params` (AC-001..AC-004) and
    // `parse_query_param` (AC-005). The cells below invoke the pure
    // functions directly, asserting the values they produce against the
    // behavioral contract. The subprocess/wiremock cells for the wiring
    // layer (argv, `--help`, method-orthogonality, JSON envelope, ordering)
    // live in `tests/api_query_param.rs`.

    /// Pinned M1 message, byte-for-byte from BC-X.16.002's "Pinned error
    /// messages" block. Shared helper so every direct-call/subprocess cell
    /// citing M1 renders it identically rather than retyping the literal.
    fn m1_message(raw: &str) -> String {
        format!("--query-param must be in NAME=VALUE format (got: {raw})")
    }

    /// Pinned M2 message, byte-for-byte from BC-X.16.002's "Pinned error
    /// messages" block (contains an em dash, U+2014, written as `\u{2014}`
    /// to keep the literal unambiguous in source).
    fn m2_message(raw: &str) -> String {
        format!(
            "--query-param NAME cannot be empty (got: {raw}) \u{2014} use NAME=VALUE, e.g. -q maxResults=50"
        )
    }

    // ── AC-001: separator oracle + pinned examples ──────────────────────

    /// A small NAME alphabet shared by the "existing query" and "new pairs"
    /// generators below, so the proptest can and does exercise
    /// EC-X.16.001-12 (a `-q` NAME colliding with an existing query NAME).
    fn arb_name() -> impl Strategy<Value = String> {
        proptest::sample::select(vec!["a", "b", "fields", "x"]).prop_map(|s| s.to_string())
    }

    /// Reference implementation of VP-API-QP-001's separator case-split —
    /// used ONLY by this module's proptest oracles, never by production
    /// code. Mirrors BC-X.16.001 Behavior 1 / Postcondition 2 verbatim: the
    /// `?`-presence check runs FIRST; only once a `?` is found does the
    /// empty-or-`&`-terminated query-component check apply.
    fn expected_separator(pre: &str) -> &'static str {
        match pre.find('?') {
            None => "?",
            Some(qpos) => {
                let q = &pre[qpos + 1..];
                if q.is_empty() || q.ends_with('&') {
                    ""
                } else {
                    "&"
                }
            }
        }
    }

    /// Generates `(pre, frag, pairs)` triples for the separator oracle:
    /// `pre` covers no-query (optionally `&`-terminated, EC-14), an empty
    /// query component (EC-8 form 1), an `&`-terminated query component
    /// (EC-8 form 2), and a non-empty query component optionally ending in
    /// a literal `?` (EC-9) whose NAME may collide with a generated new
    /// pair's NAME (EC-12); `frag` covers no fragment and a fragment
    /// containing `?`, `&` and further `#` characters (VP-API-QP-001's
    /// generator requirement that a fragment can itself contain any of
    /// `?`/`&`/`#`).
    fn arb_pre_frag_pairs() -> impl Strategy<Value = (String, String, Vec<(String, String)>)> {
        let base = "/[a-zA-Z0-9_./-]{1,8}";

        let pre_strategy = prop_oneof![
            (base, any::<bool>()).prop_map(|(b, amp)| if amp { format!("{b}&") } else { b }),
            base.prop_map(|b| format!("{b}?")),
            (base, "[a-zA-Z0-9=]{0,8}").prop_map(|(b, q)| format!("{b}?{q}&")),
            (base, arb_name(), "[a-zA-Z0-9]{0,6}", any::<bool>()).prop_map(
                |(b, name, val, end_q)| {
                    let tail = if end_q { "?" } else { "" };
                    format!("{b}?{name}={val}{tail}")
                }
            ),
        ];

        let frag_strategy = prop_oneof![
            Just(String::new()),
            "[a-zA-Z0-9?&#]{0,8}".prop_map(|f| format!("#{f}")),
        ];

        let pairs_strategy = proptest::collection::vec((arb_name(), "[a-zA-Z0-9]{0,6}"), 1..4);

        (pre_strategy, frag_strategy, pairs_strategy)
    }

    proptest! {
        /// VP-API-QP-001 separator oracle (BC-X.16.001 Behavior 1,
        /// Postcondition 2, EC-X.16.001-4/5/8/9/12/14): for `path = pre +
        /// frag` and a non-empty pair list, `append_query_params(path,
        /// pairs) == pre + s + enc_pairs + frag`, where `enc_pairs` joins
        /// `urlencoding::encode(NAME)=urlencoding::encode(VALUE)` with `&`,
        /// and `s` is `expected_separator(pre)` above.
        #[test]
        fn test_bc_x_16_001_append_query_params_separator_oracle(
            (pre, frag, pairs) in arb_pre_frag_pairs()
        ) {
            let path = format!("{pre}{frag}");
            let sep = expected_separator(&pre);
            let enc_pairs = pairs
                .iter()
                .map(|(n, v)| format!("{}={}", urlencoding::encode(n), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            let expected = format!("{pre}{sep}{enc_pairs}{frag}");

            prop_assert_eq!(append_query_params(&path, &pairs), expected);
        }
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_empty_value_wire_assembly() {
        // P22-002: new direct-call pinned example, the WIRE half of
        // EC-X.16.001-1 (`k=`, empty VALUE, allowed) — AC-005 owns the
        // PARSE half via `parse_query_param`.
        let out = append_query_params("/x", &[("k".to_string(), String::new())]);
        assert_eq!(out, "/x?k=");
    }

    #[test]
    fn test_bc_x_16_001_ec4_appends_with_ampersand_when_query_already_present() {
        let out = append_query_params(
            "/rest/api/3/search?existing=1",
            &[("new".to_string(), "2".to_string())],
        );
        assert_eq!(out, "/rest/api/3/search?existing=1&new=2");
    }

    #[test]
    fn test_bc_x_16_001_ec5_query_inserted_before_fragment() {
        let out = append_query_params("/s#frag", &[("k".to_string(), "v".to_string())]);
        assert_eq!(out, "/s?k=v#frag");
    }

    #[test]
    fn test_bc_x_16_001_pinned_fragment_containing_hash_and_question_mark() {
        // F-005(a): kills a `find` -> `rfind` regression on the `#` split.
        // The FIRST `#` delimits pre/frag, even when the fragment itself
        // contains further `?`/`#` characters. An `rfind`-based split would
        // instead treat `#a?b` as part of `pre`, moving the assembled query
        // past the LAST `#` and producing `/x#a?b&k=v#c`.
        let out = append_query_params("/x#a?b#c", &[("k".to_string(), "v".to_string())]);
        assert_eq!(out, "/x?k=v#a?b#c");
    }

    #[test]
    fn test_bc_x_16_001_empty_query_component_directly_followed_by_fragment() {
        // `/s?#f` + k=v -> `/s?k=v#f`: the query component (text after the
        // first `?`) is empty, so no separator is added, and the pair lands
        // before `#`.
        let out = append_query_params("/s?#f", &[("k".to_string(), "v".to_string())]);
        assert_eq!(out, "/s?k=v#f");
    }

    #[test]
    fn test_bc_x_16_001_ec8_no_separator_when_query_component_empty_or_amp_terminated() {
        // Form 1: bare `?` with no query pairs yet.
        let out1 = append_query_params(
            "/rest/api/3/search?",
            &[("new".to_string(), "2".to_string())],
        );
        assert_eq!(out1, "/rest/api/3/search?new=2");

        // Form 2: existing query already ends in `&`.
        let out2 = append_query_params(
            "/rest/api/3/search?a=1&",
            &[("new".to_string(), "2".to_string())],
        );
        assert_eq!(out2, "/rest/api/3/search?a=1&new=2");
    }

    #[test]
    fn test_bc_x_16_001_ec9_ampersand_joined_when_query_content_ends_in_literal_question_mark() {
        let out = append_query_params(
            "/rest/api/3/search?jql=why?",
            &[("k".to_string(), "v".to_string())],
        );
        assert_eq!(out, "/rest/api/3/search?jql=why?&k=v");
    }

    #[test]
    fn test_bc_x_16_001_ec12_colliding_name_neither_deduped_nor_overridden() {
        let out = append_query_params(
            "/s?fields=summary",
            &[("fields".to_string(), "status".to_string())],
        );
        assert_eq!(out, "/s?fields=summary&fields=status");
    }

    #[test]
    fn test_bc_x_16_001_ec14_fresh_question_mark_introduced_despite_trailing_ampersand() {
        let out = append_query_params("/x&", &[("k".to_string(), "v".to_string())]);
        assert_eq!(out, "/x&?k=v");
    }

    // ── AC-002: repeated-names oracle + pinned decode example ───────────

    /// A NAME=VALUE pair list, as produced by both the "existing query" and
    /// "new `-q` flags" generators below — factored into a named alias so
    /// `arb_existing_and_new_pairs`'s return type stays readable (clippy
    /// `type_complexity`).
    type NameValuePairs = Vec<(String, String)>;

    /// Generates `(existing_pairs, new_pairs)` where `existing_pairs` seeds
    /// an ALREADY-ENCODED, `+`-free, `#`-free, no-empty-segment query
    /// string (VP-API-QP-002 generator constraint), and `new_pairs` is the
    /// simulated `-q` flag list — both drawn from the same small NAME
    /// alphabet so repeats/collisions are exercised.
    fn arb_existing_and_new_pairs() -> impl Strategy<Value = (NameValuePairs, NameValuePairs)> {
        let existing = proptest::collection::vec((arb_name(), "[a-zA-Z0-9]{0,6}"), 0..3);
        let new_pairs = proptest::collection::vec((arb_name(), "[a-zA-Z0-9]{0,6}"), 1..4);
        (existing, new_pairs)
    }

    proptest! {
        /// VP-API-QP-002 (BC-X.16.001 Behavior 2, Postcondition 4): decoding
        /// `append_query_params`'s output query with
        /// `url::form_urlencoded::parse` reproduces `existing ++ new_pairs`
        /// (same length, flag order, no dedup, pre-existing pairs first).
        /// The generator round-trip assertion
        /// (`generated_existing_pairs == existing_pairs`) is asserted FIRST
        /// to confirm the hand-built existing query decodes back to the
        /// generated `existing` pairs (an empty `existing` is a valid
        /// generated case).
        #[test]
        fn test_bc_x_16_001_append_query_params_repeated_names_oracle(
            (existing_pairs, new_pairs) in arb_existing_and_new_pairs()
        ) {
            let in_query = existing_pairs
                .iter()
                .map(|(n, v)| format!("{}={}", urlencoding::encode(n), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            let path = if in_query.is_empty() {
                "/s".to_string()
            } else {
                format!("/s?{in_query}")
            };

            let generated_existing_pairs: Vec<(String, String)> =
                url::form_urlencoded::parse(in_query.as_bytes())
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect();
            prop_assert_eq!(&generated_existing_pairs, &existing_pairs);

            let out = append_query_params(&path, &new_pairs);
            let out_query = out.split_once('?').map(|(_, q)| q).unwrap_or("");
            let decoded: Vec<(String, String)> = url::form_urlencoded::parse(out_query.as_bytes())
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect();

            let mut expected = existing_pairs.clone();
            expected.extend(new_pairs.clone());
            prop_assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_pinned_repeated_names_decode_example() {
        let out = append_query_params(
            "/s?fields=summary",
            &[("fields".to_string(), "status".to_string())],
        );
        let out_query = out.split_once('?').map(|(_, q)| q).unwrap();
        let decoded: Vec<(String, String)> = url::form_urlencoded::parse(out_query.as_bytes())
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(
            decoded,
            vec![
                ("fields".to_string(), "summary".to_string()),
                ("fields".to_string(), "status".to_string()),
            ]
        );
    }

    // ── AC-003: encoding-exactly-once biased proptest + pinned ──────────

    /// Every byte of `s` is either an RFC 3986 UNRESERVED byte
    /// (`A-Za-z0-9-._~`) or a `%HH` triplet with UPPERCASE hex digits.
    fn is_valid_percent_encoded(s: &str) -> bool {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                i += 1;
            } else if b == b'%' {
                if i + 3 > bytes.len() {
                    return false;
                }
                let (h1, h2) = (bytes[i + 1], bytes[i + 2]);
                let is_upper_hex = |b: u8| b.is_ascii_digit() || (b'A'..=b'F').contains(&b);
                if !is_upper_hex(h1) || !is_upper_hex(h2) {
                    return false;
                }
                i += 3;
            } else {
                return false;
            }
        }
        true
    }

    /// A biased strategy over-sampling `%`, `&`, `=`, `#`, `+`, space, `?`,
    /// CR/LF, and pre-encoded look-alikes (`%25`, `%2B`, `%20`), mixed with
    /// fully arbitrary Unicode scalar values (VP-API-QP-003's "arbitrary
    /// UTF-8, biased" strategy).
    fn arb_biased_string() -> impl Strategy<Value = String> {
        let special_chars = ['%', '&', '=', '#', '+', ' ', '?', '\r', '\n'];
        let special_char_strategy =
            proptest::collection::vec(proptest::sample::select(special_chars.to_vec()), 0..6)
                .prop_map(|cs| cs.into_iter().collect::<String>());

        let lookalike_strategy =
            proptest::sample::select(vec!["%25", "%2B", "%20"]).prop_map(|s| s.to_string());

        let arbitrary_strategy = proptest::collection::vec(any::<char>(), 0..8)
            .prop_map(|cs| cs.into_iter().collect::<String>());

        prop_oneof![
            3 => special_char_strategy,
            2 => lookalike_strategy,
            3 => arbitrary_strategy,
        ]
    }

    proptest! {
        /// VP-API-QP-003 (BC-X.16.001 Behavior 3, Postcondition 3,
        /// EC-X.16.001-3/10/11): (a) round-trip — `decode(encode(v)) == v`
        /// for NAME and VALUE, where `encode(v)` is the segment EXTRACTED
        /// from `append_query_params`'s own output (never a direct
        /// `urlencoding::encode` call — a direct call would be tautological
        /// and vacuous); (b) alphabet — every byte is RFC 3986 unreserved
        /// or an uppercase `%HH` triplet, space -> `%20`,
        /// never `+`; (c) encoder identity — the appended pair equals
        /// `format!("{}={}", urlencoding::encode(name),
        /// urlencoding::encode(value))` byte-for-byte; (d) no trimming is
        /// implied by (a)'s exact round-trip on whitespace-bearing inputs.
        #[test]
        fn test_bc_x_16_001_append_query_params_encodes_exactly_once(
            (name, value) in (arb_biased_string(), arb_biased_string())
        ) {
            let pairs = vec![(name.clone(), value.clone())];
            let out = append_query_params("/x", &pairs);

            let query = out.strip_prefix("/x?")
                .expect("a fresh leading '?' must introduce the assembled query for a bare path");
            let (enc_name, enc_value) = query
                .split_once('=')
                .expect("the NAME/VALUE joiner '=' must be literal and unencoded");

            // (c) encoder identity.
            let expected_enc_name = urlencoding::encode(&name);
            let expected_enc_value = urlencoding::encode(&value);
            prop_assert_eq!(enc_name, expected_enc_name.as_ref());
            prop_assert_eq!(enc_value, expected_enc_value.as_ref());

            // (a) round-trip, extracted from the function's own output.
            prop_assert_eq!(urlencoding::decode(enc_name).unwrap().into_owned(), name);
            prop_assert_eq!(urlencoding::decode(enc_value).unwrap().into_owned(), value);

            // (b) alphabet: unreserved bytes or uppercase %HH; never '+'.
            prop_assert!(is_valid_percent_encoded(enc_name));
            prop_assert!(is_valid_percent_encoded(enc_value));
            prop_assert!(!enc_name.contains('+'));
            prop_assert!(!enc_value.contains('+'));
        }
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_encodes_asterisk_to_percent_2a() {
        let out = append_query_params("/x", &[("k".to_string(), "*".to_string())]);
        assert_eq!(out, "/x?k=%2A");
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_encodes_space_to_percent_20() {
        let out = append_query_params("/x", &[("k".to_string(), " ".to_string())]);
        assert_eq!(out, "/x?k=%20");
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_does_not_trim_whitespace_only_name() {
        let out = append_query_params("/x", &[(" ".to_string(), "v".to_string())]);
        assert_eq!(out, "/x?%20=v");
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_does_not_trim_value_whitespace() {
        let out = append_query_params("/x", &[("k".to_string(), " v ".to_string())]);
        assert_eq!(out, "/x?k=%20v%20");
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_encodes_percent_to_percent_25() {
        let out = append_query_params("/x", &[("k".to_string(), "%".to_string())]);
        assert_eq!(out, "/x?k=%25");
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_encodes_plus_to_percent_2b() {
        let out = append_query_params("/x", &[("k".to_string(), "+".to_string())]);
        assert_eq!(out, "/x?k=%2B");
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_encodes_unicode_e_acute_utf8_bytes() {
        let out = append_query_params("/x", &[("summary".to_string(), "café".to_string())]);
        assert_eq!(out, "/x?summary=caf%C3%A9");
    }

    #[test]
    fn test_bc_x_16_001_append_query_params_encodes_literal_percent25_to_percent2525() {
        let out = append_query_params("/x", &[("k".to_string(), "%25".to_string())]);
        assert_eq!(out, "/x?k=%2525");
    }

    // ── AC-004: zero-flag identity proptest ──────────────────────────────

    /// Generates arbitrary paths for the zero-flag identity property,
    /// including paths with an existing query, a trailing `?` or `&`, and a
    /// `#fragment` (VP-API-QP-004(1)'s pinned generator scope).
    fn arb_zero_flag_path() -> impl Strategy<Value = String> {
        let base = "/[a-zA-Z0-9_./-]{1,10}";
        prop_oneof![
            base.prop_map(|b| b.to_string()),
            base.prop_map(|b| format!("{b}?")),
            base.prop_map(|b| format!("{b}&")),
            (base, "[a-zA-Z0-9=&]{0,10}").prop_map(|(b, q)| format!("{b}?{q}")),
            (base, "[a-zA-Z0-9=&]{0,10}").prop_map(|(b, q)| format!("{b}?{q}#frag")),
            base.prop_map(|b| format!("{b}#frag?x&y")),
        ]
    }

    proptest! {
        /// VP-API-QP-004(1): `append_query_params(p, &[])` is the identity
        /// on `p` for arbitrary `p` (BC-X.16.001 Postcondition 1 /
        /// Behavior 5). This is a DIRECT call to `append_query_params`,
        /// exercising the pure function's zero-pairs short-circuit in
        /// isolation — distinct from the zero-flag wiremock examples in
        /// `tests/api_query_param.rs`, which exercise the same identity
        /// through the full `handle_api` argv-to-request path.
        #[test]
        fn test_bc_x_16_001_append_query_params_zero_pairs_is_identity(
            path in arb_zero_flag_path()
        ) {
            prop_assert_eq!(append_query_params(&path, &[]), path.clone());
        }
    }

    // ── AC-005: parse_query_param partition oracle + pinned ─────────────

    #[derive(Debug)]
    enum ParseQueryParamCase {
        /// No `=` at all (including the empty string) — must report M1.
        NoEquals(String),
        /// `raw = "=" + rest` (empty NAME) — must report M2.
        EmptyName(String),
        /// `raw = "{name}={rest}"`, NAME non-empty and `=`-free (may be
        /// whitespace-only) — must parse `Ok((name, rest))`.
        WellFormed(String, String),
    }

    /// M1 raw values: NO `=` anywhere, but MAY carry leading and/or
    /// trailing whitespace (a fault-kill pin against `{raw}` being
    /// replaced by a trimmed or re-split value).
    fn arb_m1_raw() -> impl Strategy<Value = String> {
        ("[ \t]{0,3}", "[a-zA-Z0-9 \t]{0,8}", "[ \t]{0,3}")
            .prop_map(|(lead, body, trail)| format!("{lead}{body}{trail}"))
    }

    /// M2 raw values: `"=" + rest`, where `rest` may itself contain `=`
    /// and/or trailing whitespace. No LEADING whitespace before the `=` is
    /// generated here — that would make the NAME whitespace-only rather
    /// than empty, moving the case into `WellFormed` (EC-X.16.001-10), not
    /// M2.
    fn arb_m2_raw() -> impl Strategy<Value = String> {
        ("[a-zA-Z0-9=]{0,10}", "[ \t]{0,3}").prop_map(|(rest, trail)| format!("={rest}{trail}"))
    }

    /// Well-formed `(NAME, rest)` pairs: NAME is non-empty and `=`-free
    /// (may be whitespace-only, EC-X.16.001-10 — a dedicated whitespace-only
    /// branch below generates this case deliberately rather than leaving
    /// it to chance); `rest` may
    /// itself contain `=` and whitespace (EC-X.16.001-2, "split on the
    /// FIRST `=` only", plus F-001(a)'s no-trim guarantee) and may be empty
    /// (EC-X.16.001-1, empty VALUE is allowed).
    fn arb_ok_name_rest() -> impl Strategy<Value = (String, String)> {
        let name_strategy = prop_oneof![
            1 => "[ \t]{1,8}",
            4 => "[a-zA-Z0-9 \t]{1,8}",
        ];
        let rest_strategy = "[a-zA-Z0-9=\t ]{0,10}";
        (name_strategy, rest_strategy)
    }

    fn arb_parse_query_param_case() -> impl Strategy<Value = ParseQueryParamCase> {
        prop_oneof![
            arb_m1_raw().prop_map(ParseQueryParamCase::NoEquals),
            arb_m2_raw().prop_map(ParseQueryParamCase::EmptyName),
            arb_ok_name_rest().prop_map(|(n, r)| ParseQueryParamCase::WellFormed(n, r)),
        ]
    }

    proptest! {
        /// VP-API-QP-005(1) partition proptest on `parse_query_param`:
        /// any `raw` with no `=` (including `""`) -> `Err` == M1 rendered
        /// with `raw` byte-for-byte; `"=" + any` -> `Err` == M2 rendered
        /// with `raw`; a non-empty `=`-free NAME (including whitespace-
        /// only) + `"=" + any` -> `Ok((NAME, rest))`, `rest` keeping every
        /// later `=` and possibly empty. Each `Err` case asserts its own
        /// distinguishing substring is present unconditionally, then (after
        /// `prop_assume!`-filtering `raw` values that themselves contain
        /// D1 or D2) asserts the OTHER substring is absent.
        #[test]
        fn test_bc_x_16_002_parse_query_param_partition_oracle(
            case in arb_parse_query_param_case()
        ) {
            match case {
                ParseQueryParamCase::NoEquals(raw) => {
                    let err = parse_query_param(&raw).unwrap_err();
                    let msg = err.to_string();
                    prop_assert_eq!(&msg, &m1_message(&raw));
                    prop_assert!(msg.contains("must be in NAME=VALUE format"));
                    prop_assume!(
                        !raw.contains("must be in NAME=VALUE format")
                            && !raw.contains("NAME cannot be empty")
                    );
                    prop_assert!(!msg.contains("NAME cannot be empty"));
                }
                ParseQueryParamCase::EmptyName(raw) => {
                    let err = parse_query_param(&raw).unwrap_err();
                    let msg = err.to_string();
                    prop_assert_eq!(&msg, &m2_message(&raw));
                    prop_assert!(msg.contains("NAME cannot be empty"));
                    prop_assume!(
                        !raw.contains("must be in NAME=VALUE format")
                            && !raw.contains("NAME cannot be empty")
                    );
                    prop_assert!(!msg.contains("must be in NAME=VALUE format"));
                }
                ParseQueryParamCase::WellFormed(name, rest) => {
                    let raw = format!("{name}={rest}");
                    let (parsed_name, parsed_rest) = parse_query_param(&raw).unwrap();
                    prop_assert_eq!(parsed_name, name);
                    prop_assert_eq!(parsed_rest, rest);
                }
            }
        }
    }

    #[test]
    fn test_bc_x_16_002_parse_query_param_empty_raw_reports_m1() {
        let err = parse_query_param("").unwrap_err();
        assert_eq!(err.to_string(), m1_message(""));
    }

    #[test]
    fn test_bc_x_16_002_parse_query_param_does_not_trim_whitespace_only_name() {
        // F-001(a): a `name.trim().is_empty()` regression would misclassify
        // a whitespace-only NAME as M2 (empty NAME) instead of parsing it.
        let result = parse_query_param(" =v").unwrap();
        assert_eq!(result, (" ".to_string(), "v".to_string()));
    }

    #[test]
    fn test_bc_x_16_002_parse_query_param_does_not_trim_value_whitespace() {
        // F-001(a): a `rest.trim()` regression on VALUE would strip the
        // leading/trailing spaces below instead of preserving them verbatim.
        let result = parse_query_param("k= v ").unwrap();
        assert_eq!(result, ("k".to_string(), " v ".to_string()));
    }

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
        // Propagated std::io::Error — wording differs by OS:
        //   Unix:    "No such file or directory" (ENOENT)
        //   Windows: "The system cannot find the file specified." (ERROR_FILE_NOT_FOUND)
        // Match on ErrorKind when possible; fall back to OS-gated substrings.
        let is_not_found = err
            .downcast_ref::<std::io::Error>()
            .map(|e| e.kind() == std::io::ErrorKind::NotFound)
            .unwrap_or_else(|| {
                let s = err.to_string().to_lowercase();
                s.contains("no such file") || s.contains("cannot find")
            });
        assert!(is_not_found, "expected NotFound error, got: {err}");
    }

    #[test]
    fn test_resolve_body_at_dash_reads_stdin() {
        let stdin_content = br#"{"from":"stdin"}"#;
        let stdin = Cursor::new(&stdin_content[..]);
        let result = resolve_body(Some("@-"), stdin).unwrap();
        assert_eq!(result, Some(r#"{"from":"stdin"}"#.to_string()));
    }
}
