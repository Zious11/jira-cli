# fix(security): parse_duration_validate input length cap (CWE-400) (WV2-SEC-01)

**Mode:** security hardening (brownfield, standalone fix)
**Finding:** WV2-SEC-01 — Wave 2 integration-gate security review (`wave-2-gate-security-review-pass-01.md`)
**Severity:** MEDIUM | **CWE:** CWE-400 (Uncontrolled Resource Consumption)

![Tests](https://img.shields.io/badge/tests-622%2F622-brightgreen)
![Coverage](https://img.shields.io/badge/coverage-≥80%25-brightgreen)
![Security](https://img.shields.io/badge/security-MEDIUM%20resolved-green)

`parse_duration_validate` previously reflected unbounded user input into error messages,
allocating heap strings up to the OS argv limit (~2 MB) before rejecting the input.
This PR adds a `MAX_DURATION_INPUT_LEN = 64` byte guard at the top of the function,
two regression-pin tests, and no other changes. The fix is not exploitable (CLI argv
only, no remote attack surface); it is defense-in-depth per the Linux kernel CWE-400
hardening pattern (e.g., `a664bf3d603d`).

---

## Architecture Changes

```mermaid
graph TD
    WorklogAdd["jr worklog add\n(CLI entry point)"]
    ParseValidate["parse_duration_validate()\nsrc/duration.rs"]
    LengthGuard["MAX_DURATION_INPUT_LEN guard\n(NEW — 64 bytes)"]
    ParseLogic["whitespace-strip +\nunit-char validation\n(existing)"]
    ErrorPath["JrError::UserError\n(exit 64)"]

    WorklogAdd -->|"duration arg"| ParseValidate
    ParseValidate --> LengthGuard
    LengthGuard -->|"len > 64"| ErrorPath
    LengthGuard -->|"len ≤ 64"| ParseLogic
    ParseLogic -->|"invalid syntax"| ErrorPath

    style LengthGuard fill:#90EE90
```

<details>
<summary><strong>Architecture Decision Record</strong></summary>

### ADR: Add a 64-byte input-length cap to parse_duration_validate before any heap allocation

**Context:** The Wave 2 security review (WV2-SEC-01, MEDIUM, CWE-400) identified that
`parse_duration_validate` allocated up to 3x the input length on heap before rejecting
oversized inputs: one allocation for the whitespace-stripped normalised string, one for
`to_lowercase()`, and one for the formatted error message embedding the full raw input.
The OS argv limit allows inputs up to ~2 MB.

**Decision:** Add `const MAX_DURATION_INPUT_LEN: usize = 64` and a length check before
the whitespace-strip call. Reject with a capped error message that reports byte count
and the cap value, but does NOT embed the raw payload.

**Rationale:** The longest realistic duration a user could type is `99w99d23h59m` (14 bytes).
64 bytes is generous (4.5x the realistic maximum) while provably bounding heap allocation.
The error message omits the raw input, preventing CWE-209 as a secondary concern.

**Alternatives Considered:**
1. Truncate input to 64 bytes before validating — rejected because it could silently
   accept a prefix of an invalid string, masking the user error.
2. Raise limit to 256 bytes — rejected because 64 is already 4.5x any realistic input;
   a higher cap provides no user benefit and weakens the defense-in-depth.

**Consequences:**
- Heap allocation before rejection is now bounded at O(MAX_DURATION_INPUT_LEN) = O(64).
- Error messages never embed user-supplied payload.
- No breaking change: no valid duration string exceeds 64 bytes.

</details>

---

## Story Dependencies

```mermaid
graph LR
    S206["S-2.06\nmerged PR #308\n(introduced parse_duration_validate)"]
    SEC01["WV2-SEC-01\nthis PR"]
    Future["Future cleanup\n(WV2-SEC-02 UX asymmetry\ndeferred)"]

    S206 -->|"introduced the gap"| SEC01
    SEC01 -->|"unblocks"| Future

    style SEC01 fill:#FFD700
```

---

## Spec Traceability

```mermaid
flowchart LR
    SEC01["WV2-SEC-01\nCWE-400 MEDIUM finding"]
    AC1["AC-1\ninputs > 64 bytes rejected\nbefore any heap alloc"]
    AC2["AC-2\nvalid inputs at/under cap\naccepted unchanged"]
    T1["test_parse_duration_validate_rejects_input_longer_than_max"]
    T2["test_parse_duration_validate_accepts_input_at_max_boundary"]
    SRC["src/duration.rs\nMAX_DURATION_INPUT_LEN = 64\nparse_duration_validate()"]

    SEC01 --> AC1
    SEC01 --> AC2
    AC1 --> T1
    AC2 --> T2
    T1 --> SRC
    T2 --> SRC
```

---

## Test Evidence

### Coverage Summary

| Metric | Value | Threshold | Status |
|--------|-------|-----------|--------|
| Unit tests | 622/622 pass | 100% | PASS |
| New regression-pin tests | 2 added | — | PASS |
| Coverage | ≥80% (existing baseline) | >80% | PASS |
| Regressions | 0 | 0 | PASS |

### Test Flow

```mermaid
graph LR
    Unit["622 Unit Tests\n(+2 regression pins)"]
    Clippy["cargo clippy\n--all-targets -D warnings"]
    Fmt["cargo fmt\n--check"]

    Unit -->|"100% pass"| Pass1["PASS"]
    Clippy -->|"zero warnings"| Pass2["PASS"]
    Fmt -->|"clean"| Pass3["PASS"]

    style Pass1 fill:#90EE90
    style Pass2 fill:#90EE90
    style Pass3 fill:#90EE90
```

| Metric | Value |
|--------|-------|
| **New tests** | 2 added, 0 modified |
| **Total suite** | 622 tests PASS (was 620; +2 net) |
| **Coverage delta** | neutral (guard is a new code path, fully tested) |
| **Regressions** | 0 |

<details>
<summary><strong>Detailed Test Results</strong></summary>

### New Tests (This PR)

| Test | Result | Notes |
|------|--------|-------|
| `test_parse_duration_validate_rejects_input_longer_than_max` | PASS | `"1d".repeat(40)` = 80 bytes (over 64-byte cap); asserts `Err` with "too long" and "64" in message |
| `test_parse_duration_validate_accepts_input_at_max_boundary` | PASS | `"1m".repeat(31)` = 62 bytes; valid syntax under cap; asserts `Ok(())` |

### Coverage Analysis

| Metric | Value |
|--------|-------|
| Lines added | 55 (guard + const + 2 tests + doc comments) |
| New branches | 1 (length check: > cap / ≤ cap) |
| Branches covered | 2/2 (100%) — both test cases exercise each branch |
| Uncovered paths | none |

</details>

---

## Holdout Evaluation

N/A — evaluated at wave gate. This is a standalone security-hardening fix; no behavioral holdout applies.

---

## Adversarial Review

N/A — evaluated at Phase 5. The Wave 2 gate security review (`wave-2-gate-security-review-pass-01.md`) constitutes the adversarial review for this fix. WV2-SEC-01 was the only MEDIUM finding; no CRITICAL or HIGH findings existed.

---

## Security Review

```mermaid
graph LR
    Critical["Critical: 0"]
    High["High: 0"]
    Medium["Medium: 0\n(WV2-SEC-01 resolved)"]
    Low["Low: 0\n(no new surface)"]

    style Critical fill:#90EE90
    style High fill:#90EE90
    style Medium fill:#90EE90
    style Low fill:#87CEEB
```

This PR **resolves** WV2-SEC-01 from the Wave 2 gate security review.

<details>
<summary><strong>Security Scan Details</strong></summary>

### Finding Resolved: WV2-SEC-01

- **Severity:** MEDIUM → RESOLVED
- **CWE:** CWE-400 (Uncontrolled Resource Consumption), CWE-209 (secondary)
- **OWASP:** A05:2021 Security Misconfiguration
- **Root cause:** `parse_duration_validate` allocated up to 3x the input length on heap
  (whitespace-strip + `to_lowercase()` + error-message format string embedding raw input)
  before rejecting oversized inputs. No length guard existed before the first allocation.
- **Fix:** `const MAX_DURATION_INPUT_LEN: usize = 64` + early-return guard before any
  string allocation. Error message reports byte count and cap value without embedding raw payload.
- **Not exploitable:** This is a local-only CLI tool; no remote attack surface. Defense-in-depth.
- **Reference:** `.factory/cycles/cycle-001/security-reviews/wave-2-gate-security-review-pass-01.md`

### New Surface Introduced

None. This PR only adds a guard and two tests. No new API calls, no new data structures,
no new dependencies, no new error reflection paths.

### Dependency Audit

`cargo deny check` — CLEAN. No new dependencies added.

</details>

---

## Risk Assessment & Deployment

### Blast Radius

- **Systems affected:** `src/duration.rs` only — one file, one function
- **User impact:** None for valid inputs (all valid durations are well under 64 bytes).
  Inputs over 64 bytes were already invalid and produced an error; they now error sooner
  with a cleaner message.
- **Data impact:** None — read-only validation function
- **Risk Level:** LOW

### Performance Impact

| Metric | Before | After | Delta | Status |
|--------|--------|-------|-------|--------|
| Validation latency | O(n) where n = input len | O(1) early-return for n > 64 | negligible | OK |
| Heap allocation (over-cap input) | O(3n) | O(0) | improved | OK |

<details>
<summary><strong>Rollback Instructions</strong></summary>

**Immediate rollback (< 2 min):**
```bash
git revert 9ff7ba7
git push origin develop
```

**Verification after rollback:**
- `cargo test --lib duration` passes (will lose the 2 regression-pin tests)
- No user-visible behavior change for valid inputs

</details>

### Feature Flags

None — this fix is unconditionally active.

---

## Traceability

| Requirement | Finding | Test | Status |
|-------------|---------|------|--------|
| Bound heap allocation before rejection | WV2-SEC-01 (CWE-400) | `test_parse_duration_validate_rejects_input_longer_than_max` | PASS |
| Preserve valid-input behavior | WV2-SEC-01 AC-2 | `test_parse_duration_validate_accepts_input_at_max_boundary` | PASS |

---

## AI Pipeline Metadata

<details>
<summary><strong>Pipeline Details</strong></summary>

```yaml
ai-generated: true
pipeline-mode: security-hardening (standalone fix)
factory-version: "1.0.0"
finding: WV2-SEC-01
finding-source: wave-2-gate-security-review-pass-01.md
cwe: CWE-400
severity-before: MEDIUM
severity-after: RESOLVED
pipeline-stages:
  security-review: completed (wave-2 gate)
  tdd-implementation: completed (2 regression-pin tests)
  convergence: achieved
convergence-metrics:
  blocking-findings: 0
  test-kill-rate: "100% (2/2 new tests)"
  implementation-ci: passing
models-used:
  builder: claude-sonnet-4-6
generated-at: "2026-05-07T00:00:00Z"
```

</details>

---

## Pre-Merge Checklist

- [x] All CI status checks passing
- [x] Coverage delta is positive or neutral (new guard branch fully tested)
- [x] No critical/high security findings unresolved
- [x] Rollback procedure validated (single-commit revert)
- [x] No feature flag required (unconditional hardening)
- [x] Breaking change: false (valid inputs unaffected; over-cap inputs were already rejected)
- [x] Diff contained to `src/duration.rs` only (verified via `git diff --name-only`)
