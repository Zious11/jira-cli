# S-3.04 Review Findings

## Convergence Table

| Cycle | Findings | Blocking | Non-Blocking | Fixed | Status |
|-------|----------|----------|--------------|-------|--------|
| 1     | 2        | 0        | 2            | N/A   | APPROVE |

---

## Cycle 1 — Full Review

**Reviewer:** pr-reviewer  
**Date:** 2026-05-09  
**Branch:** feat/S-3.04-multi-cloudid-disambiguation  
**PR:** #320  
**Commits reviewed:** 7c83907, bfbda6a, b84c940

### Scope Assessment

The PR diff against `origin/develop` contains exactly 3 commits unique to S-3.04:
- `7c83907` — 1050-line integration test file (`tests/multi_cloudid_disambiguation.rs`)
- `bfbda6a` — 6 files, 222 insertions / 54 deletions (source + test changes)
- `b84c940` — 25 files of demo evidence (binary + text)

The large `git diff develop..feature` stat (~8228 lines) is due to ancestor commits from other wave-3 stories (S-3.01, S-3.02, etc.) that were on the feature branch before they merged to develop. GitHub's PR diff correctly shows only S-3.04 changes relative to `develop`. No size concern.

---

### AC Compliance Check

| AC | Claim | Implementation | Test | Status |
|----|-------|---------------|------|--------|
| AC-001 | `--cloud-id` flag recognized in help + selects correct resource | `src/cli/mod.rs:227` — `cloud_id: Option<String>` in `AuthCommand::Login`. Help text present. | `test_cloud_id_flag_recognized_in_help`, `test_cloud_id_flag_picks_named_resource_not_first` | PASS |
| AC-002 | `--no-input` + multi-org → exit 64 with "Multiple Atlassian orgs" + "--cloud-id" | `src/api/auth.rs` — `no_input` branch returns `JrError::UserError` (exit 64). Message contains both strings. | `test_no_input_multi_org_exits_64_with_actionable_error`, `test_no_input_multi_org_lists_available_cloud_ids_in_error` | PASS |
| AC-003 | Single-org path unchanged | `len == 1` branch → `resources[0].id.clone()` — no change to behavior | `test_single_resource_no_regression_single_org_path` | PASS |
| AC-004 | Callback URL invariant `127.0.0.1:53682` | `--cloud-id` is a post-exchange filter. `build_authorize_url` unchanged. | `test_callback_url_contains_127_0_0_1_and_port_53682`, `test_cloud_id_flag_does_not_change_redirect_uri_in_authorize_url` | PASS |
| AC-005 | Interactive stdin selection picks correct resource | Non-TTY fallback: 1-based numeric stdin read → `resources[idx - 1].id.clone()` | `test_interactive_select_via_stdin_picks_second_resource` | PASS |
| AC-006 | Error listing renders name + URL + cloudId | Format string `"  {} — {} ({})", r.id, r.name, r.url` (error path); `"{} ({}) [cloudId: {}]", r.name, r.url, r.id` (prompt path). Test asserts all three. | `test_interactive_render_shows_name_url_and_id` | PASS |

### BC Compliance Check

| BC | Claim | Verified |
|----|-------|---------|
| BC-1.5.038 | multi-cloud disambiguation primary | PASS — `len > 1` branch implemented with all three paths |
| BC-1.1.007 | profile precedence unchanged | PASS — `cloud_id` scoped to Login subcommand only |
| BC-1.5.031 | callback URL fixed at `127.0.0.1:53682` | PASS — `build_authorize_url` call site unchanged |

---

### Findings

#### Finding 1 — SUGGESTION (non-blocking): `--cloud-id` not on `jr auth refresh` subcommand

**Severity:** suggestion  
**Category:** coverage  
**Location:** `src/cli/auth/refresh.rs`

When `refresh_credentials` calls `login_oauth`, it passes `cloud_id_override: None`. This is correct for auto-refresh flows, but a user who ran `jr auth refresh` manually against a multi-org account would hit the interactive prompt (or exit 64 if `--no-input` is set). The story spec says "Add to `jr auth login` subcommand (and potentially `jr auth refresh` if applicable)" — the implementer chose to scope it to login only.

**Assessment:** Acceptable for this story's scope. The story spec explicitly lists this as "potentially" in scope and the implementation note marks `None` as "backward-compatible." Deferred to a follow-up story if users request it. Does NOT block merge.

#### Finding 2 — NIT: `JrError::UserError` used for 0-resource path (spec showed `NotAuthenticated`)

**Severity:** nit  
**Category:** coherence  
**Location:** `src/api/auth.rs` — `match resources.len() { 0 => ... }`

The story spec pseudocode shows `Err(JrError::NotAuthenticated { ... })` for the empty-resources case. The implementation uses `JrError::UserError(...)` which also exits 64. Both are semantically reasonable — 0 accessible resources after a successful token exchange is a user/consent issue, not an auth code issue. Exit code is identical. No user-visible regression.

**Assessment:** Nit only. The practical behavior (exit 64 with actionable message) matches spec intent. Does NOT block merge.

---

### Triage Routing

| Finding | Severity | Category | Route | Action |
|---------|----------|----------|-------|--------|
| `--cloud-id` not on `jr auth refresh` | suggestion | coverage | deferred | No action in this PR — scope is correct for S-3.04 |
| `JrError::UserError` vs `NotAuthenticated` for 0-resource | nit | coherence | none | Acceptable — exit code matches, message is actionable |

---

### Overall Verdict

**APPROVE** — All 6 ACs pass. All 3 BC anchors satisfied. 12/12 integration tests cover the full disambiguation matrix. 612/612 lib unit tests pass (no regression). BC-1.5.031 callback URL invariant preserved with explicit regression-pin test. No blocking findings. H-047 ready to flip KNOWN-GAP → MUST-PASS post-merge.
