# S-3.03 Review Findings

## Convergence Tracking

| Cycle | Findings | Blocking | Fixed | Remaining |
|-------|----------|----------|-------|-----------|
| 1 (internal review) | 2 CI regressions | 2 | 2 | 0 |
| 2 (re-run) | 0 | 0 | 0 | 0 → APPROVE |

## Cycle 1 Findings

| # | Finding | Severity | Category | Resolution |
|---|---------|----------|----------|-----------|
| 1 | `test_401_scope_mismatch_returns_insufficient_scope` and `test_401_scope_mismatch_matches_case_insensitively` failed: blanket-401 auto-refresh triggered before InsufficientScope check, reaching real Atlassian endpoint with blank creds, returning NotAuthenticated instead of InsufficientScope | BLOCKING | CI regression | Fixed in `fix(S-3.03)` commit `4a5d557`: read + inspect 401 body before entering auto-refresh path; scope-mismatch bodies short-circuit to InsufficientScope directly |
| 2 | `test_401_returns_not_authenticated` (and related) failed: Basic-auth clients (`JiraClient::new_for_test`) triggered auto-refresh, which is semantically incorrect — Basic auth uses API tokens, not OAuth refresh tokens | BLOCKING | CI regression | Fixed in same commit: gate auto-refresh on `self.auth_header.starts_with("Bearer ")` |

## Cycle 2 Results

All 22 api_client tests PASS. All 8/8 always-run S-3.03 integration tests PASS. All 8 CI checks PASS.

## PR #321 Outcome

- Merged: 2026-05-09T21:23:31Z
- Merge SHA: `597dd23c92854f54dcf31ea246a16ca12b5b569c`
- CI Run: `25612107567` — 8/8 PASS
- Convergence: 2 cycles (1 fix cycle required)
