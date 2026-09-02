//! Integration tests for multi-profile auth workflows.

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
mod common;
use common::assertions::assert_json_error_envelope;
use predicates::prelude::PredicateBooleanExt;

fn jr() -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    // Scrub every JR_* env var that Config::load merges via figment's
    // Env::prefixed("JR_"). Without this, a developer with a direnv-set
    // JR_INSTANCE_URL / JR_PROFILE / etc. would have those values flow
    // into the test's "fresh install" config and either trigger legacy
    // migration unexpectedly or make assertions about empty profiles
    // fail. Pinning the list here so future JR_* additions don't
    // silently re-introduce flakiness on dev machines.
    cmd.env_remove("JR_CONFIG_DIR")
        .env_remove("JR_CACHE_DIR")
        .env_remove("JR_PROFILE")
        .env_remove("JR_DEFAULT_PROFILE")
        .env_remove("JR_INSTANCE_URL")
        .env_remove("JR_INSTANCE_AUTH_METHOD")
        .env_remove("JR_INSTANCE_CLOUD_ID")
        .env_remove("JR_INSTANCE_ORG_ID")
        .env_remove("JR_INSTANCE_OAUTH_SCOPES")
        .env_remove("JR_FIELDS_TEAM_FIELD_ID")
        .env_remove("JR_FIELDS_STORY_POINTS_FIELD_ID")
        .env_remove("JR_DEFAULTS_OUTPUT")
        .env_remove("JR_BASE_URL")
        .env_remove("JR_AUTH_HEADER")
        .env_remove("JR_EMAIL")
        .env_remove("JR_API_TOKEN")
        .env_remove("JR_OAUTH_CLIENT_ID")
        .env_remove("JR_OAUTH_CLIENT_SECRET");
    cmd
}

fn fresh_config_dir() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let cfg = dir.path().join("jr").join("config.toml");
    std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    (dir, cfg)
}

#[test]
fn auth_switch_unknown_profile_exits_64() {
    let (dir, _path) = fresh_config_dir();
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "switch", "ghost"])
        .assert()
        .failure()
        .code(64);
}

#[test]
fn auth_list_shows_no_profiles_for_fresh_install() {
    let (dir, _path) = fresh_config_dir();
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicates::str::contains("[]"));
}

/// Regression: `jr auth status` against a fresh install (no [profiles]
/// in config — or no config at all) must succeed with a "not configured"
/// message, not error with "unknown profile". Setup scripts and CI use
/// `auth status` as a first-run probe before deciding whether to call
/// `jr init` or `jr auth login`.
#[test]
fn auth_status_fresh_install_no_profiles_succeeds() {
    let (dir, _path) = fresh_config_dir(); // no config.toml written
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "status"])
        .assert()
        .success()
        .stderr(predicates::str::contains("No profiles configured"));
}

#[test]
fn auth_status_unknown_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.example"
auth_method = "api_token"
"#,
    )
    .unwrap();
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "status", "--profile", "ghost"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

#[test]
fn auth_logout_unknown_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "logout", "--profile", "ghost"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

#[test]
fn auth_remove_active_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://x.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "remove", "default", "--no-input"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("cannot remove active"));
}

#[test]
fn precedence_flag_overrides_env_overrides_config() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "from-config"
[profiles.from-config]
url = "https://from-config.example"
[profiles.from-env]
url = "https://from-env.example"
[profiles.from-flag]
url = "https://from-flag.example"
"#,
    )
    .unwrap();

    let out = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .env("JR_PROFILE", "from-env")
        .args(["--profile", "from-flag", "auth", "list", "--output", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|_| panic!("parse: {stdout}"));
    let active: Vec<&serde_json::Value> = parsed
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["active"].as_bool() == Some(true))
        .collect();
    assert_eq!(
        active.len(),
        1,
        "expected exactly one active profile, got {}: {parsed:?}",
        active.len()
    );
    assert_eq!(active[0]["name"], "from-flag");
}

/// Regression (Copilot round-10): the global `--profile` flag was being
/// dropped by `auth status`, `auth login`, `auth refresh`, and `auth logout`
/// because each handler reloaded config internally and only saw the
/// subcommand-level `--profile`. main.rs now composes an effective profile
/// (`subcmd.profile.or(cli.profile)`) so the global flag propagates.
///
/// Gated behind `JR_RUN_KEYRING_TESTS=1` because `auth status` reaches
/// `load_api_token(&target)` → `keyring::Entry::get_password()`, which can
/// block under Keychain contention on macOS or hang on Linux CI without a
/// secret-service daemon (#526-F6-KEYRING-GATE).
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn global_profile_flag_targets_auth_status() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.sandbox]
url = "https://sandbox.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // Global `--profile sandbox` without subcommand-level `--profile`.
    // Status output must reflect sandbox, not default.
    let out = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "sandbox", "auth", "status"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("sandbox") || combined.contains("https://sandbox.example"),
        "global --profile flag should target sandbox; got: {combined}"
    );
}

/// Ungated substitute coverage for the global-`--profile`→subcommand fallback
/// fork in `src/main.rs` (the `effective_profile = subcmd.profile.or_else(||
/// cli.profile.clone())` branch in `AuthCommand::Status`).
///
/// # Why this test exists
///
/// `global_profile_flag_targets_auth_status` (above) was keyring-gated in
/// `#526-F6-KEYRING-GATE` because `auth status` against an existing profile
/// reaches `load_api_token(&target)` → `keyring::Entry::get_password()`,
/// which can block on CI without a secret-service daemon. That left the
/// global-flag propagation path with ZERO default-CI coverage.
///
/// This test recovers that coverage without touching the keychain by exploiting
/// the strict active-profile-existence guard in `Config::load_with` (called as
/// the first statement in `src/cli/auth/status.rs::status()`). When
/// `Config::load_with(Some("ghost"))` is invoked, `load_inner(strict=true)`
/// checks whether the resolved active profile exists in `[profiles]`; because
/// `"ghost"` is absent, it returns `JrError::UserError("unknown profile: …")`
/// before any credential probe occurs. The `contains_key` guard inside
/// `status.rs` is a redundant second backstop for the explicit `--profile` path
/// but is never reached here — the config-load boundary fires first.
///
/// If the global `--profile ghost` flag were dropped (not propagated from
/// `cli.profile` into `effective_profile`), the active profile would fall back
/// to `"default"` (which exists in the test config), `Config::load_with` would
/// succeed, the status handler would proceed to the keyring probe, and the
/// process would exit with a different code — NOT 64. Therefore exit 64 proves
/// the global flag was propagated all the way into `Config::load_with`.
#[test]
fn test_global_profile_flag_propagates_to_auth_status_unknown_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.sandbox]
url = "https://sandbox.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // Global `--profile ghost` positioned BEFORE the subcommand — this is the
    // fork that main.rs::run() handles with `effective_profile =
    // profile.or_else(|| cli.profile.clone())` for AuthCommand::Status.
    // "ghost" is NOT in [profiles], so the unknown-profile guard in status.rs
    // fires before any keyring probe and exits 64.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "status"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

/// Ungated coverage for the global-`--profile`→subcommand fallback in
/// `src/main.rs` for `AuthCommand::Logout`.
///
/// Mirrors `test_global_profile_flag_propagates_to_auth_status_unknown_profile_exits_64`.
/// `handle_logout` calls `Config::load_with(Some("ghost"))` (strict load) first,
/// which surfaces "unknown profile: ghost" before any keyring probe. Exit 64 proves
/// that `main.rs` composed `effective_profile = profile.or_else(|| cli.profile.clone())`
/// and passed it to `handle_logout` — if the global flag were dropped the active
/// profile "default" would succeed the load and the exit code would differ.
#[test]
fn test_global_profile_flag_propagates_to_auth_logout_unknown_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "logout"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

/// Ungated coverage for the global-`--profile`→subcommand fallback in
/// `src/main.rs` for `AuthCommand::Refresh`.
///
/// `refresh_credentials` calls `Config::load_with(Some("ghost"))` (strict load)
/// as its first statement, which surfaces "unknown profile: ghost" before any
/// credential access. Exit 64 proves global `--profile` reached `refresh_credentials`.
#[test]
fn test_global_profile_flag_propagates_to_auth_refresh_unknown_profile_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "refresh", "--no-input"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

/// Ungated coverage for the global-`--profile`→subcommand fallback in
/// `src/main.rs` for `AuthCommand::Login`.
///
/// Unlike logout/refresh, `handle_login` uses `Config::load_lenient_with`
/// (it must create new profiles), so the "unknown profile" path is not
/// triggered here. Instead this test uses `--no-input` without `--url` to
/// hit `prepare_login_target`'s "--url required" guard (exit 64), which is
/// reached ONLY when `effective_profile` is propagated and the target profile
/// (ghost) has no URL configured. If the global flag were dropped, the fallback
/// active profile "default" already has a URL, so the guard would not fire and
/// exit 0.
#[test]
fn test_global_profile_flag_propagates_to_auth_login_no_url_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // ghost is not in [profiles]; no --url; --no-input cannot prompt → exit 64
    // "--url required when the target profile has no URL configured"
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "login", "--no-input"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("--url required"));
}

/// Regression: round-4's unified active-profile existence check at
/// `Config::load` time broke `jr auth login --profile newprof --url ...`
/// because the profile didn't exist yet. `handle_login` now uses
/// `Config::load_lenient` to skip that check, restoring the documented
/// "login creates profile if absent" behavior.
///
/// Gated behind `JR_RUN_KEYRING_TESTS=1` because `login_token` writes the
/// shared API token to the keyring, which Linux CI may not have.
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn auth_login_creates_new_profile_with_url() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://existing.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // login --profile newprof should succeed and create the profile,
    // even though newprof isn't in [profiles] yet at load time.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .env("JR_EMAIL", "user@example.com")
        .env("JR_API_TOKEN", "token-value")
        .args([
            "auth",
            "login",
            "--profile",
            "newprof",
            "--url",
            "https://newprof.example",
            "--no-input",
        ])
        .assert()
        .success();

    // Verify the profile was added to config.
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("[profiles.newprof]"), "saved: {saved}");
    assert!(saved.contains("https://newprof.example"), "saved: {saved}");
}

/// Regression: when `JR_PROFILE` points at a profile that doesn't exist
/// in `[profiles]` AND the user runs `jr auth login --profile <other>`
/// to create that other profile, login must still succeed. Round-5 found
/// that `login_token`/`login_oauth` reloaded config via strict
/// `Config::load()` after `handle_login`'s lenient load, which re-fired
/// the unknown-active-profile check on the unrelated `JR_PROFILE` value
/// and aborted the in-flight creation. Both internal reloads now use
/// `load_lenient` to match the orchestrator.
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn auth_login_with_jr_profile_pointing_to_unrelated_profile_still_creates_target() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://existing.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // JR_PROFILE points to a non-existent profile, but --profile points
    // to a different new profile that login should create. Login must
    // succeed despite the JR_PROFILE mismatch — login uses lenient load
    // throughout so the strict active-profile existence check never
    // fires for the in-flight creation.
    Command::cargo_bin("jr")
        .unwrap()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .env("JR_PROFILE", "ghost")
        .env("JR_EMAIL", "user@example.com")
        .env("JR_API_TOKEN", "token-value")
        .args([
            "auth",
            "login",
            "--profile",
            "fresh",
            "--url",
            "https://fresh.example",
            "--no-input",
        ])
        .assert()
        .success();

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("[profiles.fresh]"), "saved: {saved}");
}

// ---------------------------------------------------------------------------
// S-663-1 — `auth switch --profile <X>` exit-64 guard (issue #663).
// BC-1.2.047 (NEW) + BC-1.2.018 (AMENDED carve-out).
//
// RED GATE: the guard does not exist yet in `src/main.rs`'s
// `AuthCommand::Switch` dispatch arm — these tests are expected to FAIL
// (wrong exit code / missing stderr text) until it is implemented.
// ---------------------------------------------------------------------------

/// The fixed guard message (BC-1.2.047 Postcondition 3) — a substring
/// pinned across every AC-1/3/4 test below.
const GUARD_MESSAGE_SUBSTR: &str = "--profile is not valid for 'auth switch'";

/// AC-1 (BC-1.2.047 Postconditions 2/3, VP-663-001): basic rejection, human
/// mode. `jr auth switch --profile foo foo` (both existing profiles) exits
/// 64 with the guard message on stderr, and does not write config.toml
/// (mtime unchanged — proves the guard fires before `handle_switch`'s write
/// path runs at all).
#[test]
fn test_bc_1_2_047_auth_switch_with_profile_flag_exits_64() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.foo]
url = "https://foo.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    let mtime_before = std::fs::metadata(&path).unwrap().modified().unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "switch", "--profile", "foo", "foo"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR));

    let mtime_after = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        mtime_before, mtime_after,
        "config.toml must not be written when the --profile guard rejects the command"
    );
}

/// AC-2 (BC-1.2.047 Postcondition 4, VP-663-002): `--output json` error
/// envelope. stdout MUST be empty; stderr parses as JSON with keys
/// {"error","code"}; code == 64. Channel-separation invariant (#526).
#[test]
fn test_bc_1_2_047_auth_switch_with_profile_flag_json_error_envelope_stderr_stdout_empty() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.foo]
url = "https://foo.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    let out = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args([
            "auth",
            "switch",
            "--profile",
            "foo",
            "foo",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert_json_error_envelope(
        &out,
        64,
        "S-663-1 auth switch --profile guard JSON envelope",
    );
}

/// AC-3 (BC-1.2.047 Postcondition 1/5, EC-1.2.047-2): the guard fires
/// BEFORE `Config::load_with`'s active-profile existence check is
/// reachable. `--profile bogus` (a profile that does not exist) must
/// still produce THIS guard's message, never an "unknown profile"
/// existence-check message — and must not touch config.toml.
#[test]
fn test_bc_1_2_047_auth_switch_guard_fires_before_config_load_no_existence_check() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.realprofile]
url = "https://real.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    let mtime_before = std::fs::metadata(&path).unwrap().modified().unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "switch", "--profile", "bogus", "realprofile"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR))
        .stderr(predicates::str::contains("unknown profile").not());

    let mtime_after = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        mtime_before, mtime_after,
        "config.toml must not be written when the bogus --profile value is rejected by the guard"
    );
}

/// AC-4 (BC-1.2.047 EC-1.2.047-1/-3): the "confusing incantation" the
/// issue reports (`--profile foo foo`, both values coinciding with a real
/// profile) still rejects; and the guard is order-independent — putting
/// `--profile` AFTER the positional target rejects identically.
#[test]
fn test_bc_1_2_047_auth_switch_profile_flag_rejected_regardless_of_order_or_value() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.foo]
url = "https://foo.example"
auth_method = "api_token"
[profiles.realprofile]
url = "https://real.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // The confusing incantation: --profile foo foo, both real, coincide.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "switch", "--profile", "foo", "foo"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR));

    // Flag placed AFTER the positional target — clap's global-arg parsing
    // accepts either order; the guard must reject identically.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "switch", "realprofile", "--profile", "bogus"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR));
}

/// NIT-4 (pr-reviewer, PR #696): the leading-flag form — `--profile` placed
/// BEFORE the `auth switch` subcommand entirely (`jr --profile X auth switch
/// Y`) — is accepted by clap (it's a global arg) and must reject identically
/// to the subcommand-position forms already covered above. This closes the
/// third of three argv shapes clap accepts for a `global = true` flag:
/// before the subcommand, after it, and interleaved with the positional.
#[test]
fn test_bc_1_2_047_auth_switch_profile_flag_rejected_when_leading_before_subcommand() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.realprofile]
url = "https://real.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "realprofile", "auth", "switch", "realprofile"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR));
}

/// AC-5 (BC-1.2.047 EC-1.2.047-4, VP-663-003 — protects the direnv-scoped
/// sandbox workflow): `JR_PROFILE=sandbox` alone (the global `--profile`
/// FLAG absent) must NOT trip the guard. The switch proceeds normally.
#[test]
fn test_bc_1_2_047_auth_switch_jr_profile_env_var_not_rejected() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.sandbox]
url = "https://sandbox.example"
auth_method = "api_token"
[profiles.realprofile]
url = "https://real.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // jr() scrubs JR_PROFILE by default (see its doc comment); set it
    // explicitly here to simulate a direnv-scoped sandbox export. No
    // global `--profile` flag is passed on the command line.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .env("JR_PROFILE", "sandbox")
        .args(["auth", "switch", "realprofile"])
        .assert()
        .success()
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR).not());

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(
        saved.contains("default_profile = \"realprofile\""),
        "expected switch to realprofile to succeed and persist; saved: {saved}"
    );
}

/// AC-6 (BC-1.2.047 EC-1.2.047-5): a syntactically-INVALID `--profile`
/// value is pre-empted by the EARLIER `config::validate_profile_name`
/// check in `run()` (before command dispatch) — it must exit 64 via the
/// charset-violation message, NOT this BC's "not valid for 'auth switch'"
/// message. A test asserting the wrong message here tests the wrong layer.
#[test]
fn test_bc_1_2_047_auth_switch_charset_invalid_profile_preempted_by_validate_profile_name() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
[profiles.realprofile]
url = "https://real.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "switch", "--profile", "in!valid", "realprofile"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains(
            "Profile name contains invalid characters (use a-z, 0-9, -, _)",
        ))
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR).not());
}

/// AC-7 (BC-1.2.047 Invariant 1, BC-1.2.018 amended): `Login`/`Status`/
/// `Refresh`/`Logout` are unaffected by this story's guard — each
/// continues to compose `subcmd.profile.or(cli.profile)` exactly as
/// before. Regression pin: none of the four emit the new Switch-only
/// guard message, and their pre-existing global-`--profile`-propagation
/// behavior (exit 64 via their own downstream checks) is unchanged.
#[test]
fn test_bc_1_2_018_auth_login_status_refresh_logout_profile_composition_unaffected() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // status: global --profile ghost -> unknown-profile exit 64 (unchanged
    // BC-1.2.018 composition path), never the Switch-only guard message.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "status"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"))
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR).not());

    // logout: same pattern.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "logout"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"))
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR).not());

    // refresh: same pattern (--no-input required, non-interactive).
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "refresh", "--no-input"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"))
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR).not());

    // login: uses lenient load (no "unknown profile" check) — hits the
    // "--url required" guard instead when the target profile has no URL.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--profile", "ghost", "auth", "login", "--no-input"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("--url required"))
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR).not());
}

/// AC-8 (BC-1.2.018 LOW-2 clarification): `List`/`Remove` continue to
/// pass `cli.profile.as_deref()` straight through (no `.or()`
/// composition) — `--profile` is fully HONORED, never rejected, on
/// either subcommand. `auth switch` remains the sole subcommand where
/// `--profile` is rejected.
#[test]
fn test_bc_1_2_018_auth_list_remove_profile_flag_still_honored_not_rejected() {
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "default"
[profiles.default]
url = "https://default.example"
[profiles.sandbox]
url = "https://sandbox.example"
[profiles.other]
url = "https://other.example"
"#,
    )
    .unwrap();

    // list: --profile sandbox honored directly (no .or() composition) —
    // "sandbox" is reported as the active profile, and the guard message
    // never appears.
    let out = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "list", "--profile", "sandbox", "--output", "json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&out.stderr).contains(GUARD_MESSAGE_SUBSTR),
        "auth list --profile must never emit the Switch-only guard message"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|_| panic!("parse: {stdout}"));
    let active: Vec<&serde_json::Value> = parsed
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["active"].as_bool() == Some(true))
        .collect();
    assert_eq!(
        active.len(),
        1,
        "expected exactly one active profile: {parsed:?}"
    );
    assert_eq!(active[0]["name"], "sandbox");

    // remove: --profile sandbox honored directly, removing an unrelated
    // target ("other") that is neither the resolved active profile nor
    // the persisted default_profile — succeeds, never rejected.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args([
            "auth",
            "remove",
            "other",
            "--profile",
            "sandbox",
            "--no-input",
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains(GUARD_MESSAGE_SUBSTR).not());

    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(
        !saved.contains("[profiles.other]"),
        "expected 'other' profile to be removed; saved: {saved}"
    );
}
