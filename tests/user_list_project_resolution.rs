//! Hermetic wiremock integration tests for BC-X.7.002 (`jr user list`
//! `--project` resolution order, issue #862, cycle-014 STORY-A
//! `S-cycle14-user-list-project-resolution`).
//!
//! Every test in this file follows the hermetic setup pinned by
//! `.factory/cycles/cycle-014/phase-f2-spec-evolution/verification-delta.md`
//! §2: fresh per-test `JR_CONFIG_DIR`/`JR_CACHE_DIR`, a `cwd` with no
//! ancestor `.jr.toml` (a case needing one writes it into its own temp
//! `cwd`), `JR_BASE_URL` pointed at the wiremock server, `JR_AUTH_HEADER`
//! supplying auth, and every other ambient `JR_`-prefixed variable removed
//! so a developer/CI environment's stray `JR_*` variable can't leak a
//! configured default into these tests.
//!
//! VP-USER-LIST-PROJECT-001(c) wiring-layer cells covered here:
//! EC-X.7.002-1/2/3 (split three ways)/5/6, plus the `--help` cell
//! (VP-USER-LIST-PROJECT-001(d)). EC-X.7.002-4's cell and the
//! `--all`-pagination cells (VP(c)'s Postcondition-5 half) live in
//! `tests/user_commands.rs` and `tests/user_pagination.rs` respectively,
//! per the story's Task 5/6 file split.

#[allow(dead_code)]
mod common;

use assert_cmd::Command;
use tempfile::TempDir;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::fixtures;

// ── Harness ──────────────────────────────────────────────────────────────

/// Every ambient `JR_`-prefixed variable this harness does NOT itself set,
/// pinned per `verification-delta.md` §2 step 5 (mirrors
/// `tests/auth_profiles.rs`'s pinned scrub list) so a stray `JR_*` variable
/// in the ambient shell (e.g. a direnv-set `JR_PROFILE`) cannot leak a
/// configured default into these tests.
fn scrub_ambient_jr_env(cmd: &mut Command) -> &mut Command {
    cmd.env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_FIELDS_TEAM_FIELD_ID")
        .env_remove("JR_FIELDS_STORY_POINTS_FIELD_ID")
        .env_remove("JR_DEFAULTS_OUTPUT")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET")
}

/// Hermetic `jr` command builder. `JR_CONFIG_DIR`/`JR_CACHE_DIR` are set to
/// `<home>/jr` — the convention `global_config_dir()`'s `JR_CONFIG_DIR`
/// debug seam expects directly (it returns the env value verbatim as the
/// directory containing `config.toml`, no implicit `jr` join), matching
/// `tests/multi_profile_fields.rs`'s harness. `JR_BASE_URL` points at the
/// wiremock server, `JR_AUTH_HEADER` bypasses keychain credential loading,
/// `--no-input` is always passed, and every other ambient `JR_*` var is
/// scrubbed.
fn jr_cmd(
    server_uri: &str,
    cache_home: &std::path::Path,
    config_home: &std::path::Path,
) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_BASE_URL", server_uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_CACHE_DIR", cache_home.join("jr"))
        .env("JR_CONFIG_DIR", config_home.join("jr"))
        .arg("--no-input");
    scrub_ambient_jr_env(&mut cmd);
    cmd
}

/// Writes a single-profile `config.toml` (`default_profile = "default"`)
/// with an optional profile-level `project` default.
fn write_default_profile_config(
    config_home: &std::path::Path,
    base_url: &str,
    project: Option<&str>,
) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    let project_line = project
        .map(|p| format!("project = \"{p}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        dir.join("config.toml"),
        format!(
            "default_profile = \"default\"\n[profiles.default]\nurl = \"{base_url}\"\nauth_method = \"api_token\"\n{project_line}"
        ),
    )
    .unwrap();
}

/// Writes a two-profile `config.toml` — `default` and `alt` — each with its
/// own optional `project` default, for EC-X.7.002-5's non-default
/// `--profile` cell.
fn write_two_profile_config(
    config_home: &std::path::Path,
    base_url: &str,
    default_project: Option<&str>,
    alt_project: Option<&str>,
) {
    let dir = config_home.join("jr");
    std::fs::create_dir_all(&dir).unwrap();
    let default_project_line = default_project
        .map(|p| format!("project = \"{p}\"\n"))
        .unwrap_or_default();
    let alt_project_line = alt_project
        .map(|p| format!("project = \"{p}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        dir.join("config.toml"),
        format!(
            "default_profile = \"default\"\n\
             [profiles.default]\n\
             url = \"{base_url}\"\n\
             auth_method = \"api_token\"\n\
             {default_project_line}\n\
             [profiles.alt]\n\
             url = \"{base_url}\"\n\
             auth_method = \"api_token\"\n\
             {alt_project_line}"
        ),
    )
    .unwrap();
}

/// Writes a `.jr.toml` with the given `project` key into `cwd`.
fn write_jr_toml(cwd: &std::path::Path, project: &str) {
    std::fs::write(cwd.join(".jr.toml"), format!("project = \"{project}\"\n")).unwrap();
}

/// Verifies `cwd` has no `.jr.toml` in itself or any ancestor directory —
/// `verification-delta.md` §2 step 2's MUST precondition for every test
/// relying on "no `.jr.toml` ancestor". Panics loudly, naming the
/// offending ancestor, rather than silently skipping: a Rust early-return
/// 'skip' would report PASS and hide the violated precondition.
fn assert_no_ancestor_jr_toml(cwd: &std::path::Path) {
    let mut dir = Some(cwd);
    while let Some(d) = dir {
        let candidate = d.join(".jr.toml");
        assert!(
            !candidate.exists(),
            "hermeticity precondition violated: found unexpected .jr.toml at {candidate:?} \
             (test cwd {cwd:?} must have no ancestor .jr.toml)"
        );
        dir = d.parent();
    }
}

// ── EC-X.7.002-1 (AC-005): both local AND global --project → LOCAL wins ───

/// AC-005 / EC-X.7.002-1: `jr --project GLOBAL user list --project LOCAL`
/// resolves to LOCAL via clap's own global-value propagation — exactly one
/// request carrying `projectKeys=LOCAL`. Classification: GREEN-nonexempt
/// (PRE-EXISTING-BEHAVIOR) — the local `--project LOCAL` flag alone already
/// satisfied the pre-story required `String` field, and continues to pass
/// unchanged against Task 1's stub short-circuit.
#[tokio::test]
async fn test_bc_x_7_002_ec1_both_flags_local_wins() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "LOCAL"))
        .and(query_param("query", ""))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "GLOBAL"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["--project", "GLOBAL", "user", "list", "--project", "LOCAL"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── EC-X.7.002-2 (AC-002): global --project only ───────────────────────────

/// AC-002 / EC-X.7.002-2: `jr --project FOO user list` (no local flag, no
/// configured default) resolves to FOO via clap propagation — the exact
/// invocation issue #862 reported as broken (previously clap exit 2).
/// Classification: WIRING-EXEMPT / GREEN-at-stub — resolves to `Some(...)`
/// via clap propagation alone and short-circuits straight to HTTP via
/// Task 1's stub wiring, never reaching `resolve_user_list_project`.
#[tokio::test]
async fn test_bc_x_7_002_ec2_global_project_only_resolves() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["--project", "FOO", "user", "list"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0 (this is the #862 bug fix itself); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── EC-X.7.002-3 (AC-003): configured default only, split three ways ──────

/// AC-003 / EC-X.7.002-3 sub-cell 1: `.jr.toml`-only. No local/global flag,
/// no profile default configured — the `.jr.toml` project resolves.
/// Classification: RED-at-stub — `project` is `None` at `handle_list`'s
/// entry, reaching the stub's `todo!()` via the child process (exit 101).
#[tokio::test]
async fn test_bc_x_7_002_ec3_jr_toml_only_resolves() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    assert_no_ancestor_jr_toml(cwd.path());
    write_jr_toml(cwd.path(), "JRT");
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "JRT"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["user", "list"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0 with project resolved from .jr.toml; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// AC-003 / EC-X.7.002-3 sub-cell 2: profile-only. No local/global flag, no
/// `.jr.toml` — the active profile's configured `project` default
/// resolves. Classification: RED-at-stub.
#[tokio::test]
async fn test_bc_x_7_002_ec3_profile_default_only_resolves() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;
    write_default_profile_config(config.path(), &server.uri(), Some("FOO"));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["user", "list"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0 with project resolved from the profile default; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// AC-003 / EC-X.7.002-3 sub-cell 3: both a `.jr.toml` project AND a
/// profile default are configured — `.jr.toml` wins (`Config::project_key`'s
/// own fallback order), so the resolved key is JRT, never FOO.
/// Classification: RED-at-stub.
#[tokio::test]
async fn test_bc_x_7_002_ec3_jr_toml_wins_over_profile_default() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    assert_no_ancestor_jr_toml(cwd.path());
    write_jr_toml(cwd.path(), "JRT");
    let server = MockServer::start().await;
    write_default_profile_config(config.path(), &server.uri(), Some("FOO"));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "JRT"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["user", "list"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0 with .jr.toml winning over the profile default; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── EC-X.7.002-5 (AC-009): non-default --profile's own configured default ─

/// AC-009 / EC-X.7.002-5: no local/global flag; `--profile alt` selects a
/// non-default profile with its own configured project default (`ALT`),
/// distinct from the `default` profile's own default (`DEF`) — the
/// resolved key must be `ALT`, proving `handle`/`handle_list` use the
/// already-loaded `&Config` (which reflects the `--profile` selection)
/// rather than reloading config. Classification: RED-at-stub.
#[tokio::test]
async fn test_bc_x_7_002_ec5_non_default_profile_own_default_resolves() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;
    write_two_profile_config(config.path(), &server.uri(), Some("DEF"), Some("ALT"));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "ALT"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "DEF"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["--profile", "alt", "user", "list"])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0 with the `alt` profile's own default (ALT) resolved, not `default`'s (DEF); stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── EC-X.7.002-6 (AC-006): --project "" passes through, config skipped ────

/// AC-006 / EC-X.7.002-6 (D-380): `jr user list --project ""` passes the
/// empty string through as-is — exactly one request whose `projectKeys`
/// value is empty — and does NOT consult the configured profile default
/// (`FOO`), even though one is present. Classification: GREEN-nonexempt
/// (PRE-EXISTING-BEHAVIOR) — the local `--project ""` flag alone already
/// satisfied the pre-story required `String` field with an empty value,
/// and continues to pass unchanged against Task 1's stub short-circuit.
#[tokio::test]
async fn test_bc_x_7_002_ec6_empty_string_project_skips_configured_default() {
    let cache = TempDir::new().unwrap();
    let config = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    assert_no_ancestor_jr_toml(cwd.path());
    let server = MockServer::start().await;
    write_default_profile_config(config.path(), &server.uri(), Some("FOO"));

    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", ""))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/user/assignable/multiProjectSearch"))
        .and(query_param("projectKeys", "FOO"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(fixtures::multi_project_user_search_response(vec![])),
        )
        .expect(0)
        .mount(&server)
        .await;

    let output = jr_cmd(&server.uri(), cache.path(), config.path())
        .args(["user", "list", "--project", ""])
        .current_dir(cwd.path())
        .output()
        .unwrap();

    server.verify().await;
    assert!(
        output.status.success(),
        "expected exit 0 with the empty string passed through, not the configured default; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── AC-008: --help pins the new fallback-order wording ────────────────────

/// AC-008 / VP-USER-LIST-PROJECT-001(d): `jr user list --help` exits 0 and
/// its whitespace-collapsed stdout contains both pinned substrings from
/// BC-X.7.002 Fix step 1's new help text. Never reaches `handle_list`/the
/// resolver — no wiremock server, no async runtime needed. Classification:
/// RED-at-stub — the pinned wording isn't added until Task 8.
#[test]
fn test_bc_x_7_002_help_pins_project_resolution_wording() {
    let output = Command::cargo_bin("jr")
        .unwrap()
        .args(["user", "list", "--help"])
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
        collapsed.contains(
            "Project key (overrides the configured default project). Required when no project is configured in"
        ),
        "expected the pinned fallback-order lead-in substring in --help output; got: {collapsed}"
    );
    assert!(
        collapsed.contains("or the active profile"),
        "expected the pinned \"or the active profile\" substring in --help output; got: {collapsed}"
    );
}
