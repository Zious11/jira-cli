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
    let _ = site_url;
    todo!(
        "S-cycle4-cloud-id-correctness: implement https-only precondition + bare GET + bounded timeout + no-redirect + cloudId-only parse per ADR-0022 §1"
    )
}
