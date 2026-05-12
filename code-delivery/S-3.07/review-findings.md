# S-3.07 Review Findings

Story: LOW NFR code fixes + JRACLOUD-94632 anti-loop guard
PR: #315
Branch: feat/s-3-07-low-nfr-fixes-and-jracloud-anti-loop

---

## Convergence Table

| Cycle | Findings | Blocking | Fixed | Remaining |
|-------|----------|----------|-------|-----------|
| 1     | 0        | 0        | 0     | 0 → APPROVE |

---

## Cycle 1 — Review Findings

**Verdict: APPROVE**

### Checklist Results

#### (1) Retry-After cap — both call sites covered?

PASS. `src/api/client.rs` shows the cap check (`if delay > MAX_RETRY_AFTER_SECS`) at two
locations:
- Line 318 (inside `send()` 429 handler)
- Line 425 (inside `send_raw()` 429 handler)

Both call sites verified. The cap is symmetric; neither path can bypass it.

#### (2) Anti-loop guard edge case — None cursor handling?

PASS. The guard at `src/api/jira/issues.rs:100`:

```rust
if next_cursor.is_some() && next_cursor == prev_cursor {
```

The `is_some()` check correctly short-circuits when `next_cursor` is `None`. When
the server returns no `nextPageToken` (i.e., `next_cursor == None`), `page_has_more`
should be false and the loop should have already broken at the `if !page_has_more { break; }`
check above. The guard is positioned after that break, so a legitimate terminal `None`
cursor never reaches the guard. The `is_some()` guard is belt-and-suspenders correctness
for a server that might return `isLast: false` with no token (another known JRACLOUD
inconsistency). This is correct and conservative.

#### (3) Profile name validation order — length before charset?

PASS. `src/config.rs:119-134` checks `name.is_empty() || name.len() > 64` first (returns
"too long" message), then charset (returns "invalid characters" message). Order is correct
and matches BC-6.1.004 spec requirement.

#### (4) AC-001 `start_paused = true` omission — intentional and documented?

PASS. The test file documents the omission extensively at lines 57-76:

> `start_paused = true` is intentionally absent. `start_paused + wiremock` is incompatible:
> tokio auto-advances the virtual clock before the mock server's TCP accept task is scheduled,
> causing `timeout` to fire at T=10s instantly.

The PR description also documents this in the "AC-001 Implementation Note" section. The
wall-clock test correctly verifies the abort semantics without the time-control incompatibility.
This is the right trade-off for this scenario.

#### (5) Any other blocking issues?

None found.

Additional observations (non-blocking):
- `Cargo.toml` comment says "enables tokio::time::pause() + start_paused = true" but AC-001
  doesn't use `start_paused`. The comment is slightly misleading but the feature itself is
  correct. `test-util` is required for `tokio::time::timeout` in async tests. Non-blocking.
- `more_available` is not updated when the anti-loop guard fires (stays `false`). This means
  `SearchResult.has_more` reports `false` even though there may be more results. Given that
  the guard fires on a server bug (looping cursor), `false` is a reasonable defensive default
  (callers get "done" rather than "keep trying"). This is acceptable behavior.
- Test file `rate_limit_cap_tests.rs` uses mixed naming conventions (some AC-prefixed names,
  some `test_` prefix). Per CLAUDE.md convention, new tests should use `test_<verb>_<subject>`
  format, but this was an intentional deviation noted in the story spec (the test naming
  reflects AC identifiers for direct traceability). Non-blocking per precedent.

---

## Triage Routing

No findings require routing. All checks passed.

---

## Final Status

**APPROVE** — PR #315 is ready to merge. No blocking findings in cycle 1.
