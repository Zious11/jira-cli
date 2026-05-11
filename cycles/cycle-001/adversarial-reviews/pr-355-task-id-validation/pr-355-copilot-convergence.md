---
document_type: copilot-convergence-record
pr: 355
branch: chore/task-id-validation-332
head_sha: 62766f4
closes_issues: ["#332"]
rounds: 3
final_trajectory: "3→1→0"
converged: true
convergence_round: 3
review_round_1_id: 4265474208
review_round_1_submitted: 2026-05-11T16:27:55Z
review_round_2_id: 4265541072
review_round_2_submitted: 2026-05-11T16:37:30Z
review_round_3_id: 4265717871
review_round_3_submitted: 2026-05-11T17:01:09Z
pr_state: OPEN
mergeable: true
merge_state_status: CLEAN
ci_status: "8/8 green"
threads_total: 4
threads_resolved: 4
---

# PR #355 Copilot Convergence Record

**PR:** https://github.com/Zious11/jira-cli/pull/355
**Branch:** chore/task-id-validation-332
**Final tip SHA:** 62766f4
**Closes:** #332 on merge
**Final trajectory:** 3→1→0 (3 rounds)

## Summary

PR #355 implemented defense-in-depth security validation for `BulkSubmitResponse.task_id`
(issue #332). The `task_id` field is subsequently used in URL paths and terminal output;
CR/LF injection from a hostile/spoofed Atlassian response is the primary threat vector.

The convergence record documents two real security catches across 3 rounds, plus a notable
Perplexity calibration event: R1 Perplexity-confirmed a path-confusion vulnerability
(RFC 3986 §5.2.4 dot-segments); R2 Perplexity contradicted observable Rust Debug formatter
behavior, which was corrected via local empirical verification.

Round 3 returned 0 inline comments — Phase 8 stop condition met.

## Round 1 (2026-05-11T16:27:55Z)

**Review id:** 4265474208
**Inline comments:** 3
**All valid**

### Finding 1 — Security (line 90): dot-segment path-confusion vulnerability

The initial allowlist permitted `.` and `..` as standalone `task_id` values. URL normalizers
following RFC 3986 §5.2.4 resolve dot-segments BEFORE transmission, so a `task_id` of `..`
would rewrite the URL path `/rest/api/3/bulk/queue/..` to its parent endpoint `/rest/api/3/bulk/`
— a real path-confusion attack class.

**Validation (Perplexity per DEC-018):**

Query: "RFC 3986 Section 5.2.4 'Remove Dot Segments': when a URL path ends with /. or /..,
do common HTTP client libraries (reqwest, curl, hyper) and intermediate proxies apply
dot-segment removal BEFORE sending the request..."

**Result: CONFIRMED** — Perplexity returned a clear "YES" with detailed evidence: reqwest/hyper/curl
all apply §5.2.4 before send, normalizing `/rest/api/3/bulk/queue/..` to `/rest/api/3/bulk/`
at client level. `urlencoding` does NOT percent-encode `.`, so the dot-segment reaches the
normalizer intact. Path-confusion attack class confirmed.

Citations:
- https://datatracker.ietf.org/doc/html/rfc3986
- https://uri.thephpleague.com/uri/7.0/rfc3986/

**Fix:** Added explicit `if task_id == "." || task_id == ".."` rejection BEFORE the length/charset
checks, with a dedicated error message citing RFC 3986 §5.2.4. Added 2 new dot-segment tests
(`"."` → rejected, `".."` → rejected) plus 1 boundary test confirming dots within a longer
valid token are still accepted (`.well-known` class). Triage: **Fix now** — confirmed real
security vulnerability.

### Finding 2 — UX (line 76): unactionable oversized-taskId error hint

The oversized-taskId error message included a literal ellipsis `jr api ...` as the hint for
what to do next. This is not actionable — users cannot literally run `jr api ...`.

**Validation:** Local file inspection. The existing codebase convention for "re-run the source
command" hints uses the originating command pattern (e.g., "re-run the bulk command").

**Fix:** Reworded both the oversized-taskId and empty-taskId error messages to use the
actionable "re-run the bulk command" pattern matching existing convention.

### Finding 3 — Consistency (line 299): misleading test comment about `..` and urlencoding

A test comment claimed `..` was "harmless after urlencoding::encode" — overlooking RFC 3986
§5.2.4 (as confirmed by Finding 1). The comment would mislead future readers about the actual
risk vector.

**Fix:** Corrected the test comment to accurately reflect that dot-segments are not
percent-encoded by `urlencoding::encode` and that the R1 fix explicitly rejects them.

**Fix commit:** b120032 (+64 -17 lines)
**Threads:** 3 created; 3/3 resolved after b120032 push

## Round 2 (2026-05-11T16:37:30Z)

**Review id:** 4265541072
**Inline comments:** 1
**Valid (security)**

### Finding — CWE-117 (line 180): `await_bulk_task` interpolates unvalidated task_id before poll_bulk_task

`poll_bulk_task` now validates `task_id` at its call site. However, `await_bulk_task` interpolates
the unvalidated `task_id` into its timeout error message BEFORE `poll_bulk_task` is called.
Specifically: if `timeout=0`, the deadline check fires first and emits the error message
containing the raw `task_id`. This reintroduces control-character/log/terminal injection for any
caller that passes a malicious ID to `await_bulk_task` directly.

**Validation (Perplexity per DEC-018):**

Query: "Rust standard library: does the Debug formatter (`{:?}`) for `&str` escape ASCII control
characters like \r, \n, \0, \t, and ANSI terminal escape sequences? Is using `{:?}` instead of
`{}` (Display) considered a defense against CWE-117?"

**Perplexity result: INCORRECT.** Perplexity claimed with high confidence that `{:?}` does NOT
escape `\r\n\0\t\x1b` for `&str`, citing `https://doc.rust-lang.org/std/fmt/trait.Debug.html`.
The behavioral claim was a hallucination.

**Local empirical verification (CONTRADICTED Perplexity):**
```rust
fn main() {
    let s = "abc\r\ndef\0\t\x1b[31mred\x1b[0m";
    println!("Display: {}", s);  // renders literal control chars
    println!("Debug:   {:?}", s); // "abc\r\ndef\0\t\u{1b}[31mred\u{1b}[0m"
}
```
Output via `| cat -v` confirmed: `{:?}` for `&str` DOES escape \r→`\r`, \n→`\n`, \0→`\0`,
\t→`\t`, \x1b→`\u{1b}` via `str::escape_debug`. Perplexity hallucinated.

**Calibration note:** This is the third documented instance of Perplexity hallucinating about
observable Rust stdlib behavior while citing correct documentation URLs. The tiered-validation
strategy handles this correctly: run the program for empirically-verifiable behavior.

**Triage decision:** Rather than debate Display vs Debug formatter semantics (moot given the
empirical result), the architecturally clean fix was to add `validate_task_id(task_id)?` at the
VERY START of `await_bulk_task`, before deadline computation. This guarantees ALL interpolation
sites inside the function see only ASCII-allowlisted input — defense-in-depth, formatter-agnostic.

**Fix:** Added `validate_task_id(task_id)?;` as the first statement of `await_bulk_task`. Docstring
updated to credit CWE-117 defense-in-depth rationale. Triage: **Fix now** — genuine security gap
(entry-point validation missing; timeout=0 path was unguarded).

**Fix commit:** 62766f4 (+10 lines)
**Thread:** 1 created; 1/1 resolved after 62766f4 push

## Round 3 (2026-05-11T17:01:09Z)

**Review id:** 4265717871
**Inline comments:** 0
**Verbatim:** "Copilot reviewed 1 out of 1 changed files in this pull request and generated
no new comments."

### Phase 8 stop condition

Stop condition met at Round 3. The spec states: "The overview comment alone (no file-level
findings) is not a reason to continue." Round 3 produced 0 inline findings. Convergence
declared. No Round 4 dispatched.

## Trajectory Analysis

**Pattern:** 3→1→0 — both non-zero rounds addressed real security findings.

- R1: 3 findings (path-confusion vulnerability, UX actionability, test comment accuracy).
  Perplexity correctly confirmed RFC 3986 §5.2.4 dot-segment behavior.
- R2: 1 finding (entry-point validation gap; CWE-117 timeout=0 path). Perplexity incorrectly
  described Rust Debug formatter behavior; local empirical verification produced the correct answer.
- R3: 0 findings. Convergence.

Both non-zero rounds were genuine value-adds. R1 caught a path-confusion vulnerability that the
initial PR's test suite would not have detected (tests used well-formed task_ids). R2 caught a
subtle entry-validation gap on the `timeout=0` code path.

## Notable Observations

1. **R1 was a real security catch.** The `.` and `..` allowlist gap in the initial implementation
   was a genuine path-confusion vulnerability. Perplexity validation (DEC-018) correctly confirmed
   RFC 3986 §5.2.4 behavior for reqwest/hyper/curl. No false-positive.

2. **R2 was a real security catch.** The `await_bulk_task` entry-validation gap was subtle: the
   `timeout=0` deadline-check-before-first-poll path bypassed the `poll_bulk_task` call-site
   validation entirely. The correct fix was entry validation at `await_bulk_task`, not a
   formatter-switching workaround.

3. **Perplexity calibration event (R2).** Perplexity hallucinated about Rust `{:?}` Debug escape
   behavior while citing correct documentation URLs. The tiered-validation backstop (local empirical
   verification) caught the hallucination before the wrong diagnosis was acted on. Net result:
   DEC-018 still produced the correct final answer; the calibration is now codified in lessons.md.

4. **DEC-018 earned its keep on R1** (Perplexity correctly confirmed §5.2.4) but caused calibration
   friction on R2 (Perplexity contradicted Rust empirical behavior). Net win: standing rule plus
   tiered-validation backstop produced the right answer in both cases.

## CI Status

**Head SHA:** 62766f4
**CI result:** 8/8 green

| Job | Result |
|-----|--------|
| Format | green |
| Clippy | green |
| Test (ubuntu) | green |
| Test (macos) | green |
| MSRV 1.85.0 | green |
| Deny | green |
| Coverage | green |
| Secret Scan | green |

## Final PR State

| Field | Value |
|-------|-------|
| **State** | OPEN |
| **Mergeable** | true |
| **Merge state status** | CLEAN |
| **CI** | 8/8 green on 62766f4 |
| **Threads** | 4 created (3 R1 + 1 R2); 4/4 resolved |
| **Convergence** | CONVERGED at Round 3 |
| **Validation method** | R1: Perplexity confirmed RFC 3986 §5.2.4; R2: Perplexity contradicted Rust Debug escape → local empirical verification authoritative |
| **Awaiting** | Human merge |
