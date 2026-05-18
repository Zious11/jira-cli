---
document_type: architecture-decision-record
adr_number: "0014"
status: Accepted
date: 2026-05-18
supersedes: []
superseded_by: []
related: ["ADR-0001", "BC-3.3.001", "BC-3.8.001", "BC-3.8.010", "BC-1.3.023", "R-H288-1", "R-M288-1"]
---

# ADR-0014: JSM Request Creation Dispatch Fork in `jr issue create`

## Status

**Accepted** (2026-05-18). Scoped to issue #288 (feat: JSM request type support).

## Context

Jira Cloud exposes two distinct issue-creation APIs with different behavioral contracts:

- **Platform path:** `POST /rest/api/3/issue` — creates any issue type in any project; accepts `issuetype`, `summary`, `description`, `assignee`, `labels`, etc. as `fields` map. Used by all current `jr issue create` invocations.
- **JSM portal path:** `POST /rest/servicedeskapi/request` — creates a JSM customer request; requires `serviceDeskId`, `requestTypeId`, `requestFieldValues` map, and optionally `raiseOnBehalfOf` (accountId). Portal-path submissions go through JSM automation, SLA tracking, and customer portal visibility rules. They cannot carry arbitrary `fields` (only declared `requestFieldValues`); they do not accept `issuetype` (the request type IS the type).

These two paths are not substitutes for each other:
- A caller using the platform path on a JSM project creates an agent-side issue that bypasses portal workflows.
- A caller using the JSM portal path on a non-JSM project gets a 400 or 404 (no service desk).
- The response shapes differ: platform returns `{"id": "...", "key": "...", "self": "..."}` at root; portal returns `{"issueId": "...", "issueKey": "...", ...}` in a different envelope.

Three alternative designs were evaluated:

### Option A: Separate top-level subcommand (`jr request create`)

A new `jr request create --request-type NAME` subcommand, completely separate from `jr issue create`. Both subcommands exist in parallel; users choose.

**Rejected.** Creates command surface duplication: users creating JSM issues must learn two commands with overlapping semantics. The `--request-type` flag is the natural disambiguation signal — its presence unambiguously indicates JSM portal intent. A separate subcommand would also require duplicating all the project-resolution, ADF, and `--output` flag handling already in `handle_create`. The ADR-0001 (thin client, DRY structure) principle disfavors this.

### Option B: Automatic JSM routing (detect project type server-side, always use JSM path on JSM projects)

When a project is a JSM project, silently route `jr issue create` to the portal path regardless of `--request-type`.

**Rejected.** This is a silent behavioral change that breaks the existing platform-path usage for users who intentionally create agent-side issues on JSM projects (e.g., create internal issues, sub-tasks). The platform path is the regression baseline (BC-3.3.001). Auto-routing would require detecting the project type on every `issue create` call (an additional HTTP round-trip) and would silently change behavior for existing users without opt-in. F1 delta analysis flagged this as HIGH regression risk.

### Option C (Selected): Conditional dispatch fork gated on `--request-type` presence

When `--request-type NAME-OR-ID` is provided:
- Resolve the service desk for the project (via `require_service_desk` already in `api/jsm/servicedesks.rs`)
- Resolve the request type ID (via partial-name match against cached/fetched request type list)
- Build `requestFieldValues` from `--field NAME=VALUE` pairs
- Dispatch to `POST /rest/servicedeskapi/request`
- Normalize response to `{"key": "KEY"}` to preserve the existing `--output json` contract

When `--request-type` is absent:
- Platform path is taken. No behavior change. Existing platform-path tests are the regression guard.

## Decision

**Implement the conditional dispatch fork (Option C) inside `src/cli/issue/create.rs::handle_create`.**

The fork gate is `request_type.is_some()`. The gate is evaluated once, after flag parsing, before project resolution. The two branches are structurally independent: the JSM branch resolves a service desk and request type ID; the platform branch resolves an issue type. The branches share: project resolution, ADF description building, `--output` format, and `--no-input` semantics.

The `--type` flag (issue type) is IGNORED and a warning is emitted when `--request-type` is set, because request types already encode the issue type. It is NOT an error (to avoid breaking automation that always passes `--type`).

The JSON output shape `{"key": "KEY"}` is preserved across both branches by extracting `issueKey` from the JSM response and emitting the same `IssueCreatedOutput` struct used by the platform path.

## Consequences

### Positive

- **Zero regression risk for existing users.** Platform path is structurally unchanged; existing integration tests (`tests/issue_create_json.rs`, `tests/issue_commands.rs`, `tests/issue_write_holdouts.rs`) are the regression guard.
- **Minimal API surface growth.** No new top-level command is needed for the creation path. `jr requesttype list/fields` remain separate discovery commands.
- **Consistent `--output json` contract.** Both paths emit `{"key": "KEY"}`, preserving automation compatibility (BC-3.3.001 / BC-3.8.001).
- **`--no-input` parity.** Ambiguous request-type partial-match surfaces `JrError::UserError(exit 64)` in non-interactive mode, matching the `partial_match` module's `Ambiguous` arm contract established in `cli/queue.rs`.

### Negative / Risks

- **`handle_create` grows further** (estimated +120–160 LOC on an already-1,601 LOC file). This worsens R-M5 but does not trigger the ADR-0012 shard rule (which applies to individual functions or files that exceed ~1000 LOC AND have high branch density in a single function; the new branch is structurally separate and the function is already over the threshold).
- **`--type` flag interaction** requires explicit documentation and a warning (not an error), to avoid breaking existing automation pipelines that always pass `--type`. F5 adversarial review must verify the warning fires correctly.
- **Developer Console coordination.** The `write:servicedesk-request` OAuth scope addition (in `DEFAULT_OAUTH_SCOPES`) must be registered in the Atlassian Developer Console before this code ships. CI cannot catch a mismatch; the pinning test `default_oauth_scopes_pins_the_full_set_with_offline_access` detects code-side drift, but not console-side drift. Manual staging validation is required before merge to `develop`. (Tracked as R-H288-1 in risk-register.md §issue-288 block.)

### Impact on Codebase

- `BC-3.3.001` (issue create platform path) gains a conditional clause: "when `--request-type` is absent"
- `BC-3.8.001` (new: issue create JSM path) is the mirror clause: "when `--request-type` is present"
- `NEW-INV-XXX` for the fork-gate invariant: "`request_type.is_some()` is the sole dispatch signal; no project-type detection occurs at the gate"

## References

- ADR-0001 (Thin Client — DRY, no duplication of HTTP plumbing)
- BC-3.3.001 (platform create path — regression baseline)
- BC-3.8.001 (JSM create path — new)
- F1 architect-input-288.md §Architecture Delta — Conditional dispatch fork
- F1 delta-analysis-288.md §Risk Assessment (HIGH: conditional dispatch fork)
- `src/api/jsm/servicedesks.rs::require_service_desk` (reused, not modified)
- `src/partial_match.rs` (reused for request type name resolution)
