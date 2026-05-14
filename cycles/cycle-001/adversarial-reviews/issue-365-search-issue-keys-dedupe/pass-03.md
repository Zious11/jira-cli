---
document_type: adversarial-pass
phase: F1d
pass: 3
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.2
date: 2026-05-14
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 4
findings_blocking: 0
findings_concern: 1
findings_nit: 3
---

# F1d Pass 3 — Issue #365 — NOT-CLEAN

**Spec version reviewed:** 0.1.2  
**Verdict:** NOT-CLEAN (1 CONCERN surfaced; counter reset to 0/3)  
**Spec amendment after this pass:** v0.1.2 → v0.1.3

## Findings

### CONCERN-1 — Parent-spec propagation gaps not covered by this spec's Doc and Spec Fallout

The Doc and Spec Fallout section addressed `src/api/jira/issues.rs` rustdoc
updates, `src/api/jira/issues.rs` inline comment updates, and the BC-2.6.050
body update. However, it did not address the four required updates to the parent
spec `docs/specs/2026-05-13-search-issue-keys.md`:

1. Close-out note on the "New follow-up (deferred)" bullet (line 276).
2. Update test inventory entry #13 (line 243) to reflect rename + assertion flip.
3. Strike the "Possible duplicate keys on guard-abort" risks bullet (line 258) and
   replace with a RESOLVED annotation.
4. Update the Backwards Compatibility paragraph (line 271) about the `--max`
   spurious error.

All four are required in the same PR as the implementation to prevent the parent
spec from becoming durably stale after the feature lands.

**Resolution (v0.1.3):** Doc and Spec Fallout section extended with a new
§`docs/specs/2026-05-13-search-issue-keys.md` subsection specifying all four
updates with exact replacement text and line-number references for each. Framed
as "F3 implementer instruction" to make the responsibility explicit.

### NIT-1 — Dedupe algorithm comparison table missing MSRV column

The Validated API Facts table comparing dedupe candidates (HashSet retain,
IndexSet, itertools::unique()) did not include an MSRV (Minimum Supported Rust
Version) column. This is relevant because the project's MSRV constraints affect
which candidates are admissible.

**Resolution (v0.1.3):** MSRV column added. IndexSet: MSRV 1.85 (indexmap 2.14);
itertools::unique(): MSRV 1.63 (itertools 0.14). HashSet retain (std): no MSRV
constraint beyond stdlib. Confirmed HashSet retain wins on zero-dependency grounds
regardless of MSRV.

### NIT-2 — Perplexity itertools accuracy gap not noted in spec

The research report documented that Perplexity incorrectly claimed
`itertools::unique()` is consecutive-only. The spec did not note this for the
adversary to be aware of during review.

**Resolution (v0.1.3):** Research caveat block added to the Validated API Facts
section noting the Perplexity claim, the refutation via WebFetch against docs.rs,
and the clarification that this does not affect the algorithm choice (HashSet wins
on no-new-dependency grounds regardless).

### NIT-3 — Spec version trajectory not cross-referenced in frontmatter

The frontmatter had no field linking the spec's version history to this
adversarial review cycle, making it impossible to determine at a glance
which spec version corresponds to which adversarial pass.

**Resolution (v0.1.3):** Frontmatter `related_research` field already present
(`issue-365-design-validation.md`). Version trajectory reconstruction is left to
the convergence-summary document; no additional frontmatter field added.
Recorded as expected behavior.

## Routing Decision

CONCERN-1 (parent-spec propagation gaps) is a genuine spec completeness gap
requiring amendment. Counter reset to 0/3. Spec amended to v0.1.3.
