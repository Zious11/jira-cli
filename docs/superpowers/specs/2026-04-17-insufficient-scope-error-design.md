# Insufficient-Scope 401 Error — Design Spec

**Issue:** #185 — granular-scoped personal API tokens return 401 on POST but succeed on PUT.

**Scope:** Detect the Atlassian API gateway's `"scope does not match"` 401 response and surface a distinct, actionable `JrError::InsufficientScope` with workaround guidance. This is a jr-side UX improvement for an external Atlassian gateway bug; jr cannot fix the underlying server behavior.

**Non-goals:**
- Changing authentication flow, OAuth handling, or token storage.
- Modifying verbose/debug logging (already shipped in #183).
- Adding configurable OAuth scopes (tracked separately in #184).
- Retry, fallback, or automatic scope upgrade — the user must change credentials themselves.

---

## Motivation

Validated via Perplexity (community.developer.atlassian.com, atlassian-python-api#1618, multiple community posts): granular-scoped personal API tokens (`write:issue:jira`, `write:comment:jira`, etc.) authenticate correctly for GET and PUT, but return `401 {"code": 401, "message": "Unauthorized; scope does not match"}` on all POST requests. Atlassian has not publicly acknowledged the bug; no JRACLOUD/JSWCLOUD ticket exists as of April 2026. Confirmed workarounds are: use a classic token with `write:jira-work` scope, or switch to OAuth 2.0 (3LO).

Today, jr maps any 401 to a generic `JrError::NotAuthenticated` ("Not authenticated. Run \"jr auth login\" to connect.") — misleading for this case, because the token *is* authenticated; only this one class of requests is refused. Without `--verbose` (which exposes the raw body per #183), users have no indication of the true cause.

## Change summary

1. New error variant `JrError::InsufficientScope { message: String }` in `src/error.rs`, exit code `2` (same class as `NotAuthenticated`).
2. `parse_error` in `src/api/client.rs` restructured to read the body first, then branch: on 401 + body message containing `"scope does not match"` (case-insensitive substring, ASCII), return `InsufficientScope`; otherwise unchanged behavior.
3. Six new tests: four integration (positive-path detection, case-insensitive match, 401 fall-through, and non-401 status-gate behavior) and two unit (`InsufficientScope` exit code and `Display` rendering).

No API/CLI surface changes. No new dependencies. No caller updates needed — all callers already handle `anyhow::Error` returned from `parse_error`.

## Error variant

```rust
#[error(
    "Insufficient token scope: {message}\n\n\
     The Atlassian API gateway rejects granular-scoped personal tokens on POST \
     requests (while PUT/GET succeed). Workarounds:\n  \
     • Use a classic token with \"write:jira-work\" scope instead of granular scopes, or\n  \
     • Try OAuth 2.0 (run \"jr auth login --oauth\") — may avoid this bug, not verified\n\
     See https://github.com/Zious11/jira-cli/issues/185 for details."
)]
InsufficientScope { message: String },
```

The raw gateway `message` is stored verbatim so it surfaces in logs, `--verbose` output, and any future structured-JSON error path. Exit code `2` parallels `NotAuthenticated` — both represent "credentials need user attention."

**OAuth hedge language:** the issue body admits OAuth-as-workaround is untested. The error text reflects that honestly with "may avoid this bug, not verified" — discoverable without over-promising. If OAuth turns out to share the bug, affected users hit the same error with OAuth selected, and `#185` becomes the place to update either the text or the recommendation.

## Dispatch logic

```rust
async fn parse_error(response: Response) -> anyhow::Error {
    let status = response.status().as_u16();
    let message = match response.bytes().await {
        Ok(body) => extract_error_message(&body),
        Err(e) => format!("Could not read error response: {e}"),
    };

    if status == 401 {
        // Match the Atlassian gateway's exact 401 shape for granular-token
        // POST rejections. Case-insensitive guards against future capitalization
        // drift; substring (not exact equality) guards against prefix/suffix
        // wording shifts like ";" punctuation changes.
        if message.to_ascii_lowercase().contains("scope does not match") {
            return JrError::InsufficientScope { message }.into();
        }
        return JrError::NotAuthenticated.into();
    }

    JrError::ApiError { status, message }.into()
}
```

Graceful degradation: if Atlassian fixes the bug, changes wording, or localizes the message, the match fails and users get the same generic `NotAuthenticated` they'd get today — no regression surface.

## Testing

All tests mirror existing patterns (`tests/api_client.rs:72` for wiremock, `src/error.rs:45` for exit-code unit tests).

**Integration tests** (`tests/api_client.rs`):

1. `test_401_scope_mismatch_returns_insufficient_scope` — positive path. Wiremock `POST /rest/api/3/issue` returns `401 {"code": 401, "message": "Unauthorized; scope does not match"}`. Asserts error display contains `"Insufficient token scope"`, the raw gateway message, `"write:jira-work"`, `"OAuth 2.0"`, and the `#185` issue link.

2. `test_401_without_scope_mismatch_falls_through_to_not_authenticated` — negative boundary. Wiremock `GET /rest/api/3/myself` returns `401 {"code": 401, "message": "Session expired"}` (no "scope" substring). Asserts `"Not authenticated"` and explicitly `NOT "Insufficient token scope"`. Pins the fall-through independent of any other existing 401 test's body-string choice.

3. `test_401_scope_mismatch_matches_case_insensitively` — Wiremock 401 body uses title case (`"Scope Does Not Match"`). Asserts the error still dispatches to `InsufficientScope`. Pins `to_ascii_lowercase()` usage so a future drop trips CI.

4. `test_non_401_with_scope_substring_does_not_dispatch_to_insufficient_scope` — Wiremock returns `403` with body containing `"scope does not match"`. Asserts the error is `ApiError (403)`, not `InsufficientScope`. Pins the `status == 401` gate.

**Unit tests** (`src/error.rs`):

5. `insufficient_scope_exit_code` — `assert_eq!(JrError::InsufficientScope { message: "x".into() }.exit_code(), 2)`.

6. `insufficient_scope_display_includes_workarounds` — builds the variant and asserts Display contains `"Insufficient token scope"`, the raw message, `"write:jira-work"`, `"OAuth 2.0"`, and the `#185` issue link.

Existing `test_401_returns_not_authenticated` remains unchanged — it continues to cover the canonical bad-auth case.

## Architecture decisions

**Substring match, not regex.** The gateway string is stable across all reported cases (community.developer.atlassian.com posts span ≥2 years, same wording). Regex adds no value and enlarges the change surface.

**Case-insensitive (ASCII).** Server-generated error strings from the auth layer are always ASCII. `to_ascii_lowercase` avoids locale-dependent Unicode case folding and is cheaper than `to_lowercase`.

**Body-first read in `parse_error`.** `parse_error` is the error-only path (only called from `client.rs:213-214` and `222` on 4xx/5xx). Always reading the body there costs nothing extra vs. the current design, which already reads the body for non-401 errors. The small overhead on auth failures is dominated by the RTT.

**No `--verbose` coupling.** Even without `--verbose`, the user now gets the actionable error. `--verbose` (from #183) continues to expose the full raw body for power users.

**Store the raw `message` field.** Lets the user search for "scope does not match" online and find community posts. Also keeps the structured-JSON error surface (future `--output json` for errors) forward-compatible: the raw server message will be carryable as a distinct field.

## Rejected alternatives

- **Close as external bug, add README entry only (issue-author's literal framing):** Loses the agent-friendly actionable-error property of the CLI. #183's verbose-body log helps power users but doesn't surface the workaround in the default output path.
- **Detect via Basic-auth-only + POST combination (no body inspection):** Brittle — false positives for any classic-token user who hits a real 401 on a POST (e.g., expired session).
- **Auto-retry the POST with different auth:** jr has no way to upgrade a token's scope at runtime and would silently hide the problem.
- **Map to `JrError::ApiError` with custom message:** Loses the distinct exit code and makes the `exit_code()` match arm leaky (would need string-matching on the message).

## Files touched

- `src/error.rs` — new variant, `exit_code()` arm, two unit tests (exit code + Display content).
- `src/api/client.rs` — restructure `parse_error` body-read ordering, add 401-body-substring branch (~15 lines).
- `tests/api_client.rs` — four new integration tests (positive dispatch, fall-through boundary, case-insensitivity, non-401 status gate).

No changes to `Cargo.toml`, CLI definitions, or any caller of `parse_error`.
