//! Hermetic wiremock/subprocess integration tests for BC-X.16.001/BC-X.16.002
//! (`jr api --query-param`/`-q NAME=VALUE`, issue #583, cycle-014 STORY-C
//! `S-cycle14-api-query-param`).
//!
//! Every test in this file follows the hermetic setup pinned by
//! `.factory/cycles/cycle-014/phase-f2-spec-evolution/verification-delta.md`
//! §2: fresh per-test `JR_CONFIG_DIR`/`JR_CACHE_DIR`, a `cwd` with no
//! ancestor `.jr.toml`, `JR_BASE_URL` pointed at a wiremock server,
//! `JR_AUTH_HEADER` supplying auth, and every other ambient `JR_`-prefixed
//! variable removed (`common::hermetic::scrub_ambient_jr_env`) so a
//! developer/CI environment's stray `JR_*` variable can't leak a configured
//! default into these tests (except the `--help` pin, which exits before any
//! config loading). Kept separate from `tests/cli_handler.rs`
//! (already ~2,200 LOC) per the story's File Structure Requirements.
//!
//! Every "exactly once"/multiplicity claim below is asserted by decoding
//! the SINGLE received request's raw query via `url::Url::query_pairs()`
//! (`single_received_request`/`query_pairs_of` below) — never via
//! wiremock's `query_param` matcher alone, which also matches duplicate
//! occurrences of a key and cannot observe multiplicity on its own.
//!
//! Direct-call cells for `append_query_params`/`parse_query_param`
//! (AC-001..AC-005's proptests and pinned examples) live in
//! `src/cli/api.rs`'s `#[cfg(test)] mod tests` instead — see that module's
//! direct-call cells, which exercise the pure functions in isolation, as
//! the complement to this file's subprocess cells, which exercise the same
//! behavior through the full CLI argv surface (clap parsing, `handle_api`
//! wiring, and the wire-level HTTP request).

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Pinned message helpers (kept in lockstep with `src/cli/api.rs`'s own
// copies — both are transcriptions of BC-X.16.002's "Pinned error
// messages" block, not a shared `pub` symbol, since these functions are
// `pub(crate)`-invisible to an integration test binary in a different
// crate-test target) ─────────────────────────────────────────────────────

/// Distinguishing substring for M1 (missing `=`).
const D1: &str = "must be in NAME=VALUE format";
/// Distinguishing substring for M2 (empty NAME).
const D2: &str = "NAME cannot be empty";

fn m1_message(raw: &str) -> String {
    format!("--query-param must be in NAME=VALUE format (got: {raw})")
}

fn m2_message(raw: &str) -> String {
    format!(
        "--query-param NAME cannot be empty (got: {raw}) \u{2014} use NAME=VALUE, e.g. -q maxResults=50"
    )
}

// ── Harness ──────────────────────────────────────────────────────────────

/// Fresh, hermetic `(cache dir, config dir, cwd)` triple for one test —
/// `jr api` needs no `config.toml` at all (`JR_BASE_URL`/`JR_AUTH_HEADER`
/// fully seed the client), so unlike `tests/user_list_project_resolution.rs`
/// this harness never writes one.
struct Harness {
    cache: TempDir,
    config: TempDir,
    cwd: TempDir,
}

impl Harness {
    fn new() -> Self {
        let cache = TempDir::new().unwrap();
        let config = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        common::hermetic::assert_no_ancestor_jr_toml(cwd.path());
        Self { cache, config, cwd }
    }

    /// Hermetic `assert_cmd::Command` builder: every ambient `JR_*` var is
    /// scrubbed (`common::hermetic::scrub_ambient_jr_env`) BEFORE the seams
    /// below are set, `JR_BASE_URL` points at `server_uri`, `JR_AUTH_HEADER`
    /// bypasses keychain credential loading, and `--no-input` is always
    /// passed.
    fn cmd(&self, server_uri: &str) -> Command {
        let mut c = Command::cargo_bin("jr").unwrap();
        common::hermetic::scrub_ambient_jr_env(
            &mut c,
            &[
                "JR_BASE_URL",
                "JR_AUTH_HEADER",
                "JR_CACHE_DIR",
                "JR_CONFIG_DIR",
            ],
        );
        c.env("JR_BASE_URL", server_uri)
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("JR_CACHE_DIR", self.cache.path().join("jr"))
            .env("JR_CONFIG_DIR", self.config.path().join("jr"))
            .current_dir(self.cwd.path())
            .arg("--no-input");
        c
    }

    /// Same hermetic env, targeting a raw `std::process::Command` instead
    /// of `assert_cmd::Command` — required by VP-API-QP-006(iii)'s
    /// held-open-stdin cell below, which needs a live `ChildStdin` handle
    /// that `assert_cmd::Command::output()` cannot provide: assert_cmd
    /// 2.2.2's `output()`/timeout path closes the child's stdin before
    /// waiting (`wait_with_input_output` → wait-timeout 0.2.1
    /// `drop(self.stdin.take())`; the untimed path uses `Child::wait`,
    /// which also drops stdin), so it cannot hold a pipe open.
    fn std_cmd(&self, server_uri: &str) -> std::process::Command {
        let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_jr"));
        // F-003: share `common::hermetic::is_scrubbable`'s scrub logic
        // (vars_os + trimming) rather than a second, hand-rolled `vars()`
        // implementation — see that function's doc comment.
        for (key_os, _) in std::env::vars_os() {
            let key_lossy = key_os.to_string_lossy();
            if common::hermetic::is_scrubbable(key_lossy.trim(), &[]) {
                c.env_remove(&key_os);
            }
        }
        c.env("JR_BASE_URL", server_uri)
            .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
            .env("JR_CACHE_DIR", self.cache.path().join("jr"))
            .env("JR_CONFIG_DIR", self.config.path().join("jr"))
            .current_dir(self.cwd.path())
            .arg("--no-input");
        c
    }
}

/// Mounts catch-all `.expect(0)` mocks for ANY GET and ANY POST request —
/// the all-or-nothing / pre-flight zero-HTTP guarantee (VP-API-QP-006's own
/// "every mock `.expect(0)`" requirement), mirroring
/// `tests/issue_commands.rs::s606_1_expect_zero_http`'s established
/// pattern in this repo.
async fn expect_zero_http(server: &MockServer) {
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(server)
        .await;
}

/// Asserts exactly ONE request was received and returns it. The caller then
/// inspects `req.url.query_pairs()` directly — the multiplicity-observing
/// mechanism this file uses throughout, never a wiremock `query_param`
/// matcher (see module doc).
async fn single_received_request(server: &MockServer) -> wiremock::Request {
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "expected exactly one received request, got {}",
        requests.len()
    );
    requests.into_iter().next().unwrap()
}

/// Decodes `req.url.query_pairs()` into an owned `Vec<(String, String)>`,
/// in wire order.
fn query_pairs_of(req: &wiremock::Request) -> Vec<(String, String)> {
    req.url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

// ── AC-002: argv cells (handler wiring) ─────────────────────────────────

/// EC-X.16.001-13: `-q fields=summary,status` (comma inside VALUE) — the
/// `-q` clap field's plain `Vec<String>` declaration (no `value_delimiter`)
/// means this is ONE occurrence producing ONE pair, never a `value_delimiter`
/// split into `fields=summary` and a bare `status`.
#[tokio::test]
async fn test_bc_x_16_001_ec13_comma_in_value_produces_exactly_one_pair() {
    let h = Harness::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "fields=summary,status"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let req = single_received_request(&server).await;
    assert_eq!(req.url.query(), Some("fields=summary%2Cstatus"));
    let pairs = query_pairs_of(&req);
    assert_eq!(
        pairs,
        vec![("fields".to_string(), "summary,status".to_string())],
        "expected exactly ONE pair, comma preserved as ordinary VALUE content"
    );
}

/// Repeated-flags argv cell (handler wiring): `-q fields=summary -q
/// fields=status` — BOTH pairs present, in flag order, on the single
/// received request. Exercises `handle_api`'s hand-off from the parsed `-q`
/// `Vec` to `append_query_params`, which the direct-call proptests
/// (targeting `append_query_params` in isolation) cannot see.
#[tokio::test]
async fn test_bc_x_16_001_repeated_flags_produce_both_pairs_in_flag_order() {
    let h = Harness::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "fields=summary", "-q", "fields=status"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let req = single_received_request(&server).await;
    assert_eq!(req.url.query(), Some("fields=summary&fields=status"));
    let pairs = query_pairs_of(&req);
    assert_eq!(
        pairs,
        vec![
            ("fields".to_string(), "summary".to_string()),
            ("fields".to_string(), "status".to_string()),
        ]
    );
}

/// Mixed argv cell: `'/x?a=1' -q b=2 -q b=3` — the pre-existing query pair
/// stays first (verbatim), followed by both new pairs in flag order.
#[tokio::test]
async fn test_bc_x_16_001_mixed_existing_query_and_repeated_flags_all_present() {
    let h = Harness::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x?a=1", "-q", "b=2", "-q", "b=3"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let req = single_received_request(&server).await;
    assert_eq!(req.url.query(), Some("a=1&b=2&b=3"));
    let pairs = query_pairs_of(&req);
    assert_eq!(
        pairs,
        vec![
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string()),
            ("b".to_string(), "3".to_string()),
        ]
    );
}

/// F-001(b): `-q " =v" -q "k= v "` — a whitespace-only NAME and a VALUE
/// with leading/trailing spaces must survive to the wire byte-for-byte
/// (never trimmed). Asserts BOTH the raw received query string AND the
/// decoded `query_pairs()` multiplicity/values on the single received
/// request — a `name.trim().is_empty()` or `rest.trim()` regression in
/// `parse_query_param` would either misreport the first pair as M2 (empty
/// NAME, non-zero exit) or silently drop the VALUE's surrounding spaces.
#[tokio::test]
async fn test_bc_x_16_001_query_param_does_not_trim_whitespace_in_name_or_value() {
    let h = Harness::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", " =v", "-q", "k= v "])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let req = single_received_request(&server).await;
    assert_eq!(req.url.query(), Some("%20=v&k=%20v%20"));
    let pairs = query_pairs_of(&req);
    assert_eq!(
        pairs,
        vec![
            (" ".to_string(), "v".to_string()),
            ("k".to_string(), " v ".to_string()),
        ],
        "expected exactly two pairs, whitespace preserved verbatim in both NAME and VALUE"
    );
}

// ── AC-003: --help pin ──────────────────────────────────────────────────

/// VP-API-QP-003(e): `jr api --help` exits 0 and its whitespace-collapsed
/// stdout contains the pinned substring `"do not pre-encode"` (Behavior 3).
/// Never reaches `handle_api` — no wiremock server, no async runtime
/// needed.
#[test]
fn test_bc_x_16_001_help_pins_do_not_pre_encode_substring() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["api", "--help"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected --help to exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let collapsed = stdout.split_whitespace().collect::<Vec<_>>().join(" ");

    assert!(
        collapsed.contains("do not pre-encode"),
        "expected the pinned 'do not pre-encode' substring in --help output; got: {collapsed}"
    );
}

// ── AC-004: method orthogonality + zero-flag wiremock examples ──────────

/// VP-API-QP-004(structural): a table-driven test over
/// GET/POST/PUT/PATCH/DELETE, each with and without `-d`, asserts the SAME
/// received query pairs for every method/body combination, and a request
/// body equal to the `-d` input (or empty when `-d` is absent) — the query
/// is never gated on the method, and `-d` content is never merged into the
/// query or vice versa.
#[tokio::test]
async fn test_bc_x_16_001_query_assembly_identical_across_methods_and_body_presence() {
    let methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
    let bodies: [Option<&str>; 2] = [None, Some(r#"{"k":"v"}"#)];

    for method_flag in methods {
        for body in bodies {
            let h = Harness::new();
            let server = MockServer::start().await;
            Mock::given(method(method_flag))
                .and(path("/x"))
                .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
                .expect(1)
                .mount(&server)
                .await;

            let mut cmd = h.cmd(&server.uri());
            cmd.args(["api", "/x", "-X", method_flag, "-q", "a=1"]);
            if let Some(b) = body {
                cmd.args(["-d", b]);
            }
            let output = cmd.output().unwrap();

            server.verify().await;
            assert!(
                output.status.success(),
                "method {method_flag} body {body:?}: expected exit 0; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );

            let req = single_received_request(&server).await;
            assert_eq!(
                req.url.query(),
                Some("a=1"),
                "method {method_flag} body {body:?}: query must be independent of method/body"
            );
            let pairs = query_pairs_of(&req);
            assert_eq!(
                pairs,
                vec![("a".to_string(), "1".to_string())],
                "method {method_flag} body {body:?}"
            );

            match body {
                Some(b) => {
                    let recv_body = String::from_utf8_lossy(&req.body);
                    assert_eq!(
                        recv_body, b,
                        "method {method_flag}: request body must equal the -d input, never the query"
                    );
                }
                None => {
                    assert!(
                        req.body.is_empty(),
                        "method {method_flag}: request body must be empty when -d is absent"
                    );
                }
            }
        }
    }
}

/// VP-API-QP-004(2), example 1: `jr api rest/api/3/myself` (no `-q` flags)
/// → the received request has NO query string at all — not even a bare
/// `?`. `handle_api` always runs the parser, and with zero `-q` flags
/// `append_query_params` is the identity (BC-X.16.001 Postcondition 1) on
/// `normalize_path`'s output — this cell is a regression guard on that
/// pre-existing behavior.
#[tokio::test]
async fn test_bc_x_16_001_zero_flags_myself_path_has_no_query_at_all() {
    let h = Harness::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/myself"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "rest/api/3/myself"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let req = single_received_request(&server).await;
    assert_eq!(req.url.path(), "/rest/api/3/myself");
    assert_eq!(
        req.url.query(),
        None,
        "expected no query string at all (not even a bare '?')"
    );
}

/// VP-API-QP-004(2), example 2: `jr api "/rest/api/3/search?jql=a&"` (no
/// `-q` flags) → the received request's raw query is byte-identical to
/// `normalize_path`'s own output, `jql=a&`, unchanged — the same zero-flag
/// identity as the example above.
#[tokio::test]
async fn test_bc_x_16_001_zero_flags_preserves_existing_trailing_ampersand_query() {
    let h = Harness::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/rest/api/3/search?jql=a&"])
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let req = single_received_request(&server).await;
    assert_eq!(req.url.query(), Some("jql=a&"));
}

/// F-005(c): VP-API-QP-004(2)'s zero-flag identity holds for every HTTP
/// method `jr api -X` supports (GET/POST/PUT/PATCH/DELETE, per
/// `HttpMethod` in `src/cli/api.rs`) — the query is never gated on
/// the method choice, including when no `-q` flags are present at all.
#[tokio::test]
async fn test_bc_x_16_001_zero_flags_no_query_across_all_methods() {
    let methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
    for method_flag in methods {
        let h = Harness::new();
        let server = MockServer::start().await;
        Mock::given(method(method_flag))
            .and(path("/x"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
            .expect(1)
            .mount(&server)
            .await;

        let output = h
            .cmd(&server.uri())
            .args(["api", "/x", "-X", method_flag])
            .output()
            .unwrap();

        server.verify().await;
        assert!(
            output.status.success(),
            "method {method_flag}: expected exit 0; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let req = single_received_request(&server).await;
        assert_eq!(
            req.url.query(),
            None,
            "method {method_flag}: expected no query string at all with zero -q flags"
        );
    }
}

// ── AC-005/AC-006: wiremock M1/M2 cells + JSON envelope ──────────────────

/// VP-API-QP-005(2), M1 cell: `-q foo` exits 64, zero HTTP, and stderr
/// contains the M1 message rendered byte-for-byte.
#[tokio::test]
async fn test_bc_x_16_002_ec1_missing_equals_exits_64_with_m1_message() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "foo"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&m1_message("foo")),
        "expected M1 message; got: {stderr}"
    );
}

/// VP-API-QP-005(2), M2 cell: `-q =v` exits 64, zero HTTP, and stderr
/// contains the M2 message rendered byte-for-byte.
#[tokio::test]
async fn test_bc_x_16_002_ec2_empty_name_exits_64_with_m2_message() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "=v"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&m2_message("=v")),
        "expected M2 message; got: {stderr}"
    );
}

/// VP-API-QP-005(2) `--output json` cells: for BOTH M1 (`-q foo`) and M2
/// (`-q =v`) with `--output json`, the `{"error","code"}` envelope is
/// parsed from STDERR (`src/main.rs`'s top-level `eprintln!`), its
/// `"error"` string equals the rendered message exactly, `"code"` is 64,
/// and stdout is empty on this pre-flight exit-64 path.
#[tokio::test]
async fn test_bc_x_16_002_output_json_envelope_on_stderr_for_m1_and_m2() {
    // M1.
    {
        let h = Harness::new();
        let server = MockServer::start().await;
        expect_zero_http(&server).await;

        let output = h
            .cmd(&server.uri())
            .args(["api", "/x", "-q", "foo", "--output", "json"])
            .output()
            .unwrap();

        server.verify().await;
        assert_eq!(output.status.code(), Some(64));
        assert!(
            output.stdout.is_empty(),
            "stdout must be empty on a pre-flight exit-64 error"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        let envelope: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|e| {
            panic!("expected a JSON {{error,code}} envelope on stderr; got: {stderr} ({e})")
        });
        assert_eq!(envelope["error"].as_str(), Some(m1_message("foo").as_str()));
        assert_eq!(envelope["code"].as_i64(), Some(64));
    }

    // M2.
    {
        let h = Harness::new();
        let server = MockServer::start().await;
        expect_zero_http(&server).await;

        let output = h
            .cmd(&server.uri())
            .args(["api", "/x", "-q", "=v", "--output", "json"])
            .output()
            .unwrap();

        server.verify().await;
        assert_eq!(output.status.code(), Some(64));
        assert!(
            output.stdout.is_empty(),
            "stdout must be empty on a pre-flight exit-64 error"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        let envelope: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap_or_else(|e| {
            panic!("expected a JSON {{error,code}} envelope on stderr; got: {stderr} ({e})")
        });
        assert_eq!(envelope["error"].as_str(), Some(m2_message("=v").as_str()));
        assert_eq!(envelope["code"].as_i64(), Some(64));
    }
}

// ── AC-009: attached-form argv cells (EC-X.16.002-5..10) ────────────────

/// EC-X.16.002-5: `-q=v` (short-flag attached, ONE `=`) — clap strips
/// exactly one leading `=`, delivering `raw = "v"` → M1 `(got: v)`, D2
/// absent.
#[tokio::test]
async fn test_bc_x_16_002_ec5_attached_short_form_one_equals_reports_m1() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q=v"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&m1_message("v")), "got: {stderr}");
    assert!(stderr.contains(D1), "got: {stderr}");
    assert!(!stderr.contains(D2), "got: {stderr}");
}

/// EC-X.16.002-6: `-q==v` (short-flag attached, TWO `=`) — clap strips only
/// the FIRST `=`, delivering `raw = "=v"` → M2 `(got: =v)`, D1 absent.
#[tokio::test]
async fn test_bc_x_16_002_ec6_attached_short_form_two_equals_reports_m2() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q==v"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&m2_message("=v")), "got: {stderr}");
    assert!(stderr.contains(D2), "got: {stderr}");
    assert!(!stderr.contains(D1), "got: {stderr}");
}

/// EC-X.16.002-7: `--query-param==v` (long-flag attached, TWO `=`) — clap's
/// `split_once("=")` produces the same `raw = "=v"`, BYTE-IDENTICAL to
/// `-q==v`'s stderr (EC-6). This cell additionally runs the EC-6 invocation
/// and asserts byte-identical stderr between the two, per the clause's own
/// bullet.
#[tokio::test]
async fn test_bc_x_16_002_ec7_attached_long_form_two_equals_reports_m2_byte_identical_to_ec6() {
    let h6 = Harness::new();
    let server6 = MockServer::start().await;
    expect_zero_http(&server6).await;
    let output6 = h6
        .cmd(&server6.uri())
        .args(["api", "/x", "-q==v"])
        .output()
        .unwrap();
    server6.verify().await;

    let h7 = Harness::new();
    let server7 = MockServer::start().await;
    expect_zero_http(&server7).await;
    let output7 = h7
        .cmd(&server7.uri())
        .args(["api", "/x", "--query-param==v"])
        .output()
        .unwrap();
    server7.verify().await;

    assert_eq!(
        output7.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output7.stderr)
    );
    let stderr7 = String::from_utf8_lossy(&output7.stderr);
    assert!(stderr7.contains(&m2_message("=v")), "got: {stderr7}");
    assert!(stderr7.contains(D2), "got: {stderr7}");
    assert!(!stderr7.contains(D1), "got: {stderr7}");

    assert_eq!(
        output6.stderr, output7.stderr,
        "-q==v and --query-param==v must produce byte-identical stderr"
    );
}

/// EC-X.16.002-8: `-q -x=1` (hyphen-leading token as the space-separated
/// value) — `-q` has NO `allow_hyphen_values`, so clap treats `-x=1` as a
/// new, unrecognized flag: exit 2 (NOT 64), stderr contains `unexpected
/// argument`, does NOT contain `Not authenticated` (which also exits 2),
/// and contains NEITHER D1 nor D2 (`parse_query_param` never ran) — this
/// is clap's own argv-parsing rejection, before `handle_api` runs at all.
#[tokio::test]
async fn test_bc_x_16_002_ec8_hyphen_leading_value_fails_at_clap_level_exit_2() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "-x=1"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unexpected argument"), "got: {stderr}");
    assert!(!stderr.contains("Not authenticated"), "got: {stderr}");
    assert!(!stderr.contains(D1), "got: {stderr}");
    assert!(!stderr.contains(D2), "got: {stderr}");
}

/// EC-X.16.002-9: three forms of an empty raw value — `-q=`, `--query-param=`,
/// `-q ""` — all reach `parse_query_param` as `raw = ""`, classifying as M1
/// `(got: )` (NOT M2: the missing-`=` check runs before any NAME-emptiness
/// check). Merged into ONE test, since all three forms exercise the same
/// `parse_query_param` code path with an identical `raw`.
#[tokio::test]
async fn test_bc_x_16_002_ec9_empty_raw_value_three_forms_report_m1() {
    let cases: [&[&str]; 3] = [
        &["api", "/x", "-q="],
        &["api", "/x", "--query-param="],
        &["api", "/x", "-q", ""],
    ];

    for args in cases {
        let h = Harness::new();
        let server = MockServer::start().await;
        expect_zero_http(&server).await;

        let output = h.cmd(&server.uri()).args(args).output().unwrap();

        server.verify().await;
        assert_eq!(
            output.status.code(),
            Some(64),
            "args {args:?}: stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&m1_message("")),
            "args {args:?}: got: {stderr}"
        );
        assert!(stderr.contains(D1), "args {args:?}: got: {stderr}");
        assert!(!stderr.contains(D2), "args {args:?}: got: {stderr}");
    }
}

/// EC-X.16.002-10: `-q` as the LAST argv token (a genuinely MISSING value,
/// not an empty one) — clap itself rejects this before `parse_query_param`
/// ever runs: exit 2, stderr contains `a value is required for`, does NOT
/// contain `Not authenticated`, contains NEITHER D1 nor D2.
#[tokio::test]
async fn test_bc_x_16_002_ec10_missing_value_as_last_token_fails_at_clap_level_exit_2() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(2),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("a value is required for"), "got: {stderr}");
    assert!(!stderr.contains("Not authenticated"), "got: {stderr}");
    assert!(!stderr.contains(D1), "got: {stderr}");
    assert!(!stderr.contains(D2), "got: {stderr}");
}

// ── AC-007: all-or-nothing + first-malformed-reported ───────────────────

/// VP-API-QP-006(i), cell 1: `-q a=1 -q bad` exits 64 with NO request sent
/// — the well-formed pair is never transmitted.
#[tokio::test]
async fn test_bc_x_16_002_all_or_nothing_malformed_second_flag_no_request_sent() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "a=1", "-q", "bad"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// VP-API-QP-006(i), cell 2: `-q bad -q a=1` (order swapped) exits 64 with
/// NO request sent.
#[tokio::test]
async fn test_bc_x_16_002_all_or_nothing_malformed_first_flag_no_request_sent() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "bad", "-q", "a=1"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// VP-API-QP-006(ii), cell 1: `-q foo -q =v` reports M1 naming `foo` (D2
/// absent) — the FIRST malformed flag in flag order is the one reported.
#[tokio::test]
async fn test_bc_x_16_002_first_malformed_reported_m1_before_m2_in_flag_order() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "foo", "-q", "=v"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&m1_message("foo")), "got: {stderr}");
    assert!(stderr.contains(D1), "got: {stderr}");
    assert!(!stderr.contains(D2), "got: {stderr}");
}

/// VP-API-QP-006(ii), cell 2: `-q =v -q foo` reports M2 naming `=v` (D1
/// absent) — the FIRST malformed flag in flag order is the one reported.
#[tokio::test]
async fn test_bc_x_16_002_first_malformed_reported_m2_before_m1_in_flag_order() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "=v", "-q", "foo"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&m2_message("=v")), "got: {stderr}");
    assert!(stderr.contains(D2), "got: {stderr}");
    assert!(!stderr.contains(D1), "got: {stderr}");
}

// ── AC-008: pre-flight ordering ──────────────────────────────────────────

/// VP-API-QP-006(iii): `-q` validation runs BEFORE `resolve_body`'s
/// blocking `-d @-` stdin read (EC-X.16.002-4). Spawned via raw
/// `std::process::Command` (see `Harness::std_cmd`'s doc comment for why
/// assert_cmd cannot be used here), with the `ChildStdin` handle kept alive
/// — never written to, never dropped — for the whole wait: a `/dev/null` or
/// closed pipe would give immediate EOF and prove nothing, since only a
/// TRULY held-open pipe can distinguish "validated first" from "read
/// first, then would-have-blocked". Polls `try_wait()` against a ~5s
/// deadline; on the deadline it kills the child and fails, since an
/// implementation that read the body before validating `-q` would block
/// until killed.
#[tokio::test]
async fn test_bc_x_16_002_query_param_validated_before_resolve_body_blocks_on_stdin() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let mut child = h
        .std_cmd(&server.uri())
        .args(["api", "/x", "-X", "POST", "-d", "@-", "-q", "bad"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn compiled jr binary (CARGO_BIN_EXE_jr)");

    let stdin = child.stdin.take().expect("stdin was not piped");
    // Deliberately kept alive, never written to — dropped only after the
    // child has already exited or the deadline has been hit below.

    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("try_wait failed") {
            break status;
        }
        if Instant::now() >= deadline {
            drop(stdin);
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "jr did not exit within 5s while stdin was held open -- -q validation must run \
                 BEFORE resolve_body's blocking `-d @-` stdin read (BC-X.16.002 Postcondition 1 \
                 / EC-X.16.002-4)"
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    drop(stdin);

    let mut stderr_buf = Vec::new();
    child
        .stderr
        .take()
        .expect("stderr was not piped")
        .read_to_end(&mut stderr_buf)
        .unwrap();
    let stderr = String::from_utf8_lossy(&stderr_buf);

    server.verify().await;
    assert_eq!(status.code(), Some(64), "stderr: {stderr}");
    assert!(
        stderr.contains(D1),
        "expected M1's distinguishing substring on stderr; got: {stderr}"
    );
}

/// VP-API-QP-006(iv): `-q bad -H "malformed-no-colon"` reports the `-q`
/// error (M1), NOT `parse_header`'s `Key: Value` error — `-q` validation
/// runs strictly before `-H`/`--header` parsing.
#[tokio::test]
async fn test_bc_x_16_002_query_param_error_reported_before_malformed_header() {
    let h = Harness::new();
    let server = MockServer::start().await;
    expect_zero_http(&server).await;

    let output = h
        .cmd(&server.uri())
        .args(["api", "/x", "-q", "bad", "-H", "malformed-no-colon"])
        .output()
        .unwrap();

    server.verify().await;
    assert_eq!(
        output.status.code(),
        Some(64),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(D1),
        "expected the -q M1 error, not parse_header's; got: {stderr}"
    );
    assert!(
        !stderr.contains("Key: Value"),
        "must report the -q error before parse_header's 'Key: Value' error; got: {stderr}"
    );
}
