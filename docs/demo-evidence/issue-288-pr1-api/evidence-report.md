---
document_type: demo-evidence-report
story_id: issue-288-pr1-api
evidence_class: api-only (no CLI surface)
timestamp: 2026-05-18
producer: demo-recorder
---

# Demo Evidence: issue-288-pr1-api

## Why No VHS/Playwright

`issue-288-pr1-api` is the pure API + types layer for JSM request creation and request-type
discovery. It introduces `src/api/jsm/requests.rs`, `src/api/jsm/request_types.rs`, and
`src/types/jsm/request_type.rs` — Rust modules with no binary entry points and no CLI
dispatch surface.

There is nothing a user (or VHS) can invoke at the terminal. The CLI commands
(`jr requesttype list`, `jr requesttype fields`, `jr issue create --request-type`) arrive
in pr2 and pr4. Recording a VHS tape of `cargo test` output is explicitly prohibited by the
Demo Recorder charter ("cargo test output is NOT a demo").

For this story the appropriate evidence is proof that each acceptance criterion is covered by
a passing test. The artifact files in this directory are captured command-line output saved
verbatim so reviewers can see exact test names, counts, and exit codes without re-running the
suite.

---

## AC → Test Mapping

| AC | Acceptance Criterion | Test(s) | File |
|----|----------------------|---------|------|
| AC-001 | `create_jsm_request` POSTs to `/rest/servicedeskapi/request`, deserializes `JsmRequestCreated` | `test_create_jsm_request_posts_to_servicedeskapi_and_returns_issue_key` | tests/jsm_request_api.rs |
| AC-002 | `list_request_types` paginates via `ServiceDeskPage` (`isLastPage`), accumulates all pages | `test_list_request_types_paginates_is_last_page` | tests/jsm_request_api.rs |
| AC-003 | `search_query=Some(...)` forwarded as `searchQuery` param; `None` → param absent (enforced by `query_param_is_missing`) | `test_list_request_types_search_query_forwarded` (positive) + `test_list_request_types_search_query_absent_when_none` (negative) | tests/jsm_request_api.rs |
| AC-004 | `get_request_type_fields` GETs `.../requesttype/{rtId}/field`, returns `RequestTypeFieldsResponse` | `test_get_request_type_fields_returns_field_list` | tests/jsm_request_api.rs |
| AC-005 | `RequestType` serde struct matches Atlassian camelCase shape (round-trip verified); `RequestTypeField` shape verified implicitly via AC-004's deserialization of the fields endpoint response | `test_request_type_struct_round_trip` (RequestType) + `test_get_request_type_fields_returns_field_list` (RequestTypeField) | tests/jsm_request_api.rs |
| AC-006 | `JsmRequestCreated` exposes `issue_key` and `issue_id`; `issue_id` is `Option` | `test_jsm_request_created_extracts_issue_key` | tests/jsm_request_api.rs |
| AC-007 | Release gate: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check` all exit 0 | See release-gate table below | — |

---

## Captured Evidence

| File | Description |
|------|-------------|
| [`cargo-test-jsm-request-api.txt`](cargo-test-jsm-request-api.txt) | `cargo test --test jsm_request_api` — AC-001..AC-006 direct coverage |
| [`cargo-clippy.txt`](cargo-clippy.txt) | `cargo clippy --all-targets -- -D warnings` — AC-007 lint gate |
| [`cargo-fmt-check.txt`](cargo-fmt-check.txt) | `cargo fmt --all -- --check` — AC-007 format gate |
| [`cargo-test-all.txt`](cargo-test-all.txt) | `cargo test` (full suite) — regression evidence, confirms no existing tests broken |

---

## AC-007 Release-Gate Status

| Gate | Command | Exit Code | Verdict |
|------|---------|-----------|---------|
| Unit + integration tests (jsm_request_api) | `cargo test --test jsm_request_api` | 0 | PASS |
| Lint (zero warnings) | `cargo clippy --all-targets -- -D warnings` | 0 | PASS |
| Format check | `cargo fmt --all -- --check` | 0 | PASS |
| Full regression suite | `cargo test` | 0 | PASS |

**AC-007 verdict: PASS**

All 7 tests in `tests/jsm_request_api.rs` pass. No clippy warnings. No formatting
violations. Full suite regression is clean.
