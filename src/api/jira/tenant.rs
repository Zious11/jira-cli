//! API-token `cloud_id` acquisition via the unauthenticated, per-site
//! `/_edge/tenant_info` endpoint (S-cycle4-cloud-id-correctness, ADR-0022,
//! BC-1.2.052/053/054, A-PA-LOW-001).
//!
//! Mirrors `oauth_login`'s existing direct-`reqwest` calls to
//! `accessible-resources` in `src/api/auth.rs` — a `JiraClient` cannot yet be
//! constructed at login time (no `cloud_id`/auth header exists yet for the
//! profile being created), so this is a plain `reqwest` call, never routed
//! through `JiraClient` (BC-1.2.052 Invariant 1).

use futures::StreamExt;

/// Response shape for `GET {site}/_edge/tenant_info`. Only the `cloudId`
/// field is parsed; any other field present is ignored (serde default —
/// NOT deny-unknown-fields, BC-1.2.052 Postcondition 2).
#[derive(serde::Deserialize)]
struct TenantInfo {
    #[serde(rename = "cloudId")]
    cloud_id: String,
}

/// Upper bound on the `tenant_info` response body, in bytes
/// (FIX-F5-CYCLE4-2 hardening #1c). The real payload is a handful of short
/// string fields (`cloudId`, `baseUrl`, `activation`) — 64 KiB is
/// generously above any legitimate size while bounding memory use against
/// an oversized or hostile response from the user's own site.
const MAX_TENANT_INFO_RESPONSE_BYTES: usize = 64 * 1024;

/// Returns `site_url` trimmed of leading/trailing whitespace if the
/// trimmed value satisfies the `https://`-only precondition, else `None`.
///
/// Host-pure and unit-tested directly (see `tests` below) so that the
/// precondition check and the string actually used to build the request
/// URL can never drift apart (FIX-F5-CYCLE4-2 hardening #1b) — before this
/// helper existed, the precondition checked `site_url.trim()` but the
/// non-override request-base fallback used `site_url` as-is (only trimming
/// a trailing `/`), so a `site_url` with leading/trailing whitespace could
/// pass the precondition and still build a malformed request URL.
fn validate_and_trim_site_url(site_url: &str) -> Option<&str> {
    let trimmed = site_url.trim();
    if trimmed.to_ascii_lowercase().starts_with("https://") {
        Some(trimmed)
    } else {
        None
    }
}

/// Returns `true` if `cloud_id` is a plausible Jira Cloud `cloudId` shape:
/// non-empty and containing only ASCII alphanumerics and hyphens (no
/// whitespace, no control/punctuation characters). Real Atlassian
/// `cloudId`s are UUIDs — a strict subset of what this accepts — but this
/// check is deliberately loose rather than a precise UUID-format
/// validator: its purpose is to reject an empty or obviously-garbage
/// response (FIX-F5-CYCLE4-2 hardening #1a), not to enforce UUID syntax a
/// future, differently-shaped legitimate value might not match.
fn is_plausible_cloud_id(cloud_id: &str) -> bool {
    !cloud_id.is_empty()
        && cloud_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Fetch the `cloudId` for a Jira Cloud site via the unauthenticated,
/// per-site `GET {site_url}/_edge/tenant_info` endpoint.
///
/// Used by the API-token login path (`login_token`), which has no
/// `accessible-resources`-equivalent discovery step (that endpoint is
/// OAuth-Bearer-only, BC-1.2.052 Description).
///
/// Contract (BC-1.2.052 Postcondition 2, Invariants 1/3; ADR-0022 §1):
/// - No `Authorization` header is attached.
/// - No query string is appended (a trailing `?_r=...` cache-buster has
///   been observed to 403).
/// - A 10-second bounded timeout.
/// - `redirect::Policy::none()` — a 3xx response is surfaced as an ordinary
///   non-2xx status, never followed cross-host (EC-1.2.052-2, Pass-1
///   adversarial review Finding #12).
/// - `site_url` MUST start with `https://` (case-insensitive) or the fetch
///   is skipped entirely, making zero network requests (Pass-4 adversarial
///   review Finding #4).
///
/// Callers are expected to treat any `Err` as a soft-fail (BC-1.2.052
/// Postcondition 3) — this function never panics and never blocks a login.
pub async fn fetch_cloud_id(site_url: &str) -> anyhow::Result<String> {
    let trimmed_site_url = validate_and_trim_site_url(site_url).ok_or_else(|| {
        anyhow::anyhow!("tenant_info lookup skipped: site URL does not use https://")
    })?;

    // Debug-only test seam (S-cycle4-cloud-id-correctness): when set,
    // redirects the ACTUAL GET request to `JR_TENANT_INFO_URL` while
    // `site_url` itself remains what the `https://`-prefix precondition
    // above validates. `wiremock` has no HTTPS/TLS support, so a genuine
    // 200-plus-`cloudId` response can only be exercised in tests by
    // pointing the real request elsewhere while a plausible `https://`
    // `site_url` still satisfies the precondition check honestly.
    //
    // Gated behind `#[cfg(debug_assertions)]` exactly like the sibling
    // `JR_BASE_URL` seam (`src/config.rs::Config::base_url`,
    // `src/api/client.rs::JiraClient::from_config`) — release binaries
    // never read this env var, so this cannot be used to redirect a real
    // user's tenant_info lookup to an attacker-controlled host. See
    // CLAUDE.md's "AI Agent Notes" `JR_TENANT_INFO_URL` entry and
    // `tests/jr_tenant_info_url_release_gate.rs`.
    //
    // FIX-F5-CYCLE4-2 hardening #1b: both arms now derive from
    // `trimmed_site_url` (the SAME trimmed string the precondition above
    // just validated) rather than re-deriving from the untrimmed
    // `site_url` — previously only a trailing `/` was stripped here, so
    // leading/trailing whitespace that passed the precondition (which
    // trims before checking) could still flow into a malformed request
    // URL.
    #[cfg(debug_assertions)]
    let base = std::env::var("JR_TENANT_INFO_URL").unwrap_or_else(|_| trimmed_site_url.to_string());
    #[cfg(not(debug_assertions))]
    let base = trimmed_site_url.to_string();

    let url = format!("{}/_edge/tenant_info", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("tenant_info lookup failed: HTTP {}", response.status());
    }

    // FIX-F5-CYCLE4-2 hardening #1c: bound the response body before
    // buffering it. Content-Length is checked first as a fast rejection
    // path; the streamed read below is authoritative regardless, since
    // Content-Length can be absent (chunked transfer) or misreported.
    if let Some(len) = response.content_length() {
        if len > MAX_TENANT_INFO_RESPONSE_BYTES as u64 {
            anyhow::bail!(
                "tenant_info response body too large ({len} bytes, max {MAX_TENANT_INFO_RESPONSE_BYTES})"
            );
        }
    }
    let mut body: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        body.extend_from_slice(&chunk);
        if body.len() > MAX_TENANT_INFO_RESPONSE_BYTES {
            anyhow::bail!(
                "tenant_info response body exceeded {MAX_TENANT_INFO_RESPONSE_BYTES} bytes"
            );
        }
    }

    let info: TenantInfo = serde_json::from_slice(&body)
        .map_err(|e| anyhow::anyhow!("tenant_info response body was not valid JSON: {e}"))?;

    // FIX-F5-CYCLE4-2 hardening #1a: an empty or implausibly-shaped
    // cloudId is treated as a fetch failure — it flows into the EXISTING
    // documented soft-fail path (BC-1.2.053 Postcondition 2/3), never
    // persisted.
    if !is_plausible_cloud_id(&info.cloud_id) {
        anyhow::bail!("tenant_info response cloudId field is empty or malformed");
    }

    Ok(info.cloud_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_and_trim_site_url_trims_leading_and_trailing_whitespace() {
        assert_eq!(
            validate_and_trim_site_url("  https://example.atlassian.net  "),
            Some("https://example.atlassian.net")
        );
    }

    #[test]
    fn test_validate_and_trim_site_url_rejects_non_https_scheme() {
        assert_eq!(
            validate_and_trim_site_url("http://example.atlassian.net"),
            None
        );
    }

    #[test]
    fn test_validate_and_trim_site_url_rejects_whitespace_only_input() {
        assert_eq!(validate_and_trim_site_url("   "), None);
    }

    #[test]
    fn test_validate_and_trim_site_url_accepts_case_insensitive_scheme() {
        assert_eq!(
            validate_and_trim_site_url("HTTPS://example.atlassian.net"),
            Some("HTTPS://example.atlassian.net")
        );
    }

    #[test]
    fn test_is_plausible_cloud_id_rejects_empty_string() {
        assert!(!is_plausible_cloud_id(""));
    }

    #[test]
    fn test_is_plausible_cloud_id_rejects_internal_whitespace() {
        assert!(!is_plausible_cloud_id("abc def"));
        assert!(!is_plausible_cloud_id("abc\tdef"));
        assert!(!is_plausible_cloud_id("abc\ndef"));
    }

    #[test]
    fn test_is_plausible_cloud_id_rejects_whitespace_only() {
        assert!(!is_plausible_cloud_id("   "));
    }

    #[test]
    fn test_is_plausible_cloud_id_rejects_disallowed_characters() {
        assert!(!is_plausible_cloud_id("abc/def"));
        assert!(!is_plausible_cloud_id("<script>alert(1)</script>"));
    }

    #[test]
    fn test_is_plausible_cloud_id_accepts_real_uuid_shape() {
        assert!(is_plausible_cloud_id(
            "f47ac10b-58cc-4372-a567-0e02b2c3d479"
        ));
    }

    /// Regression pin: `tests/cloud_id_tenant_info.rs`'s mock responses use
    /// these exact literal `cloudId` values — the validator must never
    /// reject them (both are non-empty, whitespace-free, and
    /// alphanumeric-and-hyphen-only, same shape class as a real UUID).
    #[test]
    fn test_is_plausible_cloud_id_accepts_existing_test_fixture_values() {
        assert!(is_plausible_cloud_id("the-real-cloud-id"));
        assert!(is_plausible_cloud_id("irrelevant-for-this-test"));
    }
}
