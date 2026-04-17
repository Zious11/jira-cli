# Per-Command Error-Path Coverage — Design Spec

**Issue:** #187 — audit per-command integration coverage for 5xx, 401, and network-drop error paths.

**Scope:** Test-only. Add three integration tests per read-command test file (5xx, 401, network-drop) to pin end-to-end error propagation from the `JiraClient` layer through the CLI binary to stderr and exit code. No production code changes.

**Non-goals:**
- Write-command error coverage (create/edit/transition/assign/link/worklog-add) — deferred; separate audit.
- Client-layer retry/backoff tests — already covered in `src/api/rate_limit.rs` tests.
- Per-command timeout and retry-exhaustion tests — flagged in the #114 silent-failure review but out of scope here; will be filed as separate issues if reviewer asks.
- Simulating "true" 401 mid-session (stateful auth flow) — from the client's perspective a mid-session 401 is indistinguishable from an initial 401; see Caveats.

---

## Motivation

`JiraClient` has centralized 401 and retry handling (`src/api/client.rs`, `src/api/rate_limit.rs`), but most per-command integration tests cover only 2xx happy paths and a handful of 4xx cases. Only `tests/issue_list_errors.rs` has per-command 5xx coverage (board-config 500, sprint 500). There is no per-command 401 coverage anywhere — all 401 tests live in `tests/api_client.rs` (client layer, not binary). There are no network-drop tests at all.

Silent-failure-hunter review of #114 (user commands) flagged this: "client handles it" is not sufficient because the user-visible message depends on both layers. A regression in error propagation — for example, a future refactor losing the friendly `NetworkError` message and letting a raw `reqwest::Error` Display leak through — would not be caught by any current test. This spec adds a minimum floor of end-to-end coverage for every read-command test file.

## Scope decision

Three shapes were considered (see Rejected alternatives). Chosen: **one 5xx + one 401 + one network-drop test per command-group test file**, targeting one representative read command per file. This matches the issue's wording ("one integration test each for") and gives end-to-end pinning for every command family without N-duplicating near-identical tests per subcommand.

## Coverage matrix

| Test file | # existing tests | Representative command | Placement |
| --- | --- | --- | --- |
| `tests/board_commands.rs` | 23 | `jr board list` | inline |
| `tests/sprint_commands.rs` | 20 | `jr sprint current` | inline |
| `tests/user_commands.rs` | 11 | `jr user search` | inline |
| `tests/worklog_commands.rs` | 4 | `jr worklog list` | inline |
| `tests/team_commands.rs` | 4 | `jr team list` | inline |
| `tests/queue.rs` | 14 | `jr queue list` | inline |
| `tests/project_commands.rs` | 14 | `jr project statuses` | inline |
| `tests/comments.rs` | 8 | `jr issue comments` | inline |
| `tests/assets.rs` | 38 | `jr assets search` | **new `tests/assets_errors.rs`** |
| `tests/issue_commands.rs` | 98 | `jr issue view` | **new `tests/issue_view_errors.rs`** |
| `tests/issue_list_errors.rs` | 9 (has 5xx) | `jr issue list` | inline — add 401 + network-drop only |

Placement rule: **file has <30 tests** → add inline; **file has ≥30 tests** → split into `*_errors.rs` (follows `issue_list_errors.rs` precedent for `issue_commands.rs`).

Total new tests: **32** — three per file for 10 files + two filling the 401 and network-drop gap in the existing `issue_list_errors.rs`.

## Test shape

All three variants share a skeleton: start a `MockServer`, configure one mock, spawn the `jr` binary via `assert_cmd::Command` with `JR_BASE_URL` + `JR_AUTH_HEADER` env overrides, assert exit code and stderr content.

**Variant 1 — 5xx (server error surfaces friendly message):**

```rust
#[tokio::test]
async fn <cmd>_server_error_surfaces_friendly_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/api/3/<endpoint>"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "errorMessages": ["Internal server error"],
            "errors": {}
        })))
        .mount(&server).await;

    let output = Command::cargo_bin("jr").unwrap()
        .env("JR_BASE_URL", server.uri())
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["<cmd>", ...]).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API error (500)"), "got: {stderr}");
    assert!(!stderr.contains("panic"), "got: {stderr}");
}
```

**Variant 2 — 401 (not-authenticated dispatches re-auth message):** same skeleton, `ResponseTemplate::new(401)` with the canonical `{"errorMessages": ["..."], "errors": {}}` body. Assertions: `status.code() == Some(2)`, stderr contains both `"Not authenticated"` and `"jr auth login"`.

**Variant 3 — Network drop (friendly reach error, no panic):**

```rust
#[tokio::test]
async fn <cmd>_network_drop_surfaces_reach_error() {
    let server = MockServer::start().await;
    let uri = server.uri();
    drop(server); // port closes; subsequent connects get ECONNREFUSED

    let output = Command::cargo_bin("jr").unwrap()
        .env("JR_BASE_URL", uri)
        .env("JR_AUTH_HEADER", "Basic dGVzdDp0ZXN0")
        .args(["<cmd>", ...]).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Could not reach"), "got: {stderr}");
    assert!(stderr.contains("check your connection"), "got: {stderr}");
    assert!(!stderr.contains("panic"), "got: {stderr}");
}
```

### Key assertions per variant

| Variant | Exit code | Stderr must contain | Stderr must NOT contain |
| --- | --- | --- | --- |
| 5xx | `1` | `"API error (500)"` | `"panic"` |
| 401 | `2` | `"Not authenticated"`, `"jr auth login"` | `"panic"` |
| Network drop | `1` | `"Could not reach"`, `"check your connection"` | `"panic"` |

The negative assertions (`!contains("panic")`) are the point of this audit: silent-failure-hunter flagged that a future refactor could drop friendly error handling without any current test failing. These negative checks close that regression surface.

## Caveats

**"401 mid-session" is client-indistinguishable from initial 401.** Perplexity research (community.developer.atlassian.com) confirms Jira Cloud's response body for an expired-token 401 is the same `{"errorMessages": [...], "errors": {}}` shape as for an invalid-credential 401. The client cannot distinguish between "token never worked" and "token just expired" without external state. This spec therefore tests "endpoint returns 401" — which is the only distinction CLI behavior depends on. If stateful expiry detection ever becomes desirable, it would be a separate feature (likely involving a `/rest/api/3/myself` pre-flight check), not a test-coverage gap.

**Wiremock `drop(server)` is the idiomatic network-drop pattern.** Confirmed via the wiremock-rs README ("When a `MockServer` instance goes out of scope, the corresponding HTTP server running in the background is shut down to free up the port it was using"). Wiremock has no built-in "close connection mid-request" API; drop-before-request is the supported pattern. Risk of OS port reuse in the tiny window between `drop(server)` and the client's connect is bounded by the OS ephemeral-port pool (macOS 49152-65535 = 16,384 ports; Linux typically 32768-60999 = 28,232 ports) and the narrow millisecond window — essentially unreachable in practice. If a reused port ever produced a spurious 200 response, the `.contains("Could not reach")` assertion fails loudly instead of silently passing.

**429 retry-exhaustion path is explicitly out of scope.** It's tested at the client layer in `src/api/rate_limit.rs` and re-testing per-command would only exercise the same `send_with_retry` code path.

## Architecture decisions

**End-to-end via `assert_cmd::Command`, not unit tests on the client.** The point of this audit is to pin user-visible behavior — stderr text, exit code, no panics. Unit-testing the client layer would duplicate `tests/api_client.rs` without covering the CLI-to-stderr glue in `src/main.rs:28-50`.

**Canonical Jira error body shape.** All 401 and 500 mocks use `{"errorMessages": [...], "errors": {}}`. Validated via Perplexity (developer.atlassian.com/cloud/jira/platform/rest/v3/intro/, community.atlassian.com). Matches existing `issue_list_errors.rs` mocks.

**`assert!(!stderr.contains("panic"))` as the regression gate.** Rust panics in the CLI binary would print `"thread 'main' panicked at ..."` to stderr (via the default panic hook). Asserting `"panic"` is absent pins "we never panic on any error path", which is what the #114 silent-failure-hunter review asked for.

**Representative command per file, not per subcommand.** The CLI→client→stderr error path is identical across subcommands within a module; testing every subcommand would be 3× the tests without new coverage of distinct code paths.

## Files touched

- `tests/board_commands.rs` — +3 inline tests (5xx, 401, network-drop) for `jr board list`.
- `tests/sprint_commands.rs` — +3 inline tests for `jr sprint current`.
- `tests/user_commands.rs` — +3 inline tests for `jr user search`.
- `tests/worklog_commands.rs` — +3 inline tests for `jr worklog list`.
- `tests/team_commands.rs` — +3 inline tests for `jr team list`.
- `tests/queue.rs` — +3 inline tests for `jr queue list`.
- `tests/project_commands.rs` — +3 inline tests for `jr project statuses`.
- `tests/comments.rs` — +3 inline tests for `jr issue comments`.
- `tests/issue_list_errors.rs` — +2 inline tests (401 + network-drop) for `jr issue list`; 5xx already covered.
- `tests/assets_errors.rs` — NEW file, 3 tests for `jr assets search`.
- `tests/issue_view_errors.rs` — NEW file, 3 tests for `jr issue view`.

No changes to `Cargo.toml`, `src/`, CLI definitions, or any production code path. No new dependencies.

## Rejected alternatives

- **Option A — full matrix (3 tests × every read subcommand, ~54 tests).** Most thorough, but near-duplicates per module exercise the same CLI-to-stderr glue; the marginal test adds little behavioral coverage for 3× the maintenance surface.
- **Option C — gaps only (skip `issue_list_errors.rs` since it has 5xx).** Leaves the 401 and network-drop gaps in `issue list`. Small gain in test-count minimalism isn't worth the uneven coverage.
- **Unit tests on the client layer.** Would overlap with `tests/api_client.rs` and miss the `src/main.rs` glue.
- **Mock 429 retry-exhaustion per command.** Already tested in `src/api/rate_limit.rs`; per-command would re-exercise the same path.
- **Close connection mid-request (partial reads).** No wiremock-rs API for this; the drop-before-request pattern is the idiomatic alternative and covers the regression surface we care about (CLI never panics, always surfaces a friendly message).
