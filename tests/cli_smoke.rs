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

#[test]
fn test_queue_view_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["queue", "view", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("View issues in a queue"))
        .stdout(predicate::str::contains("--limit"));
}

#[test]
fn test_queue_list_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["queue", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List queues"));
}

#[test]
fn test_assets_view_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "view", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--no-attributes"));
}

#[test]
fn test_sprint_add_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "add", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Add issues to a sprint"))
        .stdout(predicate::str::contains("--sprint"))
        .stdout(predicate::str::contains("--current"))
        .stdout(predicate::str::contains("--board"));
}

#[test]
fn test_sprint_remove_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "remove", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Remove issues from sprint"))
        .stdout(predicate::str::contains("ISSUES"));
}

#[test]
fn test_sprint_add_sprint_and_current_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "add", "--sprint", "100", "--current", "FOO-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_sprint_add_requires_sprint_or_current() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "add", "FOO-1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--sprint"));
}

#[test]
fn test_assets_schemas_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "schemas", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List object schemas"));
}

#[test]
fn test_assets_types_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "types", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List object types"))
        .stdout(predicate::str::contains("--schema"));
}

#[test]
fn test_assets_schema_help() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["assets", "schema", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show attributes"))
        .stdout(predicate::str::contains("--schema"));
}
