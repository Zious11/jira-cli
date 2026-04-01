use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("A fast CLI for Jira Cloud"));
}

#[test]
fn test_version_flag() {
    Command::cargo_bin("jr")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("jr"));
}

#[test]
fn test_no_args_shows_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_edit_description_and_description_stdin_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "edit",
            "FOO-1",
            "--description",
            "text",
            "--description-stdin",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_assets_tickets_open_and_status_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "tickets", "OBJ-1", "--open", "--status", "Done"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
