//! Per-profile single-flight coordinator for OAuth token refresh.
//!
//! # Mutex layering rule
//!
//! Outer `std::sync::Mutex<HashMap<...>>` is held ONLY BRIEFLY for HashMap
//! lookup/insert; it is released BEFORE any `.await`. Inner
//! `tokio::sync::Mutex<RefreshState>` is held across the refresh `.await`.
//!
//! `tokio::sync::Mutex` is mandatory for the inner lock because:
//! - It does NOT poison on panic — correct semantic for refresh. A panicked
//!   refresh should not permanently break the coordinator for subsequent calls.
//! - It is held across `.await` points, which `std::sync::Mutex` must not be.
//!
//! Source: S-3.03 v2 spec; also documented in CLAUDE.md gotcha section.

use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use tokio::sync::Mutex as TokioMutex;

use crate::error::JrError;

/// Per-profile refresh state cached in memory.
///
/// `last_result` is set by the first task that wins the inner lock and
/// performs the refresh. Subsequent waiters (tasks that acquire the inner lock
/// only after the winner has finished) read `last_result` and either reuse the
/// cached access token or short-circuit to the cached error.
#[derive(Default)]
pub(crate) struct RefreshState {
    /// New access token from a successful refresh. Set before `last_result`.
    pub last_access_token: Option<String>,
    /// New refresh token from a successful refresh.
    pub last_refresh_token: Option<String>,
    /// `Some(Ok(()))` on success; `Some(Err(msg))` on failure.
    /// `None` means the refresh has not been attempted for this lock epoch.
    pub last_result: Option<Result<(), String>>,
}

/// Global per-profile coordinator map.
///
/// Keyed by profile name. Values are `Arc<TokioMutex<RefreshState>>` that
/// survive across calls within the same process lifetime — serving as the
/// in-process single-flight gate.
static COORDINATOR: OnceLock<StdMutex<HashMap<String, Arc<TokioMutex<RefreshState>>>>> =
    OnceLock::new();

fn coordinator() -> &'static StdMutex<HashMap<String, Arc<TokioMutex<RefreshState>>>> {
    COORDINATOR.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Reset the coordinator state for a given profile.
///
/// Exposed for testing only: allows tests to clear cached `last_result` so
/// each test starts with a fresh coordinator epoch. Not called in production.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn reset_for_test(profile: &str) {
    if let Ok(mut map) = coordinator().lock() {
        map.remove(profile);
    }
}

/// Single-flight refresh gate for a given profile.
///
/// At most one refresh HTTP call is made per (profile, token_url_epoch) per
/// coordinator epoch. The coordinator is keyed on the profile name combined
/// with the current `JR_OAUTH_TOKEN_URL` env-var value. This ensures that:
///
/// 1. Integration tests (which set unique `JR_OAUTH_TOKEN_URL` paths per test)
///    get isolated coordinator entries — a cached failure from test AC-003 does
///    not short-circuit test AC-001 when they run sequentially in the same process.
/// 2. Concurrent tasks within a single test (AC-005, AC-006) share the coordinator
///    because they all see the same `JR_OAUTH_TOKEN_URL`.
/// 3. In production (no `JR_OAUTH_TOKEN_URL`), the key is `{profile}:` which is
///    stable for the lifetime of a single `jr` invocation (the process doesn't
///    change the env var at runtime).
///
/// # Algorithm
///
/// 1. Acquire the outer `StdMutex` BRIEFLY to get (or insert) the
///    per-profile `Arc<TokioMutex<RefreshState>>`. Release BEFORE any `.await`.
/// 2. Acquire the inner `TokioMutex<RefreshState>` — this blocks concurrent
///    callers for the same profile across `.await` points.
/// 3. If `last_result` is already set:
///    - `Ok(())` → return cached access token (waiter path).
///    - `Err(msg)` → short-circuit to `NotAuthenticated` (no thundering herd).
/// 4. Otherwise: call `refresh_fn` (the first caller wins).
///    - Success → cache tokens + `Ok(())` in `RefreshState`; return access token.
///    - Failure → cache `Err(msg)` in `RefreshState`; propagate error.
///
/// # Returns
///
/// `Ok(new_access_token)` on success. `Err(JrError::NotAuthenticated { hint })`
/// on refresh failure (invalid_grant or any other HTTP error from Atlassian).
/// `token_url_snapshot` is the already-resolved token URL, snapshotted by the
/// caller (in `JiaClient::send`) before any `.await`. It forms part of the
/// coordinator map key so that tests which set unique `JR_OAUTH_TOKEN_URL` per
/// test get isolated coordinator entries without cross-test state leakage.
pub(crate) async fn refresh_with_single_flight<F, Fut>(
    profile: &str,
    token_url_snapshot: &str,
    refresh_fn: F,
) -> Result<String, JrError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(String, String), anyhow::Error>>,
{
    // Build the coordinator map key: {profile}:{token_url_snapshot}.
    //
    // Including the token URL in the key ensures that integration tests —
    // which set unique `JR_OAUTH_TOKEN_URL` paths per test — get isolated
    // coordinator entries. Without this, a cached `Err` from test AC-003
    // (invalid_grant) would short-circuit test AC-001 when they run
    // sequentially in the same process. In production, the token URL is the
    // stable `https://auth.atlassian.com/oauth/token`, so the key is
    // `{profile}:https://auth.atlassian.com/oauth/token`.
    let coordinator_key = format!("{profile}:{token_url_snapshot}");

    // Step 1: get-or-insert per-profile inner mutex (BRIEF lock; released before .await)
    let inner_arc = {
        let mut map = coordinator()
            .lock()
            .expect("coordinator outer mutex must not be poisoned (no panic-with-lock path)");
        map.entry(coordinator_key)
            .or_insert_with(|| Arc::new(TokioMutex::new(RefreshState::default())))
            .clone()
    }; // <-- StdMutexGuard dropped here; outer lock released BEFORE the .await below

    // Step 2: hold inner mutex across the refresh .await (only blocks same-profile callers)
    let mut state = inner_arc.lock().await;

    // Step 3a: a prior waiter's refresh succeeded — reuse cached token (no HTTP call)
    if let Some(Ok(())) = &state.last_result {
        if let Some(token) = &state.last_access_token {
            return Ok(token.clone());
        }
    }

    // Step 3b: a prior waiter's refresh failed — short-circuit (no thundering herd)
    if let Some(Err(err_msg)) = &state.last_result {
        return Err(JrError::NotAuthenticated {
            hint: format!(
                "Token refresh failed: {}. Run 'jr auth refresh' to re-authenticate.",
                err_msg
            ),
        });
    }

    // Step 4: we are the winner — call refresh_fn under the inner lock
    match refresh_fn().await {
        Ok((access, refresh)) => {
            state.last_access_token = Some(access.clone());
            state.last_refresh_token = Some(refresh);
            state.last_result = Some(Ok(()));
            Ok(access)
        }
        Err(e) => {
            let err_msg = e.to_string();
            state.last_result = Some(Err(err_msg.clone()));
            Err(JrError::NotAuthenticated {
                hint: "run 'jr auth refresh' to re-authenticate".to_string(),
            })
        }
    }
}
