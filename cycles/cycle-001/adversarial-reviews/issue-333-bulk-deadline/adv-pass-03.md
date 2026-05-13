---
document_type: adversarial-review
phase: F5
issue: 333
commit: 0439243
branch: fix/issue-333-bulk-deadline
pass: 03
producer: vsdd-factory:adversary
timestamp: 2026-05-12
status: applied
---

# Adversarial Review — Issue #333 Bulk Deadline (Pass 03)

Commit reviewed: `0439243` (pass-02 apply set + 2 unit-test gaps) on top of `dbb4e5f` / `618ca14`
Verdict: **NOT CLEAN-PASS** — 0 BLOCKING + 3 CONCERN + 5 NIT.

The CONCERN cluster centers on a single propagation gap: **pass-02 introduced `JrError::DeadlineExceeded` and exit code 124 at the INNER `send_inner` sites only; the OUTER-loop site, the precedence comment, the docs, and the integration assertions were not updated to reflect the new contract.**

## Important Findings (CONCERN)

### C-1 (HIGH) — Outer-loop deadline-exceeded returns exit code 1, INNER returns 124

`bulk.rs:397-403` (top-of-loop deadline check) uses `anyhow::anyhow!` → exit 1. The two inner sites (entry-point + 429 clamp) return `JrError::DeadlineExceeded` → exit 124. Same root cause (caller deadline expired), different exit codes — the variant taxonomy the pass-02 C-2 work introduced is undermined.

**Fix applied:** Convert `bulk.rs:397-403` to `JrError::DeadlineExceeded { remaining_ms: 0, message: "[deadline:bulk-outer] Bulk task {task_id} did not complete within {timeout}s. Check Jira for task status." }`. Site-tag prefix added per research-validation pass-04 Q1 (prefer site tags in message string over schema-extending the variant — the variant is six lines old with zero match-arm consumers).

### C-2 (MEDIUM) — BC-X.4.009 cap-vs-deadline precedence comment is stale; exit codes now diverge

`client.rs:559-563` precedence comment said "user-impact is equivalent" between cap-abort and deadline-clamp. Pre-pass-02 both returned `ApiError(429)` → exit 1 (equivalent). Post-pass-02 cap-abort → exit 1, deadline-clamp → exit 124. Comment is stale; the user-impact is NO LONGER equivalent.

**Fix applied:** REORDERED the checks (option a from adversary's recommendation; option b would have been to just update the comment). Deadline-clamp now fires BEFORE the cap-vs-Retry-After comparison. A 429 with `Retry-After > 60s` AND expired deadline now surfaces as `DeadlineExceeded` (exit 124), not as the cap message (exit 1). Industry precedent unanimous per research-validation pass-04 Q2: aws-smithy-rs, tokio::time::timeout, kubectl client-go, RFC 9110 §10.2.3 all treat client-side deadline as a hard contract that supersedes server-advisory Retry-After.

### C-3 (MEDIUM) — Integration tests do NOT pin exit code 124 — pass-02 C-2 contract is unverified end-to-end

Both integration tests asserted only `!output.status.success()` — any non-zero exit passes. A regression that reverted `DeadlineExceeded` back to `ApiError(429)` would NOT be caught (1 vs 124, both non-zero).

**Fix applied:** Added `assert_eq!(output.status.code(), Some(124), ...)` to both integration tests. Tightened the stderr assertion from OR-chain ("deadline" OR task_id OR "timeout") to required-substring ("deadline" only — now produced by the `DeadlineExceeded` Display "Deadline exceeded: ...").

## Observations (NIT) — ALL APPLIED

| ID | Fix |
|---|---|
| N-1 | `ClampResult::Expired` docstring updated from `JrError::ApiError` to `JrError::DeadlineExceeded`; pre-F5-pass-02 history noted with research-validation-pass-03.md Q2 reference |
| N-2 | `story.md` AC-005 annotated-as-superseded with AC-005-v2 block; original text preserved; reversal rationale + 4-document reference trail added |
| N-3 | `CLAUDE.md:72` tree comment updated from `(0/1/2/64/78/130)` to `(0/1/2/64/78/124/130)` |
| N-4 | `docs/superpowers/specs/2026-03-26-jrerror-exit-codes-design.md` exit-code mapping table updated — added `JrError::DeadlineExceeded` row (124) AND back-filled missing `JrError::InsufficientScope` row (2) |
| N-5 | `send_bounded` rustdoc updated — entry-point condition now correctly states "remaining budget < 1ms (deadline already passed OR within the tokio timer-wheel 1ms floor)"; site tags `[deadline:send-entry]`, `[deadline:429-retry]`, `[deadline:bulk-outer]` enumerated |

## Triage Summary

All 8 findings APPLIED (3 CONCERN + 5 NIT). 0 deferred. The cluster was tightly coupled around a single root cause (`DeadlineExceeded` not propagated everywhere) so the fix is one cohesive commit.

## Novelty Assessment

All 8 findings are NEW post-`0439243`. None restate pass-01 or pass-02. The closest restatement-candidate (C-2 builds on pass-01 NIT-2) was about a different aspect — pass-01 NIT-2 was about message-level precedence with same variant; pass-03 C-2 is about variant + exit-code divergence post-pass-02.

## Convergence Trajectory

- Pass 01: 14 findings (0 BLOCKING + 7 CONCERN + 7 NIT)
- Pass 02: 7 findings (1 BLOCKING + 2 CONCERN + 4 NIT)
- Pass 03 (this): 8 findings (0 BLOCKING + 3 CONCERN + 5 NIT)

Trend: BLOCKING-count converged (1 → 0). CONCERN-count flat-to-decreasing (7 → 2 → 3). NIT-count flat (7 → 4 → 5). The CONCERN bump at pass-03 was entirely the propagation cluster — fixing it should drop pass-04 to CLEAN if no new shape-of-problem is found.

Counter remains at 0 CLEAN-PASSes. Need 3 consecutive CLEAN passes to converge.

## [process-gap] Codification

Adversary flagged a `[process-gap]` candidate at pass-03: "When introducing a new `JrError` variant with a new exit code, propagate to:
1. The CLAUDE.md exit-code summary line (`error.rs # JrError enum with exit codes (...)`)
2. The exit-code design spec table
3. Sibling deadline-exceeded sites in the same module pipeline
4. Integration-test exit-code assertions (not just unit tests on `exit_code()`)"

This is the second time a doc-fallout-style propagation gap surfaces in this release (first was JR_BULK_* env vars from #335/#357). n=2/3 per S-7.02 codification threshold. The codified rule for the JR_* case lives in CLAUDE.md "AI Agent Notes". A parallel rule for the JrError-variant case should be added in the same section. **DEFERRED to a follow-up factory-artifacts commit (not part of #333's PR).**
