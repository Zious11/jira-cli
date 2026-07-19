//! CLI-level integration tests for `jr issue attachment list`.
//!
//! RED GATE: all 14 tests FAIL because `jr issue attachment list` does not
//! exist yet — `AttachmentSubcommand` enum and dispatch arm are created in
//! Task 2 of S-576-1. Until Task 2 completes, every subprocess exits 2
//! (clap: unrecognised subcommand) instead of the expected exit codes.
//! After Task 2 adds `todo!()` stubs, subprocesses exit 101 (Rust panic).
//!
//! BC anchors: BC-2.7.001, BC-2.7.002, BC-2.7.003, BC-2.7.004, BC-2.7.005,
//!             BC-2.7.006, BC-2.7.011 (display-sanitization primary clause)
//! VPs: VP-576-004 (list half — BC-2.7.002 authority)
//! Story: S-576-1, GitHub issues #576 #585

use assert_cmd::Command;
use serde_json::Value;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ---------------------------------------------------------------------------
// Harness helper
// ---------------------------------------------------------------------------

/// Build a `jr` subprocess pointing at `server_uri` with full XDG/cache/config
/// isolation via per-test TempDirs. Callers supply all command-line flags.
fn jr_cmd(server_uri: &str, cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Build a single Jira attachment object as returned in `fields.attachment[]`.
///
/// `display_name = None` → `author.displayName` is null.
/// `account_id   = None` → `author.accountId` is null.
/// Both `None`           → `"author": null` (no author object).
fn make_attachment(
    id: &str,
    filename: &str,
    mime: &str,
    size: u64,
    display_name: Option<&str>,
    account_id: Option<&str>,
) -> Value {
    let author = match (display_name, account_id) {
        (None, None) => Value::Null,
        (dn, aid) => serde_json::json!({
            "accountId": aid,
            "displayName": dn,
        }),
    };
    serde_json::json!({
        "id": id,
        "filename": filename,
        "mimeType": mime,
        "size": size,
        "created": "2026-07-10T14:23:11.000+0000",
        "author": author,
        "self": format!("https://example.atlassian.net/rest/api/3/attachment/{id}"),
        "content": format!("https://example.atlassian.net/rest/api/3/attachment/content/{id}"),
    })
}

/// Build the full `GET /rest/api/3/issue/{key}?fields=attachment` response body.
fn issue_attachment_response(key: &str, attachments: Vec<Value>) -> Value {
    serde_json::json!({
        "key": key,
        "fields": {
            "attachment": attachments
        }
    })
}

// ---------------------------------------------------------------------------
// AC-001 / BC-2.7.001 — table six columns in display order; display-sanitization
// ---------------------------------------------------------------------------

/// Verify that `jr issue attachment list FOO-1` renders a comfy-table with
/// the six BC-2.7.001 columns in order: ID | Filename | Type | Size | Created
/// | Author. Also checks:
/// - Human-readable size (43008 bytes → `42.0 KB`).
/// - Full author fallback chain: displayName → accountId → "(anonymous)".
/// - `display_sanitize_filename` replaces extended-set chars with `?` in the
///   Filename cell (BC-2.7.011): U+202E (RLO bidi), U+2028 (LINE SEPARATOR),
///   and \0 (null byte) each become `?`.
/// - "Thumbnail" column NOT present.
///
/// RED GATE: exits 2 (clap unknown subcommand) → `exit code == Some(0)` fails.
#[tokio::test]
async fn test_bc_2_7_001_table_six_columns_order() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Poisoned filename: U+202E (RLO bidi) + U+2028 (line sep) + \0 (null byte).
    // Each extended-set char → '?' after display_sanitize_filename.
    let bidi_filename = "safe\u{202E}rlo\u{2028}sep\x00nul.txt";
    let sanitized_filename = "safe?rlo?sep?nul.txt";

    let attachments = vec![
        // 43008 bytes = exactly 42.0 KB; displayName author
        make_attachment(
            "10001",
            "screenshot.png",
            "image/png",
            43008,
            Some("Alice Operator"),
            Some("acct-001"),
        ),
        // accountId-only author (displayName = null)
        make_attachment(
            "10002",
            "report.pdf",
            "application/pdf",
            1024,
            None,
            Some("acct-no-displayname"),
        ),
        // no author → "(anonymous)" in table
        make_attachment("10003", "data.csv", "text/csv", 512, None, None),
        // filename with bidi/control chars → sanitized in Filename cell
        make_attachment(
            "10004",
            bidi_filename,
            "text/plain",
            100,
            Some("Mallory"),
            Some("acct-mallory"),
        ),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_attachment_response("FOO-1", attachments)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "attachment", "list", "FOO-1"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-2.7.001: must exit 0 on success; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    // Column headers present and in BC-mandated display order
    let id_pos = stdout
        .find("ID")
        .expect("BC-2.7.001: 'ID' column header not found in stdout");
    let filename_pos = stdout
        .find("Filename")
        .expect("BC-2.7.001: 'Filename' column header not found in stdout");
    let type_pos = stdout
        .find("Type")
        .expect("BC-2.7.001: 'Type' column header not found in stdout");
    let size_pos = stdout
        .find("Size")
        .expect("BC-2.7.001: 'Size' column header not found in stdout");
    let created_pos = stdout
        .find("Created")
        .expect("BC-2.7.001: 'Created' column header not found in stdout");
    let author_pos = stdout
        .find("Author")
        .expect("BC-2.7.001: 'Author' column header not found in stdout");

    assert!(
        id_pos < filename_pos,
        "BC-2.7.001: 'ID' must appear before 'Filename'; stdout: {stdout}"
    );
    assert!(
        filename_pos < type_pos,
        "BC-2.7.001: 'Filename' must appear before 'Type'; stdout: {stdout}"
    );
    assert!(
        type_pos < size_pos,
        "BC-2.7.001: 'Type' must appear before 'Size'; stdout: {stdout}"
    );
    assert!(
        size_pos < created_pos,
        "BC-2.7.001: 'Size' must appear before 'Created'; stdout: {stdout}"
    );
    assert!(
        created_pos < author_pos,
        "BC-2.7.001: 'Created' must appear before 'Author'; stdout: {stdout}"
    );

    // Thumbnail must NOT appear in table
    assert!(
        !stdout.to_lowercase().contains("thumbnail"),
        "BC-2.7.001: 'Thumbnail' column must NOT appear in table output; stdout: {stdout}"
    );

    // Human-readable size: 43008 bytes = 42.0 KB
    assert!(
        stdout.contains("42.0 KB"),
        "BC-2.7.001: 43008 bytes must render as '42.0 KB'; stdout: {stdout}"
    );

    // Author fallback chain
    assert!(
        stdout.contains("Alice Operator"),
        "BC-2.7.001: displayName 'Alice Operator' must appear in Author column; stdout: {stdout}"
    );
    assert!(
        stdout.contains("acct-no-displayname"),
        "BC-2.7.001: accountId 'acct-no-displayname' must appear when displayName is null; \
         stdout: {stdout}"
    );
    assert!(
        stdout.contains("(anonymous)"),
        "BC-2.7.001 EC-2.7.001-3: '(anonymous)' must appear when author is null; \
         stdout: {stdout}"
    );

    // BC-2.7.011 display-sanitization: extended-set chars → '?'
    assert!(
        stdout.contains(sanitized_filename),
        "BC-2.7.011: sanitized filename '{}' must appear in Filename cell; stdout: {stdout}",
        sanitized_filename
    );
    assert!(
        !stdout.contains('\u{202E}'),
        "BC-2.7.011: raw U+202E (RLO bidi) MUST NOT appear in stdout; stdout: {stdout}"
    );
    assert!(
        !stdout.contains('\u{2028}'),
        "BC-2.7.011: raw U+2028 (LINE SEPARATOR) MUST NOT appear in stdout; stdout: {stdout}"
    );
    assert!(
        !stdout.contains('\x00'),
        "BC-2.7.011: raw null byte MUST NOT appear in stdout; stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-002 / BC-2.7.001 EC-2.7.001-1 — zero attachments: empty stdout + stderr hint
// ---------------------------------------------------------------------------

/// Verify zero-attachment behavior:
///   Human mode: exit 0, empty stdout (pipe-friendly), stderr contains
///               `"No attachments on FOO-1."`.
///   JSON mode:  exit 0, stdout `[]`, stderr empty.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_001_zero_attachments_empty_stdout_stderr_hint() {
    // --- Sub-assertion 1: human mode ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", vec![])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args(["issue", "attachment", "list", "FOO-1"])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "EC-2.7.001-1 human: must exit 0 on zero attachments; \
             got {:?}\nstderr: {stderr}\nstdout: {stdout}",
            output.status.code()
        );
        assert!(
            stdout.trim().is_empty(),
            "EC-2.7.001-1 human: stdout must be empty (pipe-friendly, no table); \
             got: {stdout}"
        );
        assert!(
            stderr.contains("No attachments on FOO-1."),
            "EC-2.7.001-1 human: stderr must contain 'No attachments on FOO-1.'; \
             got: {stderr}"
        );
    }

    // --- Sub-assertion 2: JSON mode ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", vec![])),
            )
            .expect(1)
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args(["issue", "attachment", "list", "FOO-1", "--output", "json"])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "EC-2.7.001-1 JSON: must exit 0 on zero attachments; \
             got {:?}\nstderr: {stderr}\nstdout: {stdout}",
            output.status.code()
        );

        let parsed: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "EC-2.7.001-1 JSON: stdout must be valid JSON; \
                 error: {e}\nstdout: {stdout}"
            )
        });
        assert_eq!(
            parsed,
            Value::Array(vec![]),
            "EC-2.7.001-1 JSON: stdout must be empty array `[]`; got: {stdout}"
        );
        assert!(
            stderr.trim().is_empty(),
            "EC-2.7.001-1 JSON: stderr must be empty in JSON mode \
             (empty array `[]` is self-describing; hint suppressed per EC-2.7.001-1); \
             got: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-003 (1/2) / BC-2.7.002 / VP-576-004 list half — curated JSON shape
// ---------------------------------------------------------------------------

/// Verify that `attachment list --output json` returns a pretty-printed JSON
/// array where each element has EXACTLY the keys
/// {author, contentUrl, created, filename, id, mimeType, size} in BTreeMap
/// alphabetical order, with:
/// - "self"    OMITTED (VP-576-004 assertion 1).
/// - "content" renamed to "contentUrl" (VP-576-004 assertion 2).
/// - `size` is a raw u64 integer.
/// - `author` is `null` when the API response has no author.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_002_json_shape_curated_form() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let attachments = vec![
        // Has all fields incl. "self" and "content" in the fixture
        make_attachment(
            "10042",
            "screenshot.png",
            "image/png",
            43008,
            Some("Alice Operator"),
            Some("acct-001"),
        ),
        // Null author → JSON must emit "author": null
        make_attachment("10043", "orphan.pdf", "application/pdf", 2048, None, None),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_attachment_response("FOO-1", attachments)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "attachment", "list", "FOO-1", "--output", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-2.7.002: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    let parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-2.7.002: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });
    let arr = parsed
        .as_array()
        .expect("BC-2.7.002: JSON output must be an array");
    assert_eq!(
        arr.len(),
        2,
        "BC-2.7.002: expected 2 elements in JSON array"
    );

    let expected_keys: std::collections::BTreeSet<&str> = [
        "author",
        "contentUrl",
        "created",
        "filename",
        "id",
        "mimeType",
        "size",
    ]
    .iter()
    .copied()
    .collect();

    for (i, elem) in arr.iter().enumerate() {
        let obj = elem
            .as_object()
            .unwrap_or_else(|| panic!("BC-2.7.002: element {i} must be a JSON object"));

        let actual_keys: std::collections::BTreeSet<&str> =
            obj.keys().map(|k| k.as_str()).collect();

        // VP-576-004 assertion 1: "self" must be omitted
        assert!(
            !actual_keys.contains("self"),
            "VP-576-004: element {i} MUST NOT contain 'self' key; got keys: {actual_keys:?}"
        );

        // VP-576-004 assertion 2: "content" renamed → "contentUrl"
        assert!(
            !actual_keys.contains("content"),
            "VP-576-004: element {i} MUST NOT contain 'content' key \
             (must be renamed to 'contentUrl'); got keys: {actual_keys:?}"
        );
        assert!(
            actual_keys.contains("contentUrl"),
            "VP-576-004: element {i} MUST contain 'contentUrl' key; \
             got keys: {actual_keys:?}"
        );

        // Exact key set (BTreeMap-alphabetical per P19-001)
        assert_eq!(
            actual_keys, expected_keys,
            "BC-2.7.002: element {i} must have exactly keys \
             {{author,contentUrl,created,filename,id,mimeType,size}}; \
             got: {actual_keys:?}"
        );

        // size must be a raw integer (u64), not a string
        assert!(
            obj["size"].is_number(),
            "BC-2.7.002: 'size' must be a raw integer (u64), not a string; \
             element {i}: {}",
            obj["size"]
        );
    }

    // Second attachment has null author → JSON emits "author": null
    assert_eq!(
        arr[1]["author"],
        Value::Null,
        "BC-2.7.002: author must be JSON null when API response has no author; \
         got: {}",
        arr[1]["author"]
    );

    // --- (a) + (d): full-author fixture — extra Jira fields stripped; accountId < displayName order ---
    //
    // Jira API author sub-object includes fields beyond {accountId, displayName}:
    // self, avatarUrls, accountType, timeZone. BC-2.7.002 v1.3.95 P1-002 ruling:
    // emitted "author" must contain ONLY {accountId, displayName}; all other
    // Jira author fields must be stripped. BTreeMap ordering (d): accountId < displayName.
    {
        let server_fa = MockServer::start().await;
        let cache_fa = tempfile::tempdir().unwrap();
        let config_fa = tempfile::tempdir().unwrap();

        let full_author_attach = serde_json::json!({
            "id": "10099",
            "filename": "full-author.png",
            "mimeType": "image/png",
            "size": 1024,
            "created": "2026-07-10T14:23:11.000+0000",
            "author": {
                "accountId": "acct-full",
                "displayName": "Full Author",
                "self": "https://example.atlassian.net/rest/api/3/user?accountId=acct-full",
                "avatarUrls": {"48x48": "https://example.atlassian.net/avatar/acct-full"},
                "accountType": "atlassian",
                "timeZone": "America/New_York"
            },
            "self": "https://example.atlassian.net/rest/api/3/attachment/10099",
            "content": "https://example.atlassian.net/rest/api/3/attachment/content/10099"
        });

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", vec![full_author_attach])),
            )
            .expect(1)
            .mount(&server_fa)
            .await;

        let out_fa = jr_cmd(&server_fa.uri(), cache_fa.path(), config_fa.path())
            .args(["issue", "attachment", "list", "FOO-1", "--output", "json"])
            .output()
            .unwrap();

        let stdout_fa = String::from_utf8_lossy(&out_fa.stdout);
        let stderr_fa = String::from_utf8_lossy(&out_fa.stderr);

        assert_eq!(
            out_fa.status.code(),
            Some(0),
            "BC-2.7.002 (a) full-author: must exit 0; stderr: {stderr_fa}\nstdout: {stdout_fa}"
        );

        let arr_fa = serde_json::from_str::<Value>(&stdout_fa)
            .expect("BC-2.7.002 (a) full-author: must be valid JSON")
            .as_array()
            .expect("must be array")
            .clone();
        assert_eq!(
            arr_fa.len(),
            1,
            "BC-2.7.002 (a) full-author: expected 1 element; stdout: {stdout_fa}"
        );

        let author_fa = arr_fa[0]["author"]
            .as_object()
            .expect("BC-2.7.002 (a) full-author: 'author' must be an object when present");

        // Exact key set: ONLY {accountId, displayName}.
        // Nested self/avatarUrls/accountType/timeZone from Jira response MUST NOT appear.
        let author_keys_fa: std::collections::BTreeSet<&str> =
            author_fa.keys().map(|k| k.as_str()).collect();
        let expected_author_keys_fa: std::collections::BTreeSet<&str> =
            ["accountId", "displayName"].iter().copied().collect();
        assert_eq!(
            author_keys_fa, expected_author_keys_fa,
            "BC-2.7.002 (a) P1-002: author must have EXACTLY {{accountId, displayName}}; \
             nested self/avatarUrls/accountType/timeZone MUST NOT appear; \
             got keys: {:?}\nstdout: {stdout_fa}",
            author_keys_fa
        );

        // (d) BTreeMap key ordering within author: accountId < displayName (alphabetical).
        let acct_pos = stdout_fa
            .find("\"accountId\"")
            .expect("BC-2.7.002 (d): 'accountId' key not found in stdout");
        let disp_pos = stdout_fa
            .find("\"displayName\"")
            .expect("BC-2.7.002 (d): 'displayName' key not found in stdout");
        assert!(
            acct_pos < disp_pos,
            "BC-2.7.002 (d) BTreeMap: 'accountId' must appear before 'displayName' in the \
             serialized JSON (BTreeMap alphabetical order); \
             acct_pos={acct_pos}, disp_pos={disp_pos}\nstdout: {stdout_fa}"
        );
    }

    // --- (b): partial-author — author present, both subfields null → curated {accountId:null, displayName:null} ---
    //
    // BC-2.7.002 v1.3.95: when Jira returns an author object with null sub-fields,
    // emit "author": {"accountId": null, "displayName": null} (the curated two-field
    // form), NOT "author": null (top-level null is reserved for fully absent author).
    {
        let server_pa = MockServer::start().await;
        let cache_pa = tempfile::tempdir().unwrap();
        let config_pa = tempfile::tempdir().unwrap();

        let partial_author_attach = serde_json::json!({
            "id": "10098",
            "filename": "partial-author.pdf",
            "mimeType": "application/pdf",
            "size": 512,
            "created": "2026-07-10T14:23:11.000+0000",
            "author": {
                "accountId": null,
                "displayName": null
            },
            "self": "https://example.atlassian.net/rest/api/3/attachment/10098",
            "content": "https://example.atlassian.net/rest/api/3/attachment/content/10098"
        });

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(issue_attachment_response(
                    "FOO-1",
                    vec![partial_author_attach],
                )),
            )
            .expect(1)
            .mount(&server_pa)
            .await;

        let out_pa = jr_cmd(&server_pa.uri(), cache_pa.path(), config_pa.path())
            .args(["issue", "attachment", "list", "FOO-1", "--output", "json"])
            .output()
            .unwrap();

        let stdout_pa = String::from_utf8_lossy(&out_pa.stdout);
        let stderr_pa = String::from_utf8_lossy(&out_pa.stderr);

        assert_eq!(
            out_pa.status.code(),
            Some(0),
            "BC-2.7.002 (b) partial-author: must exit 0; \
             stderr: {stderr_pa}\nstdout: {stdout_pa}"
        );

        let arr_pa = serde_json::from_str::<Value>(&stdout_pa)
            .expect("BC-2.7.002 (b) partial-author: must be valid JSON")
            .as_array()
            .expect("must be array")
            .clone();
        assert_eq!(
            arr_pa.len(),
            1,
            "BC-2.7.002 (b) partial-author: expected 1 element; stdout: {stdout_pa}"
        );

        let author_pa = &arr_pa[0]["author"];

        // Must NOT be top-level null; that is reserved for fully absent author.
        assert!(
            !author_pa.is_null(),
            "BC-2.7.002 (b) P1-002 partial-author: 'author' must be the curated two-field \
             object {{\"accountId\": null, \"displayName\": null}}, NOT top-level null; \
             top-level null is reserved for Jira API returning null for 'author'; \
             here the author object is present with null sub-fields; \
             got: {author_pa}\nstdout: {stdout_pa}"
        );

        let author_obj_pa = author_pa
            .as_object()
            .expect("BC-2.7.002 (b) partial-author: 'author' must be a JSON object");
        assert_eq!(
            author_obj_pa.get("accountId"),
            Some(&Value::Null),
            "BC-2.7.002 (b) partial-author: accountId must be null; stdout: {stdout_pa}"
        );
        assert_eq!(
            author_obj_pa.get("displayName"),
            Some(&Value::Null),
            "BC-2.7.002 (b) partial-author: displayName must be null; stdout: {stdout_pa}"
        );
        let author_keys_pa: std::collections::BTreeSet<&str> =
            author_obj_pa.keys().map(|k| k.as_str()).collect();
        let expected_pa_keys: std::collections::BTreeSet<&str> =
            ["accountId", "displayName"].iter().copied().collect();
        assert_eq!(
            author_keys_pa, expected_pa_keys,
            "BC-2.7.002 (b) partial-author: 'author' must have exactly \
             {{accountId, displayName}}; got: {:?}\nstdout: {stdout_pa}",
            author_keys_pa
        );
    }
}

// ---------------------------------------------------------------------------
// AC-003 (2/2) / BC-2.7.002 — #526 invariant: output::render_json not to_string_pretty
// ---------------------------------------------------------------------------

/// Verify that `--output json` output is pretty-printed (consistent with
/// `output::render_json` — JSON render invariant #526). Structural signals:
/// - Valid JSON.
/// - Output starts with `[` (array).
/// - Contains newlines (not compact single-line).
/// - Contains 2-space indentation (render_json convention).
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_002_json_uses_render_json_not_string_pretty() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let attachments = vec![make_attachment(
        "10042",
        "file.png",
        "image/png",
        1024,
        Some("Alice"),
        Some("acct-001"),
    )];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_attachment_response("FOO-1", attachments)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "attachment", "list", "FOO-1", "--output", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-2.7.002 #526: must exit 0; got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    // Must be valid JSON
    let _parsed: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("BC-2.7.002 #526: stdout must be valid JSON; error: {e}\nstdout: {stdout}")
    });

    // Must start with '[' (array output)
    assert!(
        stdout.trim_start().starts_with('['),
        "BC-2.7.002 #526: output must be a JSON array starting with '['; stdout: {stdout}"
    );

    // Pretty-printed: must contain newlines (compact would be one line)
    assert!(
        stdout.contains('\n'),
        "BC-2.7.002 #526: output must be pretty-printed (newlines present); \
         compact output forbidden by render_json invariant #526; stdout: {stdout}"
    );

    // 2-space indentation consistent with output::render_json
    assert!(
        stdout.contains("  \""),
        "BC-2.7.002 #526: output must have 2-space indentation (output::render_json); \
         stdout: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// AC-004 / BC-2.7.001 EC-2.7.001-2 — filter-count hint: deliberate asymmetry
// ---------------------------------------------------------------------------

/// Verify the filter-count hint (EC-2.7.001-2) with all three sub-cases:
/// (a) `--output json` + reducing filter (N < M) → filtered JSON stdout AND
///     stderr `"Showing N of M attachments."` (hint fires in JSON mode —
///     deliberate asymmetry with EC-2.7.001-1 zero-attachment suppression).
/// (b) N == M (filter active, no exclusions) → NO hint in either mode.
/// (c) Human mode + reducing filter (N < M) → hint on stderr.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_001_filter_count_hint_fires_when_reduced() {
    // Fixture: 2 attachments (1 image, 1 PDF)
    let mixed_two: Vec<Value> = vec![
        make_attachment(
            "10001",
            "photo.png",
            "image/png",
            1024,
            Some("Alice"),
            Some("acct-001"),
        ),
        make_attachment(
            "10002",
            "report.pdf",
            "application/pdf",
            2048,
            Some("Bob"),
            Some("acct-002"),
        ),
    ];

    // --- (a) JSON mode + reducing filter: hint fires on stderr ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", mixed_two.clone())),
            )
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "mime=image/*",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "EC-2.7.001-2 (a): must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        let arr = serde_json::from_str::<Value>(&stdout)
            .expect("(a): stdout must be valid JSON")
            .as_array()
            .expect("(a): output must be array")
            .clone();
        assert_eq!(
            arr.len(),
            1,
            "EC-2.7.001-2 (a): filtered JSON array must have 1 element (only image/*); \
             stdout: {stdout}"
        );

        assert!(
            stderr.contains("Showing 1 of 2 attachments."),
            "EC-2.7.001-2 (a): stderr must contain 'Showing 1 of 2 attachments.' \
             even in JSON mode (deliberate asymmetry with EC-2.7.001-1); got: {stderr}"
        );
    }

    // --- (b) N == M (filter active, no rows excluded): NO hint ---
    {
        // Both attachments are images → filter mime=image/* → N=2, M=2 → no hint
        let two_images: Vec<Value> = vec![
            make_attachment(
                "10001",
                "photo.png",
                "image/png",
                1024,
                Some("Alice"),
                Some("acct-001"),
            ),
            make_attachment(
                "10002",
                "photo.jpg",
                "image/jpeg",
                2048,
                Some("Bob"),
                Some("acct-002"),
            ),
        ];

        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", two_images)),
            )
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "mime=image/*",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "EC-2.7.001-2 (b) N==M: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        assert!(
            !stderr.contains("Showing"),
            "EC-2.7.001-2 (b) N==M clause: hint must NOT fire when displayed count \
             equals total count (N==M); got stderr: {stderr}"
        );
    }

    // --- (c) Human mode + reducing filter: hint fires ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", mixed_two.clone())),
            )
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "mime=image/*",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);

        assert_eq!(
            output.status.code(),
            Some(0),
            "EC-2.7.001-2 (c) human: must exit 0; got {:?}\nstderr: {stderr}",
            output.status.code()
        );

        assert!(
            stderr.contains("Showing 1 of 2 attachments."),
            "EC-2.7.001-2 (c): human mode must emit 'Showing 1 of 2 attachments.' \
             when filter reduces N < M; got: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-005 / BC-2.7.003 — mime glob filter; wildcard crosses "/"; case-insensitive
// ---------------------------------------------------------------------------

/// Verify `--filter mime=image/*` retains only image/* attachments.
/// `*` matches any sequence including `/` (crosses the subtype boundary).
/// Matching is case-insensitive: `IMAGE/*` must behave identically to `image/*`.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_003_mime_filter_image_wildcard() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let attachments = vec![
        make_attachment(
            "10001",
            "photo.png",
            "image/png",
            1024,
            Some("Alice"),
            Some("acct-001"),
        ),
        make_attachment(
            "10002",
            "photo.jpg",
            "image/jpeg",
            2048,
            Some("Alice"),
            Some("acct-001"),
        ),
        make_attachment(
            "10003",
            "report.pdf",
            "application/pdf",
            4096,
            Some("Bob"),
            Some("acct-002"),
        ),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_attachment_response("FOO-1", attachments)),
        )
        .mount(&server)
        .await;

    // --- lowercase wildcard ---
    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "attachment",
            "list",
            "FOO-1",
            "--filter",
            "mime=image/*",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "BC-2.7.003: must exit 0; got {:?}\nstderr: {stderr}",
        output.status.code()
    );

    let arr = serde_json::from_str::<Value>(&stdout)
        .expect("BC-2.7.003: must be valid JSON")
        .as_array()
        .expect("BC-2.7.003: must be array")
        .clone();

    assert_eq!(
        arr.len(),
        2,
        "BC-2.7.003: mime=image/* must return 2 images (not the PDF); stdout: {stdout}"
    );

    for (i, elem) in arr.iter().enumerate() {
        let mime = elem["mimeType"].as_str().unwrap_or_default();
        assert!(
            mime.starts_with("image/"),
            "BC-2.7.003: element {i} must have image/* mimeType; got: {mime}"
        );
    }
    assert!(
        !stdout.contains("application/pdf"),
        "BC-2.7.003: PDF must be filtered out by mime=image/*; stdout: {stdout}"
    );

    // --- case-insensitive: uppercase filter must work identically ---
    let server2 = MockServer::start().await;
    let cache_dir2 = tempfile::tempdir().unwrap();
    let config_dir2 = tempfile::tempdir().unwrap();

    let attachments2 = vec![
        make_attachment(
            "10001",
            "photo.png",
            "image/png",
            1024,
            Some("Alice"),
            Some("acct-001"),
        ),
        make_attachment(
            "10002",
            "report.pdf",
            "application/pdf",
            4096,
            Some("Bob"),
            Some("acct-002"),
        ),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_attachment_response("FOO-1", attachments2)),
        )
        .mount(&server2)
        .await;

    let output2 = jr_cmd(&server2.uri(), cache_dir2.path(), config_dir2.path())
        .args([
            "issue",
            "attachment",
            "list",
            "FOO-1",
            "--filter",
            "mime=IMAGE/*",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    let arr2 = serde_json::from_str::<Value>(&stdout2)
        .unwrap_or_else(|e| panic!("BC-2.7.003 uppercase: invalid JSON: {e}\nstdout: {stdout2}"))
        .as_array()
        .expect("must be array")
        .clone();
    assert_eq!(
        arr2.len(),
        1,
        "BC-2.7.003: mime=IMAGE/* (uppercase) must match image/png (case-insensitive); \
         stdout: {stdout2}"
    );

    // --- (b) star-crosses-slash: mime=image* (no slash in pattern) MUST match image/png ---
    // Proves '*' crosses the '/' boundary between media-type and subtype (BC-2.7.003 glob semantics).
    // This sub-assertion MAY already pass (it's a pin — the current impl's '*' already crosses '/').
    {
        let server_sc = MockServer::start().await;
        let cache_sc = tempfile::tempdir().unwrap();
        let config_sc = tempfile::tempdir().unwrap();

        let attachments_sc = vec![
            make_attachment(
                "20001",
                "photo.png",
                "image/png",
                1024,
                Some("A"),
                Some("a"),
            ),
            make_attachment(
                "20002",
                "doc.pdf",
                "application/pdf",
                2048,
                Some("B"),
                Some("b"),
            ),
        ];

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", attachments_sc)),
            )
            .mount(&server_sc)
            .await;

        let out_sc = jr_cmd(&server_sc.uri(), cache_sc.path(), config_sc.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "mime=image*",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let s_sc = String::from_utf8_lossy(&out_sc.stdout);
        assert_eq!(
            out_sc.status.code(),
            Some(0),
            "BC-2.7.003 (b) star-crosses-slash: must exit 0; got {:?}",
            out_sc.status.code()
        );
        let arr_sc = serde_json::from_str::<Value>(&s_sc)
            .expect("BC-2.7.003 (b) star-crosses-slash: must be valid JSON")
            .as_array()
            .expect("must be array")
            .clone();
        // mime=image* (no slash) must match image/png — '*' crosses '/' — and exclude application/pdf.
        assert_eq!(
            arr_sc.len(),
            1,
            "BC-2.7.003 (b) star-crosses-slash: mime=image* must match image/png \
             (proves '*' crosses '/'); expected 1 result, got {}; stdout: {s_sc}",
            arr_sc.len()
        );
        assert_eq!(
            arr_sc[0]["mimeType"], "image/png",
            "BC-2.7.003 (b) star-crosses-slash: matched element must be image/png; stdout: {s_sc}"
        );
        assert!(
            !s_sc.contains("application/pdf"),
            "BC-2.7.003 (b) star-crosses-slash: application/pdf must NOT match mime=image*; \
             stdout: {s_sc}"
        );
    }

    // --- (a) ? wildcard: mime=image/pn? matches image/png but NOT image/jpeg ---
    // BC-2.7.003: '?' matches any single character. image/pn? matches image/png (last char 'g')
    // but not image/jpeg (multiple chars after 'image/').
    {
        let server_q = MockServer::start().await;
        let cache_q = tempfile::tempdir().unwrap();
        let config_q = tempfile::tempdir().unwrap();

        let attachments_q = vec![
            make_attachment(
                "30001",
                "photo.png",
                "image/png",
                1024,
                Some("A"),
                Some("a"),
            ),
            make_attachment(
                "30002",
                "photo.jpg",
                "image/jpeg",
                2048,
                Some("B"),
                Some("b"),
            ),
        ];

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", attachments_q)),
            )
            .mount(&server_q)
            .await;

        let out_q = jr_cmd(&server_q.uri(), cache_q.path(), config_q.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "mime=image/pn?",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let s_q = String::from_utf8_lossy(&out_q.stdout);
        assert_eq!(
            out_q.status.code(),
            Some(0),
            "BC-2.7.003 (a) ? wildcard: must exit 0; got {:?}",
            out_q.status.code()
        );
        let arr_q = serde_json::from_str::<Value>(&s_q)
            .expect("BC-2.7.003 (a) ? wildcard: must be valid JSON")
            .as_array()
            .expect("must be array")
            .clone();
        // mime=image/pn? must match image/png (one char 'g') and NOT image/jpeg.
        assert_eq!(
            arr_q.len(),
            1,
            "BC-2.7.003 (a) ? glob: mime=image/pn? must match image/png (1 result) \
             and NOT image/jpeg; got {} results; stdout: {s_q}",
            arr_q.len()
        );
        assert_eq!(
            arr_q[0]["mimeType"], "image/png",
            "BC-2.7.003 (a) ? glob: matched element must be image/png; stdout: {s_q}"
        );
        assert!(
            !s_q.contains("image/jpeg"),
            "BC-2.7.003 (a) ? glob: image/jpeg must NOT match mime=image/pn?; stdout: {s_q}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-006 / BC-2.7.004 — name glob filter; AND composition; same-filename no-dedup
// ---------------------------------------------------------------------------

/// Verify:
/// - `--filter name=<glob>` retains only matching filenames (case-insensitive).
/// - Multiple `--filter` flags combine with AND semantics.
/// - Same-filename attachments are ALL returned (no dedup — JRACLOUD-96384).
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_004_name_filter_glob_and_composition() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // Fixtures: 3 distinct names + 2 with the same filename "dupe.txt"
    let attachments = vec![
        make_attachment(
            "10001",
            "screenshot.png",
            "image/png",
            1024,
            Some("A"),
            Some("a"),
        ),
        make_attachment(
            "10002",
            "screenshot.jpg",
            "image/jpeg",
            2048,
            Some("A"),
            Some("a"),
        ),
        make_attachment(
            "10003",
            "report.pdf",
            "application/pdf",
            4096,
            Some("B"),
            Some("b"),
        ),
        make_attachment("10004", "dupe.txt", "text/plain", 100, Some("C"), Some("c")),
        make_attachment("10005", "dupe.txt", "text/plain", 200, Some("C"), Some("c")), // same filename, different ID
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_attachment_response("FOO-1", attachments)),
        )
        .mount(&server)
        .await;

    // --- (1) name glob: screenshot* matches screenshot.png + screenshot.jpg ---
    let out1 = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "attachment",
            "list",
            "FOO-1",
            "--filter",
            "name=screenshot*",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let s1 = String::from_utf8_lossy(&out1.stdout);
    assert_eq!(
        out1.status.code(),
        Some(0),
        "BC-2.7.004 (1): must exit 0; got {:?}",
        out1.status.code()
    );
    let arr1 = serde_json::from_str::<Value>(&s1)
        .expect("BC-2.7.004 (1): must be valid JSON")
        .as_array()
        .expect("must be array")
        .clone();
    assert_eq!(
        arr1.len(),
        2,
        "BC-2.7.004 (1): name=screenshot* must return 2 files (png + jpg); stdout: {s1}"
    );

    // --- (2) AND composition: name=screenshot* AND mime=image/png → 1 result ---
    let out2 = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "attachment",
            "list",
            "FOO-1",
            "--filter",
            "name=screenshot*",
            "--filter",
            "mime=image/png",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let s2 = String::from_utf8_lossy(&out2.stdout);
    assert_eq!(
        out2.status.code(),
        Some(0),
        "BC-2.7.004 (2): must exit 0; got {:?}",
        out2.status.code()
    );
    let arr2 = serde_json::from_str::<Value>(&s2)
        .expect("BC-2.7.004 (2): must be valid JSON")
        .as_array()
        .expect("must be array")
        .clone();
    assert_eq!(
        arr2.len(),
        1,
        "BC-2.7.004 (2): AND semantics must return only screenshot.png; stdout: {s2}"
    );
    assert_eq!(
        arr2[0]["mimeType"], "image/png",
        "BC-2.7.004 (2): AND result must be image/png"
    );

    // --- (3) same filename: dupe.txt → both returned (no dedup, JRACLOUD-96384) ---
    let out3 = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "attachment",
            "list",
            "FOO-1",
            "--filter",
            "name=dupe.txt",
            "--output",
            "json",
        ])
        .output()
        .unwrap();
    let s3 = String::from_utf8_lossy(&out3.stdout);
    assert_eq!(
        out3.status.code(),
        Some(0),
        "BC-2.7.004 (3): must exit 0; got {:?}",
        out3.status.code()
    );
    let arr3 = serde_json::from_str::<Value>(&s3)
        .expect("BC-2.7.004 (3): must be valid JSON")
        .as_array()
        .expect("must be array")
        .clone();
    assert_eq!(
        arr3.len(),
        2,
        "BC-2.7.004 (3) JRACLOUD-96384: both dupe.txt attachments must be returned \
         without dedup; stdout: {s3}"
    );
    assert_ne!(
        arr3[0]["id"], arr3[1]["id"],
        "BC-2.7.004 (3): both dupe.txt entries must have different IDs"
    );
    assert_eq!(
        arr3[0]["filename"], arr3[1]["filename"],
        "BC-2.7.004 (3): both entries must share the filename 'dupe.txt'"
    );

    // --- (a) ? wildcard in name filter: report-?.pdf matches report-1.pdf NOT report-10.pdf ---
    // BC-2.7.004: '?' matches any single character. report-?.pdf matches report-1.pdf
    // (single char '1' between '-' and '.') but NOT report-10.pdf (two chars '10').
    {
        let server_q4 = MockServer::start().await;
        let cache_q4 = tempfile::tempdir().unwrap();
        let config_q4 = tempfile::tempdir().unwrap();

        let attachments_q4 = vec![
            make_attachment(
                "40001",
                "report-1.pdf",
                "application/pdf",
                1024,
                Some("A"),
                Some("a"),
            ),
            make_attachment(
                "40002",
                "report-10.pdf",
                "application/pdf",
                2048,
                Some("B"),
                Some("b"),
            ),
        ];

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", attachments_q4)),
            )
            .mount(&server_q4)
            .await;

        let out_q4 = jr_cmd(&server_q4.uri(), cache_q4.path(), config_q4.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "name=report-?.pdf",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let s_q4 = String::from_utf8_lossy(&out_q4.stdout);
        assert_eq!(
            out_q4.status.code(),
            Some(0),
            "BC-2.7.004 (a) ? wildcard: must exit 0; got {:?}",
            out_q4.status.code()
        );
        let arr_q4 = serde_json::from_str::<Value>(&s_q4)
            .expect("BC-2.7.004 (a) ? wildcard: must be valid JSON")
            .as_array()
            .expect("must be array")
            .clone();
        // report-?.pdf must match report-1.pdf (single char: '1') but NOT report-10.pdf (two chars: '10').
        assert_eq!(
            arr_q4.len(),
            1,
            "BC-2.7.004 (a) ? glob: name=report-?.pdf must match report-1.pdf (1 result, \
             single char between '-' and '.') and NOT report-10.pdf (two chars after '-'); \
             got {} results; stdout: {s_q4}",
            arr_q4.len()
        );
        assert_eq!(
            arr_q4[0]["filename"], "report-1.pdf",
            "BC-2.7.004 (a) ? glob: matched element must be report-1.pdf; stdout: {s_q4}"
        );
        assert!(
            !s_q4.contains("report-10.pdf"),
            "BC-2.7.004 (a) ? glob: report-10.pdf must NOT match name=report-?.pdf; \
             stdout: {s_q4}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-007 / BC-2.7.005 — size-max filter; parse error (pre-HTTP); zero-byte edge case
// ---------------------------------------------------------------------------

/// Verify:
/// - `--filter size-max=<bytes>` retains only files with size ≤ limit.
/// - Non-integer argument exits 64 BEFORE any HTTP call (wiremock `.expect(0)`).
/// - `--filter size-max=0` retains zero-byte attachments only.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_005_size_max_filter_and_parse_error() {
    // --- (1) size-max filter: retains only files ≤ 50000 bytes ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        let attachments = vec![
            make_attachment(
                "10001",
                "small.txt",
                "text/plain",
                1000,
                Some("A"),
                Some("a"),
            ),
            make_attachment(
                "10002",
                "large.bin",
                "application/octet-stream",
                1_000_000,
                Some("B"),
                Some("b"),
            ),
        ];

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", attachments)),
            )
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "size-max=50000",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-2.7.005 (1): must exit 0; got {:?}",
            output.status.code()
        );
        let arr = serde_json::from_str::<Value>(&stdout)
            .expect("BC-2.7.005 (1): valid JSON required")
            .as_array()
            .expect("must be array")
            .clone();
        assert_eq!(
            arr.len(),
            1,
            "BC-2.7.005 (1): size-max=50000 must keep only the 1000-byte file; \
             stdout: {stdout}"
        );
        assert_eq!(
            arr[0]["filename"], "small.txt",
            "BC-2.7.005 (1): retained file must be 'small.txt'"
        );
    }

    // --- (2) non-integer: exit 64 BEFORE any HTTP (expect 0 requests) ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", vec![])),
            )
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "size-max=not_a_number",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "BC-2.7.005 EC-2.7.005-1: non-integer size-max must exit 64 BEFORE HTTP; \
             got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        // stderr must mention the invalid value or that an integer is expected
        assert!(
            stderr.to_lowercase().contains("size-max")
                || stderr.to_lowercase().contains("integer")
                || stderr.to_lowercase().contains("byte"),
            "BC-2.7.005 EC-2.7.005-1: stderr must mention size-max/integer/byte; \
             got: {stderr}"
        );
    }

    // --- (3) size-max=0: retains only zero-byte attachments ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        let attachments = vec![
            make_attachment("10001", "empty.txt", "text/plain", 0, Some("A"), Some("a")),
            make_attachment(
                "10002",
                "nonempty.txt",
                "text/plain",
                1,
                Some("B"),
                Some("b"),
            ),
        ];

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/FOO-1"))
            .and(query_param("fields", "attachment"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", attachments)),
            )
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "size-max=0",
                "--output",
                "json",
            ])
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(
            output.status.code(),
            Some(0),
            "BC-2.7.005 (3) size-max=0: must exit 0; got {:?}",
            output.status.code()
        );
        let arr = serde_json::from_str::<Value>(&stdout)
            .expect("BC-2.7.005 (3): valid JSON required")
            .as_array()
            .expect("must be array")
            .clone();
        assert_eq!(
            arr.len(),
            1,
            "BC-2.7.005 (3): size-max=0 must return only zero-byte files; \
             stdout: {stdout}"
        );
        assert_eq!(
            arr[0]["filename"], "empty.txt",
            "BC-2.7.005 (3): retained file must be 'empty.txt' (size=0)"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-008 / BC-2.7.003 EC-2.7.003-2 — invalid filter key or missing '='
// ---------------------------------------------------------------------------

/// Verify that malformed `--filter` values are rejected BEFORE any HTTP call:
/// - Missing `=`: exit 64, canonical message with accepted-keys list.
/// - Unknown key before `=`: exit 64, "Unknown filter key" message.
///
/// RED GATE: exits 2 (clap unknown subcommand before any validation fires);
/// `.expect(0)` on mock passes but exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_003_invalid_filter_key_exits_64() {
    // --- (1) missing '=': "noequalssign" ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", vec![])),
            )
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "noequalssign",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "EC-2.7.003-2 missing-=: must exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains(
                "Invalid filter 'noequalssign': expected key=value form. \
                 Accepted keys: mime=, name=, size-max=."
            ),
            "EC-2.7.003-2 missing-=: stderr must contain canonical error with \
             accepted-keys list; got: {stderr}"
        );
    }

    // --- (2) unknown key: "type=foo" ---
    {
        let server = MockServer::start().await;
        let cache_dir = tempfile::tempdir().unwrap();
        let config_dir = tempfile::tempdir().unwrap();

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(issue_attachment_response("FOO-1", vec![])),
            )
            .expect(0)
            .mount(&server)
            .await;

        let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
            .args([
                "issue",
                "attachment",
                "list",
                "FOO-1",
                "--filter",
                "type=foo",
            ])
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(64),
            "EC-2.7.003-2 unknown-key: must exit 64; got {:?}\nstderr: {stderr}",
            output.status.code()
        );
        assert!(
            stderr.contains("Unknown filter key 'type'. Accepted keys: mime=, name=, size-max=."),
            "EC-2.7.003-2 unknown-key: stderr must contain canonical error string; \
             got: {stderr}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-009 (1/5) / BC-2.7.006 — 404 → exit 64 + canonical message
// ---------------------------------------------------------------------------

/// Verify that a 404 from the issue GET exits 64 with
/// `"Issue <KEY> not found or not accessible."`.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_006_unknown_key_exits_64() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FAKE-999"))
        .and(query_param("fields", "attachment"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "errorMessages": [
                "Issue does not exist or you do not have permission to see it."
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "attachment", "list", "FAKE-999"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(64),
        "BC-2.7.006 404: must exit 64; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Issue FAKE-999 not found or not accessible."),
        "BC-2.7.006 404: stderr must contain 'Issue FAKE-999 not found or not accessible.'; \
         got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 (2/5) / BC-2.7.006 — 401 → exit 2 + auth hints
// ---------------------------------------------------------------------------

/// Verify that a 401 from the issue GET exits 2 with "Not authenticated" AND
/// "jr auth login" in stderr (loose-substring form per v1.3.88).
///
/// RED GATE: exits 2 from clap (same code, but wrong stderr content) →
/// stderr content assertions fail.
#[tokio::test]
async fn test_bc_2_7_006_key_401_exit_2() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "errorMessages": [
                "Client must be authenticated to access this resource."
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "attachment", "list", "FOO-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(2),
        "BC-2.7.006 401: must exit 2; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Not authenticated"),
        "BC-2.7.006 401: stderr must contain 'Not authenticated'; got: {stderr}"
    );
    assert!(
        stderr.contains("jr auth login"),
        "BC-2.7.006 401: stderr must contain 'jr auth login'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 (3/5) / BC-2.7.006 — 403 → exit 1 + permission denied
// ---------------------------------------------------------------------------

/// Verify that a 403 from the issue GET exits 1 with
/// `"Permission denied: cannot access issue <KEY>."`.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_006_key_403_exit_1() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "errorMessages": [
                "You do not have permission to view this issue."
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "attachment", "list", "FOO-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-2.7.006 403: must exit 1; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Permission denied: cannot access issue FOO-1."),
        "BC-2.7.006 403: stderr must contain \
         'Permission denied: cannot access issue FOO-1.'; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 (4/5) / BC-2.7.006 — 5xx → exit 1 + "API error ("
// ---------------------------------------------------------------------------

/// Verify that a 5xx from the issue GET exits 1 with `"API error ("` in stderr.
///
/// RED GATE: exits 2 (clap unknown subcommand) → exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_006_key_5xx_exit_1() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error."]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args(["issue", "attachment", "list", "FOO-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-2.7.006 5xx: must exit 1; got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("API error ("),
        "BC-2.7.006 5xx: stderr must contain 'API error ('; got: {stderr}"
    );
    // Must report the 500 status, NOT "Permission denied" (which belongs to 403 only).
    // This assertion kills the mutant `*status == 403` → `true` (attachments.rs):
    // with that mutation a 5xx response is re-wrapped as a 403 "Permission denied" error,
    // which would produce "API error (403): Permission denied …" instead of
    // "API error (500): …".
    assert!(
        stderr.contains("500"),
        "BC-2.7.006 5xx: stderr must contain the 500 status code; got: {stderr}"
    );
    assert!(
        !stderr.contains("Permission denied"),
        "BC-2.7.006 5xx: stderr must NOT contain 'Permission denied' \
         (that message is reserved for 403 responses only); got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// EC-MUTANT-001 — all-filtered-out: empty stdout + filter-count hint to stderr
// ---------------------------------------------------------------------------

/// Verify the filter-count hint fires when ALL attachments are filtered out (n=0 < total>0).
///
/// This test kills the surviving mutants on the `if n < total` guard inside the
/// `else if filtered.is_empty()` arm (src/cli/issue/attachments.rs):
/// - Mutation `<` → `==`: `if 0 == total` is false when total > 0 → hint suppressed → FAIL.
/// - Mutation `<` → `>`:  `if 0 > total` is always false → hint suppressed → FAIL.
/// - Mutation `<` → `<=`: `if 0 <= total` is always true (equivalent mutation) → hint fires
///   → test passes. This is an EQUIVALENT mutation: when filtered.is_empty(), n is always 0,
///   so `0 < total` and `0 <= total` are indistinguishable for any total > 0.
///
/// This test also verifies stdout is empty (pipe-friendly) when all rows are filtered out.
#[tokio::test]
async fn test_bc_2_7_001_all_filtered_out_empty_stdout_hint_to_stderr() {
    let server = MockServer::start().await;
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    // 2 PDF attachments — mime=image/* will match none of them.
    let attachments = vec![
        make_attachment(
            "10001",
            "report.pdf",
            "application/pdf",
            4096,
            Some("Alice"),
            Some("acct-001"),
        ),
        make_attachment(
            "10002",
            "contract.pdf",
            "application/pdf",
            8192,
            Some("Bob"),
            Some("acct-002"),
        ),
    ];

    Mock::given(method("GET"))
        .and(path("/rest/api/3/issue/FOO-1"))
        .and(query_param("fields", "attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(issue_attachment_response("FOO-1", attachments)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache_dir.path(), config_dir.path())
        .args([
            "issue",
            "attachment",
            "list",
            "FOO-1",
            "--filter",
            "mime=image/*",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(0),
        "EC-MUTANT-001 all-filtered-out: must exit 0; \
         got {:?}\nstderr: {stderr}\nstdout: {stdout}",
        output.status.code()
    );

    // stdout must be empty — no table rendered when filter removes all rows.
    assert!(
        stdout.trim().is_empty(),
        "EC-MUTANT-001 all-filtered-out: stdout must be empty \
         (pipe-friendly when filter removes all rows); got: {stdout}"
    );

    // Hint must fire on stderr: "Showing 0 of 2 attachments."
    // This assertion kills `<` → `==` and `<` → `>` mutations.
    assert!(
        stderr.contains("Showing 0 of 2 attachments."),
        "EC-MUTANT-001 all-filtered-out: stderr must contain \
         'Showing 0 of 2 attachments.' when filter removes all rows; got: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// AC-009 (5/5) / BC-2.7.006 — network error → exit 1 + "Could not reach"
// ---------------------------------------------------------------------------

/// Verify that a network-unreachable error exits 1 with "Could not reach" in
/// stderr (loose-substring; full literal per `src/error.rs::JrError::NetworkError`:
/// `Could not reach <host> — check your connection`).
///
/// Uses port 1 on loopback — almost universally refused, giving a fast
/// connection-refused error without a real network call.
///
/// RED GATE: exits 2 (clap unknown subcommand before any network attempt) →
/// exit-code assertion fails.
#[tokio::test]
async fn test_bc_2_7_006_key_network_exit_1() {
    let cache_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("jr")
        .unwrap()
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("XDG_CACHE_HOME", cache_dir.path())
        .env("JR_CACHE_DIR", cache_dir.path().join("jr"))
        .env("XDG_CONFIG_HOME", config_dir.path())
        .env("JR_CONFIG_DIR", config_dir.path().join("jr"))
        .args(["issue", "attachment", "list", "FOO-1"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "BC-2.7.006 network: must exit 1 on network error; \
         got {:?}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("Could not reach"),
        "BC-2.7.006 network: stderr must contain 'Could not reach' \
         (full: 'Could not reach <host> — check your connection'); got: {stderr}"
    );
}
