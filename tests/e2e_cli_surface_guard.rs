//! CLI surface smoke tests for the commands exercised by the E2E coverage-v2 tests.
//!
//! These tests run in normal `cargo test` (no gate required) and verify that
//! the CLI flags for `issue link`, `issue unlink`, `issue link-types`, and
//! `issue remote-link` are registered and visible in `--help` output. They
//! exist as a fast, always-run guard so that refactors which accidentally
//! drop or rename flags fail immediately without needing a live Jira site.

use assert_cmd::Command;
use predicates::prelude::*;

/// `jr issue link --help` must mention both positional key args and `--type`.
#[test]
fn test_issue_link_help_shows_type_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "link", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--type"));
}

/// `jr issue link --help` must describe the link direction.
#[test]
fn test_issue_link_help_shows_description() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "link", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Link two issues"));
}

/// `jr issue unlink --help` must mention `--type` (type-scoped unlink flag).
#[test]
fn test_issue_unlink_help_shows_type_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "unlink", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--type"));
}

/// `jr issue link-types --help` exits 0 (command is registered).
#[test]
fn test_issue_link_types_help_exits_ok() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "link-types", "--help"])
        .assert()
        .success();
}

/// `jr issue remote-link --help` must mention `--url` (required flag).
#[test]
fn test_issue_remote_link_help_shows_url_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "remote-link", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--url"));
}

/// `jr issue remote-link --help` must mention `--title` (optional flag).
#[test]
fn test_issue_remote_link_help_shows_title_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "remote-link", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--title"));
}

/// `jr issue edit --help` must mention `--label` (add/remove syntax).
#[test]
fn test_issue_edit_help_shows_label_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "edit", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--label"));
}
