//! API-token `cloud_id` acquisition via the unauthenticated, per-site
//! `/_edge/tenant_info` endpoint (S-cycle4-cloud-id-correctness, ADR-0022,
//! BC-1.2.052/053/054, A-PA-LOW-001).
//!
//! Mirrors `oauth_login`'s existing direct-`reqwest` calls to
//! `accessible-resources` in `src/api/auth.rs` — a `JiraClient` cannot yet be
//! constructed at login time (no `cloud_id`/auth header exists yet for the
//! profile being created), so this is a plain `reqwest` call, never routed
//! through `JiraClient` (BC-1.2.052 Invariant 1).

/// Response shape for `GET {site}/_edge/tenant_info`. Only the `cloudId`
/// field is parsed; any other field present is ignored (serde default —
/// NOT deny-unknown-fields, BC-1.2.052 Postcondition 2).
#[derive(serde::Deserialize)]
struct TenantInfo {
    #[serde(rename = "cloudId")]
    cloud_id: String,
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
    if !site_url.trim().to_ascii_lowercase().starts_with("https://") {
        anyhow::bail!("tenant_info lookup skipped: site URL does not use https://");
    }

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
    #[cfg(debug_assertions)]
    let base = std::env::var("JR_TENANT_INFO_URL")
        .unwrap_or_else(|_| site_url.trim_end_matches('/').to_string());
    #[cfg(not(debug_assertions))]
    let base = site_url.trim_end_matches('/').to_string();

    let url = format!("{}/_edge/tenant_info", base.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("tenant_info lookup failed: HTTP {}", response.status());
    }
    let info: TenantInfo = response.json().await?;
    Ok(info.cloud_id)
}
