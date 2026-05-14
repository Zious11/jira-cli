---
document_type: adversarial-pass
phase: F1d
pass: 10
cycle: 3-feature-search-issue-keys-dedupe-365
spec_version_reviewed: 0.1.8
date: 2026-05-14
verdict: CLEAN
counter_after: 2/3
findings_total: 5
findings_blocking: 0
findings_concern: 0
findings_nit: 5
---

# F1d Pass 10 — Issue #365 — CLEAN (2/3)

**Spec version reviewed:** v0.1.8 (unchanged)  
**Verdict:** CLEAN — 0 BLOCKING, 0 CONCERN. 5 NITs noted but below threshold for counter reset.  
**Counter:** 2/3  
**Spec amendment after this pass:** none (NITs do not trigger amendment at this convergence stage)

## Review Summary

Security, race, resource, and observability lens pass.

## NIT-Level Observations (below amendment threshold)

### NIT-1 — No explicit resource-cleanup note for `HashSet<String>` per-iteration allocation

The spec correctly notes that the HashSet is allocated per iteration and
garbage-collected (dropped) at the end of each iteration's dedupe block.
No explicit note confirms that the `HashSet` lifetime ends before the loop
continues (it is scoped by `{...}` braces). This is obvious from the pseudocode
but worth a one-word mention.

**Disposition:** NIT. The brace-scoped block in the pseudocode makes this clear
to Rust readers. No amendment needed.

### NIT-2 — Risk 5 does not note whether the triple-collision is reproducible in a test

Risk 5 describes the triple-collision scenario but does not note whether it is
exercised by any test. The spec elsewhere notes "neither arm of the Apr 2025
overshoot detector is exercised by any test in this PR." A brief statement that
this is intentionally unexercised (not an oversight) would be clearer.

**Disposition:** NIT. The existing text "neither arm of that detector... is
exercised by any test in this PR; both remain theoretically described in this Risk
and unexercised" is sufficient. No amendment needed.

### NIT-3 — `test_search_issue_keys_guard_abort_all_keys_duplicate` scenario says "within-page duplicate — unusual but legal"

The spec describes within-page duplicates as "unusual but legal under extreme
drift." No citation or evidence for this claim is present. If the Jira server
deduplicated responses before sending (which is a reasonable assumption for a
well-behaved server), this scenario might be impossible in practice.

**Disposition:** NIT. The claim is a conservative defensive assumption; even if
the server never sends within-page duplicates, the test is harmless. No
citation required. No amendment needed.

### NIT-4 — References section ordering

The References section lists `std::vec::Vec::retain` before
`std::collections::HashSet::insert` but the implementation uses `HashSet::insert`
semantically first (the return value drives `retain`'s decision). Minor ordering
preference.

**Disposition:** NIT. Reference ordering is cosmetic. No amendment needed.

### NIT-5 — `CLAUDE.md — no change required` section title uses em-dash inconsistently with other subsections

Other Doc and Spec Fallout subsections use `—` (em-dash) in their section
headers; the CLAUDE.md subsection uses a different dash character.

**Disposition:** NIT. Cosmetic; no amendment needed.

## Security / Race / Resource Assessment

- No new concurrent data structures introduced.
- `HashSet<String>` is stack-allocated with heap backing; no `Arc`/`Mutex`
  required; no cross-thread access.
- No new I/O paths, network calls, or file operations.
- No new exit codes, flags, or stderr messages.
- The dedupe is purely in-memory within the pagination loop; no shared state.
- No TOCTOU surfaces.

No security or concurrency concerns. Counter advances to 2/3.
