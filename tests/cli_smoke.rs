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

// F-L3: issue comment add positional message accepts leading-dash values
#[test]
fn test_comment_message_leading_dash_value_accepted() {
    // `jr issue comment add FOO-1 "- a note"` — leading-dash message must land in
    // message field, not be rejected as an unknown flag.
    let cli = Cli::try_parse_from(["jr", "issue", "comment", "add", "FOO-1", "- a note"])
        .expect("leading-dash comment message must parse (allow_hyphen_values)");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Add { key, message, .. } = sub {
                assert_eq!(key, "FOO-1", "key must be FOO-1");
                assert_eq!(
                    message.as_deref(),
                    Some("- a note"),
                    "leading-dash comment message must land in message field, not be absorbed as a flag"
                );
            } else {
                panic!("expected CommentSubcommand::Add");
            }
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

// O-3: remote-link --title accepts leading-dash values
#[test]
fn test_remote_link_title_leading_dash_value_accepted() {
    // `jr issue remote-link PROJ-1 --url https://example.com --title "- important ref"`
    // The --title field is user-authored free text; a value beginning with '-' must land
    // in title, not be treated as an unknown flag.
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "remote-link",
        "PROJ-1",
        "--url",
        "https://example.com",
        "--title",
        "- important ref",
    ])
    .expect("leading-dash title on issue remote-link must parse (allow_hyphen_values)");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::RemoteLink { key, title, .. } = *command {
            assert_eq!(key, "PROJ-1", "key must be PROJ-1");
            assert_eq!(
                title.as_deref(),
                Some("- important ref"),
                "leading-dash title must land in title field, not be absorbed as a flag"
            );
        } else {
            panic!("expected IssueCommand::RemoteLink");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

// O-1: positional + trailing flag interaction — leading-dash comment message followed by
// a named flag must not cause the message to greedily swallow the flag token.
#[test]
fn test_comment_message_leading_dash_followed_by_flag_does_not_swallow_flag() {
    // `jr issue comment add FOO-1 "- a note" --output json`
    // The positional message has allow_hyphen_values; the trailing --output global flag
    // must still parse correctly (output == Json) rather than being consumed as message text.
    let cli = Cli::try_parse_from([
        "jr", "issue", "comment", "add", "FOO-1", "- a note", "--output", "json",
    ])
    .expect("leading-dash comment message with trailing --output flag must parse");
    assert!(
        matches!(cli.output, jr::cli::OutputFormat::Json),
        "--output json must parse correctly after a leading-dash positional message"
    );
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Add { key, message, .. } = sub {
                assert_eq!(key, "FOO-1", "key must be FOO-1");
                assert_eq!(
                    message.as_deref(),
                    Some("- a note"),
                    "message must be the dash value, not the flag token"
                );
            } else {
                panic!("expected CommentSubcommand::Add");
            }
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

// O-2: worklog message-before-duration ordering
// `jr worklog add FOO-1 --message "- dash msg" 1h`
// The --message option with allow_hyphen_values precedes the required `duration` positional.
// Clap must assign "- dash msg" to message and "1h" to duration.
#[test]
fn test_worklog_add_message_before_duration_leading_dash_accepted() {
    let cli = Cli::try_parse_from([
        "jr",
        "worklog",
        "add",
        "FOO-1",
        "--message",
        "- dash msg",
        "1h",
    ])
    .expect("worklog add: --message before duration with leading-dash value must parse");
    if let jr::cli::Command::Worklog { command } = cli.command {
        if let jr::cli::WorklogCommand::Add {
            key,
            duration,
            message,
        } = command
        {
            assert_eq!(key, "FOO-1", "key must be FOO-1");
            assert_eq!(
                message.as_deref(),
                Some("- dash msg"),
                "leading-dash message must land in message field"
            );
            assert_eq!(duration, "1h", "duration must be 1h");
        } else {
            panic!("expected WorklogCommand::Add");
        }
    } else {
        panic!("expected Command::Worklog");
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

// F5-P5-01 regression tests: comment add bool flags must not be absorbed as positional message.
//
// `message` is an `Option<String>` with `allow_hyphen_values = true`, which means a
// missing positional must NOT shadow named flags. Empirically verified (2026-06-10):
//   - `jr issue comment add FOO-1 --stdin`   → message=None, stdin=true  (CORRECT)
//   - `jr issue comment add FOO-1 --markdown` → message=None, markdown=true (CORRECT)
//   - `jr issue comment add FOO-1 --stdin --output json` → message=None, stdin=true (CORRECT)
// These tests convert the nightly-only E2E guarantee into a fast-CI invariant and lock
// the binding against future clap upgrades.

#[test]
fn test_comment_flag_stdin_not_absorbed_as_positional_message() {
    // `jr issue comment add FOO-1 --stdin` — with no positional message, --stdin must
    // set the bool field, NOT be consumed as the message value.
    let cli = Cli::try_parse_from(["jr", "issue", "comment", "add", "FOO-1", "--stdin"])
        .expect("jr issue comment add FOO-1 --stdin must parse successfully");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Add {
                key,
                message,
                stdin,
                ..
            } = sub
            {
                assert_eq!(key, "FOO-1");
                assert_eq!(
                    message, None,
                    "--stdin must NOT be absorbed as message value; message must be None"
                );
                assert!(stdin, "--stdin flag must be true");
            } else {
                panic!("expected CommentSubcommand::Add");
            }
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_comment_flag_markdown_not_absorbed_as_positional_message() {
    // `jr issue comment add FOO-1 --markdown` — with no positional message, --markdown
    // must set the bool field, NOT be consumed as the message value.
    let cli = Cli::try_parse_from(["jr", "issue", "comment", "add", "FOO-1", "--markdown"])
        .expect("jr issue comment add FOO-1 --markdown must parse successfully");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Add {
                key,
                message,
                markdown,
                ..
            } = sub
            {
                assert_eq!(key, "FOO-1");
                assert_eq!(
                    message, None,
                    "--markdown must NOT be absorbed as message value; message must be None"
                );
                assert!(markdown, "--markdown flag must be true");
            } else {
                panic!("expected CommentSubcommand::Add");
            }
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_comment_flag_stdin_with_output_json_not_absorbed_as_positional_message() {
    // `jr issue comment add FOO-1 --stdin --output json` — --stdin and the global
    // --output flag must both parse; message must remain None.
    let cli = Cli::try_parse_from([
        "jr", "issue", "comment", "add", "FOO-1", "--stdin", "--output", "json",
    ])
    .expect("jr issue comment add FOO-1 --stdin --output json must parse successfully");
    assert!(
        matches!(cli.output, jr::cli::OutputFormat::Json),
        "--output json must parse correctly"
    );
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Add {
                key,
                message,
                stdin,
                ..
            } = sub
            {
                assert_eq!(key, "FOO-1");
                assert_eq!(
                    message, None,
                    "--stdin must NOT be absorbed as message value; message must be None"
                );
                assert!(stdin, "--stdin flag must be true");
            } else {
                panic!("expected CommentSubcommand::Add");
            }
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

#[test]
fn test_comment_real_dash_message_with_markdown_flag_both_bind_correctly() {
    // POSITIVE companion: `jr issue comment add FOO-1 "- a note" --markdown`
    // A real leading-dash message AND a trailing bool flag must both bind correctly.
    // Proves that allow_hyphen_values on the positional does not shadow trailing flags
    // when a real message value is present.
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "comment",
        "add",
        "FOO-1",
        "- a note",
        "--markdown",
    ])
    .expect("jr issue comment add FOO-1 \"- a note\" --markdown must parse successfully");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Add {
                key,
                message,
                markdown,
                ..
            } = sub
            {
                assert_eq!(key, "FOO-1");
                assert_eq!(
                    message.as_deref(),
                    Some("- a note"),
                    "leading-dash message must land in message field"
                );
                assert!(
                    markdown,
                    "--markdown flag must be true even when a real message precedes it"
                );
            } else {
                panic!("expected CommentSubcommand::Add");
            }
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

// ── S-577-1 Task-1: Red-Gate tests for CommentSubcommand refactor (issue #577) ──────────────
//
// VP-577-008 / VP-577-014 / VP-577-015 / VP-577-020: MUST FAIL (red) before the
// IssueCommand::Comment enum change lands; MUST PASS (green) after the try_parse
// InvalidSubcommand intercept is in place.
// AC-013 positive: MUST FAIL pre-change; MUST PASS post-change.
// AC-013 negative: GREEN throughout — proves the intercept is scoped to the
// `issue comment` subpath and does not fire for unrelated InvalidSubcommand errors.

/// Build a `jr` subprocess command for CLI smoke red-gate tests.
///
/// Sets `JR_AUTH_HEADER` and `JR_BASE_URL` (non-existent server at port 1) so
/// that invocations which parse successfully pre-change proceed past keychain /
/// config loading, attempt HTTP against a non-existent server, and exit with a
/// non-2 exit code — making the red-gate assertions (exit 2 + migration hint)
/// fail for the right reason before the enum change.
///
/// XDG/JR dir overrides isolate the subprocess from any real config or cache on
/// the test machine.  Does NOT add `--no-input` or `--output` defaults; callers
/// supply all args.
fn jr_cmd_with_xdg(cache_dir: &std::path::Path, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("jr").unwrap();
    cmd.env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .env("JR_BASE_URL", "http://127.0.0.1:1")
        .env("XDG_CACHE_HOME", cache_dir)
        .env("JR_CACHE_DIR", cache_dir.join("jr"))
        .env("XDG_CONFIG_HOME", config_dir)
        .env("JR_CONFIG_DIR", config_dir.join("jr"));
    cmd
}

// VP-577-008 — AC-001
// Red-gate: pre-change `jr issue comment FOO-1 "some text"` parses OK as the leaf
// Comment variant; handler proceeds to HTTP against the non-existent server and
// exits non-2 → assertions on code(2) + migration hint fail (correct red).
// Post-change: try_parse intercept catches InvalidSubcommand → exit 2 + hint.
#[test]
fn test_bc_3_5_012_old_flat_comment_form_exits_2_with_migration_hint() {
    let cache_dir = tempfile::TempDir::new().unwrap();
    let config_dir = tempfile::TempDir::new().unwrap();
    jr_cmd_with_xdg(cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "FOO-1", "some text"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "use `jr issue comment add` instead",
        ));
}

// VP-577-014 — AC-002
// Red-gate: pre-change `jr issue comment` exits 2 (missing required <key>), but
// stderr contains the missing-arg error, not the add/delete/edit/view listing.
// Post-change: Comment is a subcommand group; bare invocation shows the listing
// and does NOT fire the custom InvalidSubcommand hint (MissingSubcommand path).
// Note: the about-text for `IssueCommand::Comment` intentionally says
// "use `jr issue comments`" (plural) — the `.not()` check is for the migration
// hint specifically ("use `jr issue comment add` instead"), not that substring.
#[test]
fn test_bc_3_5_012_bare_comment_emits_clap_listing_not_custom_hint() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "comment"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("add"))
        .stderr(predicate::str::contains("delete"))
        .stderr(predicate::str::contains("edit"))
        .stderr(predicate::str::contains("view"))
        .stderr(predicate::str::contains("use `jr issue comment add` instead").not());
}

// VP-577-015 — AC-003 (list token)
// Red-gate: pre-change `jr issue comment list FOO-1` binds "list" as KEY and
// "FOO-1" as message; handler proceeds to HTTP → non-2 exit → assertions fail.
// Post-change: "list" triggers InvalidSubcommand intercept → exit 2 + plural hint.
#[test]
fn test_bc_3_5_012_comment_list_token_emits_plural_hint() {
    let cache_dir = tempfile::TempDir::new().unwrap();
    let config_dir = tempfile::TempDir::new().unwrap();
    jr_cmd_with_xdg(cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "list", "FOO-1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("jr issue comments"));
}

// VP-577-020 — AC-003 (ls token + mixed-case)
// Red-gate: pre-change "ls" and "LS" bind as KEY; same HTTP-fail failure mode as
// VP-577-015. Post-change: eq_ignore_ascii_case match → plural hint for both forms.
#[test]
fn test_bc_3_5_012_comment_ls_mixed_case_emits_plural_hint() {
    let cache_dir = tempfile::TempDir::new().unwrap();
    let config_dir = tempfile::TempDir::new().unwrap();
    // lowercase "ls"
    jr_cmd_with_xdg(cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "ls", "FOO-1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("jr issue comments"));
    // UPPERCASE "LS" — case-insensitive match via eq_ignore_ascii_case (EC-3.5.012-1)
    jr_cmd_with_xdg(cache_dir.path(), config_dir.path())
        .args(["issue", "comment", "LS", "FOO-1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("jr issue comments"));
}

// AC-013 positive — EC-011
// Red-gate: pre-change `jr --output json issue comment FOO-1 "text"` parses
// --output json as a global flag, then Comment leaf variant parses key=FOO-1,
// message="text"; handler proceeds to HTTP → non-2 exit → assertions fail.
// Post-change: intercept uses err.context() (NOT argv positional scanning) to
// detect `issue comment` under a global-flag-first invocation → exit 2 + hint.
#[test]
fn test_bc_3_5_012_global_flag_before_comment_uses_context_interception() {
    let cache_dir = tempfile::TempDir::new().unwrap();
    let config_dir = tempfile::TempDir::new().unwrap();
    jr_cmd_with_xdg(cache_dir.path(), config_dir.path())
        .args(["--output", "json", "issue", "comment", "FOO-1", "text"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "use `jr issue comment add` instead",
        ));
}

// AC-013 negative — EC-010 (GREEN throughout)
// `jr issue foo BAR-1` triggers InvalidSubcommand under `issue`, NOT under
// `issue comment` — the custom intercept must NOT fire here.  Clap renders its
// own error. GREEN pre-change (no migration hint existed then either) and GREEN
// post-change (intercept is scoped to the `issue comment` subpath only).
#[test]
fn test_bc_3_5_012_non_comment_invalid_subcommand_no_migration_hint() {
    Command::cargo_bin("jr")
        .unwrap()
        .args(["issue", "foo", "BAR-1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("use `jr issue comment add` instead").not());
}

// ── VP-577-018 / VP-577-019 / AC-012 — parse-level tests for CommentSubcommand ──────────────
//
// VP-577-018 (BC-3.5.012, allow_hyphen_values on comment add): `jr issue comment add`
// accepts a leading-dash positional body.  Parallel to the migrated F-L3 tests, but
// named under the BC-3.5.012 namespace for traceability.
#[test]
fn test_bc_3_5_012_comment_add_allows_leading_dash_body() {
    let cli = Cli::try_parse_from(["jr", "issue", "comment", "add", "FOO-1", "- a task"])
        .expect("comment add with leading-dash body must parse successfully");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Add { key, message, .. } = sub {
                assert_eq!(key, "FOO-1");
                assert_eq!(
                    message.as_deref(),
                    Some("- a task"),
                    "leading-dash body must land in message, not be absorbed as a flag"
                );
            } else {
                panic!("expected CommentSubcommand::Add");
            }
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

// VP-577-019 (BC-3.5.012, allow_hyphen_values on comment edit): `jr issue comment edit`
// accepts a leading-dash positional body text.
#[test]
fn test_bc_3_5_012_comment_edit_allows_leading_dash_body() {
    let cli = Cli::try_parse_from([
        "jr",
        "issue",
        "comment",
        "edit",
        "FOO-1",
        "- updated note",
        "--id",
        "10001",
    ])
    .expect("comment edit with leading-dash body must parse successfully");
    if let jr::cli::Command::Issue { command } = cli.command {
        if let jr::cli::IssueCommand::Comment { command: sub } = *command {
            if let jr::cli::CommentSubcommand::Edit { key, text, id, .. } = sub {
                assert_eq!(key, "FOO-1");
                assert_eq!(id, "10001");
                assert_eq!(
                    text.as_deref(),
                    Some("- updated note"),
                    "leading-dash body must land in text, not be absorbed as a flag"
                );
            } else {
                panic!("expected CommentSubcommand::Edit");
            }
        } else {
            panic!("expected IssueCommand::Comment");
        }
    } else {
        panic!("expected Command::Issue");
    }
}

// AC-012 parse-level mutual-exclusion tests for `jr issue comment edit`.
// Clap rejects conflicting flags before the handler runs, so these are
// safe to test against the binary even with todo!() stubs.

// EC-2a: --file and --stdin are mutually exclusive on comment edit.
#[test]
fn test_bc_3_5_009_ec2_edit_file_and_stdin_exit_2() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue", "comment", "edit", "FOO-1", "--id", "10001", "--file", "foo.txt", "--stdin",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}

// EC-3: positional text and --file are mutually exclusive on comment edit.
#[test]
fn test_bc_3_5_009_ec3_edit_file_and_positional_exit_2() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue", "comment", "edit", "FOO-1", "text", "--id", "10001", "--file", "foo.txt",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}

// EC-4: positional text and --stdin are mutually exclusive on comment edit.
#[test]
fn test_bc_3_5_009_ec4_edit_stdin_and_positional_exit_2() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue", "comment", "edit", "FOO-1", "text", "--id", "10001", "--stdin",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}

// BC-3.5.011: --internal and --public are mutually exclusive on comment edit.
#[test]
fn test_bc_3_5_011_edit_internal_and_public_both_exit_2() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "issue",
            "comment",
            "edit",
            "FOO-1",
            "--id",
            "10001",
            "--internal",
            "--public",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}
