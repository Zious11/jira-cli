//! CLI-level integration tests for `jr issue attachment delete`.
//!
//! RED GATE: all tests in this file FAIL because `handle_attachment_delete`,
//! `attachment_delete_confirmation_gate`, `filter_attachments_older_than`, and
//! `parse_age_duration` contain `todo!()` — the spawned subprocess exits 101
//! (Rust panic) instead of the expected exit codes and output.
//!
//! After Task 3/4/5 (handler + gate + filter implementation), all tests become GREEN.
//!
//! BC anchors: BC-3.9.008, BC-3.9.010, BC-3.9.013, BC-3.9.015, BC-3.9.016,
//!             BC-3.9.019, BC-3.9.020
//! VPs: VP-576-002 (delete confirmation gate confirm/cancel)
//! Security: SEC-576-011 (CWE-116 display_sanitize_filename in gate prompt)
//! DEC: DEC-168 (404 on targeted delete exits 64 + surfaces body)
//! Story: S-576-4, GitHub issue #576

use assert_cmd::Command;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

fn jr_cmd_with_xdg(
    server_uri: &str,
    cache_dir: &std::path::Path,
    config_dir: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache_dir)
        .env("JR_CONFIG_DIR", config_dir);
    cmd
}

// ---------------------------------------------------------------------------
// AC-001: DELETE endpoint + AID validation + 404 exit 64 (BC-3.9.008)
// ---------------------------------------------------------------------------

/// BC-3.9.008: `jr issue attachment delete <AID>` issues `DELETE /rest/api/3/attachment/{id}`.
/// AID validation (`^[0-9]+$`) fires before any HTTP call; non-numeric AID → exit 64.
/// 204/200 → success; 404 (targeted single-AID) → exit 64 (DEC-168).
#[tokio::test]
async fn test_bc_3_9_008_delete_endpoint_aid_validation_404_exit_64() {
    todo!(
        "S-576-4: stub — assert DELETE /rest/api/3/attachment/{{id}} issued; \
         non-numeric AID exits 64; 404 exits 64 per DEC-168"
    )
}

// ---------------------------------------------------------------------------
// AC-002: single-AID confirmation gate (BC-3.9.015; VP-576-002)
// ---------------------------------------------------------------------------

/// BC-3.9.015: AID validation (`^[0-9]+$`) fires BEFORE the confirmation gate prompt.
/// Invalid AID exits 64 immediately; no prompt shown; no HTTP call issued.
#[tokio::test]
async fn test_bc_3_9_015_aid_validation_before_gate() {
    todo!(
        "S-576-4: stub — invalid AID exits 64 with canonical message; \
         assert no prompt text on stderr; assert zero HTTP calls issued"
    )
}

/// VP-576-002 confirm variant: confirming the gate proceeds to DELETE and exits 0.
///
/// Uses `JR_STDIN_IS_TTY=1` seam + stdin `"y\n"` to exercise the interactive path.
#[tokio::test]
async fn test_vp_576_002_delete_gate_confirm_proceeds() {
    todo!(
        "S-576-4: stub — JR_STDIN_IS_TTY=1; stdin 'y'; assert DELETE issued; exit 0"
    )
}

/// VP-576-002 cancel variant: cancelling the gate issues ZERO DELETEs and exits 0.
///
/// Uses `JR_STDIN_IS_TTY=1` seam + stdin `"n\n"` to exercise the cancel path.
#[tokio::test]
async fn test_vp_576_002_delete_gate_cancel_stays() {
    todo!(
        "S-576-4: stub — JR_STDIN_IS_TTY=1; stdin 'n'; assert zero DELETEs; \
         stderr contains 'Deletion cancelled.'; exit 0"
    )
}

/// BC-3.9.015 EOF path: EOF on gate stdin (`read_line` returns `Ok(0)`) → exit 130.
///
/// Uses `JR_STDIN_IS_TTY=1` seam + empty stdin to exercise EOF.
#[tokio::test]
async fn test_bc_3_9_015_gate_eof_exits_130() {
    todo!(
        "S-576-4: stub — JR_STDIN_IS_TTY=1; stdin EOF; assert exit 130 (JrError::Interrupted)"
    )
}

// ---------------------------------------------------------------------------
// AC-003: single-AID JSON response shape (BC-3.9.010)
// ---------------------------------------------------------------------------

/// BC-3.9.010: `jr issue attachment delete <AID> --yes --output json` returns
/// `{"deleted":true,"id":"<AID>"}` (BTreeMap-alphabetical; via output::render_json).
/// Human-mode: `"Deleted attachment <AID>."` to stdout.
#[tokio::test]
async fn test_bc_3_9_010_single_aid_json_shape() {
    todo!(
        "S-576-4: stub — wiremock returns 204; assert JSON shape {{deleted:true,id:AID}}; \
         assert human mode stdout 'Deleted attachment <AID>.'"
    )
}

// ---------------------------------------------------------------------------
// AC-004: bulk --yes required; fail-soft on 404; non-404 aborts (BC-3.9.016/010/013)
// ---------------------------------------------------------------------------

/// BC-3.9.013 + BC-3.9.010 EC-3.9.010-5: all-404 bulk delete is benign (not exit 64).
/// Returns `{"count":0,"deleted":false,"ids":[]}` + human stderr hint (EC-3.9.010-5).
#[tokio::test]
async fn test_bc_3_9_013_bulk_delete_fail_soft_all_404() {
    todo!(
        "S-576-4: stub — two AIDs, both return 404; assert exit 0; \
         assert JSON {{count:0,deleted:false,ids:[]}}; \
         assert human-mode stderr contains 'No attachments deleted'"
    )
}

/// BC-3.9.016 EC-3.9.016-1/-8: missing `--yes` on bulk path → exit 64 with canonical strings.
/// Sub-case (a): `--issue <KEY> --older-than 30d` without `--yes` → EC-3.9.016-1 string.
/// Sub-case (b): `<AID1> <AID2>` without `--yes` → EC-3.9.016-8 string.
/// Assert: exit 64 + correct canonical string + zero DELETEs issued.
#[tokio::test]
async fn test_bc_3_9_016_bulk_requires_yes_exits_64() {
    todo!(
        "S-576-4: stub — sub-case (a) --older-than without --yes → exit 64 + \
         '--older-than requires --yes to confirm bulk deletion.'; \
         sub-case (b) multi-AID without --yes → exit 64 + \
         '--yes is required to delete multiple attachments without a confirmation prompt.'; \
         both: assert zero DELETEs"
    )
}

/// BC-3.9.013 EC-3.9.010-4: non-404 error on any AID ABORTS the sequence.
/// First AID returns 204 (success); second AID returns 403 → sequence stops.
/// Assert: first deletion stands (not reversed); 403 surfaced; exit 1.
#[tokio::test]
async fn test_bc_3_9_010_bulk_delete_non_404_aborts_sequence() {
    todo!(
        "S-576-4: stub — AID1→204, AID2→403; assert sequence stops after AID2 DELETE; \
         assert first deletion completed and not reversed; \
         assert 403 error surfaced to stderr; exit 1"
    )
}

/// BC-3.9.010: partial-404 bulk — AID2→404 is benign; AID1+AID3 succeed.
/// Assert: all 3 DELETEs issued; AID2 excluded from response ids;
/// result `{"count":2,"deleted":true,"ids":["AID1","AID3"]}`; exit 0.
#[tokio::test]
async fn test_bc_3_9_010_bulk_partial_404_skip_continues() {
    todo!(
        "S-576-4: stub — AID1→204, AID2→404, AID3→204; assert all 3 DELETEs issued; \
         AID2 absent from ids; JSON {{count:2,deleted:true,ids:[AID1,AID3]}}; exit 0"
    )
}

// ---------------------------------------------------------------------------
// AC-005: bulk JSON response shape (BC-3.9.010)
// ---------------------------------------------------------------------------

/// BC-3.9.010: `jr issue attachment delete <AID1> <AID2> --yes --output json` returns
/// `{"count":2,"deleted":true,"ids":["<AID1>","<AID2>"]}` (BTreeMap-alphabetical;
/// `ids` in request-order).
#[tokio::test]
async fn test_bc_3_9_010_bulk_json_shape() {
    todo!(
        "S-576-4: stub — two AIDs, both 204; assert JSON shape \
         {{count:2,deleted:true,ids:[AID1,AID2]}}"
    )
}

// ---------------------------------------------------------------------------
// AC-006: --issue KEY + --older-than + --yes combined (BC-3.9.016 + BC-3.9.019)
// ---------------------------------------------------------------------------

/// BC-3.9.016 + BC-3.9.019: `jr issue attachment delete --issue <KEY> --older-than <DUR> --yes`.
/// Wire flow: fetch issue attachment list → age filter → bulk DELETE.
/// BC-3.9.019 canonical strings:
///   (a) N>0 human run → stderr contains pre-DELETE HINT + success summary.
///   (b) N>0 JSON run → NEITHER string on stderr (JSON-suppressed).
///   (c) zero-match human run → stderr contains zero-match echo.
#[tokio::test]
async fn test_bc_3_9_019_issue_key_older_than_resolution() {
    todo!(
        "S-576-4: stub — fetch attachment list; age filter; bulk DELETE; \
         sub-assertion (a) N>0 human: stderr has 'Deleting N attachment(s)...' + \
         'Deleted N attachment(s)...'; \
         sub-assertion (b) N>0 JSON: NEITHER string on stderr; \
         sub-assertion (c) zero-match human: stderr has 'No attachments older than...'"
    )
}

// ---------------------------------------------------------------------------
// AC-007: --older-than duration parsing via parse_age_duration (BC-3.9.019)
// ---------------------------------------------------------------------------

/// BC-3.9.019 EC-3.9.019-3/8: `--older-than <DUR>` duration parsing integration.
/// Invalid duration → exit 64 with EC-3.9.019-3 canonical error string.
/// Valid duration filters correctly; no matches → exit 0 + empty JSON shape.
#[tokio::test]
async fn test_bc_3_9_019_older_than_parse_age_duration_filter() {
    todo!(
        "S-576-4: stub — invalid duration exits 64 with canonical EC-3.9.019-3 message; \
         valid duration filters attachments by created timestamp; \
         no matches → exit 0 + {{count:0,deleted:false,ids:[]}}"
    )
}

// Unit test test_bc_3_9_019_ec_8_parse_age_duration_1d_is_24h lives in
// src/cli/issue/attachments.rs #[cfg(test)] (EC-3.9.019-8; private helper).

// ---------------------------------------------------------------------------
// AC-008: --dry-run single-AID (BC-3.9.020 EC-3.9.020-3)
// ---------------------------------------------------------------------------

/// BC-3.9.020 EC-3.9.020-3: `jr issue attachment delete <AID> --dry-run`.
/// AID validation fires (guards NOT suppressed); gate suppressed; no DELETE issued.
/// Human: stderr hint `"--dry-run has no effect on single-ID delete; omit the flag."` + exit 0.
/// JSON (`--output json`): `{"attachments":[{"id":"<AID>"}],"dryRun":true,"ids":["<AID>"]}`.
/// Invalid AID → exit 64 (guard not suppressed per EC-3.9.020-3).
#[tokio::test]
async fn test_bc_3_9_020_dry_run_single_aid() {
    todo!(
        "S-576-4: stub — valid AID + --dry-run: no DELETE; human stderr hint; exit 0; \
         JSON shape {{attachments:[{{id:AID}}],dryRun:true,ids:[AID]}}; \
         invalid AID + --dry-run: exit 64 (guard fires)"
    )
}

// ---------------------------------------------------------------------------
// AC-009: --dry-run bulk (BC-3.9.020 EC-3.9.020-1/2)
// ---------------------------------------------------------------------------

/// BC-3.9.020 EC-3.9.020-1/2: bulk `--dry-run` (multi-AID or `--issue/--older-than`).
/// No DELETE issued; --yes NOT required on --dry-run.
/// Human: table + `"<N> attachment(s) would be deleted. Run without --dry-run to confirm."`.
/// JSON: `{"attachments":[{"filename":"<n>","id":"<AID>"}],"dryRun":true,"ids":[...]}`.
/// Zero matches: `{"attachments":[],"dryRun":true,"ids":[]}`.
#[tokio::test]
async fn test_bc_3_9_020_dry_run_bulk() {
    todo!(
        "S-576-4: stub — --issue --older-than --dry-run: fetch list, apply filter, \
         no DELETE; human table + would-delete summary; \
         JSON shape {{attachments:[{{filename,id}}],dryRun:true,ids:[...]}}; \
         multi-AID --dry-run: per-AID metadata fetch; same JSON shape"
    )
}

// ---------------------------------------------------------------------------
// AC-010: non-interactive without --yes exits 64 (BC-3.9.015; DEC-174)
// ---------------------------------------------------------------------------

/// BC-3.9.015 EC-3.9.015-3: `--no-input` or non-TTY stdin without `--yes` → exit 64.
/// Canonical message: `"Use --yes to confirm deletion without a prompt."`.
/// No DELETE issued.
#[tokio::test]
async fn test_bc_3_9_015_non_interactive_without_yes_exits_64() {
    todo!(
        "S-576-4: stub — --no-input without --yes: exit 64; \
         stderr contains 'Use --yes to confirm deletion without a prompt.'; \
         zero HTTP calls"
    )
}

// ---------------------------------------------------------------------------
// AC-011: --issue + --older-than + --yes combined (BC-3.9.016 + BC-3.9.019)
// ---------------------------------------------------------------------------

/// BC-3.9.016 + BC-3.9.019: full combined flow.
/// Bulk forms ALWAYS require `--yes` (no interactive gate offered; BC-3.9.016 line 3705).
/// `display_sanitize_filename` applied in `--dry-run` preview table (CWE-116 / AC-009).
#[tokio::test]
async fn test_bc_3_9_016_issue_older_than_yes_combined() {
    todo!(
        "S-576-4: stub — --issue + --older-than + --yes: combines list fetch + age filter + \
         bulk DELETE; no interactive gate; exit 0 on success"
    )
}

// ---------------------------------------------------------------------------
// AC-013: VP-576-002 wiremock anchor + DEC-168 body surfacing (BC-3.9.008)
// ---------------------------------------------------------------------------

/// DEC-168 body surfacing (EC-3.9.008-2): stderr MUST BEGIN with canonical prefix
/// `"Attachment <AID> not found or not accessible."` THEN the Jira error body.
/// NOT body-only; NOT silent exit 0. Exit 64.
#[tokio::test]
async fn test_bc_3_9_008_404_body_surfaced_to_stderr() {
    todo!(
        "S-576-4: stub — wiremock returns 404 with JSON error body; \
         assert stderr BEGINS with 'Attachment <AID> not found or not accessible.'; \
         assert stderr also contains the Jira error body; exit 64"
    )
}

// ---------------------------------------------------------------------------
// AC-014: bare --issue without --older-than → exit 2 (BC-3.9.016 EC-3.9.016-9)
// ---------------------------------------------------------------------------

/// BC-3.9.016 EC-3.9.016-9: `--issue <KEY>` without `--older-than` → exit 2 (clap error).
/// No application code reached; clap `requires` constraint.
#[tokio::test]
async fn test_bc_3_9_016_issue_without_older_than_exit_2() {
    todo!(
        "S-576-4: stub — --issue FOO-1 without --older-than: exit 2 (clap requires error)"
    )
}

// ---------------------------------------------------------------------------
// AC-015: clap mutual-exclusion + required-group constraints (BC-3.9.016)
// ---------------------------------------------------------------------------

/// BC-3.9.016 EC-3.9.016-4/5/9/10: all five clap-level constraint cases → exit 2.
///   (a) `<AID> --issue FOO-1` → exit 2 (conflicts_with).
///   (b) `<AID> --older-than 7d` → exit 2 (conflicts_with).
///   (c) `--older-than 7d` (no --issue) → exit 2 (requires).
///   (d) `--issue FOO-1` (no --older-than) → exit 2 (requires).
///   (e) `delete` (no args) → exit 2 (required group).
#[tokio::test]
async fn test_bc_3_9_016_clap_mutual_exclusion_constraints() {
    todo!(
        "S-576-4: stub — five sub-cases, all exit 2 via clap error; \
         zero HTTP calls for all cases"
    )
}

// ---------------------------------------------------------------------------
// AC-016: delete error taxonomy (BC-3.9.013)
// ---------------------------------------------------------------------------

/// BC-3.9.013: 401 → exit 2; stderr contains "Not authenticated" AND "jr auth login".
#[tokio::test]
async fn test_bc_3_9_013_delete_401_exit_2() {
    todo!(
        "S-576-4: stub — wiremock returns 401; assert exit 2; \
         assert stderr contains 'Not authenticated' AND 'jr auth login'"
    )
}

/// BC-3.9.013: 403 → exit 1; Jira error body surfaced to stderr.
#[tokio::test]
async fn test_bc_3_9_013_delete_403_exit_1() {
    todo!("S-576-4: stub — wiremock returns 403; assert exit 1; assert Jira body on stderr")
}

/// BC-3.9.013: 5xx → exit 1; stderr contains `"API error ("` (loose-substring).
/// Full literal from src/error.rs: `"API error (500): <message>"`.
#[tokio::test]
async fn test_bc_3_9_013_delete_5xx_exit_1() {
    todo!(
        "S-576-4: stub — wiremock returns 500; assert exit 1; \
         assert stderr contains 'API error ('"
    )
}

/// BC-3.9.013: network failure → exit 1; stderr contains `"Could not reach"`.
/// Full literal from src/error.rs::JrError::NetworkError: `"Could not reach <host> — check your connection"`.
#[tokio::test]
async fn test_bc_3_9_013_delete_network_error_exit_1() {
    todo!(
        "S-576-4: stub — server unreachable; assert exit 1; \
         assert stderr contains 'Could not reach'"
    )
}
