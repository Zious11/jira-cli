# `format_date` / `format_comment_date` verbose parse-failure logging

**Issue:** [#214](https://github.com/Zious11/jira-cli/issues/214)

## Problem

Two timestamp formatters silently absorb parse failures:

- `format_date` in `src/cli/issue/changelog.rs` — used by `jr issue
  changelog` table rendering.
- `format_comment_date` in `src/cli/issue/list.rs` — used by `jr issue
  comments` table rendering.

Both render the raw ISO-8601 string when both `DateTime::parse_from_rfc3339`
and the Jira compact-offset fallback (`%Y-%m-%dT%H:%M:%S%.3f%z`) fail. The
user sees a broken column but no signal that a format regression has
occurred, so a future Atlassian change (e.g., dropped milliseconds,
nanosecond precision, `Z` suffix) silently breaks output alignment.

## Approach

When `--verbose` is set, emit a one-shot `eprintln!("[verbose] ...")`
per call-site per run the first time a parse fails. Subsequent failures
in the same run are suppressed by a function-local
`static AtomicBool`. No change to normal runs (without `--verbose`).

Matches the existing observability pattern at `src/api/client.rs:170-177`
where `--verbose` gates plain `eprintln!("[verbose] ...")` with no
external crates. A dedicated tracing layer is explicitly deferred per
the issue (“out of scope for #200 — file for the next observability
pass”).

### Rejected alternatives

- **Per-unique-string dedup via `OnceLock<Mutex<HashSet<String>>>`** —
  more informative for runs with multiple distinct failures but adds
  a dependency, heap allocations, and more code. YAGNI until a real
  user reports multi-format flooding.
- **Log every failure without dedup** — risk of flooding stderr when
  an entire page of 100 entries shares a broken format.
- **Adopt the `tracing` crate** — sizable infrastructure change; the
  issue explicitly keeps it out of scope.

## Algorithm

Per call-site, inside the formatter function:

```rust
static LOGGED: AtomicBool = AtomicBool::new(false);
// ... parse ...
if parse_failed {
    crate::observability::log_parse_failure_once(
        &LOGGED, "changelog", iso, verbose,
    );
    return iso.to_string();
}
```

`log_parse_failure_once` lives in a new `src/observability.rs`:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn log_parse_failure_once(
    flag: &AtomicBool,
    site: &str,
    iso: &str,
    verbose: bool,
) {
    if verbose && !flag.swap(true, Ordering::Relaxed) {
        eprintln!("[verbose] {site} timestamp failed to parse: {iso}");
    }
}
```

`swap(true, Relaxed)` is the idiomatic "set the flag and return the
previous value" primitive. `Ordering::Relaxed` is correct here because
the flag guards no shared-state initialization — worst case a narrow
race window emits two log lines, which is acceptable for an
observability signal.

## Plumbing

`JiraClient` gains a public `verbose()` accessor (field already
exists; currently private):

```rust
// src/api/client.rs
pub fn verbose(&self) -> bool { self.verbose }
```

Each formatter takes an explicit `verbose: bool` parameter:

- `format_date(iso: &str, verbose: bool) -> String`
- `format_comment_date(iso: &str, verbose: bool) -> String`

Callers thread `client.verbose()` from `handle()` down:

- `changelog::handle`: pass to `build_rows(&entries, verbose)`, which
  passes to each `format_date(&entry.created, verbose)` call.
- Comments listing (in `list.rs`): pass into `format_comment_row(..., verbose)`,
  which passes to `format_comment_date`.

No change to public surface (`--verbose` flag unchanged).

## Tests

Two integration tests added, one per call-site, using the existing
`wiremock` + `assert_cmd` pattern.

### `tests/issue_changelog.rs`

New test: `changelog_verbose_logs_parse_failure_once`. Stubs
`/rest/api/3/issue/BAD-1/changelog` to return two entries with a
malformed `"created"` value (e.g., `"not-a-date"`). Runs
`jr issue changelog BAD-1 --verbose`; asserts stderr contains exactly
one line matching `"changelog timestamp failed to parse"` even though
the response had two broken entries.

Second new test: `changelog_parse_failure_silent_without_verbose`.
Same fixture, runs without `--verbose`; asserts stderr does **not**
contain `"failed to parse"`.

### `tests/comments.rs`

Parallel pair: `comments_verbose_logs_parse_failure_once` +
`comments_parse_failure_silent_without_verbose` using the comments
endpoint and a malformed `"created"`.

### No unit tests

The dedup logic is 3 lines; integration coverage through the full
CLI pipeline is the idiomatic vehicle in this project (see existing
assert_cmd usage across tests/comments.rs, tests/issue_changelog.rs).

## Files touched

| File | Change |
|---|---|
| `src/lib.rs` | `mod observability;` |
| `src/observability.rs` | **new**, ~15 lines with one pub(crate) fn |
| `src/api/client.rs` | `pub fn verbose(&self) -> bool` getter |
| `src/cli/issue/changelog.rs` | `format_date` + `build_rows` gain `verbose: bool`; `handle` passes `client.verbose()` |
| `src/cli/issue/list.rs` | `format_comment_date` + `format_comment_row` gain `verbose: bool`; comments callers pass `client.verbose()` |
| `tests/issue_changelog.rs` | 2 new tests |
| `tests/comments.rs` | 2 new tests |

## Out of scope

- Tracing/log crate adoption (deferred to future observability pass).
- Per-unique-string dedup.
- Logging format errors at `parse_created` call sites outside these
  two formatters (e.g., the sort comparator in `changelog::handle`
  — that fallback is deterministic and not user-visible).
- Help-text edits (`--verbose` already documents “Enable verbose
  output”).

## Exit criteria

- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` green.
- The 4 new integration tests pass.
- `jr issue changelog <key> --verbose` against a response with a
  malformed timestamp emits exactly one `[verbose]` line regardless of
  how many entries share the bad format.
- No change in stderr for runs without `--verbose`.
