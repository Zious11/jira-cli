# Assets Tickets Status Filtering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--open` and `--status` client-side filtering flags to `jr assets tickets`.

**Architecture:** The Assets connected-tickets API has no server-side filtering. After fetching all tickets, filter client-side by status category color (`--open`) or status name (`--status`) before applying `--limit` truncation and display. Reuses the existing `partial_match` module for `--status` disambiguation.

**Tech Stack:** Rust, clap, serde_json (tests)

**Spec:** `docs/specs/assets-tickets-status-filter.md`

---

### Task 1: Add `--open` and `--status` CLI flags

**Files:**
- Modify: `src/cli/mod.rs:128-135` (AssetsCommand::Tickets)

- [ ] **Step 1: Add the flags**

In `src/cli/mod.rs`, update `AssetsCommand::Tickets` to add `--open` and `--status` with `conflicts_with`:

```rust
    /// Show Jira issues connected to an asset
    Tickets {
        /// Object key (e.g. OBJ-1) or numeric ID
        key: String,
        /// Maximum number of tickets to show
        #[arg(long)]
        limit: Option<u32>,
        /// Show only open tickets (excludes Done status category)
        #[arg(long, conflicts_with = "status")]
        open: bool,
        /// Filter by status (partial match supported)
        #[arg(long, conflicts_with = "open")]
        status: Option<String>,
    },
```

- [ ] **Step 2: Verify compilation**

Run: `cargo build`
Expected: Compiles (handle_tickets call site will need updating — that's Task 2).

Note: This will cause a compilation error because `handle_tickets` in `assets.rs` doesn't accept the new fields yet. If it fails, that's expected — Task 2 fixes it.

- [ ] **Step 3: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: add --open and --status flags to assets tickets (#89)"
```

---

### Task 2: Wire flags into `handle_tickets` and add filtering

**Files:**
- Modify: `src/cli/assets.rs:8-38` (handle dispatch) and `src/cli/assets.rs:171-225` (handle_tickets)

- [ ] **Step 1: Update the dispatch in `handle`**

In `src/cli/assets.rs`, update the `Tickets` match arm to pass the new fields:

```rust
        AssetsCommand::Tickets {
            key,
            limit,
            open,
            status,
        } => handle_tickets(&workspace_id, &key, limit, open, status, output_format, client).await,
```

- [ ] **Step 2: Update `handle_tickets` signature and add filtering**

Replace the entire `handle_tickets` function:

```rust
async fn handle_tickets(
    workspace_id: &str,
    key: &str,
    limit: Option<u32>,
    open: bool,
    status: Option<String>,
    output_format: &OutputFormat,
    client: &JiraClient,
) -> Result<()> {
    let object_id = objects::resolve_object_key(client, workspace_id, key).await?;
    let resp = client
        .get_connected_tickets(workspace_id, &object_id)
        .await?;

    // Apply status filtering before limit
    let filtered = filter_tickets(resp.tickets, open, status.as_deref())?;

    // Apply limit
    let tickets: Vec<_> = match limit {
        Some(n) => filtered.into_iter().take(n as usize).collect(),
        None => filtered,
    };

    match output_format {
        OutputFormat::Json => {
            println!("{}", output::render_json(&tickets)?);
        }
        OutputFormat::Table => {
            let rows: Vec<Vec<String>> = tickets
                .iter()
                .map(|t| {
                    vec![
                        t.key.clone(),
                        t.issue_type
                            .as_ref()
                            .map(|it| it.name.clone())
                            .unwrap_or_else(|| "\u{2014}".into()),
                        t.title.clone(),
                        t.status
                            .as_ref()
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| "\u{2014}".into()),
                        t.priority
                            .as_ref()
                            .map(|p| p.name.clone())
                            .unwrap_or_else(|| "\u{2014}".into()),
                    ]
                })
                .collect();

            output::print_output(
                output_format,
                &["Key", "Type", "Title", "Status", "Priority"],
                &rows,
                &tickets,
            )?;
        }
    }
    Ok(())
}
```

Note: JSON output now returns the filtered `tickets` array (not the full `ConnectedTicketsResponse` with `allTicketsQuery`). This is consistent — when you filter, the `allTicketsQuery` JQL no longer represents what's shown.

- [ ] **Step 3: Add the `filter_tickets` function**

Add above `handle_tickets` in `src/cli/assets.rs`:

```rust
use crate::error::JrError;
use crate::partial_match::{self, MatchResult};
use crate::types::assets::ConnectedTicket;

/// Filter connected tickets by status. Returns the filtered list.
///
/// `--open`: exclude tickets where status.colorName == "green" (Done category).
/// `--status`: partial match on status.name.
/// Tickets with no status are included by --open, excluded by --status.
fn filter_tickets(
    tickets: Vec<ConnectedTicket>,
    open: bool,
    status: Option<&str>,
) -> Result<Vec<ConnectedTicket>> {
    if open {
        return Ok(tickets
            .into_iter()
            .filter(|t| {
                t.status
                    .as_ref()
                    .and_then(|s| s.color_name.as_deref())
                    .map(|c| c != "green")
                    .unwrap_or(true) // Include tickets with unknown status
            })
            .collect());
    }

    if let Some(status_input) = status {
        // Collect unique status names from the response for disambiguation
        let mut seen = std::collections::HashSet::new();
        let status_names: Vec<String> = tickets
            .iter()
            .filter_map(|t| t.status.as_ref().map(|s| s.name.clone()))
            .filter(|name| seen.insert(name.clone()))
            .collect();

        let matched = match partial_match::partial_match(status_input, &status_names) {
            MatchResult::Exact(name) => name,
            MatchResult::Ambiguous(matches) => {
                return Err(JrError::UserError(format!(
                    "Ambiguous status \"{}\". Matches: {}",
                    status_input,
                    matches.join(", ")
                ))
                .into());
            }
            MatchResult::None(all) => {
                let available = if all.is_empty() {
                    "none".to_string()
                } else {
                    all.join(", ")
                };
                return Err(JrError::UserError(format!(
                    "No status matching \"{}\". Available: {}",
                    status_input, available
                ))
                .into());
            }
        };

        return Ok(tickets
            .into_iter()
            .filter(|t| {
                t.status
                    .as_ref()
                    .map(|s| s.name == matched)
                    .unwrap_or(false)
            })
            .collect());
    }

    // No filter
    Ok(tickets)
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build`
Expected: Compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add src/cli/assets.rs
git commit -m "feat: wire --open and --status filtering into assets tickets (#89)"
```

---

### Task 3: Add unit tests for `filter_tickets`

**Files:**
- Modify: `src/cli/assets.rs` (add `#[cfg(test)] mod tests` block)

- [ ] **Step 1: Add test helpers and tests**

Add at the bottom of `src/cli/assets.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::assets::{ConnectedTicket, TicketPriority, TicketStatus, TicketType};

    fn make_ticket(key: &str, status_name: &str, color: &str) -> ConnectedTicket {
        ConnectedTicket {
            key: key.to_string(),
            id: "1".to_string(),
            title: format!("Ticket {}", key),
            reporter: None,
            created: None,
            updated: None,
            status: Some(TicketStatus {
                name: status_name.to_string(),
                color_name: Some(color.to_string()),
            }),
            issue_type: Some(TicketType {
                name: "Task".to_string(),
            }),
            priority: Some(TicketPriority {
                name: "Medium".to_string(),
            }),
        }
    }

    fn make_ticket_no_status(key: &str) -> ConnectedTicket {
        ConnectedTicket {
            key: key.to_string(),
            id: "1".to_string(),
            title: format!("Ticket {}", key),
            reporter: None,
            created: None,
            updated: None,
            status: None,
            issue_type: None,
            priority: None,
        }
    }

    #[test]
    fn filter_open_excludes_done() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
            make_ticket("A-3", "To Do", "blue-gray"),
        ];
        let result = filter_tickets(tickets, true, None).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].key, "A-1");
        assert_eq!(result[1].key, "A-3");
    }

    #[test]
    fn filter_open_includes_no_status() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket_no_status("A-2"),
        ];
        let result = filter_tickets(tickets, true, None).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_status_exact_match() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
            make_ticket("A-3", "To Do", "blue-gray"),
        ];
        let result = filter_tickets(tickets, false, Some("Done")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "A-2");
    }

    #[test]
    fn filter_status_partial_match() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
        ];
        let result = filter_tickets(tickets, false, Some("prog")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "A-1");
    }

    #[test]
    fn filter_status_no_match() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
        ];
        let result = filter_tickets(tickets, false, Some("Blocked"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No status matching"));
        assert!(err.contains("In Progress"));
    }

    #[test]
    fn filter_status_ambiguous() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "In Review", "yellow"),
        ];
        let result = filter_tickets(tickets, false, Some("In"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Ambiguous"));
    }

    #[test]
    fn filter_status_excludes_no_status() {
        let tickets = vec![
            make_ticket("A-1", "Done", "green"),
            make_ticket_no_status("A-2"),
        ];
        let result = filter_tickets(tickets, false, Some("Done")).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].key, "A-1");
    }

    #[test]
    fn no_filter_returns_all() {
        let tickets = vec![
            make_ticket("A-1", "In Progress", "yellow"),
            make_ticket("A-2", "Done", "green"),
        ];
        let result = filter_tickets(tickets, false, None).unwrap();
        assert_eq!(result.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test filter_ --lib -- --nocapture`
Expected: All 8 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/cli/assets.rs
git commit -m "test: add unit tests for assets tickets status filtering (#89)"
```

---

### Task 4: Add CLI smoke test for --open/--status conflict

**Files:**
- Modify: `tests/cli_smoke.rs`

- [ ] **Step 1: Add conflict test**

Add to `tests/cli_smoke.rs`:

```rust
#[test]
fn test_assets_tickets_open_and_status_conflict() {
    Command::cargo_bin("jr")
        .unwrap()
        .args([
            "assets",
            "tickets",
            "OBJ-1",
            "--open",
            "--status",
            "Done",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}
```

- [ ] **Step 2: Run test**

Run: `cargo test test_assets_tickets_open_and_status_conflict -- --nocapture`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/cli_smoke.rs
git commit -m "test: add CLI smoke test for --open/--status conflict (#89)"
```

---

### Task 5: Run full test suite and lint

**Files:** None (verification only)

- [ ] **Step 1: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues. If any, run `cargo fmt --all` to fix.

- [ ] **Step 4: If any issues, fix and commit**

Fix any issues and commit.
