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
| `issue attachment list` | JSON array of curated attachment objects (read operation) | `[{author, contentUrl, created, filename, id, mimeType, size}]`. Keys in BTreeMap-alphabetical order. `"self"` omitted; `"content"` renamed to `"contentUrl"` (VP-576-004). `size` is raw u64. `author` is `null` when absent. Table columns: `ID \| Filename \| Type \| Size \| Created \| Author`. Implemented in `src/cli/issue/attachments.rs::handle_attachment_list`. |
| `issue attachment download` | `{"downloaded": [{filename, id, path, size}]}` (write operation; S-576-2) | Outer object has a single key `"downloaded"` containing an array of successfully downloaded entries. BTreeMap-alphabetical key order: `filename`, `id`, `path`, `size`. `filename` = RAW Jira-supplied name (pre-sanitization; P27-001 — deliberate: machine consumers see the original name). `path` = on-disk path as-constructed by `jr` (NOT canonicalized; P18-004). `size` = bytes actually written from the streaming loop, NOT the metadata size reported by Jira (P31-002). Partial/full batch failure → exit 1; manifest contains only successful entries. Implemented in `src/cli/issue/attachments.rs::handle_attachment_download`. |
| `issue attachment upload` | Success: JSON array of curated attachment objects (same shape as `attachment list` — see above). Cancel: `{"cancelled": true, "uploaded": false}` on stdout + `"Upload cancelled."` on stderr; exit 0. Dry-run: `{"dryRun": true, "wouldDelete": [{filename, id}], "wouldUpload": [{filename}]}` (camelCase keys; S-576-3). | Success array shape: `[{author, contentUrl, created, filename, id, mimeType, size}]` (identical to list output — VP-576-004). Cancel shape emitted when user answers N to `--replace-existing` confirmation prompt; `--yes` bypasses. Dry-run shape emitted with `--dry-run`; `wouldDelete` is empty when `--replace-existing` is absent or no filename matches. Table output: 4-column table (Filename / Size / ID / Created). Implemented in `src/cli/issue/attachments.rs::handle_attachment_upload`. |
| `issue attachment delete` | Single-AID success: `{"deleted": true, "id": str}`. Cancel: `{"cancelled": true, "deleted": false}` (no `id` — no HTTP confirmed it). Bulk success: `{"count": int, "deleted": bool, "ids": [str]}`. Dry-run: `{"attachments": [{id [, filename]}], "dryRun": true, "ids": [str]}` (S-576-4). | Single-AID success: 2 keys `deleted` + `id`. Cancel: 2 keys `cancelled` + `deleted`; `id` absent. Bulk (multi-AID or --issue/--older-than): 3 keys `count` + `deleted` + `ids`; `deleted` is `false` when `count==0`. Dry-run: `dryRun: true` + `ids` array + `attachments` array; single-AID dry-run has `{id}` only (no metadata fetch); bulk dry-run has `{filename, id}`. All keys alphabetical per BTreeMap ordering. Implemented in `src/cli/issue/attachments.rs::handle_attachment_delete`. |
| `issue comment add` | Jira Comment object (passthrough from `POST /rest/api/3/issue/{key}/comment`) | `{"id": str, "created": str, …}`. Passthrough; shape is Jira Cloud authoritative. Table output: `Added comment to KEY (id: N)`. Implemented in `src/cli/issue/interactions.rs::handle_comment_add`. |
| `issue comment delete` | Success: `{"deleted": true, "id": str, "key": str}`; interactive cancel: `{"cancelled": true, "deleted": false}` (no `id`/`key`) | Shipped (S-577-3). Cancel path emits when user answers N interactively; `--yes` bypasses the prompt. 404/403 → exit 64. |
| `issue comment edit` | Success: `{"changed_fields": {"body": str[, "jsm_internal": bool]}, "id": str, "key": str, "updated": true}`; interactive cancel: `{"cancelled": true, "updated": false}` (no `id`/`key`) | Body edit S-577-4; visibility flags S-577-5 — fully shipped. `changed_fields.body` carries the raw pre-trim user input string (BC-3.5.005 raw echo pin). `changed_fields.jsm_internal: bool` present only when `--internal` or `--public` is passed (VP-577-026): `true` for `--internal`, `false` for `--public`. Cancel path emits when user answers N to the `--public` confirmation prompt; `--yes` bypasses the prompt. Verified at `src/cli/issue/interactions.rs::handle_comment_edit`. |
| `issue comment view` | Jira Comment object (passthrough from `GET /rest/api/3/issue/{key}/comment/{id}?expand=properties`) | Shipped (S-577-6). Raw `serde_json::Value` passthrough — no typed round-trip. Route: `output::render_json` (#526 invariant). |
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
