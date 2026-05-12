//! Atlassian Issue Bulk Operations API.
//!
//! Supports up to 1,000 issues per call. Operations are async on the Jira side:
//! - Submit bulk operation → receive taskId
//! - Poll GET /rest/api/3/bulk/queue/{taskId} until terminal status
//!
//! Terminal statuses (per Atlassian OpenAPI, Perplexity-verified 2026-05-12):
//!   COMPLETE | FAILED | CANCELLED | DEAD | PARTIAL_FAILURE | PROCESSED_WITH_ERRORS
//! Also accepts COMPLETED as an empirical-safety alias for COMPLETE.
//! Non-terminal (continue polling): ENQUEUED | RUNNING | CANCEL_REQUESTED.
//! Unknown statuses are warned-on-first-sighting and escalate to terminal-with-
//! error after the unknown-status grace period (default 30s via
//! `DEFAULT_UNKNOWN_STATUS_GRACE_SECS`, resolved through
//! `resolve_unknown_status_grace()` — debug builds also honor the
//! `JR_BULK_UNKNOWN_GRACE_SECS` env var so CLI integration tests can drive
//! the warn+escalate path in ~1s; release builds always use the default).
//! Closes audit-followup #336.
//!
//! Per CLAUDE.md: blanket-401 → auto-refresh is already wired into JiraClient::send.
//! 429 during polling: JiraClient::send retries up to MAX_RETRIES times using
//! Retry-After, subject to MAX_RETRY_AFTER_SECS=60 cap.

use std::time::{Duration, Instant};

use crate::api::client::JiraClient;
use crate::types::jira::bulk::{
    BulkEditRequest, BulkOperationProgress, BulkSubmitResponse, BulkTransitionRequest,
};

/// Maximum number of issue keys allowed in a single bulk operation call.
///
/// Sourced from the Atlassian per-call cap documented on both
/// POST /rest/api/3/bulk/issues/fields and POST /rest/api/3/bulk/issues/transition
/// (Issue Bulk Operations API). Reused by all bulk CLI handlers
/// (`issue edit` multi-key + `--jql`, `issue move` multi-key) so that a future
/// Atlassian cap change only requires editing this single constant.
pub const BULK_MAX_KEYS: usize = 1000;

/// Exponential backoff base interval for poll retries (used when task is not yet terminal).
const POLL_BASE_SECS: u64 = 1;
/// Maximum backoff interval (caps exponential growth).
const POLL_MAX_SECS: u64 = 10;

/// Grace period after which an UNRECOGNIZED bulk task status is treated as
/// terminal-with-error. Set to 30 seconds — Perplexity-validated 2026-05-12
/// as the industry-standard heuristic (~10% of the 5-minute overall timeout;
/// matches kubectl/Helm/`gh` polling-client conventions; below human
/// perception threshold for "hanging" while long enough to absorb transient
/// deploy/rollback artifacts).
///
/// Closes audit-followup #336.
const DEFAULT_UNKNOWN_STATUS_GRACE_SECS: u64 = 30;

/// Format a `Duration` for inclusion in an operator-facing error or warning
/// message. Targets the **millisecond-to-seconds range** which covers every
/// grace value the #336 code paths can produce:
///   - the production default is 30s (`DEFAULT_UNKNOWN_STATUS_GRACE_SECS`);
///   - the env-var override `JR_BULK_UNKNOWN_GRACE_SECS` parses as `u64`
///     whole seconds, so its minimum non-zero value is 1s and 0s renders
///     as `"0ms"` (the helper's smallest representable bucket);
///   - in-lib tests pass `Duration` directly to
///     `await_bulk_task_with_grace_for_test`, smallest realistic value
///     `Duration::from_millis(200)`.
///
/// Rendering rules (in this range):
///   - sub-second values render in milliseconds (e.g., `"200ms"`);
///   - one-second-and-above values render in whole seconds (e.g., `"30s"`).
///
/// Sub-millisecond durations (`Duration::from_nanos(500)` etc.) ARE truncated
/// by `as_millis()` to `"0ms"`. This is deliberate — no current configuration
/// knob can produce a sub-ms grace, and an operator-facing escalation message
/// like "polled unrecognized status for >= 500ns" would be absurd. If a
/// future knob requires sub-ms resolution, switch this to the `humantime`
/// crate (Perplexity-validated 2026-05-12 as the idiomatic Tokio/reqwest
/// dep for multi-unit rendering).
///
/// `Duration::as_secs()` alone is unsuitable because it truncates ALL
/// sub-second values to `0` — producing `"... for >= 0s; ..."` in tests
/// that drive the grace path with `Duration::from_millis(200)`.
///
/// Manual implementation rather than pulling in `humantime` for one call
/// site, where the format vocabulary is narrow (ms vs s) and the dedicated
/// helper keeps the unit test surface focused on the two boundaries that
/// matter for #336.
fn format_grace_duration(d: Duration) -> String {
    if d < Duration::from_secs(1) {
        format!("{}ms", d.as_millis())
    } else {
        format!("{}s", d.as_secs())
    }
}

/// Resolve the unknown-status grace period for `await_bulk_task`.
///
/// Release builds always return `DEFAULT_UNKNOWN_STATUS_GRACE_SECS` (30s).
/// Debug builds also honor `JR_BULK_UNKNOWN_GRACE_SECS` (whole seconds —
/// parsed as `u64`, so `"0"`, `"1"`, `"30"` etc. but NOT `"0.5"`) so CLI-
/// level integration tests can drive the warn+escalate path through the
/// binary quickly — typically by setting it to `"0"` so escalation fires
/// on the second poll (one ~1s sleep cycle after the first sighting) —
/// without shipping the knob to production. Unparseable / non-numeric
/// values are silently ignored (fall back to default) — the env var is a
/// test seam, not user-configurable surface, and a typo shouldn't break
/// the polling loop in a dev shell.
///
/// Gated `#[cfg(debug_assertions)]` mirrors the existing `JR_BASE_URL` and
/// `JR_AUTH_HEADER` debug-only patterns (CLAUDE.md "AI Agent Notes"). Unlike
/// those env vars there is no token-leak vector, so a single-site gate is
/// sufficient — there is no production code path that reads it.
fn resolve_unknown_status_grace() -> Duration {
    #[cfg(debug_assertions)]
    {
        if let Ok(s) = std::env::var("JR_BULK_UNKNOWN_GRACE_SECS") {
            if let Ok(secs) = s.parse::<u64>() {
                return Duration::from_secs(secs);
            }
        }
    }
    Duration::from_secs(DEFAULT_UNKNOWN_STATUS_GRACE_SECS)
}

/// Maximum byte length for a taskId received from Atlassian, used as a sanity
/// cap before embedding the value into the bulk-queue URL path.
///
/// Set generously (256 bytes) because Atlassian doesn't officially document the
/// taskId format. Empirical/inferred evidence (Perplexity 2026-05-11, citing
/// Atlassian cloud-identifier conventions) suggests a `{numericPrefix}:{uuid}`
/// shape on the order of ~40-50 chars (e.g.,
/// `"123456:4ac97bc8-ab12-ab12-8d38-eda562abc123"`). The 256-byte cap leaves
/// generous headroom for future format evolution while still rejecting
/// obviously oversized responses that could indicate a hostile/spoofed server.
const MAX_TASK_ID_LEN: usize = 256;

/// Validate that a taskId received in a `BulkSubmitResponse` (or accepted by a
/// poll caller) is safe to embed in the bulk-queue URL path.
///
/// This is defense-in-depth against a hostile or spoofed Atlassian response:
/// the URL is constructed via `urlencoding::encode(task_id)` which defangs
/// path separators, but the allowlist below also rejects oversized responses,
/// embedded NUL/control bytes, identifiers containing path-traversal segment
/// separators (`/`, `\`), and the specific dot-segment values (`.`, `..`)
/// that RFC 3986 §5.2.4 URL normalizers resolve away BEFORE transmission
/// (verified Perplexity 2026-05-11: reqwest/hyper/curl all apply §5.2.4
/// before send, so `/bulk/queue/..` becomes `/bulk/` at the client — a
/// path-confusion vulnerability if `.` or `..` reaches this function).
/// Cross-relevant threat model: a `JR_BASE_URL`-controlled MitM proxy in a
/// test scenario.
///
/// Allowlist: ASCII alphanumeric + `-`, `_`, `:`, `.`. Covers UUIDs, the
/// `domainId:uuid` cloud-identifier pattern, numeric IDs, and other opaque
/// printable-ASCII tokens. The `.` character is permitted within longer
/// tokens (e.g., `v1.0`) but the standalone values `.` and `..` are
/// explicitly rejected as dot-segments.
fn validate_task_id(task_id: &str) -> anyhow::Result<()> {
    if task_id.is_empty() {
        anyhow::bail!(
            "Bulk operation response taskId is empty; cannot poll task status. \
             Re-run the bulk command — if the same error recurs, the Atlassian \
             endpoint may be misbehaving or a proxy may be intercepting responses."
        );
    }
    // Reject dot-segments BEFORE the length and charset checks. Per RFC 3986
    // §5.2.4 (Remove Dot Segments), HTTP clients including reqwest/hyper/curl
    // normalize `.` and `..` path segments BEFORE transmission, rewriting
    // `/rest/api/3/bulk/queue/..` to `/rest/api/3/bulk/` — a path-confusion
    // attack vector if a hostile/spoofed response returned `..` as taskId.
    // urlencoding::encode does NOT encode `.` so the value reaches the
    // normalizer intact. Reject as a single special case.
    if task_id == "." || task_id == ".." {
        anyhow::bail!(
            "Bulk operation response taskId is a dot-segment ({task_id:?}); URL \
             normalizers (RFC 3986 §5.2.4) resolve dot-segments before transmission, \
             which would rewrite the bulk-queue URL path to a different endpoint. \
             Rejecting as hostile/malformed."
        );
    }
    if task_id.len() > MAX_TASK_ID_LEN {
        anyhow::bail!(
            "Bulk operation response taskId is {} bytes (max allowed: {}); rejecting \
             as potentially hostile/malformed response. Re-run the bulk command — if \
             the same error recurs, the Atlassian endpoint may be misbehaving or a \
             proxy may be intercepting responses.",
            task_id.len(),
            MAX_TASK_ID_LEN
        );
    }
    if !task_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
    {
        anyhow::bail!(
            "Bulk operation response taskId contains disallowed characters; \
             allowlist is ASCII alphanumeric + '-', '_', ':', '.'. Got: {task_id:?}. \
             This indicates an unexpected Atlassian response or a hostile proxy."
        );
    }
    Ok(())
}

impl JiraClient {
    /// Submit a bulk field-edit operation.
    ///
    /// POST /rest/api/3/bulk/issues/fields
    /// Returns taskId for polling via `await_bulk_task`.
    ///
    /// Required fields per Atlassian docs (verified Perplexity 2026-05-10, PR2):
    ///   - `selected_issue_ids_or_keys` — issue keys (already taken via `keys`)
    ///   - `selected_actions` — list of field names being edited; without this
    ///     the API returns 400. Mirrors keys used inside `edited_fields`.
    ///   - `edited_fields` — per-field edit payload (shape varies by field type).
    ///
    /// `edited_fields` shape (labels example, labelsAction casing best-guess "ADD"/"REMOVE";
    /// see SCHEMA NOTES in `types/jira/bulk.rs::BulkEditRequest` for the canonical
    /// production shape and the schema-empirical-verification follow-up issue):
    /// ```json
    /// {"labels": {"labelsAction": "ADD", "labels": [{"name": "foo"}]}}
    /// ```
    pub async fn bulk_edit_fields(
        &self,
        keys: &[String],
        selected_actions: Vec<String>,
        edited_fields: serde_json::Value,
    ) -> anyhow::Result<String> {
        let body = BulkEditRequest {
            selected_issue_ids_or_keys: keys.to_vec(),
            selected_actions,
            edited_fields_input: edited_fields,
        };
        let resp: BulkSubmitResponse = self.post("/rest/api/3/bulk/issues/fields", &body).await?;
        validate_task_id(&resp.task_id)?;
        Ok(resp.task_id)
    }

    /// Submit a bulk transition operation.
    ///
    /// POST /rest/api/3/bulk/issues/transition
    /// Returns taskId for polling via `await_bulk_task`.
    ///
    /// `transition_id` is a numeric string (e.g., "31").
    /// CONFIRMED schema: top-level `transitionId` field (not nested).
    pub async fn bulk_transition(
        &self,
        keys: &[String],
        transition_id: &str,
    ) -> anyhow::Result<String> {
        let body = BulkTransitionRequest {
            selected_issue_ids_or_keys: keys.to_vec(),
            transition_id: transition_id.to_string(),
        };
        let resp: BulkSubmitResponse = self
            .post("/rest/api/3/bulk/issues/transition", &body)
            .await?;
        validate_task_id(&resp.task_id)?;
        Ok(resp.task_id)
    }

    /// Poll a single bulk task status snapshot.
    ///
    /// GET /rest/api/3/bulk/queue/{task_id}
    /// This is a low-level single-poll; use `await_bulk_task` for the full polling loop.
    pub async fn poll_bulk_task(&self, task_id: &str) -> anyhow::Result<BulkOperationProgress> {
        // Defense-in-depth: validate again at the URL-construction site, in case
        // a caller obtained the taskId via some path that bypassed validation
        // (e.g., a test fixture, a future deserialized-from-cache scenario).
        // bulk_edit_fields and bulk_transition already validate at the receive
        // boundary; this call is cheap and idempotent.
        validate_task_id(task_id)?;
        let path = format!("/rest/api/3/bulk/queue/{}", urlencoding::encode(task_id));
        self.get(&path).await
    }

    /// Poll a bulk task with exponential backoff until terminal state or timeout.
    ///
    /// Uses `JiraClient::get` → `send` for each poll, which already handles:
    ///   - 429 with Retry-After (up to MAX_RETRIES=3 and MAX_RETRY_AFTER_SECS=60 cap)
    ///   - 401 blanket auto-refresh (S-3.03 v2)
    ///
    /// On non-terminal status, sleeps with exponential backoff (1s→2s→4s→8s→10s cap).
    ///
    /// Returns `Ok(BulkOperationProgress)` when a terminal status is reached.
    /// Returns `Err(...)` when timeout is exceeded or an HTTP error occurs.
    ///
    /// Uses `DEFAULT_UNKNOWN_STATUS_GRACE_SECS` (30s) for the unknown-status
    /// escalation grace period. In-lib tests override via
    /// `await_bulk_task_with_grace_for_test`. CLI-level integration tests
    /// (in `tests/`) override via the `JR_BULK_UNKNOWN_GRACE_SECS` env var,
    /// gated `#[cfg(debug_assertions)]` so release binaries ignore it.
    pub async fn await_bulk_task(
        &self,
        task_id: &str,
        timeout: Duration,
    ) -> anyhow::Result<BulkOperationProgress> {
        self.await_bulk_task_inner(task_id, timeout, resolve_unknown_status_grace())
            .await
    }

    /// Test-only variant of `await_bulk_task` that exposes the unknown-status
    /// grace-period parameter so tests can verify the warn+escalate path
    /// quickly — typically in ~1 second of wall-clock time rather than the
    /// 30-second production default. The polling loop's `POLL_BASE_SECS` (1s)
    /// minimum sleep between polls means escalation cannot complete in
    /// strictly sub-second time, but a sub-second `unknown_status_grace`
    /// (e.g., `Duration::from_millis(200)`) guarantees the grace expiry check
    /// fires on the second poll. NOT part of the release API surface.
    #[cfg(test)]
    pub async fn await_bulk_task_with_grace_for_test(
        &self,
        task_id: &str,
        timeout: Duration,
        unknown_status_grace: Duration,
    ) -> anyhow::Result<BulkOperationProgress> {
        self.await_bulk_task_inner(task_id, timeout, unknown_status_grace)
            .await
    }

    async fn await_bulk_task_inner(
        &self,
        task_id: &str,
        timeout: Duration,
        unknown_status_grace: Duration,
    ) -> anyhow::Result<BulkOperationProgress> {
        // Validate task_id at function entry — BEFORE the deadline is computed.
        // The timeout-exceeded error message below interpolates task_id via
        // `Display` ({task_id}), which renders ASCII control characters literally
        // (CR/LF/ANSI escapes would be terminal-interpreted in stderr/logs).
        // If timeout is very small (or 0), the deadline check fires before any
        // call to poll_bulk_task — without this guard, a hostile/spoofed task_id
        // (e.g., `"abc\r\n[jr] FAKE LOG"`) would reach the error sink unsanitized.
        // CWE-117 defense-in-depth.
        validate_task_id(task_id)?;

        let deadline = Instant::now() + timeout;
        let mut backoff = POLL_BASE_SECS;

        // Unknown-status tracking (audit-followup #336). Holds the first
        // sighting timestamp + status string. Reset when a known status is
        // observed OR when a DIFFERENT unknown status appears (distinct
        // novel statuses each get their own grace period — likely indicates
        // genuine state transitions rather than stuck-in-novel-status).
        let mut unknown_state: Option<(Instant, String)> = None;

        loop {
            // Check timeout before each poll attempt.
            if Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Bulk task {task_id} did not complete within {}s timeout. \
                     Check Jira for task status.",
                    timeout.as_secs()
                ));
            }

            // poll_bulk_task → self.get → self.send (handles 429 + 401 auto-refresh).
            let progress = self.poll_bulk_task(task_id).await?;

            // Unknown-status escalation path (#336). Three cases:
            //  1. Known status — reset the unknown tracker and continue normally.
            //  2. Same unknown status as last poll AND grace exceeded — escalate.
            //  3. New unknown status OR same unknown still within grace — warn
            //     (only on first sighting per distinct status) and continue.
            if progress.is_known_status() {
                unknown_state = None;
            } else {
                let status = progress.status.clone();
                let now = Instant::now();
                match &unknown_state {
                    None => {
                        // First sighting of any unknown status — emit warning.
                        eprintln!(
                            "warning: bulk task {task_id} returned unrecognized status \
                             {status:?} — treating as non-terminal. If the operation \
                             hangs, this may be a new Atlassian status; report at \
                             https://github.com/Zious11/jira-cli/issues so the known-status \
                             list can be updated."
                        );
                        unknown_state = Some((now, status));
                    }
                    Some((first_seen, last_status)) if *last_status != status => {
                        // Different unknown status than last poll — reset tracker
                        // and re-warn so operators see the transition.
                        eprintln!(
                            "warning: bulk task {task_id} now returning a DIFFERENT \
                             unrecognized status {status:?} (was {last_status:?} for {}) \
                             — resetting grace-period counter.",
                            format_grace_duration(now.duration_since(*first_seen))
                        );
                        unknown_state = Some((now, status));
                    }
                    Some((first_seen, _)) => {
                        // Same unknown status persisting — check grace expiry.
                        if now.duration_since(*first_seen) >= unknown_status_grace {
                            return Err(anyhow::anyhow!(
                                "Bulk task {task_id} polled unrecognized status {status:?} \
                                 for >= {}; treating as terminal-with-error. If this status \
                                 indicates progress (not failure), please report at \
                                 https://github.com/Zious11/jira-cli/issues so the \
                                 known-status list can be updated.",
                                format_grace_duration(unknown_status_grace)
                            ));
                        }
                        // Within grace — fall through to sleep+retry.
                    }
                }
            }

            if progress.is_terminal() {
                // C-2: FAILED/CANCELLED/DEAD are terminal but unsuccessful.
                // Return an Err so callers surface this as a non-zero exit.
                // COMPLETE and COMPLETED (empirical safety alias) are the only
                // successful terminals — everything else is a task-level failure.
                let status = &progress.status;
                if matches!(status.as_str(), "FAILED" | "CANCELLED" | "DEAD") {
                    // Surface failureReason first if the API provided one (Perplexity-verified
                    // 2026-05-10: Atlassian FAILED responses include `failureReason: String`).
                    // Fall back to per-issue detail or the raw-API hint for older API versions
                    // or alternate failure shapes where failureReason is absent.
                    let hint = if let Some(reason) = progress.failure_reason.as_deref() {
                        reason.to_string()
                    } else if !progress.failed_accessible_issues.is_empty() {
                        let keys: Vec<&str> = progress
                            .failed_accessible_issues
                            .keys()
                            .map(String::as_str)
                            .collect();
                        format!("Failed issues: {}.", keys.join(", "))
                    } else {
                        format!(
                            "Run `jr api /rest/api/3/bulk/queue/{task_id}` to inspect the raw task state."
                        )
                    };
                    return Err(anyhow::anyhow!(
                        "Bulk task {task_id} ended with status {status}. {hint}"
                    ));
                }
                return Ok(progress);
            }

            // Not terminal yet: sleep with exponential backoff before retrying.
            let sleep_secs = backoff.min(POLL_MAX_SECS);
            tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
            backoff = (backoff * 2).min(POLL_MAX_SECS);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{MAX_TASK_ID_LEN, format_grace_duration, validate_task_id};

    #[test]
    fn test_336_format_grace_duration_sub_second_renders_ms() {
        // Sub-second values must render in milliseconds — `as_secs()` alone
        // truncates 200ms to 0, producing misleading ">= 0s" messages in
        // test diagnostics and (hypothetically) any future sub-second grace
        // configuration. Copilot R1-4 driver.
        assert_eq!(format_grace_duration(Duration::from_millis(200)), "200ms");
        assert_eq!(format_grace_duration(Duration::from_millis(1)), "1ms");
        assert_eq!(format_grace_duration(Duration::from_millis(999)), "999ms");
    }

    #[test]
    fn test_336_format_grace_duration_at_one_second_boundary_renders_s() {
        // The boundary case: exactly 1s should render as "1s" (full-second
        // resolution), not "1000ms". Above-one-second values render in
        // whole seconds.
        assert_eq!(format_grace_duration(Duration::from_secs(1)), "1s");
        assert_eq!(format_grace_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_grace_duration(Duration::from_secs(300)), "300s");
    }

    #[test]
    fn test_336_format_grace_duration_zero_renders_zero_ms() {
        // grace=0 is a legitimate test seam (`JR_BULK_UNKNOWN_GRACE_SECS=0`)
        // and must produce a coherent string rather than empty/absurd output.
        assert_eq!(format_grace_duration(Duration::ZERO), "0ms");
    }

    #[test]
    fn test_validate_task_id_accepts_uuid() {
        validate_task_id("4ac97bc8-ab12-ab12-8d38-eda562abc123").expect("plain UUID rejected");
    }

    #[test]
    fn test_validate_task_id_accepts_domain_prefixed_uuid() {
        // Atlassian cloud-identifier pattern: numericPrefix:uuid
        validate_task_id("123456:4ac97bc8-ab12-ab12-8d38-eda562abc123")
            .expect("domain-prefixed UUID rejected");
    }

    #[test]
    fn test_validate_task_id_accepts_numeric_token() {
        validate_task_id("12345").expect("numeric token rejected");
    }

    #[test]
    fn test_validate_task_id_accepts_alphanumeric_opaque() {
        validate_task_id("abcDEF123_test-token").expect("opaque token rejected");
    }

    #[test]
    fn test_validate_task_id_rejects_empty() {
        let err = validate_task_id("").expect_err("empty taskId accepted");
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_task_id_rejects_oversized() {
        let oversized = "a".repeat(MAX_TASK_ID_LEN + 1);
        let err = validate_task_id(&oversized).expect_err("oversized taskId accepted");
        assert!(err.to_string().contains("bytes"));
    }

    #[test]
    fn test_validate_task_id_accepts_at_max_length() {
        let at_max = "a".repeat(MAX_TASK_ID_LEN);
        validate_task_id(&at_max).expect("boundary-length taskId rejected");
    }

    #[test]
    fn test_validate_task_id_rejects_forward_slash() {
        let err = validate_task_id("task/123").expect_err("forward slash accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_backslash() {
        let err = validate_task_id("task\\123").expect_err("backslash accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_path_traversal_attempt() {
        // "../etc/passwd" contains "/" which is outside the allowlist, so the
        // charset rejection fires first.
        let err = validate_task_id("../etc/passwd").expect_err("path traversal accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_single_dot_segment() {
        // Standalone "." is a dot-segment per RFC 3986 §5.2.4: HTTP client
        // libraries (reqwest, hyper, curl) normalize "/path/." to "/path/"
        // BEFORE transmission. Reject explicitly so a hostile/spoofed response
        // can't rewrite the bulk-queue URL path.
        let err = validate_task_id(".").expect_err("dot-segment accepted");
        assert!(err.to_string().contains("dot-segment"));
    }

    #[test]
    fn test_validate_task_id_rejects_double_dot_segment() {
        // Standalone ".." is a dot-segment per RFC 3986 §5.2.4: HTTP client
        // libraries normalize "/path/.." to its parent BEFORE transmission.
        // urlencoding::encode does NOT escape ".", so the dot-segment reaches
        // the normalizer intact. Reject explicitly to prevent path-confusion
        // attacks (verified Perplexity 2026-05-11 against RFC 3986 §5.2.4).
        let err = validate_task_id("..").expect_err("double-dot segment accepted");
        assert!(err.to_string().contains("dot-segment"));
    }

    #[test]
    fn test_validate_task_id_accepts_dot_within_longer_token() {
        // Dots are permitted WITHIN longer tokens (e.g., a hypothetical
        // version-prefixed taskId like "v1.0:abcd-..."). Only the standalone
        // values "." and ".." are dot-segments per RFC 3986.
        validate_task_id("v1.0:abc123").expect("dotted token rejected");
    }

    #[test]
    fn test_validate_task_id_rejects_null_byte() {
        let err = validate_task_id("task\x00123").expect_err("null byte accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_newline() {
        let err = validate_task_id("task\n123").expect_err("newline accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_carriage_return() {
        // CWE-117 adjacent: CR injection into URL paths can mess with downstream
        // logging or terminal escape interpretation; defang at the source.
        let err = validate_task_id("task\r123").expect_err("CR accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_space() {
        let err = validate_task_id("task 123").expect_err("space accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }

    #[test]
    fn test_validate_task_id_rejects_non_ascii() {
        // Unicode is outside the URL-safe allowlist for this identifier; Atlassian
        // taskIds are all ASCII per observed and inferred formats.
        let err = validate_task_id("tâsk-123").expect_err("non-ASCII accepted");
        assert!(err.to_string().contains("disallowed characters"));
    }
}

#[cfg(test)]
mod unknown_status_grace_tests {
    //! Integration tests for #336 unknown-status warn+grace-period escalation.
    //!
    //! These use wiremock to drive `await_bulk_task_with_grace_for_test` (the
    //! `#[cfg(test)]` shim around `await_bulk_task_inner`) through:
    //!   1. Persistent unknown → escalation to `Err` after the grace period.
    //!   2. Known-only polling sequence → no warning path, returns `Ok`.
    //!   3. Transient unknown → known → tracker resets, returns `Ok` on the
    //!      known terminal.
    //!
    //! The grace value in each test is sub-second so the test suite completes
    //! quickly; production callers use `DEFAULT_UNKNOWN_STATUS_GRACE_SECS`
    //! (30s, Perplexity-validated 2026-05-12).
    //!
    //! NOTE: the polling loop sleeps `tokio::time::sleep` with `POLL_BASE_SECS`
    //! (1s) between polls. Each test below thus takes ~1 wall-clock second.
    //! `tokio::time::pause()` would let us advance virtually, but wiremock
    //! relies on real I/O for its mock server — pausing time risks deadlock.
    //! 1s per test is acceptable; total suite cost ~3s.
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::api::client::JiraClient;

    fn progress_json(task_id: &str, status: &str) -> serde_json::Value {
        serde_json::json!({
            "taskId": task_id,
            "status": status,
            "progressPercent": 0,
            "totalIssueCount": 0,
            "processedAccessibleIssues": [],
            "failedAccessibleIssues": {},
        })
    }

    /// Persistent unknown status → escalation to terminal-with-error after the
    /// configured grace period elapses. Verifies the **escalation half** of
    /// the #336 contract: that the polling loop converts a persistent novel
    /// status into a definitive `Err` rather than waiting the full 5-minute
    /// timeout.
    ///
    /// The **warning-emission half** of the contract (the stderr `eprintln!`
    /// fired on first sighting) is verified at the CLI-level integration
    /// test `test_336_cli_unknown_status_emits_warning_and_escalates` in
    /// `tests/issue_bulk_pr2.rs`, which drives the binary via `assert_cmd`
    /// and asserts on captured stderr. Capturing `eprintln!` from an async
    /// unit test would require a `tracing` migration (Perplexity-validated
    /// 2026-05-12: no stdlib mechanism; the `gag` / `stdio-override` crates
    /// are sync-only and unmaintained) and is out of scope for #336.
    #[tokio::test]
    async fn test_336_persistent_unknown_status_escalates_to_err_after_grace() {
        let server = MockServer::start().await;
        let task_id = "test-unknown-336";

        // Always-on mock returning a made-up status outside the documented enum.
        // `MYSTERY_STATUS_FAKE` is deliberately chosen so a future Atlassian
        // enum addition won't accidentally collide and silently invalidate the
        // assertion.
        Mock::given(method("GET"))
            .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(progress_json(task_id, "MYSTERY_STATUS_FAKE")),
            )
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Bearer test".to_string());
        let result = client
            .await_bulk_task_with_grace_for_test(
                task_id,
                Duration::from_secs(10),
                Duration::from_millis(200),
            )
            .await;

        let err = result.expect_err("expected escalation error for persistent unknown status");
        let msg = err.to_string();
        assert!(
            msg.contains("MYSTERY_STATUS_FAKE"),
            "escalation error should mention the offending status; got: {msg}"
        );
        assert!(
            msg.contains("terminal-with-error"),
            "escalation error should mention 'terminal-with-error'; got: {msg}"
        );
    }

    /// Known-only polling sequence (ENQUEUED → COMPLETE) → no escalation,
    /// `Ok(progress)` returned. Confirms the warn/grace path is not engaged
    /// for documented statuses.
    #[tokio::test]
    async fn test_336_known_status_sequence_returns_ok_without_escalation() {
        let server = MockServer::start().await;
        let task_id = "test-known-336";

        // Poll 1: ENQUEUED (known non-terminal).
        Mock::given(method("GET"))
            .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(progress_json(task_id, "ENQUEUED")),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // Poll 2+: COMPLETE (known terminal).
        Mock::given(method("GET"))
            .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(progress_json(task_id, "COMPLETE")),
            )
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Bearer test".to_string());
        let result = client
            .await_bulk_task_with_grace_for_test(
                task_id,
                Duration::from_secs(10),
                Duration::from_millis(200),
            )
            .await;

        let progress = result.expect("known-only sequence should converge on Ok");
        assert_eq!(progress.status, "COMPLETE");
    }

    /// Transient unknown then known terminal → tracker resets on the known
    /// status, and the loop returns `Ok(progress)` instead of escalating.
    /// Verifies the unknown_state-reset branch.
    #[tokio::test]
    async fn test_336_transient_unknown_then_known_resets_tracker() {
        let server = MockServer::start().await;
        let task_id = "test-transient-336";

        // Poll 1: unknown.
        Mock::given(method("GET"))
            .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(progress_json(task_id, "TRANSIENT_FAKE_STATUS")),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        // Poll 2+: COMPLETE.
        Mock::given(method("GET"))
            .and(path(format!("/rest/api/3/bulk/queue/{task_id}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(progress_json(task_id, "COMPLETE")),
            )
            .mount(&server)
            .await;

        let client = JiraClient::new_for_test(server.uri(), "Bearer test".to_string());
        // Use a long-enough grace (2s) that even if the test were slow, the
        // single unknown sighting wouldn't trigger escalation — we want to
        // assert the reset path explicitly, not race the grace clock.
        let result = client
            .await_bulk_task_with_grace_for_test(
                task_id,
                Duration::from_secs(10),
                Duration::from_secs(2),
            )
            .await;

        let progress =
            result.expect("transient unknown followed by COMPLETE should converge on Ok");
        assert_eq!(progress.status, "COMPLETE");
    }
}
