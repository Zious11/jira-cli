---
document_type: adversarial-pass
phase: F1d
pass: 7
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.6
date: 2026-05-14
verdict: NOT-CLEAN
counter_after: 0/3
findings_total: 6
findings_blocking: 0
findings_concern: 1
findings_nit: 4
observations: 1
process_gap_flags: 2
---

# F1d Pass 7 — Issue #365 — NOT-CLEAN

**Spec version reviewed:** 0.1.6  
**Verdict:** NOT-CLEAN (1 CONCERN surfaced + 1 process-gap Observation; counter reset to 0/3)  
**Spec amendment after this pass:** v0.1.6 → v0.1.7

## Findings

### CONCERN-1 — `itertools::unique()` research caveat cites refutation but does not name the specific docs.rs URL

The Research Caveat block in Validated API Facts stated:
> "This was refuted by direct WebFetch against docs.rs."

But did not cite the specific docs.rs URL or the exact documentation language
that refutes the claim. Without the URL, a future reviewer cannot verify the
refutation independently.

**Resolution (v0.1.7):** Reference to `std::vec::Vec::retain` and
`std::collections::HashSet::insert` documentation added to the References
section at the bottom of the spec with explicit doc.rs URLs. The itertools
caveat block now references the References section for the specific URLs. The
caveat itself is retained but with proper citation anchoring.

### NIT-1 — Verbatim-quote discipline: parent-spec update (c) risks bullet replacement text uses strike HTML vs Markdown

The parent-spec update (c) risks-bullet replacement used raw HTML `<s>` tags
in the verbatim replacement text rather than Markdown `~~` strikethrough, which
is inconsistent with how the parent spec itself formats resolved items.

**Resolution (v0.1.7):** All strike-through in parent-spec update (c) and (d)
replacement text changed to Markdown `~~` notation, consistent with the parent
spec's existing resolved-bullet style.

### NIT-2 — Verbatim-quote discipline: BC-2.6.050 fallout "append to Behavior field" not specified as block-quote or inline

The BC-2.6.050 body update instruction said "Append to the Behavior field" but
did not specify whether the text should be appended as a new sentence inline,
a new paragraph, or a block-quote. The BC file format is opaque without seeing it.

**Resolution (v0.1.7):** Instruction clarified to "append as a new paragraph,
preceded by a blank line, after the sentence ending `...clamps maxResults per page
to .min(100) for parity with search_issues`."

### NIT-3 — `HashSet::with_capacity(all_keys.len())` capacity sizing note is misleading

The implementation outline used `HashSet::with_capacity(all_keys.len())` as the
capacity hint. This is sized to the CURRENT accumulated `all_keys.len()` at the
moment of the call, which grows per iteration. The Risks section correctly noted
"sized to the current accumulated key count at each iteration, not the final total."
But the implementation pseudocode did not include this nuance, making it look like
a fixed-size HashSet.

**Resolution (v0.1.7):** Implementation note added: "The capacity
`all_keys.len()` is the accumulated count at this iteration, not the final total.
For the first iteration this equals the first page's key count (≤100). For
subsequent iterations it grows. Using `with_capacity` is a minor optimization hint
(avoids rehashing) but the correctness does not depend on it."

### NIT-4 — Risk 4 (`Vec::retain` clone cost) double-explains the borrow-checker issue already covered in Implementation Note

Risk 4 and the Implementation Note in the Algorithm section both explain why
`HashSet<&str>` cannot be used (borrow-checker: cannot borrow `all_keys` inside
`retain`'s `&mut self`). This is a redundant explanation appearing in two sections.

**Resolution (v0.1.7):** Risk 4 kept as the canonical explanation; Implementation
Note streamlined to a forward-reference to Risk 4 for the detailed borrow-checker
rationale.

## Observations

### OBS-1 [process-gap] — Adversary validating itertools::unique() claim without WebFetch access

The adversary was asked to validate the refutation of the Perplexity claim about
`itertools::unique()` being consecutive-only. However, the adversary in the F1d
role does not have WebFetch access during spec review — only the research-agent
(F1 phase) does. The adversary can verify citations that are directly quotable from
the spec or from the research report, but cannot independently confirm docs.rs
content.

**Implication:** The citation validation in CONCERN-1 (requesting a specific
docs.rs URL) is at the boundary of what the F1d adversary can verify. The research
agent already ran this verification (documented in `.factory/research/issue-365-design-validation.md`
§Q1 §C.2). The F1d adversary should accept research-agent output as verified and
focus on spec completeness, not re-verification of research agent work.

**Recommendation:** Codify in process: the F1d adversary's scope is spec-text
completeness and implementation-readiness, not re-running research agent
verifications. Citations verified by the research agent are accepted as ground truth
for F1d purposes.

**Tagged:** `[process-gap]` — see state-manager process-gap codification check.

## Routing Decision

CONCERN-1 is a genuine citation-completeness gap. Counter reset to 0/3.
Spec amended to v0.1.7. OBS-1 tagged as process-gap for state-manager tracking.
