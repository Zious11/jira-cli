# JSON Output Shapes Registry

This document is the canonical source for all JSON output shapes emitted when `--output json` is set on `jr` commands.

## Verb-aligned naming policy

Write operations use VERB-ALIGNED success-field names by intentional design. The vocabulary is:

| Operation | Shape | Notes |
|---|---|---|
| `issue move` (transition) | `{"key": str, "status": str, "changed": bool}` | Field name `changed` (verified at `src/cli/issue/json_output.rs`); supersedes any earlier draft text using `transitioned`. |
| `issue edit` | `{"key": str, "updated": bool}` | Verified at `src/cli/issue/json_output.rs` (`edit_response`). |
| `issue link` | `{"key1": str, "key2": str, "type": str, "linked": bool}` | Includes both keys and link type for completeness. Verified at `src/cli/issue/json_output.rs` (`link_response`). |
| `issue unlink` | `{"unlinked": bool, "count": int}` | `count` is the number of links removed. Verified at `src/cli/issue/json_output.rs` (`unlink_response`). |
| `issue assign` (changed) | `{"key": str, "assignee": str, "assignee_account_id": str, "changed": true}` | Verified at `src/cli/issue/json_output.rs` (`assign_changed_response`). |
| `issue assign` (unchanged) | `{"key": str, "assignee": str, "assignee_account_id": str, "changed": false}` | Idempotent: already assigned to target. |
| `issue assign --unassign` | `{"key": str, "assignee": null, "changed": bool}` | `changed` is false when already unassigned. |
| `issue remote-link` | `{"key": str, "id": int, "url": str, "title": str, "self": str}` | Verified at `src/cli/issue/json_output.rs` (`remote_link_response`). |
| `sprint add` | `{"sprint_id": int, "issues": [str], "added": true}` | Verified at `src/cli/sprint.rs` (`sprint_add_response`). |
| `sprint remove` | `{"issues": [str], "removed": true}` | Verified at `src/cli/sprint.rs` (`sprint_remove_response`). |
| `auth login` | `{"profile": str, "action": "login", "ok": true}` | NEW in S-2.07 v2.0.0 |
| `auth switch` | `{"profile": str, "action": "switch", "ok": true}` | NEW in S-2.07 v2.0.0 |
| `auth logout` | `{"profile": str, "action": "logout", "ok": true}` | NEW in S-2.07 v2.0.0 |
| `auth remove` | `{"profile": str, "action": "remove", "ok": true}` | NEW in S-2.07 v2.0.0 |
| `auth refresh` | `{"status": "refreshed", "auth_method": str, "next_step": str}` | EXISTING shape; preserved unchanged. Distinct because refresh triggers re-auth, not a state mutation. |
| Errors (any command) | `{"error": str, "code": int}` to stderr | Per BC-7.3.005; handled by main.rs error wrapper |

## Why the `auth refresh` asymmetry?

`auth refresh` is intentionally NOT harmonized to the `{profile, action, ok}` shape. Refresh is a re-authentication trigger (it re-runs the OAuth login flow), not a state mutation. The `auth_method` and `next_step` fields convey information specific to the refresh ceremony (which auth method was used, what the user should do next). Forcing it into the simpler shape would lose this information.

## Precedent

Verb-aligned naming is the policy used by `aws` and `kubectl` for write operations. Harmonized envelope-style (e.g., `{"success": true}`) is the alternative used by `heroku` and Salesforce CLIs. Both are defensible; we chose verb-aligned for clarity at parsing time. See `.factory/research/S-2.07-json-policy-and-conventions-research.md` for citations and trade-off analysis.

## Test coverage

Snapshot tests pinning these shapes live colocated with the implementation:
- Issue write operations: `src/cli/issue/json_output.rs` (11 existing snapshots)
- Auth subcommands: `src/cli/auth.rs::mod tests` (4 new snapshots from S-2.07 v2.0.0: `test_auth_login_json_shape`, `test_auth_switch_json_shape`, `test_auth_logout_json_shape`, `test_auth_remove_json_shape`)
- Auth refresh: `src/cli/auth.rs::mod tests` (existing tests for `refresh_success_payload`)
