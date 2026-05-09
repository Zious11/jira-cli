# S-3.07 Demo Evidence Report

Story: LOW NFR code fixes + JRACLOUD-94632 anti-loop guard
Branch: feat/s-3-07-low-nfr-fixes-and-jracloud-anti-loop
Story version: v2.0.0
Type: STRICT TDD (3 parts: A Retry-After cap, C profile name precision, D JRACLOUD-94632 cursor loop)
Recorded: 2026-05-08

---

## Per-AC Evidence

### AC-001 — Retry-After: 2400 exceeds cap → abort within wall-clock time

**Spec:** BC-X.4.009 postcondition. When Jira returns `Retry-After: 2400` (typical Atlassian value),
the client must detect `2400 > MAX_RETRY_AFTER_SECS=60`, abort the retry loop immediately with no
sleep, and return `Err` in microseconds.

- Demo: `AC-001-retry-after-cap-aborts.gif`
- Webm: `AC-001-retry-after-cap-aborts.webm`
- Tape: `AC-001-retry-after-cap-aborts.tape`
- Test: `tests/rate_limit_cap_tests.rs::ac_001_retry_after_exceeds_cap_aborts_retry`
- Verdict: **PASS**

### AC-002 — Retry-After within cap → retry preserved (regression-pin)

**Spec:** BC-X.4.009 regression-pin. When `Retry-After: 0` (within 60s cap), the retry loop
MUST proceed and return `Ok` on the subsequent 200 response.

- Demo: `AC-002-within-cap-retries.gif`
- Webm: `AC-002-within-cap-retries.webm`
- Tape: `AC-002-within-cap-retries.tape`
- Test: `tests/rate_limit_cap_tests.rs::ac_002_retry_after_within_cap_retries`
- Verdict: **PASS**

### AC-003 — MAX_RETRY_AFTER_SECS constant defined as 60

**Spec:** BC-X.4.009 invariant. `jr::api::rate_limit::MAX_RETRY_AFTER_SECS` must be `pub` and
equal `60u64` (interactive-CLI fail-fast trade-off per RFC 9110 §10.2.3).

- Demo: `AC-003-cap-constant-defined.gif`
- Webm: `AC-003-cap-constant-defined.webm`
- Tape: `AC-003-cap-constant-defined.tape`
- Test: `tests/rate_limit_cap_ac003.rs::ac_003_max_retry_after_secs_constant_defined`
- Source: `src/api/rate_limit.rs:12` — `pub const MAX_RETRY_AFTER_SECS: u64 = 60;`
- Verdict: **PASS**

### AC-NEW-B — 3-arg `parse_duration` deleted on develop (S-3.10 sequencing gate)

**Spec:** Sequencing gate — ensures S-3.10 merged before S-3.07 merges to develop.
No demo recording needed; verified by static evidence.

**Static evidence:**

```
$ git show origin/develop:src/duration.rs | grep "fn parse_duration"
21:pub fn parse_duration_validate(input: &str) -> Result<()> {
```

Only the 1-arg validator survives on `develop`. The 2-arg `parse_duration(input, hours_per_day)`
was deleted by S-3.10 at commit f492e59. AC-NEW-B is satisfied by that merge.

- Verdict: **SATISFIED** (S-3.10 merge already on develop)

### AC-006 — Profile name too long → "max 64" error message

**Spec:** BC-6.1.004. `validate_profile_name` must return `ConfigError("Profile name too long
(max 64 characters)")` when `name.len() > 64`.

- Demo: `AC-006-profile-name-too-long.gif`
- Webm: `AC-006-profile-name-too-long.webm`
- Tape: `AC-006-profile-name-too-long.tape`
- Test: `src/config.rs::config::tests::test_validate_profile_name_too_long_message`
- Source: `src/config.rs:119-124` — length check, `JrError::ConfigError("Profile name too long (max 64 characters)")`
- Verdict: **PASS**

### AC-007 — Profile name with space → "invalid characters" error message

**Spec:** BC-6.1.004. `validate_profile_name` must return `ConfigError("Profile name contains
invalid characters (use a-z, 0-9, -, _)")` when name contains any char outside `[A-Za-z0-9_-]`.

- Demo: `AC-007-profile-name-charset.gif`
- Webm: `AC-007-profile-name-charset.webm`
- Tape: `AC-007-profile-name-charset.tape`
- Test: `src/config.rs::config::tests::test_validate_profile_name_with_space_message`
- Source: `src/config.rs:126-134` — charset check, `JrError::ConfigError("Profile name contains invalid characters (use a-z, 0-9, -, _)")`
- Verdict: **PASS**

### AC-008 + AC-NEW-D — JRACLOUD-94632 anti-loop guard terminates + emits warning

**Spec:** NFR-R-F (DOCUMENT-AS-IS-FIXED routing). When `/rest/api/3/search/jql` returns the same
`nextPageToken` twice (JRACLOUD-94632 bug), `search_issues` MUST:
1. Terminate the pagination loop (AC-008) — verified by 15s `assert_cmd` timeout not firing
2. Emit `"JRACLOUD-94632"` to stderr (AC-NEW-D) — verified by `stderr.contains("JRACLOUD-94632")`

- Demo: `AC-008-jracloud-anti-loop.gif`
- Webm: `AC-008-jracloud-anti-loop.webm`
- Tape: `AC-008-jracloud-anti-loop.tape`
- Test: `tests/rate_limit_cap_tests.rs::ac_008_and_ac_new_d_search_jql_cursor_loop_terminates_with_jracloud_warning`
- Source-level evidence:

```rust
// src/api/jira/issues.rs:59-106
// Anti-loop guard: Jira Cloud /rest/api/3/search/jql intermittently returns
// the same nextPageToken twice, causing infinite pagination loops. This is a
// confirmed upstream bug — JRACLOUD-94632, JRACLOUD-92049, JRACLOUD-85546
// (also reported in atlassian/atlassian-mcp-server#118 and
// ankitpokhrel/jira-cli#898). Mirrors anti-loop pattern from get_changelog
// (lines 222-230). When the guard fires, we emit a stderr warning citing
// JRACLOUD-94632 so users have a search term.
let mut prev_cursor: Option<String> = None;

// ... pagination loop ...

// GUARD: detect repeated cursor token (next == prev) → abort + warn.
// NFR-R-F: prevents infinite loop when server returns the same
// nextPageToken twice (confirmed upstream bug JRACLOUD-94632).
if next_cursor.is_some() && next_cursor == prev_cursor {
    eprintln!(
        "[jr] WARNING: Atlassian /rest/api/3/search/jql returned the same \
         nextPageToken twice — aborting pagination loop. Some results may \
         be missing. Upstream bug: JRACLOUD-94632."
    );
    break;
}
```

- Verdict: **PASS** (AC-008 + AC-NEW-D)

---

## Coverage Summary

| AC | Artifact | Verdict |
|----|----------|---------|
| AC-001 | AC-001-retry-after-cap-aborts.gif + .webm + .tape | PASS |
| AC-002 | AC-002-within-cap-retries.gif + .webm + .tape | PASS |
| AC-003 | AC-003-cap-constant-defined.gif + .webm + .tape | PASS |
| AC-NEW-B | Static evidence in this report | SATISFIED |
| AC-006 | AC-006-profile-name-too-long.gif + .webm + .tape | PASS |
| AC-007 | AC-007-profile-name-charset.gif + .webm + .tape | PASS |
| AC-008 | AC-008-jracloud-anti-loop.gif + .webm + .tape | PASS |
| AC-NEW-D | AC-008-jracloud-anti-loop.gif (combined) | PASS |

**Total artifacts:** 18 files (6 .gif + 6 .webm + 6 .tape)
**Overall verdict: ALL ACs PASS**
