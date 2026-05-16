# Red Gate Log — S-345

## Story
S-345: Extract label-coalesce JSON builder into pure function + proptest

## Pattern
Standard Red Gate (assertion-error fail before implementation). The proptest
references the stub function `build_labels_edited_fields` which returns
`{"STUB_INTENTIONALLY_WRONG": true}`. The proptest's first invariant assertion
(top-level "labels" key MUST be present) fires immediately.

## Outcome
- Step A.1 first run: PROPTEST FAILED with assertion error referencing BC-3.4.006
  (CORRECT — discriminates the contract). Compile succeeded.
- Implementer (next dispatch) will replace the stub body; proptest expected to
  go green on first run after that.

## Evidence
- Stub function: src/cli/issue/create.rs (just above handle_edit_bulk_labels, gated #[cfg(test)] during stub phase)
- Proptest module: src/cli/issue/create.rs #[cfg(test)] mod build_labels_proptests
- Worktree commit (Red Gate): 195dc3a

## Pass 1 Adversary Fixes — Applied 2026-05-15

All four findings applied to `src/cli/issue/create.rs` in worktree commit `2cf3930`.

### F1 (CONCERN) — Verbatim schema-note restored
Replaced the paraphrased "Schema note: this pins the CURRENT shape..." paragraph with
the 4-line verbatim block from develop baseline (lines 869-872):
```
/// Shape is best-guess (unverified against live Atlassian API; tracked at #331).
/// PR2 test asserts .expect(1) on bulk POST to ensure ADD+REMOVE coalesce into ONE call,
/// but the exact JSON nesting matches a loose `body_string_contains` matcher — schema
/// accuracy is the work being deferred to #331.
```

### F2 (NIT) — Proptest tightened: sole top-level key assertion
Added `obj.len() == 1` assertion immediately after `prop_assume!`, before the `labels`
extraction. Catches schema-drift regressions like `{"labels": [...], "extra": "drift"}`.
Verified: proptest passes (1 passed, 256 cases).

### F3 (NIT) — debug_assert! for misuse precondition
Added `debug_assert!(!adds.is_empty() || !removes.is_empty(), ...)` at function entry.
Zero cost in release builds; surfaces misuse in debug/test builds.

### F5 (NIT) — Coalesce rationale comment at call site
Added two comment lines immediately above `let edited_fields = build_labels_edited_fields(...)`:
```rust
// Coalesce ADD and REMOVE into a single bulk POST when both are present.
// Both operations submitted in one request as an array of label-action objects.
```

### Deferred
- F4 (NIT): proptest block naming convention exemption — filed as future process-gap issue.
- F6 (NIT): broader proptest string strategy — [a-z]{1,10} kept per story spec suggestion.

### Verification Results
- `cargo test --lib build_labels_proptests`: 1 passed (256 cases, no shrink)
- `cargo test --test issue_bulk_pr2`: 40 passed, 0 failed
- `cargo test --test issue_bulk`: 9 passed, 0 failed
- `cargo fmt --check`: clean
- `cargo clippy --all-targets -- -D warnings`: clean (no warnings)

Worktree commit: `2cf3930`

---

## Pass 2 Adversary Fixes — Applied 2026-05-15

All three findings applied to `src/cli/issue/create.rs` in worktree commit `e7d8736`.

### F1 (CONCERN) — Handler doc-comment freshness
Replaced the stale `handle_edit_bulk_labels` doc-comment line that said "the POST body below"
and referenced "#345 extracts a pure builder" (future work that is now shipped in this PR)
with text that correctly states "the POST body is constructed by `build_labels_edited_fields`
(BC-3.4.006)". Also removed the stale `#345 extracts pure builders` forward-reference from
the dry-run block comment inside `handle_edit`.

### F3 (NIT) — #331 schema-caveat de-duplicated
Condensed the 6-line call-site comment block above `build_labels_edited_fields(...)` to
3 lines that preserve the coalesce rationale and point to the function doc-comment as the
single authoritative source for the `#331` schema caveat. Verbatim duplicated schema detail
removed from call site.

### F4 (NIT) — Valid JSON examples in `build_labels_edited_fields` doc-comment
Replaced the single-line pseudo-JSON `"ADD"|"REMOVE"` alternation (not valid JSON) with
three concrete examples: one for ADD-only, one for REMOVE-only, one for the both-action
array-form. Matches the style of the surrounding doc-comment block and satisfies CLAUDE.md
citation-discipline policy (user-facing strings must be syntactically valid).

### Deferred
- F2 (NIT): BC file update — handled by product-owner in parallel (not in this commit).
- F5 (process-gap): "re-tune caller doc-comment after pure-helper extraction" — to be
  codified in per-story-delivery checklist by state-manager at cycle close-out.

### Verification Results
- `cargo test --lib build_labels_proptests`: 1 passed (proptest green)
- `cargo test --test issue_bulk_pr2`: 40 passed, 0 failed
- `cargo test --test issue_bulk`: 9 passed, 0 failed
- `cargo fmt --check`: clean
- `cargo clippy --all-targets -- -D warnings`: clean (no warnings)
- `grep -n "#345" src/cli/issue/create.rs`: no output (zero stale forward-references)

Worktree commit: `e7d8736`

---

## Pass 3 Adversary Fixes — Applied 2026-05-15

One finding applied to `src/cli/issue/create.rs` in worktree commit `283fde8`.

### F2 (CONCERN) — Handler doc-comment internal contradiction softened

Lines 883-884 previously read:
```
/// canonical Atlassian schema (per #331) requires top-level `labelsFields`
/// array always — that's the long-term target for both code paths.
```

This contradicted line 866's explicit "best-guess pending #331 empirical verification"
qualifier. Issue #331 is still open and sandbox-blocked; the `labelsFields` claim
derives from Perplexity research, not a live API probe. Per CLAUDE.md citation-discipline
policy, the authoritative phrasing was overstated.

Replaced with:
```
/// per #331's Perplexity research, the canonical Atlassian schema is documented
/// to use a top-level `labelsFields` array — that's the long-term target for
/// both code paths once #331's empirical sandbox verification confirms it.
```

### Verification Results
- `cargo test --lib build_labels_proptests`: 1 passed (256 cases)
- `cargo test --test issue_bulk_pr2`: 40 passed, 0 failed
- `cargo test --test issue_bulk`: 9 passed, 0 failed
- `cargo fmt --check`: clean
- `cargo clippy --all-targets -- -D warnings`: clean (no warnings)

Worktree commit: `283fde8`

---

## Verbatim Red Gate proof

```
running 1 test
test cli::issue::create::build_labels_proptests::build_labels_edited_fields_invariants ... FAILED

---- cli::issue::create::build_labels_proptests::build_labels_edited_fields_invariants stdout ----

thread '...' panicked at src/cli/issue/create.rs:1509:47:
BC-3.4.006: top-level 'labels' key MUST be present

Test failed: BC-3.4.006: top-level 'labels' key MUST be present.
minimal failing input: adds = ["a"], removes = []
    successes: 0
    local rejects: 0
    global rejects: 0

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 701 filtered out
```
