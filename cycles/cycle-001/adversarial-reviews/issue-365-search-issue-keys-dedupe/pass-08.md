---
document_type: adversarial-pass
phase: F1d
pass: 8
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.7
date: 2026-05-14
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 3
findings_blocking: 0
findings_concern: 1
findings_nit: 2
---

# F1d Pass 8 — Issue #365 — NOT-CLEAN

**Spec version reviewed:** 0.1.7  
**Verdict:** NOT-CLEAN (1 CONCERN surfaced; counter reset to 0/3)  
**Spec amendment after this pass:** v0.1.7 → v0.1.8

## Findings

### CONCERN-1 — BC-2.6.050 fallout "Observation-1" tag not cross-referenced to an NIT finding

The BC-2.6.050 Doc and Spec Fallout subsection introduced an "Observation-1
(NIT-3, Option B adopted)" tag but the NIT-3 label did not correspond to any
numbered NIT in the pass-04 adversarial findings (pass-04 had NIT-1 and NIT-2;
there was no NIT-3 in pass-04). This orphaned label would confuse an implementer
trying to trace the origin of the observation.

**Resolution (v0.1.8):** The "Observation-1" tag reformatted to clarify it
originated during the spec authoring process (not from a numbered pass NIT), and
"Option B adopted" clarified as referring to the fix strategy for the stale
JRACLOUD-94632 citation (fix inline in same PR vs. defer). The NIT-3 reference
removed; the subsection now stands alone with clear provenance ("identified during
pass-04 review; BC citation stale since PR #364 rebind").

### NIT-1 — Parent-spec update (b) replacement text verbatim quote uses backtick-quoted token names inconsistently

The replacement text for parent-spec update (b) used backtick quoting for Rust
identifiers (`nextPageToken`, `has_more`, `keys`) but not for test names (quoted
with regular double-quotes). Markdown renders both similarly, but consistency aids
tooling (e.g., rustdoc cross-references).

**Resolution (v0.1.8):** All identifier references in the replacement text
normalized to backtick quoting. Test names wrapped in code spans. No behavioral
change.

### NIT-2 — "Implementer note" at end of limit-truncation test spec could be read as optional

The implementer note "adjust `more_available` assertions based on the mock's
`next_page_token` value..." — even after v0.1.6 clarified the expected value as
`!result.has_more` — retained a phrasing that made the note sound advisory rather
than normative. The v0.1.6 fix improved precision but the sentence leading into the
note still said "adjust" which implies optionality.

**Resolution (v0.1.8):** Leading sentence reworded from "adjust ... assertions"
to "The assertion MUST be `!result.has_more` for this mock setup, because..."
to make the normative status explicit.

## Routing Decision

CONCERN-1 is a genuine traceability gap requiring spec amendment. Counter reset
to 0/3. Spec amended to v0.1.8. This is expected to be the final substantive
amendment — the remaining open issues are cosmetic/precision NITs at the boundary
of what the adversary can reasonably surface.
