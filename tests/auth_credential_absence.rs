//! Integration tests for S-cycle7-credential-absence-fix.
//!
//! Covers AC-003/AC-004 (clap round-trip, DEFAULT CI), AC-006/AC-007/AC-008
//! (unknown-profile stays exit 64, discriminator, JSON envelope, DEFAULT CI),
//! and the unknown-profile half of AC-007's discriminator.
//!
//! Keyring-gated AC-001/AC-002/AC-005/AC-010 tests live in
//! `src/api/auth.rs`'s inline `#[cfg(test)]` module, consistent with the
//! project convention for tests that require the real keychain backend.

use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::TempDir;

#[allow(dead_code)]
mod common;
use common::assertions::assert_json_error_envelope;

fn jr() -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    // Scrub JR_* env vars that Config::load merges via figment's
    // Env::prefixed("JR_").  Mirrors the pattern in tests/auth_profiles.rs.
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

/// Write a minimal config with a profile named "known" so the unknown-profile
/// guard (`!config.global.profiles.contains_key(&target)`) fires on "ghost".
fn config_with_known_profile(path: &std::path::Path) {
    std::fs::write(
        path,
        r#"
default_profile = "known"
[profiles.known]
url = "https://known.example"
auth_method = "api_token"
"#,
    )
    .unwrap();
}

// ===========================================================================
// AC-003 + AC-004 — clap round-trip (DEFAULT CI, no keychain)
// ===========================================================================

/// AC-003 (BC-1.4.032 postcondition 2 / VP-AUTHDX-027 property a):
/// The remediation string `jr auth login --profile <profile>` parses
/// through the real clap surface and resolves to `AuthCommand::Login`
/// with the Login-LOCAL `profile` field set to the supplied value.
///
/// AC-004 (VP-AUTHDX-027 property a, negative anchor):
/// The OLD positional form `jr auth login <profile>` (no `--profile` flag)
/// does NOT bind `<profile>` to `AuthCommand::Login`'s local `profile`
/// field — either a clap error or the field is `None` and the positional
/// is rejected.
///
/// Both anchors live in the same test so a reviewer sees the contrast.
/// **Test method: DEFAULT CI** — pure `Cli::try_parse_from` round-trip;
/// no keychain access.
#[test]
fn test_bc_1_4_032_remediation_command_parses_against_clap() {
    use clap::Parser;
    use jr::cli::AuthCommand;
    use jr::cli::Cli;
    use jr::cli::Command as JrCommand;

    // AC-003: positive anchor — `--profile default` binds to the
    // Login-LOCAL `profile` field (not the global Cli.profile).
    let parsed = Cli::try_parse_from(["jr", "auth", "login", "--profile", "default"])
        .expect("AC-003: `jr auth login --profile default` must parse without a clap error");

    let login_profile = match parsed.command {
        JrCommand::Auth {
            command: AuthCommand::Login { profile, .. },
        } => profile,
        _ => panic!("AC-003: expected Auth Login command"),
    };
    assert_eq!(
        login_profile,
        Some("default".to_string()),
        "AC-003: AuthCommand::Login's LOCAL `profile` field must be Some(\"default\") \
         when `--profile default` is passed after `login`"
    );

    // AC-004: negative anchor — the OLD positional form `jr auth login default`
    // (no `--profile` flag) does NOT bind `"default"` to Login's local
    // `profile` field.  It either fails to parse (clap error) or the field
    // is `None` and the positional token is rejected/unconsumed.
    // This proves the pre-cycle-007 remediation string was genuinely broken,
    // not merely stylistically different from the new form.
    let result = Cli::try_parse_from(["jr", "auth", "login", "default"]);
    let positional_bound_to_profile = match result {
        Err(_) => false, // clap parse error → the positional was rejected
        Ok(ok_parse) => match ok_parse.command {
            JrCommand::Auth {
                command:
                    AuthCommand::Login {
                        profile: Some(_), ..
                    },
            } => true,
            _ => false,
        },
    };
    assert!(
        !positional_bound_to_profile,
        "AC-004: the positional form `jr auth login default` must NOT bind \
         `\"default\"` to AuthCommand::Login's local `profile` field — \
         this proves the pre-cycle-007 remediation string was broken"
    );
}

// ===========================================================================
// AC-006 — unknown profile stays exit 64 (DEFAULT CI)
// ===========================================================================

/// AC-006 (BC-1.1.004, unchanged — VP-AUTHDX-028 property 1, NEGATIVE pin):
/// `jr auth status --profile <name-not-in-config>` still exits **64** with
/// `JrError::UserError` and stderr containing `unknown profile`.
///
/// This is the regression guard proving the #786 exit-code reclassification
/// in `load_api_token` (exit 64 → exit 2) did NOT over-reach into
/// `src/cli/auth/status.rs`'s unrelated unknown-profile site
/// (PRD delta §5.1 scope-narrowing).
///
/// **Test method: DEFAULT CI** — `status.rs`'s unknown-profile check
/// (`config.global.profiles.contains_key(&target)`) fires BEFORE any
/// credential probe, so no keychain backend is touched or needed.
#[test]
fn test_bc_1_1_004_unknown_profile_stays_exit_64() {
    let (dir, path) = fresh_config_dir();
    config_with_known_profile(&path);

    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "status", "--profile", "ghost"])
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("unknown profile"));
}

// ===========================================================================
// AC-008 — unknown-profile JSON error envelope (DEFAULT CI)
// ===========================================================================

/// AC-008 (BC-1.1.004 unchanged / VP-AUTHDX-028 property 3):
/// `jr auth status --output json --profile <unknown>` emits the STANDARD
/// `{"error": "...", "code": 64}` JSON error envelope and exits 64.
///
/// Independent of this story's own scope, but pinned here because this
/// story's exit-code discriminator work is what AC-008 guards against
/// regressing.
///
/// **Test method: DEFAULT CI** — same reasoning as AC-006.
#[test]
fn test_bc_1_1_004_unknown_profile_json_envelope_is_standard_error_shape() {
    let (dir, path) = fresh_config_dir();
    config_with_known_profile(&path);

    let output = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["--output", "json", "auth", "status", "--profile", "ghost"])
        .output()
        .expect("process must spawn");

    assert_json_error_envelope(&output, 64, "AC-008: unknown-profile JSON envelope");
}

// ===========================================================================
// AC-007 — discriminator (mixed: unknown-profile half DEFAULT CI,
//           credential-absence half is keyring-gated in src/api/auth.rs)
// ===========================================================================

/// AC-007 (VP-AUTHDX-028 property 2, the load-bearing discriminator) —
/// **unknown-profile half** (DEFAULT CI).
///
/// This test is the DEFAULT CI half of the discriminator.  The companion
/// keyring-gated half
/// (`test_auth_credential_absence_vs_unknown_profile_are_distinct_codes`,
/// immediately below) exercises the credential-absence → exit 2 path.
/// Together the two tests in this module assert:
/// - unknown-profile → exit 64 (BC-1.1.004, UNCHANGED)
/// - credential-absence → exit 2 (BC-1.4.032/BC-1.4.033, NEW)
///
/// A mutant that unifies the two sites onto a single exit code (in either
/// direction) fails at least one assertion.
///
/// **Test method: DEFAULT CI** — the unknown-profile check fires before
/// any credential probe; no keychain access required.
#[test]
fn test_auth_unknown_profile_still_exits_64_not_2() {
    let (dir, path) = fresh_config_dir();
    config_with_known_profile(&path);

    // Must remain exit 64, NOT 2 — proves this story's reclassification
    // of the credential-absence branches did NOT touch the unknown-profile
    // site in status.rs.
    jr().env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .args(["auth", "status", "--profile", "ghost"])
        .assert()
        .failure()
        .code(64); // NOT 2
}

/// AC-007 (VP-AUTHDX-028 property 2, the load-bearing discriminator) —
/// **credential-absence half** (keyring-gated).
///
/// This is the companion to `test_auth_unknown_profile_still_exits_64_not_2`
/// above.  Exercises the credential-absence → exit 2 path by spawning `jr`
/// with a profile that exists in config but has no stored credentials in the
/// keychain.  `load_api_token`'s both-absent branch fires before any network
/// call, producing exit 2 (S-cycle7-credential-absence-fix) rather than the
/// old exit 64.
///
/// **Test method: keyring-gated** — `load_api_token` calls
/// `keyring::Entry::get_password()` twice (one per namespaced key), which
/// requires a keychain backend.  On Linux CI without a secret-service daemon
/// this can hang; on macOS it may prompt on novel service names.  The profile
/// name is chosen to be absent from any real keychain, so the call is
/// read-only (no writes, no side-effects).
///
/// Uses a uniquely-named profile so the test does NOT accidentally find real
/// credentials in the developer's keychain.
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn test_auth_credential_absence_vs_unknown_profile_are_distinct_codes() {
    if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
        eprintln!("SKIP: set JR_RUN_KEYRING_TESTS=1 to run keychain tests");
        return;
    }

    // Build a config that has a known profile (so the unknown-profile guard
    // in status.rs does NOT fire) and a unique profile name that will have no
    // stored credentials in the real keychain.
    let (dir, path) = fresh_config_dir();
    std::fs::write(
        &path,
        r#"
default_profile = "jr-cycle7-absent-cred-probe"
[profiles.jr-cycle7-absent-cred-probe]
url = "https://cycle7-test.example"
auth_method = "api_token"
"#,
    )
    .unwrap();

    // Run `jr issue list --no-input` — this triggers JiraClient::from_config
    // → load_api_token, which hits the both-absent branch and must exit 2
    // (S-cycle7-credential-absence-fix) NOT 64 (the old, broken contract).
    // JR_BASE_URL is set to a dummy value; load_api_token fires before any
    // HTTP connect attempt, so no real network call is made.
    let output = jr()
        .env("XDG_CONFIG_HOME", dir.path())
        .env("JR_CONFIG_DIR", dir.path().join("jr"))
        .env("JR_BASE_URL", "http://127.0.0.1:1") // unreachable; auth fires first
        .args(["issue", "list", "--no-input", "--project", "CYCLE7TEST"])
        .output()
        .expect("process must spawn");

    assert_eq!(
        output.status.code(),
        Some(2),
        "AC-007 credential-absence half: exit code must be 2 (not 64) \
         for a profile with no stored credentials; \
         stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
