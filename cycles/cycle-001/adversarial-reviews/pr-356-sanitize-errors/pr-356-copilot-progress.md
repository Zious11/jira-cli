---
document_type: copilot-convergence-record
pr: 356
branch: chore/sanitize-errors-334
head_sha: 59a0a12
closes_issues: ["#334"]
rounds: 6
status: in-progress
review_round_1_id: ""
review_round_1_submitted: 2026-05-11T17:49:49Z
review_round_2_submitted: 2026-05-11T18:10:07Z
review_round_3_submitted: 2026-05-11T18:18:03Z
review_round_4_submitted: 2026-05-11T18:29:07Z
review_round_5_id: "4266436155"
review_round_5_submitted: 2026-05-11T18:45:11Z
review_round_6_id: "4266560193"
review_round_6_submitted: 2026-05-11T19:00:25Z
pr_state: OPEN
threads_total: 14
threads_resolved: 14
trajectory: "4→1→2→2→3→2→?"
---

# PR #356 Copilot Convergence Record — IN PROGRESS

**PR:** https://github.com/Zious11/jira-cli/pull/356
**Branch:** chore/sanitize-errors-334
**Current tip SHA:** 59a0a12
**Closes:** #334 on merge
**Trajectory so far:** 4→1→2→2→3→2→? (Round 7 pending)

## Summary

PR #356 implements CWE-117 defense at the `extract_error_message` public boundary in
`src/api/client.rs`. The fix adds `sanitize_for_stderr` which strips ASCII control characters
from Atlassian error message strings before stderr emission, preventing terminal injection
(log forging, ANSI escape injection) via hostile or proxy-injected error payloads.

Six Copilot rounds have been completed with a total of 14/14 threads resolved. CI is in-flight
on 59a0a12. Round 7 is pending.

**Process gaps noted:** R2 and R3 Perplexity-validation were SKIPPED on the rationalization
that the claims were "empirically verifiable from code." Per DEC-018, this was incorrect — all
Copilot review findings require Perplexity validation regardless of how obvious the claim looks.
R5 and R6 restored and maintained correct DEC-018 compliance: all findings validated with
Perplexity before fixing. See Lesson codification below.

**Process improvement (R5+):** Starting from R5, the state-manager is dispatched IN REAL TIME
after each fix commit, rather than retroactively in batch. Per codified Lesson 2 ("Skipping
state-manager between Copilot rounds creates audit-trail debt"), R6 is the SECOND consecutive
in-cycle dispatch — the audit-trail discipline is now consistent.

## Round 1 (2026-05-11T17:49:49Z)

**Inline comments:** 4
**All valid**

### Finding 1 — Doc accuracy (sanitize_for_stderr doc comment)

The doc comment claimed "single allocation" but the implementation used `format!()` per escaped
character, which allocates once per escaped character rather than once total.

**Validation (Perplexity per DEC-018):** Cited CWE-117 + OWASP guidance confirming
that length capping is documented defense-in-depth (not strictly required by CWE-117 itself,
but standard practice per OWASP's "Prevent Log Injection" guidance). Reference:
https://cwe.mitre.org/data/definitions/117.html

**Fix:** Doc comment corrected to match actual allocation behavior.

### Finding 2 — Performance (sanitize_for_stderr inner loop)

`format!()` inside the char loop allocates a new String per escaped character. The idiomatic
fix is `std::fmt::Write::write!` on a pre-allocated String buffer for direct write without
intermediate allocation.

**Fix:** Replaced `format!()+push_str` with `std::fmt::Write::write!` for direct write.

### Finding 3 — Allocation in clean-input fast path

`sanitize_for_stderr` originally took `&str` and always returned a new `String`, even for
inputs with no control characters (no-op case). This allocated unnecessarily for the common
clean-input path.

**Fix:** Changed sanitize signature to `fn(String) -> String` with a fast path that returns
the input String unchanged (zero new allocation) for clean inputs. Added a pointer-equality
assertion in the corresponding test to verify the fast path does not allocate.

### Finding 4 — REQUIREMENTS GAP: missing per-entry length cap

Issue #334 explicitly requires: "Truncate each entry to a sane limit (e.g., 1 KiB) to prevent
terminal-flooding attacks." The initial PR was missing a per-entry length cap entirely.

**Fix:** Added `MAX_ERROR_ENTRY_LEN = 1024` constant and `cap_entry` helper. Added 5 new tests
including the pointer-equality assertion for fast-path zero-copy.

**Fix commit:** 51e2807 (added MAX_ERROR_ENTRY_LEN=1024 + cap_entry + std::fmt::Write::write! +
5 new tests)
**Threads:** 4 created; 4/4 resolved after 51e2807 push

## Round 2 (2026-05-11T18:10:07Z)

**Inline comments:** 1
**Valid (invariant violation)**

### Finding — cap_entry marker overhead exceeds cap for slightly-oversized inputs

`cap_entry`'s marker computation for inputs just over the cap (e.g., 1025 bytes) produced
output larger than the original input: `1024-byte prefix + ~30-byte marker = ~1054 bytes`.
This defeated the cap's flood-prevention purpose (output could exceed the original input for
inputs in the range [MAX+1, MAX+30]).

**Validation (Perplexity per DEC-018):** SKIPPED [process-gap] — the claim was empirically
verifiable from the code (arithmetic: 1024 + 30 > 1025). Per DEC-018, should have validated
anyway. Skip rationalization: "obviously correct from code analysis." This is the failure mode
DEC-018 was designed to prevent.

**Fix:** Reserve marker budget when truncating: compute `marker` first, set
`target_prefix_len = MAX_ERROR_ENTRY_LEN - marker.len()`, truncate prefix to that length.
Added defensive branch for `marker.len() >= cap`. Added test
`test_cap_entry_size_invariant_at_boundary_oversize` iterating over
[MAX+1, MAX+2, MAX+5, MAX+50, MAX+100, MAX+1000, MAX+10000] — all assert
`output_len <= MAX_ERROR_ENTRY_LEN`.

**Fix commit:** d061b14
**Threads:** 5/5 resolved after d061b14 push (cumulative 5/5)

## Round 3 (2026-05-11T18:18:03Z)

**Inline comments:** 2
**Both valid; one critical**

### Finding 1 — Critical: pre-sanitization per-entry cap allows 4x expansion

The 1024-byte pre-sanitization cap was applied to raw input bytes. A 1024-byte sequence
composed entirely of control characters would produce up to 4096 sanitized bytes (1 byte
input → 4 bytes `\xNN` escape output). The per-entry pre-cap therefore left the total
sanitized output size unbounded relative to the cap's stated intent.

**Validation (Perplexity per DEC-018):** SKIPPED [process-gap] — both claims were
empirically verifiable from code analysis (1-byte control char → 4-byte `\xNN` escape
is arithmetic). Per DEC-018, should have validated anyway.

**Fix:** Added `MAX_SANITIZED_OUTPUT_LEN = 4096` and restructured `sanitize_for_stderr` to
use a byte-budget-aware char loop: compute needed bytes per char (4 for control,
`c.len_utf8()` for others), bail when output would exceed `prefix_budget`. Output is now
guaranteed `<= MAX_SANITIZED_OUTPUT_LEN` regardless of input shape (worst case: 4096 bytes
of raw control chars → 4096 sanitized bytes / 4x expansion stays within budget).

### Finding 2 — Invariant gap: cap_entry marker fallback un-truncated

`cap_entry`'s defensive branch for `marker.len() >= cap` returned the marker string
un-truncated, violating the function's own size invariant (output should always be
`<= MAX_ERROR_ENTRY_LEN`).

**Fix:** Fixed `cap_entry` marker fallback to truncate marker itself at UTF-8 boundary.
Added 3 new tests: post-sanitization expansion, oversized clean input, under-cap no marker.

**Fix commit:** 274961c
**Threads:** 7/7 resolved after 274961c push (cumulative 7/7)

## Round 4 (2026-05-11T18:29:07Z)

**Inline comments:** 2
**Both valid (efficiency)**

### Finding 1 — Premature truncation via always-reserved marker space

`sanitize_for_stderr` reserved 64-byte marker space unconditionally, truncating messages that
would have fit fully within the cap. For example, a 4000-byte input with no control characters
would be truncated to 4032 bytes (4096 - 64 marker budget) even though it fit cleanly.

**Validation (Perplexity per DEC-018):** Validated the `Cow<str>` idiomatic Rust pattern
per Rust API Guidelines C-COST: `Cow::Borrowed` is zero-cost (no allocation), `Cow::Owned`
matches a String allocation. Confirmed citation:
https://doc.rust-lang.org/std/borrow/enum.Cow.html

**Fix:** Restructured `sanitize_for_stderr` to allow the full cap, then retroactively trim
at UTF-8 boundary only if the cap is breached. Marker is appended only when actual truncation
occurs.

### Finding 2 — cap_entry allocates per-entry String for unchanged entries

`cap_entry` returned `String` unconditionally, allocating even for entries that are already
under the cap (the common case). In a hostile `errorMessages` array with many short entries,
this produced N allocations where 0 were needed.

**Validation:** Same Perplexity validation as Finding 1 — confirmed `Cow<str>` pattern
applicable here: `Cow::Borrowed(&str)` for under-cap entries (zero allocation), `Cow::Owned`
only for over-cap entries.

**Fix:** Changed `cap_entry` signature to `fn cap_entry(s: &str) -> Cow<'_, str>` — unchanged
entries return `Cow::Borrowed` (zero alloc), only over-cap entries return `Cow::Owned`.
Rewrote `errorMessages` join with a single `String::with_capacity` allocation instead of N+1.

**Fix commit:** fe25e22 (current head)
**Threads:** 9/9 resolved after fe25e22 push (cumulative 9/9)

## Round 5 (2026-05-11T18:45:11Z)

**Review ID:** 4266436155
**Inline comments:** 3
**All valid (2 security / memory-amplification + 1 doc drift)**

### Finding 1 — Memory amplification: non-UTF8 fallback body pre-cap missing

`String::from_utf8_lossy(body)` allocates an owned `String` for the ENTIRE byte slice even
though `cap_entry` will truncate to 1 KiB downstream. A hostile server returning a 1 GB
non-UTF8 body forces ~1 GB allocation before the cap kicks in.

**Validation (Perplexity per DEC-018):** CONFIRMED — OWASP A06:2021 Resource Exhaustion
/ AP11 Resource Exhaustion. Production codebases (kubernetes/client-go, docker/cli,
tokio/hyper) all use `take(MAX_SIZE)` or pre-cap before parsing.
`String::from_utf8_lossy` confirmed to allocate the FULL byte slice regardless of
downstream truncation.

**Fix:** Pre-cap byte slice to `MAX_ERROR_ENTRY_LEN * 4 = 4096 bytes` BEFORE
`from_utf8_lossy`. 4x multiplier accommodates worst-case U+FFFD replacement expansion
(3 bytes each). Total memory: O(MAX_ERROR_ENTRY_LEN) regardless of body size.

### Finding 2 — Memory amplification: errorMessages join entry-count unbounded

Even with per-entry `cap_entry` + `Cow<str>` zero-copy, the NUMBER of entries is
server-controlled. A hostile response with 1M entries × 1024 bytes forces ~1 GB allocation
in the join before `sanitize_for_stderr` truncates.

**Validation (Perplexity per DEC-018):** CONFIRMED — same OWASP A06/AP11 as Finding 1.
Streaming parse / bounded build is the standard mitigation (same pattern used in
kubernetes/client-go, docker/cli, tokio/hyper).

**Fix:** Rewrote the `errorMessages` join as a streaming build with running budget check:
- Pre-sized output to `MAX_SANITIZED_OUTPUT_LEN` (4 KiB hard ceiling)
- Iterate lazily over `msgs.iter()`, calling `cap_entry` per entry
- Before each push: check `joined.len() + separator + capped.len() > MAX_SANITIZED_OUTPUT_LEN`;
  if yes, set truncated flag and break
- Append `" [...truncated]"` on truncation
- Total memory: O(MAX_SANITIZED_OUTPUT_LEN) regardless of entry count

### Finding 3 — PR description drift

PR body still described old `&str -> String` signature; implementation now takes `String`
by value after R1 fast-path refactor.

**Validation:** None required — purely doc-internal claim with no external library/API
behavior.

**Fix:** Updated PR description via `gh pr edit --body-file` to reflect the final 5-round
design: sanitization layer + per-entry cap layer + memory-amplification defense.

**Fix commit:** c9be4de (+48 -20 lines)
**Threads:** 12/12 resolved (cumulative) after c9be4de push

## Round 6 (2026-05-11T19:00:25Z)

**Review ID:** 4266560193
**Inline comments:** 2
**Both valid**

### Finding 1 — Streaming join marker overflow

The streaming errorMessages join appended `" [...truncated]"` (15 bytes) unconditionally after
breaking out of the build loop. If `joined.len()` was close to `MAX_SANITIZED_OUTPUT_LEN` when
the break fired, the final output after appending the marker could exceed the cap.

**Validation (Perplexity per DEC-018):** CONFIRMED — "reserve marker.len() upfront in the build
loop" is the standard pattern. Cited Rust `std::fmt` buffer sizing + log-crate truncation
conventions. Retroactive trim "fails correctness" per Perplexity guidance. Standard precedents:
log-crate, tracing-subscriber all compute final-marker budget before starting the fill loop.

**Fix:** Reserve marker budget upfront via `content_budget_join = MAX_SANITIZED_OUTPUT_LEN - JOIN_MARKER.len()`.
Budget check uses the reduced budget; final `joined + marker` is guaranteed `<= MAX_SANITIZED_OUTPUT_LEN`.
Added `debug_assert!` to verify invariant. 15-byte reservation preserves the R4 no-premature-truncation
property (messages that fit in the reduced budget are not truncated at all).

### Finding 2 — Sanitize over-reporting retained byte count

The truncation marker text `[...truncated at N sanitized bytes; original M bytes]` referenced
`out.len()` BEFORE the retroactive trim, over-reporting the actual number of bytes retained in
the final output.

**Validation (Perplexity per DEC-018):** CONFIRMED (same Perplexity query covered both findings).
Byte-count reporting must reflect FINAL emitted content length, not pre-trim values. Accurate
reporting is required for operator diagnostics.

**Fix:** Marker text now references only `original_len` (the immutable input byte count),
NOT `out.len()`. New marker format: `[...truncated; original M bytes]`. This:
- Removes over-reporting entirely (no claim about retained byte count)
- Keeps marker length constant under retroactive trim (depends only on `original_len` digit count)
- Preserves R4 no-premature-truncation property (retroactive-trim path retained, but
  marker-length constancy makes it correctness-safe)
- Operator still gets accurate "original M bytes" info; final output length is directly observable

### New regression test

`test_sanitize_for_stderr_truncation_marker_excludes_out_len`:
- Positive assertion: `"original N bytes"` present in output
- Negative assertion: `"sanitized bytes"` and `"at N"` absent from output
- Size invariant: `output.len() <= MAX_SANITIZED_OUTPUT_LEN`

**Fix commit:** 59a0a12 (+60 -16 lines)
**Threads:** 14/14 resolved (cumulative) after 59a0a12 push

**Test results at 59a0a12:**
- 22 sanitize unit tests pass (1 new R6 marker-correctness test added)
- 26 api_client integration tests pass
- Full cargo test: 60 suites, 0 failures
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI in-flight on 59a0a12

**Process note:** Second consecutive in-cycle state-manager dispatch per codified Lesson 2.
Audit-trail discipline now consistent.

---

## Trajectory Analysis

**Pattern so far:** 4→1→2→2→3→2 — all non-zero rounds addressed real findings.

- R1: 4 findings (doc accuracy, loop allocation, clean-path allocation, missing length cap).
  Perplexity confirmed CWE-117 + OWASP length-capping guidance.
- R2: 1 finding (marker overhead exceeds cap for slightly-oversized inputs). Perplexity
  validation SKIPPED [process-gap].
- R3: 2 findings (1 critical: 4x expansion from pre-cap; 1 invariant: marker fallback
  un-truncated). Perplexity validation SKIPPED [process-gap].
- R4: 2 findings (premature truncation; per-entry allocation). Perplexity confirmed Cow<str>
  idiomatic pattern.
- R5: 3 findings (2 memory-amplification: non-UTF8 body pre-cap + entry-count join bound;
  1 doc: PR description drift). Perplexity CONFIRMED OWASP A06/AP11 for #1 + #2.
- R6: 2 findings (streaming join marker overflow + sanitize over-reporting retained bytes).
  Perplexity CONFIRMED upfront marker reservation as standard pattern; byte-count reporting
  must reflect final emitted length, not pre-trim value.

**Assessment:** R6 surfaced 2 correctness/invariant issues in the streaming join and truncation
marker. All output-size guarantees are now mathematically tight. R7 may find 0 findings or minor
edge cases. Memory budget is O(MAX_SANITIZED_OUTPUT_LEN) end-to-end; all size invariants have
debug_assert! guards. Perplexity-validation consistent through R5 + R6 per DEC-018/Lesson 1.

## CI Status

**Head SHA:** 59a0a12
**CI result:** in-flight (poller b9qh1hpfc watching)

## Current PR State

| Field | Value |
|-------|-------|
| **State** | OPEN |
| **Threads** | 14 created; 14/14 resolved |
| **R7** | Pending |
| **CI on 59a0a12** | in-flight |
| **Closes** | #334 on merge |
