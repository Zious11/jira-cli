---
document_type: copilot-convergence-record
pr: 356
branch: chore/sanitize-errors-334
head_sha: 9acf01d
closes_issues: ["#334"]
rounds: 18
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
review_round_7_id: "4266726028"
review_round_7_submitted: 2026-05-11T19:23:31Z
review_round_8_id: "4266853645"
review_round_8_submitted: 2026-05-11T19:41:09Z
review_round_9a_id: "4266853645"
review_round_9a_submitted: 2026-05-11T19:41:09Z
review_round_9b_id: "4266950826"
review_round_9b_submitted: 2026-05-11T19:55:57Z
review_round_9c_submitted: 2026-05-11T20:08:56Z
review_round_10_id: "4268026428"
review_round_10_submitted: 2026-05-11T23:07:46Z
pr_state: OPEN
review_round_11_id: "4268102135"
review_round_11_submitted: 2026-05-11T23:27:03Z
review_round_12_id: "4268158285"
review_round_12_submitted: 2026-05-11T23:39:52Z
review_round_13_id: "4268206656"
review_round_13_submitted: 2026-05-11T23:52:40Z
review_round_14_id: "4268270089"
review_round_14_submitted: 2026-05-12T00:10:42Z
review_round_15_id: "4268312988"
review_round_15_submitted: 2026-05-12T00:23:00Z
review_round_16_id: "4268365143"
review_round_16_submitted: 2026-05-12T00:38:00Z
review_round_17_id: "4268400605"
review_round_17_submitted: 2026-05-12T00:54:00Z
review_round_18_id: "4268435007"
review_round_18_submitted: 2026-05-12T01:05:00Z
threads_total: 36
threads_resolved: 36
trajectory: "4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1"
---

# PR #356 Copilot Convergence Record — IN PROGRESS

**PR:** https://github.com/Zious11/jira-cli/pull/356
**Branch:** chore/sanitize-errors-334
**Current tip SHA:** 9acf01d
**Closes:** #334 on merge
**Trajectory so far:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1 (Round 19 pending)

## Summary

PR #356 implements CWE-117 defense at the `extract_error_message` public boundary in
`src/api/client.rs`. The fix adds `sanitize_for_stderr` which escapes ASCII control characters
from Atlassian error message strings before stderr emission, preventing terminal injection
(log forging, ANSI escape injection) via hostile or proxy-injected error payloads.

Eighteen Copilot rounds have been completed with a total of 36/36 threads resolved. CI is 8/8
green on 9acf01d. Round 19 is pending.

**Trajectory note:** R18 returned 1 finding (review 4268435007 @ 01:05Z, comment id 3223053065):
the `extract_error_message` public-API doc comment described only the ASCII escape branch
("\xNN for control chars") without mentioning the C1 Unicode escape branch ("\u{NNNN} for
U+0080..U+009F") added in R14; the threat-model phrase "CR/LF/ANSI" also omitted CSI (U+009B).
Extended the public-API doc to accurately describe both branches; expanded threat-model phrase
from "CR/LF/ANSI" to "CR/LF/ANSI/CSI".
No new tests; comment-only change; 39 sanitize tests + 26 api_client tests pass; 670 cargo test green.
1 thread resolved (PRRT_kwDORs-xfc6BQ2o4); reply 3223074074.
Trajectory is now 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1. R18 held at 1 — the R14 doc-fallout
cluster tapering is R15:2 → R16:3 → R17:1 → R18:1. All known R14 doc-fallout now corrected:
public API doc (R18), strategy bullets (R16-C1), C1 description (R16-C2), integration test
comment (R17), R-number annotations (R15-C2). Substantive defenses unchanged since R14.
Phase 8 prediction: R19 very likely 0-finding stop condition.

**Process gaps noted:** R2 and R3 Perplexity-validation were SKIPPED on the rationalization
that the claims were "empirically verifiable from code." Per DEC-018, this was incorrect — all
Copilot review findings require Perplexity validation regardless of how obvious the claim looks.
R5 and R6 restored and maintained correct DEC-018 compliance: all findings validated with
Perplexity before fixing. See Lesson codification below.

**Process improvement (R5+):** Starting from R5, the state-manager is dispatched IN REAL TIME
after each fix commit, rather than retroactively in batch. Per codified Lesson 2 ("Skipping
state-manager between Copilot rounds creates audit-trail debt"), R5 → R6 → R7 → R8 are FOUR
consecutive in-cycle dispatches — the audit-trail discipline is consistent habit.

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

## Round 7 (2026-05-11T19:23:31Z)

**Review ID:** 4266726028
**Inline comments:** 3
**All valid (documentation/annotation quality — no behavior change)**

### Finding 1 — Terminology: "strip" vs "escape" in docstring

`extract_error_message` docstring claimed the function "strips ASCII control chars" but the
implementation escapes them as visible `\xNN` literals (non-destructive, reversible
transformation preserving byte information). "Strip" implies irreversible deletion;
"escape" is the correct term for `\xNN` substitution.

**Validation (Perplexity per Lesson 1 / DEC-018):** CONFIRMED — OWASP/security-sanitization
terminology clearly distinguishes the two:
- "strip" = irreversible deletion (e.g., removing `<script>` tags from HTML)
- "escape" = reversible representation transformation (e.g., `&lt;` encoding, `\xNN` substitution)
- "neutralize" can mean either depending on context; "escape" is unambiguously correct here.
Citations: https://blog.presidentbeef.com/blog/2020/01/14/injection-prevention-sanitizing-vs-escaping/
and https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html

**Fix:** Reworded docstring to "escapes ASCII control chars from server-supplied content as
visible `\xNN` literals before they reach stderr ... while keeping the byte information
visible to the operator." Terminology now matches implementation.

### Finding 2 — Stale round annotations in inline comments

Several inline comments referenced "PR #356 R6 fix", "(R6 fix)", or "R[N] finding on PR #356"
— useful during iteration but stale post-merge. A future maintainer would need to reconstruct
the cycle history to understand these references.

**Validation (Perplexity per Lesson 1):** NO EXTERNAL CLAIM — purely project-internal
annotation cleanup. Lesson 1 wording addresses "at least one external-claim aspect"; findings
with no external claim do not require Perplexity. Skip is per-spec, not a rationalization.

**Fix:** Cleaned 6 comment sites — replaced round-specific annotations with stable descriptions.
Stable references retained: CWE-117, constant names, "issue #334" (closure persists via PR title).

### Finding 3 — Stale PR/round references in test annotations

Test comments like "Regression pin for the Copilot R2 finding on PR #356" don't decode for a
future reader without cycle history. The same annotation-cleanup concern as Finding 2, applied
to the test file.

**Validation (Perplexity per Lesson 1):** NO EXTERNAL CLAIM — same rationale as Finding 2.
Perplexity skipped per Lesson 1 wording.

**Fix:** Already addressed by the Finding 2 fix (overlapping cleanup scope). Test annotations
now describe the behavior being pinned: "Regression pin: inputs slightly larger than
MAX_ERROR_ENTRY_LEN..." instead of cycle references.

**Fix commit:** cdc4c64 (+33 -31 lines)
**Threads:** 17/17 resolved (cumulative) after cdc4c64 push (3 new R7 threads)

**Test results at cdc4c64:**
- 22 sanitize unit tests pass (no behavior change — all changes are doc/comment)
- Full cargo test: 60 suites, 0 failures
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI in-flight on cdc4c64

**Process note:** Third consecutive in-cycle state-manager dispatch per codified Lesson 2.
R5 → R6 → R7 all dispatched state-manager in real time. The discipline is now habit.

## Round 8 (2026-05-11T19:41:09Z)

**Review ID:** 4266853645
**Inline comments:** 2
**Both valid**

### Finding 1 — Errors-map memory amplification (same class as R5 errorMessages)

The errors-map extraction path used `.iter().map(...).collect()` then sorted then joined — the
same unbounded entry-count allocation pattern that R5 fixed for errorMessages. A hostile response
with 1M keys would force ~100 MB allocation before the join output is consumed.

**Validation (Perplexity per Lesson 1 / DEC-018):** RE-CITED OWASP A06/AP11 — Lesson 1 allows
re-citing prior validation for same-class findings. R5 confirmed this threat class (unbounded
entry-count allocation) for errorMessages; the errors-map path uses an identical pattern. Same
threat class, same mitigation category, prior validation stands.

**Fix:** Bounded entry count to `MAX_ERROR_PAIRS = 256` via `errors.iter().take(MAX_ERROR_PAIRS)`.
Added streaming join with upfront marker reservation mirroring the errorMessages path. Tracks both
`join_truncated` AND `pairs_truncated` states; marker text reflects active truncation condition.
Total memory: O(256 KiB) intermediate, O(4 KiB) output.

### Finding 2 — MAX_SANITIZED_OUTPUT_LEN doc inaccuracy

Doc comment said "still leaving room for the marker via reserved headroom inside
sanitize_for_stderr" — but the R4 restructure changed the implementation to retroactive trim.
The comment described the pre-R4 approach, not the current one.

**Validation (Perplexity per Lesson 1):** NO EXTERNAL CLAIM — purely doc accuracy. Lesson 1
wording requires "at least one external-claim aspect" to warrant Perplexity. A comment
describing a code mechanism has no such aspect. Skip is per-spec, not a rationalization.

**Fix:** Reworded doc to accurately describe the retroactive-trim approach: "after writing, the
buffer is trimmed at a UTF-8 boundary if it exceeds the cap, then the truncation marker is
appended."

**Fix commit:** e6262dd (+46 -7 lines)
**Threads:** 19/19 resolved after e6262dd push (2 new R8 threads)

**Test results at e6262dd:**
- 22 sanitize unit tests pass
- 26 api_client integration tests pass
- Full cargo test: 60 suites, 0 failures (parallel-execution flake in unrelated
  multi_cloudid_disambiguation test passed on single-threaded retry)
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI in-flight on e6262dd

**Process note:** Fourth consecutive in-cycle state-manager dispatch per codified Lesson 2.
R5 → R6 → R7 → R8 all dispatched state-manager in real time. The discipline is consistent habit.

---

## Round 9 (2026-05-11T19:55:57Z — R9b)

**Review ID (R9b):** 4266950826
**R9a review ID:** 4266853645 (pre-dates 85f0dd4 — re-raised prior concerns)
**R9c:** inline comments @ 2026-05-11T20:08:56Z and 20:08:57Z (re-raised prior concerns)
**New inline findings (R9b):** 2
**Both valid (memory-amplification gaps)**

### Finding 1 — Key-amplification in format!("{k}: {v}")

Server-controlled key `k` was used raw in `format!("{k}: {v}")`. With the R8 entry-count cap of
MAX_ERROR_PAIRS=256, a hostile server could send 256 entries each with a 1 MB key — intermediate
format! allocation reaches 256 MB before the final join truncates. The R8 entry-count cap was
necessary but not sufficient; key size was a separate uncapped dimension.

**Validation (Perplexity per DEC-018):** CONFIRMED — legitimate memory-amplification gap.
Keys are server-controlled and should be treated with the same cap discipline as values.

**Fix:** Wrap key in `cap_entry(k)` before `format!`. Key is now bounded to MAX_ERROR_ENTRY_LEN
(1024 bytes) before any allocation in the format! call.

### Finding 2 — Non-string errors values: v.to_string() materializes before cap

`v.to_string()` called on a `serde_json::Value` performs full JSON serialization — the entire
subtree is materialized as a String before `cap_entry` truncates the result. A single deeply
nested or large value (e.g., a 512 MB nested JSON array) forces a full allocation.

**Validation (Perplexity per DEC-018):** CONFIRMED — legitimate memory-amplification gap.
`serde_json::to_string()` / `.to_string()` on Value always allocates the full output regardless
of downstream truncation. The bounded-writer pattern prevents this.

**Fix:** New helper `serialize_value_bounded(v, MAX_ERROR_ENTRY_LEN)` uses
`serde_json::to_writer` with a `WriteZeroOnOverflow` adapter that returns `WriteZero` once the
byte limit is hit. Output is bounded to MAX_ERROR_ENTRY_LEN bytes without materializing the
full value.

### R9a / R9c Re-Raised Concerns (already addressed)

R9a (review 4266853645 @ 19:41:09Z) pre-dated commit 85f0dd4 and re-raised concerns addressed
in prior rounds. R9c (inline comments @ 20:08:56-57Z) similarly re-raised prior round concerns
mid-cycle. These were not new findings.

**Replies posted:**
- 3221850022, 3221850177, 3221850294, 3221850424 (R9a/R9b threads)
- 3222673033, 3222673079 (R9c threads)

**Total R9 threads created:** 4 (all R9b/R9c); all resolved after 85f0dd4 push.
**Cumulative threads:** 23 created; 23/23 resolved.

**Fix commit:** 85f0dd4 (pushed 2026-05-11T15:13:09-0500)
**CI result:** 8/8 green on 85f0dd4

**Test results at 85f0dd4:**
- 5 new unit tests pinning serialize_value_bounded contract
- 27 sanitize unit tests total
- 658 cargo test total green
- Parallel-execution flake: test_interactive_render_shows_name_url_and_id in multi_cloudid_disambiguation — passes single-threaded; unrelated to this change
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean

**Process note:** Fifth consecutive in-cycle state-manager dispatch per codified Lesson 2.
R5 → R6 → R7 → R8 → R9 all dispatched state-manager in real time. The discipline is fully embedded.

---

## Round 10 (2026-05-11T23:07:46Z)

**Review ID:** 4268026428
**Comment ID:** 3222691664
**Inline comments:** 1
**Valid (UX correctness gap)**

### Finding 1 — Silent truncation in serialize_value_bounded

`serialize_value_bounded` produced a truncated JSON prefix WITHOUT any visible marker when
overflow occurred. The `Bounded` writer stopped writing once the byte limit was hit, but did
not signal overflow — it simply returned fewer bytes silently. Since the returned String was
`<= limit`, the downstream `cap_entry` call did NOT add its own truncation marker either.

**Result:** Operators saw malformed-but-silently-incomplete JSON with no indication it was cut
off — a "looks valid but is actually malformed prefix" anti-pattern recognized in
tracing/slog/OpenTelemetry conventions. A JSON string value truncated mid-character would
silently produce an invalid string literal with no error hint.

**Validation (Perplexity per DEC-018):** CONFIRMED — Perplexity validated this as a legitimate
UX correctness gap matching the silent-truncation anti-pattern documented in tracing/slog/
OpenTelemetry best practices for bounded output. Standard fix: track overflow flag; reserve
marker bytes upfront so prefix-plus-marker total fits within limit.

**Fix:** `Bounded` writer tracks an `overflowed: bool` flag. `serialize_value_bounded`
reserves marker bytes upfront (`limit - " [...truncated]".len()`) so the prefix-plus-marker
total always fits within `limit`. Appends `" [...truncated]"` when `overflowed` is true.
Degenerate-case fallback: when `limit < marker.len()`, returns the marker prefix truncated
at `limit` (pinned via test).

**Thread resolved:** id 3222691664 → PRRT_kwDORs-xfc6BP1Oa
**Reply posted:** 3222725048

**New tests (3 new + 1 updated):**
1. `test_serialize_value_bounded_no_marker_no_overflow` — small value: no marker, no overflow.
2. `test_serialize_value_bounded_marker_fits_within_limit` — oversized value: marker present, output within limit.
3. `test_serialize_value_bounded_degenerate_tiny_limit` — degenerate case: limit < marker.len(); output truncated at limit.
4. Updated existing oversized test to also assert marker present (previously only checked size invariant).

**Fix commit:** f328a2f ("chore(security): append truncation marker to bounded JSON output (PR #356 R10)")
**Threads:** 24/24 resolved (1 new R10 thread resolved; cumulative)

**Test results at f328a2f:**
- 30 sanitize unit tests total (3 new + 1 updated from R10)
- 661 cargo test total green, 0 failed
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI in-flight on f328a2f (Format + Secret Scan green; 8/8 expected)

**Process note:** Sixth consecutive in-cycle state-manager dispatch per codified Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 all dispatched state-manager in real time. The discipline is fully embedded.

**Convergence signal:** R10 returned 1 new finding — DOWN from R9's 2. This is the first decline
since R5 (trajectory segment 2→1). The overall trajectory 4→1→2→2→3→2→3→2→2→1 shows the finding
count declining at R10 after holding at 2 for R9. Healthy converging signal toward the Phase 8
stop condition (0-new-comment round). R11 pending.

---

## Round 11 (2026-05-11T23:27:03Z)

**Review ID:** 4268102135
**Comment ID:** 3222756019
**Inline comments:** 1
**Valid (memory-amplification at JSON parse step — INPUT DOM)**

### Finding 1 — INPUT DOM allocation: serde_json::from_str materializes full Value before truncation

`extract_error_message_raw` called `serde_json::from_str(body_str)` to parse the JSON response
body into a `serde_json::Value` before extracting `errorMessages` / `errors` fields. This DOM
materialization allocates roughly 2-3x the body size in memory regardless of what the caller
intends to extract. All prior R5-R10 caps bounded OUTPUT only (per-entry caps, streaming joins,
bounded serializers) — none prevented the INPUT DOM from being allocated.

A hostile server returning a valid 100 MB JSON body (well under any TCP/HTTP transport limit)
would force 200-300 MB of serde_json::Value DOM allocation before any extraction or truncation
occurred. This is a distinct attack surface from the OUTPUT amplification vectors addressed
in R5-R10: it operates entirely on the INPUT side and is invisible to all downstream caps.

**Validation (Perplexity per DEC-018):** CONFIRMED — `serde_json::from_str` always materializes
a complete DOM. Byte-level gate before parse is Perplexity-validated as superior to streaming/
partial-parse approaches for this use case: zero allocation attack surface (the serde_json call
is never reached for over-threshold bodies), whereas streaming parsers still allocate proportional
to the tokens encountered before the abort point. 16 KiB threshold is generous for Jira errors
(<1 KiB typical) while closing the attack surface entirely.

**Fix:** New constant `MAX_PARSE_BODY_LEN = 16 * 1024`. In `extract_error_message_raw`, before
calling `serde_json::from_str`, check `body.len() > MAX_PARSE_BODY_LEN`. If true, skip JSON parse
and fall back to the existing byte-bounded raw-body path (which already applies `MAX_ERROR_ENTRY_LEN`
cap). For over-threshold bodies, no `serde_json::Value` DOM is created.

**Thread resolved:** id 3222756019 → PRRT_kwDORs-xfc6BQA9s
**Reply posted:** 3222775607

**New tests (3):**
1. `test_extract_skips_parse_for_huge_body` — body at `MAX_PARSE_BODY_LEN + 1`; verifies
   fallback path is taken (no JSON parse; output is byte-bounded raw snippet).
2. `test_extract_allows_normal_body` — body under threshold; verifies JSON parse path still
   active and `errorMessages` extraction works normally.
3. `test_parse_body_threshold_pinned` — asserts `MAX_PARSE_BODY_LEN == 16 * 1024` (pin
   prevents accidental constant drift).

**Fix commit:** 2ecc18c ("chore(security): byte-level size gate before JSON DOM parse (PR #356 R11)")
**Threads:** 25/25 resolved (1 new R11 thread resolved; cumulative)

**Test results at 2ecc18c:**
- 33 sanitize unit tests total (3 new from R11)
- 664 cargo test total: 664 passed, 0 failed, 10 ignored
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI: 8/8 green

**Process note:** Seventh consecutive in-cycle state-manager dispatch per codified Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 all dispatched state-manager in real time. The discipline
is fully embedded.

**Convergence signal:** R11 returned 1 new finding — same as R10 (trajectory segment ...→1→1).
Finding count has now plateaued at 1 for two consecutive rounds. This is a positive convergence
signal: Copilot is finding smaller and more marginal issues. R12 = 0 would trigger the Phase 8
stop condition (zero-new-comment round). R12 pending.

---

## Round 12 (2026-05-11T23:39:52Z)

**Review ID:** 4268158285
**Comment IDs:** 3222800383 (line null), 3222800411 (line null)
**Inline comments:** 2
**Both valid (contract violation + UX accuracy)**

### Finding 1 — `Bounded::write` violated `std::io::Write` contract

`Bounded::write` returned `Err(WriteZero)` after writing a prefix into `buf`, contradicting
the `std::io::Write` contract which mandates "If an error is returned then no bytes in the
buffer were written." The prior implementation partially wrote bytes AND returned an error —
a protocol violation that could cause serde_json's streaming serializer to produce inconsistent
output state.

**Validation (Perplexity per DEC-018):** CONFIRMED — `std::io::Write` contract is unambiguous.
Partial write + error is a well-documented protocol violation. The correct behavior for bounded
writers is: on remaining == 0, return Err(WriteZero) immediately (nothing written); on partial
fit, append the prefix, set overflowed, return Ok(buf.len()) so the caller believes all bytes
were consumed and stops retrying.

**Fix:** `Bounded::write` now returns `Err(WriteZero)` ONLY when `remaining == 0` at call entry.
For partial writes: appends only the bytes that fit, sets `overflowed = true`, returns
`Ok(buf.len())`. On the subsequent call `remaining == 0` fires immediately and returns `Err(WriteZero)`,
stopping serde_json cleanly.

### Finding 2 — Non-UTF8 fallback marker under-reported true body size

The non-UTF8 fallback path used `cap_entry` on the `from_utf8_lossy` output. `cap_entry`'s
marker reported the post-pre-cap lossy string length (max ~4096 bytes), NOT the actual
`body.len()`. For hostile or flood inputs (e.g., 1 MB non-UTF8 body), the marker
`[...truncated; original 4096 bytes]` silently under-reported the true size — operators saw
no signal that the body was unusually large.

**Validation (Perplexity per DEC-018):** CONFIRMED — accurate body-size reporting is required
for operator diagnostics; using the post-cap length hides true input size for hostile/flood
inputs. Custom marker with `body.len()` is the correct approach.

**Fix:** Bypass `cap_entry` in the non-UTF8 path. Build a custom marker:
`[...truncated, {body.len()} bytes total, non-UTF8 body]` using the true original byte count.
Explicitly flags the non-UTF8 source for disambiguation from normal JSON truncation markers.

**Threads resolved:** PRRT_kwDORs-xfc6BQI52 and PRRT_kwDORs-xfc6BQI6M
**Replies posted:** 3222826557, 3222826602

**Fix commit:** 6832967 ("chore(security): Write contract compliance + accurate non-UTF8 marker (PR #356 R12)")
**Threads:** 27/27 resolved (2 new R12 threads resolved; cumulative)

**Test results at 6832967:**
- 3 new unit tests: partial-write produces marker, 5 MB body marker reports true size, small non-UTF8 skips marker
- 36 sanitize unit tests total (3 new from R12)
- 667 cargo test total: 667 passed, 0 failed, 10 ignored
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI: 8/8 green

**Process note:** Eighth consecutive in-cycle state-manager dispatch per codified Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 → R12 all dispatched state-manager in real time. The
discipline is fully embedded.

**Trajectory note:** R12 ticked back to 2 findings (trajectory ...→1→1→2). Both findings are
non-overlapping with R11's INPUT-DOM class (contract-level + UX-level vs DOM-allocation). Not
a regression — Copilot is exploring different correctness categories. R13 pending. If R13
returns 0-1, convergence is on track. Expect 2-4 more rounds before stop condition.

---

## Round 13 (2026-05-11T23:52:40Z)

**Review ID:** 4268206656
**Comment ID:** 3222841940
**Inline comments:** 1
**Valid (documentation labeling error)**

### Finding 1 — OWASP/CWE labels incorrect for memory-amplification mitigation

Doc comments throughout `src/api/client.rs` labeled the memory-amplification mitigation as
"OWASP A06 / AP11" in multiple locations. Both labels are incorrect:

- **OWASP A06:2021** is "Vulnerable and Outdated Components" — covers dependency vulnerabilities
  and outdated software, NOT resource exhaustion or unbounded allocation.
- **"AP11"** does not correspond to any recognized standard categorization scheme (not OWASP API
  Security Top 10, not OWASP Top 10, not any CWE category notation).

The correct labels for this threat class (unbounded resource allocation from server-controlled
input — which is exactly what R5–R11 defended against):
- **OWASP API4:2023 (Unrestricted Resource Consumption)** — the specific OWASP API Security Top 10
  category for server responses that exhaust client-side memory/CPU
- **CWE-770 (Allocation of Resources Without Limits or Throttling)** — the authoritative CWE
  mapping for this defect class

**Validation (Perplexity per DEC-018):** CONFIRMED — OWASP API4:2023 is unambiguously the correct
category for unrestricted resource consumption; CWE-770 is the standard mapping. Both authoritative
references cited in commit message bcc2db4. Historical note: R5's original Perplexity validation
cited "OWASP A06/AP11" — those labels were accepted then but were incorrect. R13 correction
supersedes them in current source; historical commit messages and reply comments retain old labels
(immutable history).

**Fix:** Mechanical search-and-replace across 6 comment locations in `src/api/client.rs`:
- Old: `OWASP A06 / AP11`
- New: `OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of Resources Without Limits or Throttling)`

No behavior change; comment-only. No new tests needed (no production logic changed).

**Thread resolved:** PRRT_kwDORs-xfc6BQQan
**Reply posted:** 3222883003

**Fix commit:** bcc2db4 ("chore(security): correct OWASP/CWE labels for memory-amplification defenses (PR #356 R13)")
**Threads:** 28/28 resolved (1 new R13 thread resolved; cumulative)

**Test results at bcc2db4:**
- 36 sanitize unit tests total (no new tests — comment-only change)
- 667 cargo test total: 667 passed, 0 failed, 10 ignored
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI: 8/8 green

**Process note:** Ninth consecutive in-cycle state-manager dispatch per codified Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 → R12 → R13 all dispatched state-manager in real time.
The discipline is fully embedded — 9 rounds in.

**Convergence signal:** R13 returned 1 finding — down from R12's 2 (trajectory ...→1→1→2→1).
More significantly: the finding category has shifted from security-defense gaps (R5–R12: memory
amplification, Write contract violations, DOM allocation) to documentation quality (OWASP label
correctness). This is a qualitative convergence signal — Copilot has exhausted the security-defense
defect space and is now exploring incidental quality issues. Phase 8 stop condition (0-new-comment
round) is likely 1-2 rounds away.

---

## Round 14 (2026-05-12T00:10:42Z)

**Review ID:** 4268270089
**Comment ID:** 3222898738
**Inline comments:** 1
**Valid (defense-in-depth hardening — Unicode C1 control escape gap)**

### Finding 1 — Unicode C1 controls not escaped by `is_ascii_control()`

`sanitize_for_stderr` used `c.is_ascii_control()` to detect control characters that needed
escaping. `is_ascii_control()` covers only C0 controls (U+0000..U+001F) and DEL (U+007F) —
it does not cover Unicode C1 controls U+0080..U+009F. The C1 range includes:

- **CSI (U+009B)** — Control Sequence Introducer (same function as ESC+`[` in terminal escape
  sequences; used by ANSI/VT100 terminals to begin escape sequences such as cursor movement
  and color codes)
- **NEL (U+0085)** — Next Line (newline-equivalent in some terminal/text processing contexts)
- Other C1 controls: PAD, HOP, BPH, NBH, IND, HTS, HTJ, VTS, PLD, PLU, RI, SS2, SS3, DCS,
  PU1, PU2, STS, CCH, MW, SPA, EPA, SOS, SGCI, SCI, OSC, PM, APC, ST

In modern UTF-8 terminals, C1 code points encoded as 2-byte UTF-8 sequences (0xC2 + 0x80..0x9F)
are typically rejected as invalid sequences for terminals that expect ISO 8859-1 encoding, or
dropped as invalid continuation bytes in strict UTF-8 mode. This means C1 controls are NOT a
current exploitation vector in mainstream terminal environments. However, legacy terminals
(VT220, xterm in ISO mode), embedded systems, and non-UTF8 terminal emulators can interpret
C1 sequences directly, enabling the same terminal injection threat class as C0.

**Validation (Perplexity per DEC-018):** CONFIRMED — `char::is_control()` in Rust covers both
C0 and C1 ranges. The defense-in-depth rationale for escaping C1 controls is valid: the threat
exists in legacy/embedded terminal contexts even if not exploitable in mainstream UTF-8 terminals.
Rust's `char::is_control()` is the standard idiom for comprehensive control-character detection.

**Fix:**
- Switch `sanitize_for_stderr`'s control detection from `c.is_ascii_control()` to `c.is_control()`
- Branch on `c.is_ascii()` for escape format: ASCII controls (C0 + DEL) use `\xNN` (4 bytes, unchanged); C1 controls use `\u{NNNN}` (8 bytes)
- Fast-path scan changed from byte-level `bytes().any(|b| b.is_ascii_control())` to char-level `chars().any(|c| c.is_control())` — necessary because byte-level scan cannot distinguish C1 control code-point bytes from valid 2-byte UTF-8 continuation bytes
- The 4 KiB sanitized output cap comfortably absorbs the 8-byte `\u{NNNN}` escapes (worst case: 4 KiB cap / 8 bytes per C1 escape = 512 C1 controls, still within budget)

**3 new unit tests:**
1. `test_sanitize_csi_escape` — U+009B (CSI) → `\u{009b}` in output
2. `test_sanitize_nel_escape` — U+0085 (NEL) → `\u{0085}` in output
3. `test_sanitize_non_control_unicode_above_ascii_passes_through` — U+00C0 (LATIN CAPITAL LETTER A WITH GRAVE) must pass through unescaped (anti-regression: non-control Unicode above ASCII must not be touched)

**Thread resolved:** PRRT_kwDORs-xfc6BQamK
**Reply posted:** 3222921647

**Fix commit:** d4a07c8 ("chore(security): escape Unicode C1 controls in sanitize_for_stderr (PR #356 R14)")
**Threads:** 29/29 resolved (1 new R14 thread resolved; cumulative)

**Test results at d4a07c8:**
- 39 sanitize unit tests total (3 new from R14)
- 670 cargo test total: 670 passed, 0 failed, 10 ignored
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI: 8/8 green

**Process note:** Tenth consecutive in-cycle state-manager dispatch per Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 → R12 → R13 → R14 all dispatched state-manager in real time.
The discipline is fully embedded — 10 rounds in.

**Convergence signal:** R14 returned 1 finding — same as R13 (trajectory segment ...→1→1→2→1→1).
Two consecutive 1-finding rounds (R13, R14) with findings in the defense-in-depth / documentation
category (not security-defense gaps). Security defenses have been converged since R12; Copilot
is now exploring edge-case hardening and label correctness. Phase 8 stop condition (0-new-comment
round) is the likely outcome for R15 or R16.

---

## Round 16 (2026-05-12T00:38:00Z)

**Review ID:** 4268365143
**Comment IDs:** 3222985472, 3222985491, 3222985507
**Inline comments:** 3
**All doc-accuracy issues — consequences of R14 C1-control expansion**

### Finding 1 — Strategy bullets incomplete after R14 expansion (comment 3222985472)

The `sanitize_for_stderr` strategy block described "Replace every ASCII control character"
in its bullet list but R14 expanded the detection to `char::is_control()` (covering both
C0/DEL and C1 controls). The bullets described only the ASCII path, omitting the C1 branch.

**Validation per DEC-018:** No external claims — purely internal documentation accuracy.
Perplexity skipped per Lesson 1 (no external-claim aspect).

**Fix:** Rewrote strategy bullets to accurately list both escape branches:
- ASCII controls (C0 U+0000..U+001F + DEL U+007F): `\xNN` format (4 bytes per control char)
- Unicode C1 controls (U+0080..U+009F): `\u{NNNN}` format (8 bytes per control char)
- Both branches subject to the 4 KiB sanitized output cap; 8-byte C1 escapes still within budget
  (worst case: 4096 / 8 = 512 C1 controls fully escapable within cap)

### Finding 2 — C1 controls incorrectly described as "rejected as invalid UTF-8 continuation bytes" (comment 3222985491)

An inline comment introduced in R14 stated that C1 controls in UTF-8 encoded text are "rejected
as invalid UTF-8 continuation bytes." This is technically wrong:

- U+0080..U+009F are valid Unicode codepoints with valid 2-byte UTF-8 encodings
  (e.g., U+009B CSI = 0xC2 0x9B — a correctly-formed 2-byte sequence with a valid lead byte 0xC2
  and a valid continuation byte 0x9B)
- The actual terminal behavior: modern terminals in UTF-8 mode do NOT reject C1 2-byte sequences
  as invalid encoding — they ignore C1 semantics (treating the sequence as data, not control)
- Legacy terminals and terminals in ISO 8859-1 mode interpret C1 bytes (raw, not UTF-8 encoded)
  directly, which is where the terminal injection risk exists

**Validation per DEC-018:** No external claims about library behavior — the finding is about
UTF-8 encoding correctness (well-defined by the Unicode standard) and terminal behavior
documented in R14's own Perplexity validation. Perplexity skipped per Lesson 1.

**Fix:** Rewrote the comment to precisely distinguish:
- Raw-byte behavior: raw 0x80..0x9F bytes (ISO 8859-1 encoding) are interpreted as C1 controls
  by terminals in ISO mode
- UTF-8-encoded codepoint behavior: U+0080..U+009F encoded as 2-byte UTF-8 are NOT rejected as
  invalid — they are valid UTF-8; modern UTF-8-mode terminals simply don't interpret C1 semantics
- The defense-in-depth justification remains valid: legacy/embedded terminals and non-UTF8 terminal
  emulators can still be affected, so escaping is correct — but the mechanism is not "invalid encoding"

### Finding 3 — Integration test comment outdated: "only ASCII control bytes" (comment 3222985507)

The integration test comment in `tests/api_client.rs` stated "only ASCII control bytes are escaped"
— this was accurate before R14 but became false when R14 expanded the escape set to include Unicode
C1 controls U+0080..U+009F.

**Validation per DEC-018:** No external claims. Perplexity skipped per Lesson 1.

**Fix:** Updated comment to: "only control characters (ASCII C0/DEL and Unicode C1) are escaped;
printable Unicode passes through unchanged."

**Threads resolved:** PRRT_kwDORs-xfc6BQqRd (C1), PRRT_kwDORs-xfc6BQqRt (C2),
PRRT_kwDORs-xfc6BQqR6 (C3)
**Replies posted:** 3223009560 (C1), 3223009636 (C2), 3223009710 (C3)

**Fix commit:** dc09501 ("chore(security): correct doc strategy bullets + accurate C1 terminal behavior (PR #356 R16)")
**Threads:** 34/34 resolved (3 new R16 threads resolved; cumulative)

**Test results at dc09501:**
- 39 sanitize unit tests total (no new tests — comment-only changes)
- 26 api_client integration tests pass
- 670 cargo test total: 670 passed, 0 failed, 10 ignored (full test count unchanged)
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI: 8/8 green

**Process note:** Twelfth consecutive in-cycle state-manager dispatch per Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 → R12 → R13 → R14 → R15 → R16 all dispatched state-manager
in real time. 12 rounds of consistent Perplexity+Lesson-2 discipline across 16 review rounds.

**Convergence pattern:** R16 ticked up to 3 findings (trajectory now ...→1→1→2→3), but all 3
are doc-accuracy consequences of the R14 C1-control expansion — not new substantive issues.
The security defenses (memory-amplification bounds, Write contract compliance, DOM parse gate,
C0+C1 escape coverage) have not been touched since R14. Copilot is exploring doc accuracy of
the R14 change itself; once that doc-fallout is exhausted, the finding count should drop to 0-1.
**Prediction: R17 returns 0-1 findings — Phase 8 stop condition within reach.**

---

## Round 17 (2026-05-12T00:54:00Z)

**Review ID:** 4268400605
**Comment ID:** 3223021119
**Inline comments:** 1
**Valid (doc accuracy — integration-test header comment incomplete after R14 C1 expansion)**

### Finding 1 — Integration-test header comment describes only `\xNN` escape format (comment 3223021119)

The header comment block in `tests/api_client.rs` stated that hostile control characters render
"as \xNN literals". This was accurate before R14 but became incomplete when R14 expanded the
escape set to include Unicode C1 controls U+0080..U+009F, which are rendered as `\u{NNNN}`
(8-byte format) rather than `\xNN` (4-byte format). The comment continued to imply that only
the `\xNN` escape form was used, omitting the C1 branch entirely.

**Validation per DEC-018:** No external library or API behavior claims — purely internal
documentation accuracy. Perplexity skipped per Lesson 1 ("at least one external-claim aspect"
required). Skip is per-spec, not a rationalization.

**Fix:** Extended the header comment to explicitly cover both escape branches:
- ASCII C0/DEL controls: escaped as `\xNN` (4 bytes per character)
- Unicode C1 controls (U+0080..U+009F): escaped as `\u{NNNN}` (8 bytes per character)

No new tests; comment-only change. 39 sanitize tests + 26 api_client tests pass; 670 cargo test
green; CI 8/8 green on fb91f32.

**Thread resolved:** PRRT_kwDORs-xfc6BQwwb
**Reply posted:** 3223040033

**Fix commit:** fb91f32 ("chore(security): correct integration-test header comment for C1 escapes (PR #356 R17)")
**Threads:** 35/35 resolved (1 new R17 thread resolved; cumulative)

**Process note:** Thirteenth consecutive in-cycle state-manager dispatch per Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 → R12 → R13 → R14 → R15 → R16 → R17 all dispatched
state-manager in real time. 13 rounds of consistent Perplexity+Lesson-2 discipline across
17 review rounds.

**Convergence pattern:** R17 returned 1 finding — down from R16's 3. The R14 doc-fallout cluster
trajectory: R15:2 → R16:3 → R17:1. The cluster is tapering. Substantive defenses (memory-
amplification bounds, Write contract, DOM parse gate, C0+C1 escape coverage) fully converged
since R14 and untouched through R15-R17. **Phase 8 prediction: R18 likely returns 0 findings —
stop condition.**

---

## Round 18 (2026-05-12T01:05:00Z)

**Review ID:** 4268435007
**Comment ID:** 3223053065
**Inline comments:** 1
**Valid (doc accuracy — extract_error_message public-API doc incomplete after R14 C1 expansion)**

### Finding 1 — Public-API doc comment describes only ASCII escape branch (comment 3223053065)

The `extract_error_message` public-API doc comment (visible to all callers via rustdoc) described
only the ASCII control character escape branch: "escapes ASCII control chars ... as \xNN". This was
accurate before R14 but became incomplete when R14 expanded the escape set to include Unicode C1
controls U+0080..U+009F, which are rendered as `\u{NNNN}` (8-byte format) rather than `\xNN`
(4-byte format). In addition, the threat-model phrase "protects against CR/LF/ANSI injection"
omitted CSI (U+009B, the C1 control sequence introducer that ANSI terminals use to begin escape
sequences), making the threat model appear narrower than the implementation.

**Validation per DEC-018:** No external library or API behavior claims — purely internal
documentation accuracy. Perplexity skipped per Lesson 1 ("at least one external-claim aspect"
required). Skip is per-spec, not a rationalization.

**Fix:** Extended the public-API doc comment to accurately describe both escape branches:
- ASCII C0/DEL controls (U+0000..U+001F, U+007F): escaped as `\xNN` (4 bytes per character)
- Unicode C1 controls (U+0080..U+009F): escaped as `\u{NNNN}` (8 bytes per character)

Also expanded the threat-model phrase from "CR/LF/ANSI injection" to "CR/LF/ANSI/CSI injection"
to reflect that the C1 range explicitly covers the CSI codepoint (U+009B).

No new tests; comment-only change. 39 sanitize tests + 26 api_client tests pass; 670 cargo test
green; CI 8/8 green on 9acf01d.

**Thread resolved:** PRRT_kwDORs-xfc6BQ2o4
**Reply posted:** 3223074074

**Fix commit:** 9acf01d ("chore(security): correct extract_error_message public-API doc for C1 escapes (PR #356 R18)")
**Threads:** 36/36 resolved (1 new R18 thread resolved; cumulative)

**Process note:** Fourteenth consecutive in-cycle state-manager dispatch per Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 → R12 → R13 → R14 → R15 → R16 → R17 → R18 all dispatched
state-manager in real time. 14 rounds of consistent Perplexity+Lesson-2 discipline across
18 review rounds.

**Convergence pattern:** R18 held at 1 finding. The R14 doc-fallout cluster tapering is complete:
R15:2 → R16:3 → R17:1 → R18:1. All known doc-fallout sites from R14's C1 expansion have now been
corrected: public API doc (R18), strategy bullets in sanitize_for_stderr (R16-C1), C1 description
comment (R16-C2), integration test header comment (R17), R-number annotation cleanup (R15-C2).
Substantive defenses (memory-amplification bounds, Write contract, DOM parse gate, C0+C1 escape
coverage) fully converged since R14 and untouched through R15-R18. **Phase 8 prediction: R19
very likely returns 0 findings — stop condition.**

---

## Trajectory Analysis

**Pattern so far:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1 — all non-zero rounds addressed real findings.

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
- R7: 3 findings (terminology "strip" vs "escape" in docstring; stale round annotations in
  inline comments × 2). Perplexity CONFIRMED OWASP terminology for Finding 1; Findings 2+3
  no external claim (Perplexity skipped per Lesson 1 wording). No behavior change.
- R8: 2 findings (errors-map memory amplification — same class as R5; doc inaccuracy on
  MAX_SANITIZED_OUTPUT_LEN retroactive-trim description). Perplexity re-cited OWASP A06/AP11
  for Finding 1 (same class, Lesson 1 re-cite allowance). Finding 2 no external claim
  (Perplexity skipped per Lesson 1 wording).
- R9 (R9b — 4266950826 @ 19:55:57Z): 2 findings (key-amplification: server-controlled key k
  uncapped in format!("{k}: {v}"); non-string value v.to_string() materializes full serialization
  before cap). Both Perplexity CONFIRMED as legitimate memory-amplification gaps. Fix: cap_entry(k)
  wraps key; serialize_value_bounded new helper uses serde_json::to_writer + WriteZeroOnOverflow.
  5 new unit tests; 27 sanitize tests total; 658 cargo test green; CI 8/8 green on 85f0dd4.
  R9a + R9c re-raised prior concerns — 6 replies posted. 23/23 threads resolved.
- R10 (4268026428 @ 23:07:46Z): 1 finding (serialize_value_bounded emitted silently truncated JSON
  prefix without any marker — "looks valid but actually malformed" anti-pattern per tracing/slog/
  OpenTelemetry). Perplexity CONFIRMED UX-correctness gap. Fix: Bounded writer tracks overflowed
  flag; serialize_value_bounded reserves marker bytes upfront; appends " [...truncated]" on
  overflow; degenerate fallback pinned via test. 3 new tests + 1 updated; 30 sanitize tests total;
  661 cargo test green. 1 thread resolved (3222691664 → PRRT_kwDORs-xfc6BP1Oa); reply 3222725048.
  24/24 threads resolved. CI 8/8 green on f328a2f.
- R11 (4268102135 @ 23:27:03Z, comment 3222756019): 1 finding (INPUT DOM allocation attack surface
  — extract_error_message_raw called serde_json::from_str on the full body before any extraction,
  materializing 2-3x body size in memory; prior R5-R10 caps bounded OUTPUT only). Perplexity CONFIRMED
  byte-level size gate as superior zero-allocation fix. Fix: MAX_PARSE_BODY_LEN = 16 * 1024 constant;
  bodies >16 KiB skip JSON parse entirely and fall back to byte-bounded raw-body path. 3 new tests
  (skips-parse-for-huge-body, allows-normal-body, threshold-pinned); 33 sanitize tests total;
  664 cargo test green. 1 thread resolved (3222756019 → PRRT_kwDORs-xfc6BQA9s); reply 3222775607.
  25/25 threads resolved. CI 8/8 green on 2ecc18c.
- R12 (4268158285 @ 23:39:52Z, comments 3222800383 + 3222800411): 2 findings (Bounded::write
  violated std::io::Write contract — returned Err(WriteZero) after writing partial bytes,
  contradicting "no bytes written on error" mandate; non-UTF8 fallback marker used cap_entry
  lossy-string length max ~4096 bytes instead of true body.len(), silently under-reporting
  large/hostile bodies). Both Perplexity CONFIRMED. Fix: Bounded::write returns Err(WriteZero)
  only when remaining==0 at entry; partial writes: append prefix, set overflowed, return
  Ok(buf.len()); non-UTF8 path: bypass cap_entry, build custom marker
  `[...truncated, {body.len()} bytes total, non-UTF8 body]`. 3 new tests (partial-write marker,
  5MB body true size, small non-UTF8 skips marker); 36 sanitize tests total; 667 cargo test green.
  2 threads resolved (PRRT_kwDORs-xfc6BQI52 + PRRT_kwDORs-xfc6BQI6M); replies 3222826557 +
  3222826602. 27/27 threads resolved. CI 8/8 green on 6832967.
- R13 (4268206656 @ 23:52:40Z, comment 3222841940): 1 finding (doc comments mislabeled
  memory-amplification mitigation as "OWASP A06 / AP11" at 6 locations — OWASP A06:2021 is
  "Vulnerable and Outdated Components"; "AP11" is not a recognized standard category; correct
  labels are OWASP API4:2023 (Unrestricted Resource Consumption) / CWE-770 (Allocation of
  Resources Without Limits or Throttling)). Perplexity CONFIRMED. Fix: mechanical search-and-replace
  at 6 comment locations; no behavior change; no new tests. 1 thread resolved (PRRT_kwDORs-xfc6BQQan);
  reply 3222883003. 28/28 threads resolved. CI 8/8 green on bcc2db4.
- R14 (4268270089 @ 00:10:42Z, comment 3222898738): 1 finding (Unicode C1 controls U+0080..U+009F
  not escaped — `is_ascii_control()` only covers C0 + DEL; C1 includes CSI U+009B and NEL U+0085
  which legacy/embedded terminals can interpret as control sequences). Perplexity CONFIRMED
  defense-in-depth rationale valid. Fix: switch to `char::is_control()`; branch on `c.is_ascii()`
  for `\xNN` vs `\u{NNNN}` format; fast-path scan changed to char-level. 3 new tests (CSI escape,
  NEL escape, anti-regression non-control Unicode above ASCII); 39 sanitize tests total;
  670 cargo test green. 1 thread resolved (PRRT_kwDORs-xfc6BQamK); reply 3222921647.
  29/29 threads resolved. CI 8/8 green on d4a07c8.
- R15 (4268312988 @ 00:23:00Z, comments 3222937344 + 3222937368): 2 findings, both documentation
  cleanup. C1: fast-path comment in `sanitize_for_stderr` described byte-level scan after R14
  switched to char-level `chars().any(|c| c.is_control())` — rewritten to describe current
  char-level scan and explain why byte-level cannot be used (C1 2-byte UTF-8 sequences
  indistinguishable from valid multi-byte continuation bytes). C2: stale "(R10 finding)" annotation
  on `serialize_value_bounded` — systematic strip of ALL R-number annotations across the file
  (same class as R7 cleanup). No new tests; comment-only change; 39 sanitize tests + 670 cargo
  test unchanged and green. 2 threads resolved (PRRT_kwDORs-xfc6BQhi- + PRRT_kwDORs-xfc6BQhjV);
  replies 3222972524 + 3222972567. 31/31 threads resolved. CI 8/8 green on 7f0177d.
- R16 (4268365143 @ 00:38Z, comments 3222985472 + 3222985491 + 3222985507): 3 findings, all
  doc-accuracy consequences of R14 C1-control expansion. C1: strategy bullets described only
  ASCII escape path after R14 expanded to `char::is_control()` — rewrote to list both branches
  (`\xNN` 4 bytes for C0/DEL, `\u{NNNN}` 8 bytes for C1, both within 4 KiB cap). C2: comment
  said C1 controls are "rejected as invalid UTF-8 continuation bytes" — technically wrong
  (U+0080..U+009F are valid Unicode codepoints with valid 2-byte UTF-8 encodings; modern terminals
  in UTF-8 mode simply ignore C1 semantics, they do not reject the encoding). C3: integration test
  comment in `tests/api_client.rs` said "only ASCII control bytes are escaped" — updated to
  "only control characters (ASCII C0/DEL and Unicode C1) are escaped". No new tests; comment-only
  changes; 39 sanitize tests + 670 cargo test unchanged and green. 3 threads resolved
  (PRRT_kwDORs-xfc6BQqRd + PRRT_kwDORs-xfc6BQqRt + PRRT_kwDORs-xfc6BQqR6); replies 3223009560
  + 3223009636 + 3223009710. 34/34 threads resolved. CI 8/8 green on dc09501.
- R17 (4268400605 @ 00:54Z, comment 3223021119): 1 finding, doc-accuracy only. Integration-test
  header comment in `tests/api_client.rs` still described hostile control chars as rendering
  "as \xNN literals" — accurate before R14 but incomplete after R14 expanded the escape set to
  include Unicode C1 controls (U+0080..U+009F) which use `\u{NNNN}` format. Extended comment to
  mention both branches. No new tests; comment-only change; 39 sanitize + 26 api_client tests pass;
  670 cargo test green. 1 thread resolved (PRRT_kwDORs-xfc6BQwwb); reply 3223040033. 35/35 threads
  resolved. CI 8/8 green on fb91f32.
- R18 (4268435007 @ 01:05Z, comment 3223053065): 1 finding, doc-accuracy only. The
  `extract_error_message` public-API doc comment described only the ASCII escape branch ("\xNN")
  without mentioning the C1 Unicode escape branch ("\u{NNNN}" for U+0080..U+009F); threat-model
  phrase "CR/LF/ANSI" also omitted CSI. Extended public-API doc to cover both escape branches;
  expanded threat-model phrase to "CR/LF/ANSI/CSI". No new tests; comment-only change; 39 sanitize
  + 26 api_client tests pass; 670 cargo test green. 1 thread resolved (PRRT_kwDORs-xfc6BQ2o4);
  reply 3223074074. 36/36 threads resolved. CI 8/8 green on 9acf01d.

**Assessment:** Trajectory is now 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1. R14 doc-fallout cluster
tapering complete: R15:2 → R16:3 → R17:1 → R18:1. All known R14 doc-fallout sites corrected.
Substantive defenses (memory-amplification bounds, Write contract, DOM parse gate, C0+C1 escape
coverage) fully converged since R14 and untouched through R15-R18. 14 rounds of
Perplexity+Lesson-2 discipline through 18 review rounds. **Phase 8 prediction: R19 very likely
returns 0 findings — stop condition.**

---

## Round 15 (2026-05-12T00:23:00Z)

**Review ID:** 4268312988
**Comment IDs:** 3222937344, 3222937368
**Inline comments:** 2
**Both documentation quality (no security or behavioral gaps)**

### Finding C1 — Fast-path comment described byte-level scan after R14 switched to char-level

The fast-path short-circuit comment in `sanitize_for_stderr` still described
`bytes().any(|b| b.is_ascii_control())` semantics even though R14 had changed the
implementation to `chars().any(|c| c.is_control())`. A future reader could be confused
about why char-level iteration is used, or might "simplify" it back to byte-level without
understanding the constraint.

**Validation per DEC-018:** No external claims — the finding is entirely about internal
comment accuracy. Per Lesson 1 wording, Perplexity is not required when there is no
external-claim aspect. Skip is per-spec.

**Fix:** Rewrote the fast-path comment to describe the current char-level scan and explain
the constraint: C1 control code points (U+0080..U+009F) are encoded as 2-byte UTF-8
sequences (0xC2 0x80..0x9F) that byte-level scanning cannot distinguish from valid
2-byte UTF-8 continuation bytes. The comment now accurately represents the implementation
and prevents future "simplification" regression.

### Finding C2 — Stale R-number annotation in serialize_value_bounded marker comment

The `serialize_value_bounded` function contained a marker comment with a stale "(R10 finding)"
annotation. This is the same annotation-hygiene class as R7 (which cleaned R2/R3/R6 annotations
from production comments and test files), and the stale annotation makes the comment harder to
read without cycle history.

**Validation per DEC-018:** No external claims — same rationale as Finding C1. Perplexity skipped per Lesson 1.

**Fix:** Broader than the single flagged instance — systematic strip of ALL R-number annotations
across `src/api/client.rs`: "(R10 finding)", "(R11 finding)", "(R12 finding)", "(R9 finding)",
"(R9 defense — see comment block above)", "R10 pin: ", "R14 anti-regression: ",
"R10 degenerate case: ", "R12 pins — ", etc. Consistent with the R7 lesson that partial cleanup
invites re-flagging in subsequent rounds.

**Thread resolved:** PRRT_kwDORs-xfc6BQhi- (C1), PRRT_kwDORs-xfc6BQhjV (C2)
**Replies posted:** 3222972524 (C1), 3222972567 (C2)

**Fix commit:** 7f0177d ("chore(security): correct fast-path comment + strip stale R-number annotations (PR #356 R15)")
**Threads:** 31/31 resolved (2 new R15 threads resolved; cumulative)

**Test results at 7f0177d:**
- 39 sanitize unit tests total (no new tests — comment-only change)
- 670 cargo test total: 670 passed, 0 failed, 10 ignored
- cargo fmt --check + cargo clippy --all-targets -- -D warnings clean
- CI: 8/8 green

**Process note:** Eleventh consecutive in-cycle state-manager dispatch per Lesson 2.
R5 → R6 → R7 → R8 → R9 → R10 → R11 → R12 → R13 → R14 → R15 all dispatched state-manager
in real time. The discipline is fully embedded — 11 rounds in.

**Convergence signal:** R15 returned 2 findings but both are documentation cleanup (comment
accuracy and annotation hygiene). Trajectory is now 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2.
Substantive defenses (memory-amplification bounds, Write contract compliance, DOM parse gate,
C0+C1 escape coverage) have not been touched since R14. The quality of findings has clearly
degraded from memory-safety (R5–R11) to defense-in-depth (R12–R14) to pure documentation
cleanup (R13, R15) and label correctness (R13). R16 is the expected Phase 8 stop condition.

## Round 19 — PHASE 8 STOP CONDITION (2026-05-12T01:18:43Z)

**Review ID:** 4268474794
**Inline comments:** 0
**Stop condition:** PHASE 8 STOP CONDITION MET

### Stop Condition Verification

Review body (verbatim): "Copilot reviewed 2 out of 2 changed files in this pull request and
generated no new comments."

0 inline comments. 0 findings. The overview comment alone is not a reason to continue per
validated-feature-lifecycle Phase 8 stop condition: "a freshly-requested Copilot review posts
zero new inline comments. The overview comment alone (no file-level findings) is not a reason
to continue."

**Perplexity-validation:** Not applicable — no findings to validate.

**Fix commit:** none (stop-condition round; no changes needed)
**Threads:** 36/36 resolved (unchanged)
**Next action:** AWAITING HUMAN MERGE APPROVAL

**Process note:** Fifteenth consecutive in-cycle state-manager dispatch per Lesson 2.
The discipline held through 19 rounds across 15 state-manager dispatches without a single
skip after R4.

---

## Trajectory Analysis (final)

**Final trajectory:** 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1→0

- R1: 4 findings (doc accuracy, loop allocation, clean-path allocation, missing length cap).
  Perplexity confirmed CWE-117 + OWASP length-capping guidance.
- R2: 1 finding (marker overhead exceeds cap for slightly-oversized inputs).
- R3: 2 findings (1 critical: 4x expansion from pre-cap; 1 invariant: marker fallback un-truncated).
- R4: 2 findings (premature truncation; per-entry allocation). Perplexity confirmed Cow<str> idiom.
- R5: 3 findings (2 memory-amplification; 1 doc). Perplexity CONFIRMED OWASP API4:2023/CWE-770.
- R6: 2 findings (streaming join marker overflow; sanitize over-reporting retained bytes). Perplexity CONFIRMED.
- R7: 3 findings (terminology; stale round annotations ×2). Perplexity CONFIRMED OWASP terminology.
- R8: 2 findings (errors-map memory amplification; doc inaccuracy on retroactive-trim). Perplexity CONFIRMED.
- R9: 2 findings (key-amplification; non-string value materialization). Both Perplexity CONFIRMED.
- R10: 1 finding (truncated JSON with no marker — "looks valid but malformed" anti-pattern). Perplexity CONFIRMED.
- R11: 1 finding (INPUT DOM allocation attack surface). Perplexity CONFIRMED byte-level size gate.
- R12: 2 findings (Write contract violation; non-UTF8 fallback marker under-reporting). Both Perplexity CONFIRMED.
- R13: 1 finding (OWASP label inaccuracy at 6 locations). Perplexity CONFIRMED.
- R14: 1 finding (Unicode C1 controls not escaped). Perplexity CONFIRMED defense-in-depth. Major behavioral change.
- R15: 2 findings (fast-path comment stale post-R14; stale R-number annotations). Doc-only.
- R16: 3 findings (3 doc-accuracy consequences of R14 C1 expansion). Doc-only.
- R17: 1 finding (integration test header comment stale post-R14). Doc-only.
- R18: 1 finding (extract_error_message public-API doc missing C1 branch). Doc-only.
- R19: 0 findings. PHASE 8 STOP CONDITION MET.

**Assessment:** Full convergence. R14 introduced a major behavioral expansion (C1 controls);
R15-R18 cleaned up the resulting documentation fallout across 4 rounds. R19 confirms
all fallout resolved and no remaining gaps. Substantive defenses (memory-amplification bounds,
Write contract, DOM parse gate, C0+C1 escape coverage) fully converged at R14 and untouched
through R15-R19.

---

## CI Status

**Head SHA:** 9acf01d
**CI result:** 8/8 green

---

## CYCLE SUMMARY — PR #356 CONVERGED

| Field | Value |
|-------|-------|
| **State** | CONVERGED — awaiting human merge approval |
| **Rounds** | 19 (R0 initial PR + R1-R18 fix rounds + R19 stop) |
| **Fix commits** | 18 (51e2807 through 9acf01d) |
| **Final head** | 9acf01d |
| **Threads** | 36 created; 36/36 resolved |
| **CI on 9acf01d** | 8/8 green |
| **Tests** | 670 passed, 0 failed, 10 ignored (39 sanitize + 26 api_client) |
| **Mergeable** | CLEAN |
| **Final trajectory** | 4→1→2→2→3→2→3→2→2→1→1→2→1→1→2→3→1→1→0 |
| **Closes** | #334 on merge |

### Defense Profile (post-convergence)

| Defense Layer | Detail |
|---------------|--------|
| CWE-117 ASCII | C0/DEL (0x00-0x1F, 0x7F) → `\xNN` (4 bytes) via `is_ascii()` branch |
| CWE-117 Unicode | C1 (U+0080..U+009F) → `\u{NNNN}` (8 bytes) via `char::is_control()` |
| CWE-770 / OWASP API4:2023 UTF-8 conversion | ≤4 KiB cap before String::from_utf8_lossy |
| CWE-770 JSON parse gate | MAX_PARSE_BODY_LEN = 16 KiB; skip JSON parse for larger bodies |
| CWE-770 DOM worst case | ≤~48 KiB (16 KiB × 3 serde allocation factor) |
| CWE-770 per-entry cap | cap_entry() ≤1 KiB per key or value |
| CWE-770 streaming joins | ≤4 KiB including upfront marker reservation |
| CWE-770 final output | MAX_SANITIZED_OUTPUT_LEN = 4 KiB retroactive trim |
| std::io::Write | Bounded::write returns Ok(buf.len()) on partial writes; Err only when remaining==0 at entry |
| Truncation markers | Accurate original byte counts in all truncation paths including non-UTF8 |

### Process Metrics

| Metric | Value |
|--------|-------|
| State-manager dispatches (Lesson 2) | 15 consecutive (RECORD for this project) |
| Perplexity validations (Lesson 1 / DEC-018) | 12 |
| R14 doc-fallout cluster | R15:2 → R16:3 → R17:1 → R18:1 → R19:0 (fully resolved) |
| Rounds since last behavioral change (R14) | 5 (R15-R19 doc-only + stop) |
