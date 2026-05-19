---
document_type: delta-analysis-report
feature_name: "Platform-path inverse warning symmetry: --field/--on-behalf-of silent-drop"
issue: 383
created: 2026-05-19
spec_version_at_analysis: "N/A"
status: approved
intent: "enhancement"
feature_type: "backend"
scope_class: "standard"
severity: "N/A"
phase: F1
inputs:
  - phase-f1-delta-analysis/issue-383/impact-boundary.md (architect, F1-Step-3)
  - phase-f1-delta-analysis/issue-383/affected-artifacts.md (business-analyst, F1-Step-4)
input-hash: "[live-state — 2026-05-19]"
traces_to: "BC-3.8.012, BC-3.8.013"
project: jira-cli
issue: 383
intent: enhancement
feature_type: backend
scope_class: standard
severity: "N/A"
---

# Delta Analysis Report: Platform-Path Inverse Warning Symmetry (#383)

## Summary

Issue #383 closes the inverse-direction gap identified as O-01 in #381's adversary audit:
`--field` and `--on-behalf-of` are silently dropped when `jr issue create` runs the
platform path (no `--request-type`), yet no warning is emitted. This mirrors the
BC-3.8.011 pattern (forward direction: platform-only flags silently dropped on the JSM
path) with two new BCs (BC-3.8.012, BC-3.8.013) and two `eprintln!` checks inserted
at the top of the platform branch in `handle_create`. All changes are additive; no
existing behavior is modified. The BC next-available sequence BC-3.8.012/013 is
confirmed by `CANONICAL-COUNTS.md` (last updated 2026-05-18, BC-3.8 ends at 011).

---

## Impact Boundary

Source: `impact-boundary.md` (architect, F1-Step-3). Summarized below; see source for
line-range details and code excerpts.

| Component | Classification | Notes |
|-----------|---------------|-------|
| `src/cli/issue/create.rs` (after line 118) | MODIFIED | Insert 2 `if` blocks with `eprintln!` before `let project_key = ...` |
| `.factory/specs/prd/bc-3-issue-write.md` (BC-3.8.012) | NEW | `--field` without `--request-type` must emit stderr warning |
| `.factory/specs/prd/bc-3-issue-write.md` (BC-3.8.013) | NEW | `--on-behalf-of` without `--request-type` must emit stderr warning |
| `.factory/specs/prd/bc-3-issue-write.md` footer | MODIFIED | Increment definitional count 59 → 61; update range "001..011" → "001..013" |
| `tests/issue_create_jsm.rs` | MODIFIED | Add 2 integration tests (one per flag) |
| `src/cli/mod.rs` | DEPENDENT | Confirms no `requires` attribute; no edit needed |
| Existing BC-3.8.011 tests (lines 1587–1865) | DEPENDENT | Regression baseline; must remain passing |
| `src/api/jsm/requests.rs` | DEPENDENT | `on_behalf_of` wiring unchanged |

**Architecture verdict:** No structural change. Two `eprintln!` calls at one insertion
point. No new modules, types, API calls, or trait changes.

---

## Affected Artifacts

Source: `affected-artifacts.md` (business-analyst, F1-Step-4). Summarized below; see
source for BC body proposals and test template references.

| Artifact | Status | Change Description |
|----------|--------|--------------------|
| BC-3.8.011 | UNCHANGED | Template/precedent for the 2 new BCs |
| BC-3.8.010 | UNCHANGED | Adjacent; `--type` warning on JSM path |
| BC-3.8.001 | UNCHANGED | JSM dispatch routing |
| BC-3.3.001 | UNCHANGED | Platform path (no `--request-type`) |
| **BC-3.8.012** | **NEW** | `--field` without `--request-type` → emit 1 stderr warning |
| **BC-3.8.013** | **NEW** | `--on-behalf-of` without `--request-type` → emit 1 stderr warning |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | MODIFIED | BC-3 definitional count 59 → 61 |

**Proposed verbatim warning strings (to be locked in BC bodies):**

- BC-3.8.012: `"warning: --field is ignored without --request-type; use --request-type to submit a JSM request with custom fields"`
- BC-3.8.013: `"warning: --on-behalf-of is ignored without --request-type; use --request-type to submit a JSM request on behalf of another user"`

Both mirror the BC-3.8.011 pattern: one warning line to stderr, exit 0 on success,
JSON output shape unchanged, warning fires regardless of `--no-input` or `--output json`.

**New integration tests (CLAUDE.md naming convention):**

- `test_platform_create_field_flag_emits_warning_without_request_type`
- `test_platform_create_on_behalf_of_flag_emits_warning_without_request_type`

Both tests: mount `POST /rest/api/3/issue` (platform endpoint), pass the flag without
`--request-type`, assert `status.success()`, assert `stderr.contains(verbatim_warning)`.

**No new VPs required.** Existing VP coverage (stderr-not-stdout, exit-code unchanged,
JSON shape unchanged) is satisfied by the integration test assertion pattern already
established for BC-3.8.010/011.

---

## Files Changed / Files Unchanged

### Files to be modified or created

| File | Change |
|------|--------|
| `src/cli/issue/create.rs` | Insert 2 `eprintln!` blocks after the JSM dispatch `return` (line ~119) |
| `tests/issue_create_jsm.rs` | Append 2 new integration tests |
| `.factory/specs/prd/bc-3-issue-write.md` | Append BC-3.8.012 + BC-3.8.013 bodies; update footer count 59 → 61 |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | BC-3 definitional count 59 → 61 |

### Files unchanged (regression baseline)

| File | Why Unchanged |
|------|--------------|
| `tests/issue_create_jsm.rs` lines 1587–1865 | All 5 BC-3.8.011 per-flag warning tests must remain green |
| `tests/issue_create_jsm.rs` lines 1381–1433 | Platform 401 path must not mention `write:servicedesk-request` |
| `tests/issue_create_jsm.rs` lines 1242–1308 | BC-3.8.010 `--type` warning on JSM path unchanged |
| `tests/issue_create_jsm.rs` lines 1441–1522 | JSON output shape unchanged |
| `tests/issue_create_json.rs` | Platform path tests must remain green |
| `src/api/jsm/requests.rs` | JSM builder logic unchanged |
| `src/cli/mod.rs` | No `requires` attribute needed; no edit |
| `src/cli/issue/create.rs` lines 120–278 | Platform branch field-building logic unchanged after insertion |

---

## Risk Assessment

| Risk Type | Level | Rationale |
|-----------|-------|-----------|
| Regression | LOW | Insertion is after the JSM dispatch `return`, so JSM-path tests cannot be affected. Platform-path tests (`issue_create_json.rs`, `test_jsm_create_without_request_type_uses_platform_path`) are the primary regression guards. The 2 new warnings are emitted via `eprintln!` only when flags are explicitly set — no change to the zero-flag platform path. |
| Architecture | NONE | Pure addition of 2 `if` guards checking already-bound locals. No module, type, or dependency changes. |
| Security | NONE | No auth flow, no secret handling, no trust boundary change. Warning text is a user-facing hint, not a token or credential. |
| Performance | NONE | Check is on `Vec::is_empty()` and `Option::is_some()` — effectively free. |

---

## Classification

| Attribute | Value |
|-----------|-------|
| Intent | enhancement |
| Feature type | backend |
| Scope class | STANDARD |
| Severity | N/A (not a bug-fix; silent-drop was by design, deferred from #381 as O-01) |
| BCs modified | 0 |
| BCs new | 2 (BC-3.8.012, BC-3.8.013) |
| Tests new | 2 integration tests in `tests/issue_create_jsm.rs` |
| Scope rationale | Per F1 Step 4c, "trivial" requires zero new BCs and zero new test files. 2 new BCs disqualifies trivial by policy. Implementation is mechanically minimal (< 10 lines in `create.rs`; copy-paste mirror of BC-3.8.011 tests). One-story delivery is appropriate; no multi-wave decomposition needed. |

---

## Recommended Pipeline

```
F1 (this report) → pre-F2 validation (L-288-pr4-06) → F2 → F3 → F4 → F7
```

**Rationale for full pipeline (NOT quick-dev):** Two new BCs (BC-3.8.012, BC-3.8.013)
disqualify this from the quick-dev/trivial route per scope-class policy. The scope is
STANDARD (mechanically minimal, one-story delivery, but BC-additive).

**Pre-F2 validation gate (L-288-pr4-06):** Verify that proposed warning strings are
syntactically consistent with BC-3.8.011 pattern before codifying in BC bodies. No
external API claims to validate; the check is purely internal consistency.

**F2:** Add BC-3.8.012 and BC-3.8.013 to `bc-3-issue-write.md`. Update CANONICAL-COUNTS.
PATCH spec version bump. No new VP.

**F3:** ONE story (single delivery, ~1 story point). Proposed ID: `S-383-inverse-warning`.

**F4:** Per-story delivery (worktree → failing tests → TDD → adversary 3/3 CLEAN → push
→ pr-manager 9-step). Adversary must confirm: (a) both warning strings are verbatim-pinned
in tests; (b) JSM-path tests (BC-3.8.011 suite) remain untouched; (c) platform-path exit
code unchanged; (d) warnings do not appear in JSON output.

**F7:** Single-story scope convergence.

---

## Open Questions for Human Approval

None. This is a mechanical mirror of BC-3.8.011 (a just-merged pattern from PR #382 /
issue #288 adversary audit). The implementation site is unambiguous, the warning strings
follow established precedent, and the BC numbering is confirmed. No design choices remain
open.

**Ready to proceed to pre-F2 validation and F2 on human GO.**

---

## Change Log

- [2026-05-19] Created — F1 consolidated from impact-boundary.md (architect) +
  affected-artifacts.md (business-analyst). Status: approved. No adversarial passes
  conducted yet (simple mechanical mirror; pre-F2 validation is the next gate).
