---
document_type: adversarial-review
phase: F5
issue: 333
commit: 9c985e6
branch: fix/issue-333-bulk-deadline
pass: 05
producer: vsdd-factory:adversary
timestamp: 2026-05-12
status: CLEAN-PASS-2-of-3
---

# Adversarial Review — Issue #333 Bulk Deadline (Pass 05)

Commit reviewed: `9c985e6` (1-line cosmetic doc fix on top of `d5334fa` / `0439243` / `dbb4e5f` / `618ca14`).

**Verdict: CLEAN-PASS — counter 1 → 2.**

0 BLOCKING + 0 CONCERN + 2 NIT (cosmetic, novel) + 1 carry-forward observation.

## Fresh-context concurrency/panic/cleanup analysis (PASS)

- Concurrency safety on shared `JiraClient`: env-var snapshot before `.await`, single-flight refresh, per-task local state. No race.
- `tokio::time::sleep` panic surface: no documented panic path; `?`-chain unwind clean.
- Drop ordering on early-Err: no held resources; reqwest pool managed externally.

## Observations (NIT)

### N-1 (LOW, NEW) — JSON envelope drops `remaining_ms` field

`JrError::DeadlineExceeded { remaining_ms, message }` — under `--output json` only the `message` substring is exposed; `remaining_ms` is not a structured JSON field. JSON consumers can't access it programmatically.

The variant docstring already documents `remaining_ms` as "always 0ms at the threshold by construction; included for parity with the in-loop error message format." So this is half-documented and consistent with the pre-existing NFR-O-P decision (uniform `{"error", "code"}` JSON envelope). No action; just confirming the design holds under JSON inspection.

### N-2 (LOW, NEW) — Outer-loop clamp lacks `tracing::debug!`

`src/api/jira/bulk.rs:530-533` (the pass-02 B-1 outer-loop clamp) emits no tracing event when the backoff is clamped to remaining budget. Inner clamp at `src/api/client.rs:629-637` emits a structured `tracing::debug!` with `delay_secs`, `clamped_sleep_ms`, `attempt`, `deadline_aware`.

Observability asymmetry. `bulk.rs` has zero `tracing` usage today; adding one event would introduce a new pattern. Defer — no production user has asked for this.

## Carry-forward observation

### O-1 — `Instant::now() + timeout` panic surface (pass-01 NIT-5 still unfixed)

`bulk.rs:385` panics in debug builds if `JR_BULK_AWAIT_TIMEOUT_SECS=u64::MAX`. Pass-01 NIT-5 deferred as acceptable test-seam ergonomics; the deferral still holds. Not a new finding.

## Convergence Trajectory

| Pass | BLOCKING | CONCERN | NIT | Verdict | Counter |
|---|---|---|---|---|---|
| 01 | 0 | 7 | 7 | not clean | 0 |
| 02 | 1 | 2 | 4 | not clean | 0 |
| 03 | 0 | 3 | 5 | not clean | 0 |
| 04 | 0 | 0 | 2 | CLEAN-PASS | 1/3 |
| **05** | **0** | **0** | **2 (cosmetic, novel)** | **CLEAN-PASS** | **2/3** |

Both NITs are cosmetic/subjective and qualify under the CLEAN-PASS rule. Novelty: LOW — refinements only.

## Novelty Assessment

N-1 (JSON envelope `remaining_ms`): NEW — no prior pass walked the JSON rendering path end-to-end.

N-2 (outer-loop tracing asymmetry): NEW — no prior pass audited observability symmetry between the inner and outer clamps.

## Verdict

**CLEAN-PASS — counter 1 → 2.** Need 1 more consecutive CLEAN pass from a fresh-context adversary to converge.
