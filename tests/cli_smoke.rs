use assert_cmd::Command;
use clap::Parser;
use jr::cli::Cli;
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

// --- conflicts_with smoke tests ---

#[test]
fn test_assign_to_and_account_id_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "assign",
            "FOO-1",
            "--to",
            "Jane",
            "--account-id",
            "abc123",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_assign_to_and_unassign_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "assign", "FOO-1", "--to", "Jane", "--unassign"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_assign_account_id_and_unassign_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "assign",
            "FOO-1",
            "--account-id",
            "abc123",
            "--unassign",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_create_to_and_account_id_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "create",
            "-p",
            "FOO",
            "-t",
            "Task",
            "-s",
            "Test",
            "--to",
            "Jane",
            "--account-id",
            "abc123",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_create_description_and_description_stdin_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "create",
            "-p",
            "FOO",
            "-t",
            "Task",
            "-s",
            "Test",
            "--description",
            "text",
            "--description-stdin",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_issue_list_all_and_limit_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "list", "--all", "--limit", "10"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_issue_list_open_and_status_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "list", "--open", "--status", "Done"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_edit_points_and_no_points_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "edit", "FOO-1", "--points", "5", "--no-points"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_project_list_all_and_limit_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["project", "list", "--all", "--limit", "10"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_board_view_all_and_limit_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["board", "view", "--all", "--limit", "10"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_sprint_current_all_and_limit_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["sprint", "current", "--all", "--limit", "10"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_issue_list_created_after_and_recent_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "list",
            "--created-after",
            "2026-03-18",
            "--recent",
            "7d",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

// --- allow_hyphen_values regression tests (issue #471 task-list fix) ---
//
// Live regression anchor: tests/e2e_live.rs::test_e2e_markdown_task_list_produces_task_items
// These hermetic parse tests and the live e2e test are intentionally paired: the unit tests
// prove the parser does not reject the value; the live test proves ADF round-trip correctness.

#[test]
fn test_create_description_leading_dash_value_accepted() {
    // GFM task-list form: --description "- [ ] todo item\n- [x] done item"
    // Before fix: clap rejects with "unexpected argument '- '"
    // After fix:  parses successfully and lands in description field
    // Live anchor: test_e2e_markdown_task_list_produces_task_items (#471)
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "create",
        "-p",
        "FOO",
        "-t",
        "Task",
        "-s",
        "Summary",
        "--description",
        "- [ ] todo item\n- [x] done item",
    ])
    .expect("leading-dash description on issue create must parse (allow_hyphen_values)");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Create { description, .. } = *command {
            assert_eq!(
                description.as_deref(),
                Some("- [ ] todo item\n- [x] done item"),
                "leading-dash description must land in description field, not be absorbed elsewhere"
            );
        } else {
            panic!("expected IssueCommand::Create");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_edit_description_leading_dash_value_accepted() {
    // GFM task-list form on edit path — value must land in description field
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "edit",
        "FOO-1",
        "--description",
        "- [ ] todo item",
    ])
    .expect("leading-dash description on issue edit must parse (allow_hyphen_values)");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Edit { description, .. } = *command {
            assert_eq!(
                description.as_deref(),
                Some("- [ ] todo item"),
                "leading-dash description must land in description field on edit"
            );
        } else {
            panic!("expected IssueCommand::Edit");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_create_summary_leading_dash_value_accepted() {
    // leading-dash summary must land in summary field, not be treated as a flag
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "create",
        "-p",
        "FOO",
        "-t",
        "Task",
        "--summary",
        "- dash summary",
    ])
    .expect("leading-dash summary on issue create must parse (allow_hyphen_values)");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Create { summary, .. } = *command {
            assert_eq!(
                summary.as_deref(),
                Some("- dash summary"),
                "leading-dash summary must land in summary field on create"
            );
        } else {
            panic!("expected IssueCommand::Create");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_edit_summary_leading_dash_value_accepted() {
    // leading-dash summary on edit path
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "edit",
        "FOO-1",
        "--summary",
        "- dash summary",
    ])
    .expect("leading-dash summary on issue edit must parse (allow_hyphen_values)");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Edit { summary, .. } = *command {
            assert_eq!(
                summary.as_deref(),
                Some("- dash summary"),
                "leading-dash summary must land in summary field on edit"
            );
        } else {
            panic!("expected IssueCommand::Edit");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_worklog_add_message_leading_dash_value_accepted() {
    // leading-dash worklog message must land in message field
    let cli = Cli::try_parse_from([
        "jr",
        "worklog",
        "add",
        "FOO-1",
        "1h",
        "--message",
        "- dash message",
    ])
    .expect("leading-dash message on worklog add must parse (allow_hyphen_values)");
    if let jr::cli::Command::Worklog { command } = cli.command {
        if let jr::cli::WorklogCommand::Add { message, .. } = command {
            assert_eq!(
                message.as_deref(),
                Some("- dash message"),
                "leading-dash message must land in message field on worklog add"
            );
        } else {
            panic!("expected WorklogCommand::Add");
        }
    } else {
        panic!("expected Command::Worklog");
    }
}

// F-L3: issue comment positional message accepts leading-dash values
#[test]
fn test_comment_message_leading_dash_value_accepted() {
    // `jr issue comment FOO-1 "- a note"` was rejected with "unexpected argument '- '"
    // before allow_hyphen_values was added to the message positional.
    let cli = Cli::try_parse_from(["jr", "issue", "comment", "FOO-1", "- a note"])
        .expect("leading-dash comment message must parse (allow_hyphen_values)");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { key, message, .. } = *command {
            assert_eq!(key, "FOO-1", "key must be FOO-1");
            assert_eq!(
                message.as_deref(),
                Some("- a note"),
                "leading-dash comment message must land in message field, not be absorbed as a flag"
            );
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

// F-L1: edge cases — exactly "-" and double-dash prefix as literal description value
#[test]
fn test_create_description_single_dash_accepted() {
    // A description value of exactly "-" must parse and land in description, not be
    // treated as a stdin sentinel or short-flag prefix.
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "create",
        "-p",
        "FOO",
        "-t",
        "Task",
        "-s",
        "Summary",
        "--description",
        "-",
    ])
    .expect(r#"description value of "-" must parse (allow_hyphen_values)"#);
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Create { description, .. } = *command {
            assert_eq!(
                description.as_deref(),
                Some("-"),
                r#"description "-" must land in description field"#
            );
        } else {
            panic!("expected IssueCommand::Create");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_create_description_double_dash_prefix_accepted() {
    // A description that starts with "--" (e.g. "--markdown" as literal content)
    // must parse and land in description, not be interpreted as a flag name.
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "create",
        "-p",
        "FOO",
        "-t",
        "Task",
        "-s",
        "Summary",
        "--description",
        "--markdown",
    ])
    .expect(r#"description starting with "--" must parse (allow_hyphen_values)"#);
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Create { description, .. } = *command {
            assert_eq!(
                description.as_deref(),
                Some("--markdown"),
                r#"description "--markdown" must land in description field, not parsed as a flag"#
            );
        } else {
            panic!("expected IssueCommand::Create");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

// F-M1: conflicts_with survives allow_hyphen_values — leading-dash description value
// must NOT suppress the --description / --description-stdin mutual-exclusion guard.
#[test]
fn test_create_description_leading_dash_and_description_stdin_still_conflict() {
    // Even with allow_hyphen_values, clap must still enforce conflicts_with between
    // --description and --description-stdin. The leading-dash value must not fool clap
    // into ignoring the conflict rule.
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "create",
            "-p",
            "FOO",
            "-t",
            "Task",
            "-s",
            "Summary",
            "--description",
            "- [ ] x",
            "--description-stdin",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_edit_description_leading_dash_and_description_stdin_still_conflict() {
    // Same conflict check on the edit path with a leading-dash description value.
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "edit",
            "FOO-1",
            "--description",
            "- [ ] x",
            "--description-stdin",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
