# Phase F6 — Targeted Hardening Summary

**Feature:** S-388 / issue #388 — cross-hierarchy `edit --type` 400 enrichment + `--no-parent` fake-endpoint hint fix
**Delta commit:** `e0ea24b` (merged to `develop`; `develop` HEAD = `e0ea24b`)
**Pre-merge parent:** `a66d664`
**Date:** 2026-05-20
**Mode:** VSDD Feature Mode — Phase F6 (Targeted Hardening)
**Verdict:** F6 QUALITY GATE PASSED

## Delta Under Hardening

| Artifact | Change |
|----------|--------|
| `src/cli/issue/create.rs` | +181 LOC — `is_cross_hierarchy_type_error` pure classifier, `Classification` enum, `handle_edit` error-path dispatch block, 3 hint string consts, inline proptest |
| `src/types/jira/issue.rs` | +2 LOC — additive `IssueType.subtask: Option<bool>` field (`#[serde(default)]`) |
| `tests/issue_edit_type_errors.rs` | +933 LOC — new/strengthened tests + inline proptest companion |
| `tests/issue_edit_no_parent.rs` | +92 LOC — tightened assertions, `&&`→`||` kill-test |

`feature_type: backend`, `regression_risk: medium`, no UI surface, no external-service
interaction change → Phase F6 Steps 7b (DTU adversarial) and 7d (accessibility
re-check) are not applicable to this delta.

## Step Results

### Step 2 — Kani Formal Verification: JUSTIFIED SKIP

Project has no Kani toolchain. The only new pure function has a finite 9-state
domain (`Option<bool> × Option<bool>`); the `err: &str` arg is provably unused
for the verdict. The inline proptest exhaustively covers all 9 states + the P4
err-independence property. No overflow/bounds/unsafe surface. Detail: `kani-results.md`.

### Step 3 — Fuzz Testing: JUSTIFIED SKIP

No new untrusted-input parser. Classifier `&str` arg unused for control flow;
additive `Option<bool>` serde field via existing paths. No cargo-fuzz setup.
Inline proptest `.*` strategy already exercises arbitrary string input. Detail: `fuzz-results.md`.

### Step 4 — Mutation Testing: PASS — 100% kill rate

cargo-mutants 27.0.0, `--in-diff` scoped to S-388 delta line ranges, 8 mutants.

| Metric | Value |
|--------|-------|
| Caught | 7 |
| Missed | 0 |
| Unviable | 1 (`Default::default()` on `Classification` — no `Default` impl) |
| Timeout | 0 |
| Kill rate | 7/7 viable = 100% (target >= 90%) |

The `&&`->`||` mutant at `create.rs:898:22` (flagged by PR #397 CI) is CAUGHT.
Zero surviving mutants; no routing required. Detail: `mutation-results.md`.

### Step 5 — Security Scanning: PASS — no CRITICAL/HIGH findings

| Scan | Result |
|------|--------|
| `cargo deny check` (340 crates) | advisories ok, bans ok, licenses ok, sources ok |
| `cargo audit` (1096 advisories) | clean — no vulnerabilities, exit 0 |
| `semgrep` | not installed; justified — no injection/secrets/unsafe-deserialization surface |

CRITICAL 0 / HIGH 0 / MEDIUM 0 / LOW 0 / INFO 2 (pre-existing stale deny.toml entries).
No security-reviewer escalation. Detail: `security-scan-results.md`.

### Step 6 — Full Regression Suite: PASS

`cargo test --all-features`: 1398 passed, 0 failed, 18 ignored (gated integration tests).
All 11 new/strengthened S-388 tests pass. The known flaky
`multi_cloudid_disambiguation.rs` did not fail in the full-tree run; confirmed
passing 12/12 in isolation. Not an S-388 regression.

## F6 Quality Gate Checklist

- [x] Kani proofs — JUSTIFIED SKIP
- [x] Fuzz testing — JUSTIFIED SKIP
- [x] Mutation kill rate >= 90% on delta — 100% (7/7 viable)
- [x] No CRITICAL/HIGH security findings — 0 found
- [x] Full regression suite passes — 1398/1398
- [x] DTU adversarial — N/A (no external-service interaction change)
- [x] Accessibility re-check — N/A (backend feature, no UI)
- [x] No fix PRs needed — zero surviving mutants, zero security findings
- [x] Hardening summary written with all metrics

## Verdict

PHASE F6 COMPLETE — QUALITY GATE PASSED. The S-388 delta is cleared for Phase F7
(Delta Convergence).
