//! CLI-level integration tests for `jr issue attachment download`.
//!
//! RED GATE: all tests in this file FAIL because `handle_attachment_download`
//! contains `todo!()` — the spawned subprocess exits 101 (Rust panic) instead of
//! the expected exit codes and output.
//!
//! After Task 2 (sanitize_attachment_filename) + Task 4/5 (handler implementation),
//! all tests become GREEN.
//!
//! BC anchors: BC-2.7.007, BC-2.7.008, BC-2.7.009, BC-2.7.010, BC-2.7.011,
//!             BC-2.7.012
//! VPs: VP-576-001 (sanitize proptest lives in src/cli/issue/attachments.rs)
//! Story: S-576-2, GitHub issue #576

use assert_cmd::Command;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

/// Build a `jr` subprocess pointing at `server_uri` with full XDG/cache/config
/// isolation via per-test TempDirs. Callers supply all command-line flags.
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
// AC-001 — BC-2.7.007: two-step wire path; streaming; JRACLOUD-97046; GHSA-9857-6MW7-FQ2M
// ---------------------------------------------------------------------------

/// AC-001 / BC-2.7.007: `jr issue attachment download <KEY> --id <AID>` issues
/// step-1 metadata GET then step-2 content GET; no `?redirect=false`.
#[tokio::test]
async fn test_bc_2_7_007_two_step_streaming_wire_path() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10001",
            "filename": "test.txt",
            "size": 5,
            "mimeType": "text/plain",
            "content": format!("{}/rest/api/3/attachment/content/10001", server.uri())
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello"))
        .expect(1)
        .mount(&server)
        .await;

    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("test.txt");

    let output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10001",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: handler panics with todo!() → exit 101.
    // After implementation, assert exit_code == 0 and file exists.
    assert!(
        !output.status.success(),
        "RED GATE: expected failure (todo!() stub)"
    );
}

/// AC-001 / BC-2.7.007: content GET URL MUST NOT include `?redirect=false`.
#[tokio::test]
async fn test_bc_2_7_007_no_redirect_false_param() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let server = MockServer::start().await;

    // Mount a catch-all that accepts the content path only WITHOUT redirect=false.
    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/10002"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "10002", "filename": "f.bin", "size": 1,
            "mimeType": "application/octet-stream",
            "content": format!("{}/rest/api/3/attachment/content/10002", server.uri())
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/attachment/content/10002"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x"))
        .mount(&server)
        .await;

    let out_dir = TempDir::new().unwrap();
    let out_path = out_dir.path().join("f.bin");

    let _output = jr_cmd_with_xdg(&server.uri(), cache.path(), config.path())
        .args([
            "issue",
            "attachment",
            "download",
            "FOO-1",
            "--id",
            "10002",
            "--out",
            out_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // RED GATE: todo!() stub. After implementation, wiremock's request log must
    // NOT contain `redirect=false` query param on the content GET.
    // (Assert via server.received_requests() in implementation phase.)
    todo!("AC-001: verify no ?redirect=false in content GET URL — JRACLOUD-97046")
}

/// AC-001 / BC-2.7.007 / EC-2.7.007-3 / SEC-576-003 / GHSA-9857-6MW7-FQ2M:
/// Authorization header MUST be absent on the redirect-target request.
/// EC-2.7.007-3 DISTINCT-HOST mandate: uses 127.0.0.1 vs [::1] to avoid vacuous assertion.
#[tokio::test]
async fn test_bc_2_7_007_auth_absent_on_redirect_target() {
    todo!(
        "AC-001 / SEC-576-003: two wiremock servers (127.0.0.1 + [::1]); \
         assert Authorization header absent on [::1] redirect-target request. \
         EC-2.7.007-3 DISTINCT-HOST: same-host-different-port is vacuous."
    )
}

// ---------------------------------------------------------------------------
// AC-002 — BC-2.7.007 / P32-001: pre-flights before metadata GET
// ---------------------------------------------------------------------------

/// AC-002 / P32-001: local pre-flight checks fire BEFORE step-1 metadata GET.
#[tokio::test]
async fn test_bc_2_7_007_out_preflight_before_get_p32_001() {
    todo!(
        "AC-002: P32-001 — --out parent not-exist → exit 64 before HTTP; \
         wiremock expect(0) on metadata GET confirms no HTTP issued"
    )
}

// ---------------------------------------------------------------------------
// AC-003 — BC-2.7.007: selector required; AID validation
// ---------------------------------------------------------------------------

/// AC-003 / BC-2.7.007: non-numeric `--id` → exit 64; no selector → exit 2.
#[tokio::test]
async fn test_bc_2_7_007_selector_required_aid_validation() {
    todo!(
        "AC-003: no selector → exit 2 (clap required group); \
         --id not-a-number → exit 64 + stderr contains 'invalid attachment id:'"
    )
}

// ---------------------------------------------------------------------------
// AC-005 — BC-2.7.010: default output path SHA-1 prefix (batch)
// ---------------------------------------------------------------------------

/// AC-005 / BC-2.7.010: batch default path = `<sha1-of-id>_<sanitized-filename>`.
#[tokio::test]
async fn test_bc_2_7_010_default_path_sha1_prefix_batch() {
    todo!(
        "AC-005: --all path shape = 40-hex-sha1_sanitized-filename; \
         verify file created at expected path in TempDir"
    )
}

// ---------------------------------------------------------------------------
// AC-006 — BC-2.7.007 / P27-001 / P31-002: JSON manifest shape
// ---------------------------------------------------------------------------

/// AC-006 / P27-001 / P31-002: JSON manifest — filename=RAW, size=bytes-written.
#[tokio::test]
async fn test_bc_2_7_007_json_manifest_raw_filename_written_size_p27_p31() {
    todo!(
        "AC-006: --output json manifest; filename=RAW (pre-sanitization); \
         size=bytes-written (not metadata size); path=as-constructed (not canonicalized)"
    )
}

// ---------------------------------------------------------------------------
// AC-007 — BC-2.7.008: --all batch; fail-soft; out-dir checks; cwd default
// ---------------------------------------------------------------------------

/// AC-007 / BC-2.7.008: batch --all fail-soft; out-dir checks; partial/all-fail exit 1.
#[tokio::test]
async fn test_bc_2_7_008_all_batch_fail_soft() {
    todo!(
        "AC-007: sub-assertions — \
         (a) --out-dir not-exist → exit 64 + 'Output directory does not exist:'; \
         (b) --out-dir points at a file → exit 64 + 'Not a directory:'; \
         (c) EC-2.7.008-7 partial-fail: 1-of-2 content-GET fails → exit 1 + warning + \
             'Downloaded 1 of 2 attachments to'; \
         (d) EC-2.7.008-8 all-fail: all content-GETs fail → exit 1 + \
             'Downloaded 0 of 2 attachments to'"
    )
}

/// AC-007 / BC-2.7.008 ~785/791: --all without --out-dir downloads to cwd.
#[tokio::test]
async fn test_bc_2_7_008_all_no_out_dir_defaults_to_cwd() {
    todo!(
        "AC-007: --all without --out-dir → files land in cwd; \
         use TempDir as cwd (set process cwd before spawning)"
    )
}

/// AC-007 / EC-2.7.008-1 / EC-2.7.009-4: empty issue → hint + exit 0.
#[tokio::test]
async fn test_bc_2_7_008_empty_issue_no_attachments_hint() {
    todo!(
        "AC-007: sub-assertions — \
         (a) human mode --all on zero-attachment issue → exit 0 + \
             stderr contains 'No attachments on FOO-1.'; \
         (b) JSON mode --all on zero-attachment issue → exit 0 + \
             stdout = '{{\"downloaded\":[]}}' + NO stderr hint; \
         (c) EC-2.7.009-4: --newest N on zero-attachment issue → exit 0 + \
             stdout = '{{\"downloaded\":[]}}'"
    )
}

// ---------------------------------------------------------------------------
// AC-008 — BC-2.7.009: --newest N by created desc
// ---------------------------------------------------------------------------

/// AC-008 / BC-2.7.009: --newest N selects top-N by created descending.
#[tokio::test]
async fn test_bc_2_7_009_newest_n_by_created_desc() {
    todo!(
        "AC-008: 5-attachment issue; --newest 2; assert only the 2 most-recent \
         (by created field) are downloaded; order is descending"
    )
}

// ---------------------------------------------------------------------------
// AC-009 — BC-2.7.012: error taxonomy
// ---------------------------------------------------------------------------

/// AC-009 / BC-2.7.012: full error taxonomy — ALL rows as explicit sub-assertions.
#[tokio::test]
async fn test_bc_2_7_012_error_taxonomy() {
    todo!(
        "AC-009 sub-assertions (all explicit, no combined catch-all): \
         invalid AID (non-numeric) → exit 64 + 'invalid attachment id:'; \
         AID 404 (--id path) → exit 64 + 'Attachment' AND 'not found or not accessible'; \
         KEY 404 (--all path) → exit 64 + 'Issue' AND 'not found or not accessible'; \
         401 → exit 2 + 'Not authenticated' AND 'jr auth login'; \
         AID 403 (--id) → exit 1 + 'Permission denied: cannot access attachment '; \
         KEY 403 (--all/--newest) → exit 1 + 'Permission denied: cannot access issue '; \
         5xx single → exit 1 + 'API error ('; \
         network single → exit 1 + 'Could not reach'; \
         ENOSPC → exit 1 + 'Disk full: not enough space to write <path>'; \
         EACCES → exit 1 + 'Permission denied: cannot write to <dir>'; \
         other write error → exit 1 (unpinnable substring)"
    )
}

// ---------------------------------------------------------------------------
// AC-010 — BC-2.7.007 / EC-2.7.007-2 / JSDCLOUD-10841: platform content URL
// ---------------------------------------------------------------------------

/// AC-010 / EC-2.7.007-2: download ALWAYS uses platform content URL, not JSM links.
#[tokio::test]
async fn test_bc_2_7_007_uses_platform_content_url_not_jsm_links_ec_2_7_007_2() {
    todo!(
        "AC-010: download issues GET /rest/api/3/attachment/content/{{id}} regardless \
         of whether the issue is a JSM issue; JSDCLOUD-10841 — servicedeskapi \
         'links.content' URLs return 404 and MUST NOT be used"
    )
}

// ---------------------------------------------------------------------------
// AC-011 — BC-2.7.007: write-to-temp + atomic rename; cleanup on error
// ---------------------------------------------------------------------------

/// AC-011 / BC-2.7.007: atomic rename from temp file; cleanup on mid-stream error.
#[tokio::test]
async fn test_bc_2_7_007_atomic_rename_cleanup_on_error() {
    todo!(
        "AC-011: wiremock returns partial body then connection-reset; \
         assert final path absent (temp cleaned up); exit 1"
    )
}

/// AC-011 / BC-2.7.007 ~749: temp file naming — `tmp_<random>` in same dir as final path.
#[tokio::test]
async fn test_bc_2_7_007_temp_file_same_dir_tmp_random_prefix() {
    todo!(
        "AC-011: during download, assert temp file has 'tmp_' prefix (not '<basename>.tmp'); \
         assert temp file parent == final path parent (NOT OS temp dir — cross-fs rename); \
         EC-2.7.007-8 concurrent correctness rationale"
    )
}

// ---------------------------------------------------------------------------
// AC-012 — BC-2.7.008 / EC-2.7.008-6/7: JSON mode hint/error taxonomy
// ---------------------------------------------------------------------------

/// AC-012 / EC-2.7.008-6/7: JSON mode — per-file failures → stderr warning; exit 1.
#[tokio::test]
async fn test_bc_2_7_008_json_mode_error_vs_hint_taxonomy() {
    todo!(
        "AC-012: 2-attachment issue; one content-GET fails; --output json; \
         assert exit_code == 1 (not 0); \
         assert stderr contains 'warning: failed to download attachment <AID>:'; \
         assert stdout manifest contains ONLY the successful entry (no JSON error obj); \
         'Downloaded N of M' summary NOT emitted (hint, suppressed in JSON mode)"
    )
}

// ---------------------------------------------------------------------------
// AC-015 — BC-2.7.010 / SEC-576-011 / CWE-116: degenerate name warning
// ---------------------------------------------------------------------------

/// AC-015 / BC-2.7.010 SEC-576-011: degenerate name warning uses display_sanitize_filename.
#[tokio::test]
async fn test_bc_2_7_010_degenerate_name_warning_display_sanitized() {
    todo!(
        "AC-015: attachment whose filename sanitizes to None (degenerate); \
         human-mode stderr warning MUST contain display-sanitized form of raw name \
         (CWE-116; via display_sanitize_filename); JSON mode: no warning at all"
    )
}

// ---------------------------------------------------------------------------
// AC-016 — BC-2.7.011 / SEC-576-001: Windows device-name escape at single-id call site
// ---------------------------------------------------------------------------

/// AC-016 / SEC-576-001: Windows device name escape at single-id call site.
#[tokio::test]
async fn test_bc_2_7_011_windows_device_name_escape_single_id_call_site() {
    todo!(
        "AC-016: attachment filename 'CON' (sanitizes to Some('CON')); \
         single --id path; assert output file is '_CON' (prefixed with _); \
         batch path NOT escaped (SHA-1 prefix already disambiguates)"
    )
}

// ---------------------------------------------------------------------------
// AC-017 — BC-2.7.007/008/009: clap structural constraints + handler N-validation
// ---------------------------------------------------------------------------

/// AC-017 Layer 1: 10 clap exit-2 cases for download flag combinations.
#[test]
fn test_bc_2_7_download_clap_structural_constraints() {
    todo!(
        "AC-017 Layer 1 — 10 clap exit-2 cases: \
         (1) --id + --all → exit 2; \
         (2) --id + --newest 3 → exit 2; \
         (3) --all + --newest 3 → exit 2; \
         (4) --all + --out /tmp → exit 2; \
         (5) --newest 3 + --out /tmp → exit 2; \
         (6) no selector → exit 2; \
         (7) --newest foo (non-integer) → exit 2; \
         (8) --id + --filter k=v → exit 2; \
         (9) --out-dir /tmp (no batch selector) → exit 2; \
         (10) --out-dir /tmp --id AID → exit 2"
    )
}

/// AC-017 Layer 2: handler-level N-validation (N ≤ 0 → exit 64).
#[test]
fn test_bc_2_7_009_newest_nonpositive_exits_64() {
    todo!(
        "AC-017 Layer 2: \
         --newest 0 → exit 64 + '--newest requires a positive integer.' (EC-2.7.009-1); \
         --newest -3 → exit 64 + same message (allow_negative_numbers=true lets -3 reach handler)"
    )
}

// ---------------------------------------------------------------------------
// AC-018 — BC-2.7.007 ~747: single-id success hint to stderr
// ---------------------------------------------------------------------------

/// AC-018 / BC-2.7.007 ~747: single-id success hint emitted to stderr (not in JSON mode).
#[tokio::test]
async fn test_bc_2_7_007_single_id_success_hint_stderr() {
    todo!(
        "AC-018: successful single --id download; human mode: \
         stderr contains 'Downloaded: <path> (<size_human>).'; \
         JSON mode: no such hint (suppressed); manifest on stdout"
    )
}

// ---------------------------------------------------------------------------
// AC-019 — EC-2.7.008-10 / EC-2.7.009-3: filtered-to-zero hint
// ---------------------------------------------------------------------------

/// AC-019 / EC-2.7.008-10 / EC-2.7.009-3: filter eliminates all attachments → hint + exit 0.
#[tokio::test]
async fn test_bc_2_7_008_filtered_to_zero_hint() {
    todo!(
        "AC-019: issue has attachments; --filter mime=application/pdf matches none; \
         human mode: stderr 'No attachments matched the filter on FOO-1.'; \
         JSON mode: stdout '{{\"downloaded\":[]}}'; both exit 0. \
         Distinct from EC-2.7.008-1 (issue has zero attachments regardless of filter)"
    )
}
