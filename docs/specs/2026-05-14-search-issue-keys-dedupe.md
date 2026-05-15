---
document_type: feature-spec
issue: 365
traces_to: [BC-2.6.050, BC-2.6.051]
status: implemented
version: 0.1.12
date: 2026-05-14
related_research: .factory/research/issue-365-design-validation.md
---

# In-function Dedupe on All Exit Paths (`search_issue_keys` and `search_issues`)

**Issue:** [#365](https://github.com/Zious11/jira-cli/issues/365) — add
silent order-preserving dedupe on all exit paths of both `search_issue_keys`
and `search_issues` where drift-induced duplicates can accumulate.

## Scope

Closes the deferred follow-up from `docs/specs/2026-05-13-search-issue-keys.md`
("New follow-up (deferred): `feat(search): dedupe keys on repeated-cursor guard
abort`") by adding silent, order-preserving deduplication to **both
`search_issue_keys` (keys-only) and `search_issues` (full-body)** in
`src/api/jira/issues.rs`, on all exit paths. Under JRACLOUD-95368 live-data
drift, earlier pages may collect duplicates before the repeated `nextPageToken`
is detected and the guard fires — OR before the limit-truncation check fires.

For `search_issue_keys`: the current no-dedupe contract is pinned by test 13
in `tests/search_issue_keys.rs`. This feature flips that contract: after this
change, `all_keys` is deduplicated on all exit paths (guard-abort,
limit-truncation, and cursor-exhaustion) via an incremental `seen_keys` HashSet
maintained outside the loop — only newly-seen keys are appended.

For `search_issues`: the same root cause (JRACLOUD-95368) applies. DP-4
("symmetric treatment for `search_issues` is out of scope") is reversed by user
decision. `all_issues` is deduplicated via the same incremental seen_keys
approach (keyed on `issue.key`) as `search_issue_keys`. This
lets the `SearchResult` rustdoc drop the "may contain duplicate issues" warning,
simplifying the public contract and eliminating a latent bug for future callers
that adopt `+1` over-fetch with `search_issues`.

No new public API fields, no stderr warning changes, no exit-code changes.
All callers (`jr issue list --jql`, `jr board view`, `jr queue view`, and
`handle_edit::effective_keys`) benefit transparently. Note: `jr sprint current`
uses the Agile API endpoint (`/rest/agile/1.0/sprint/{id}/issue` via
`get_sprint_issues`) and is NOT a `search_issues` caller; it is unaffected
by JRACLOUD-95368 and by this dedupe.

> **Version 0.1.1 note (F1d Pass 1 adversarial review):** The original scope
> (v0.1.0) targeted the guard-abort path only, per research DP-2. Adversarial
> review (CONCERN-2) correctly identified that the limit-truncation path
> (lines 332–346 of `src/api/jira/issues.rs`) is also affected by drift
> duplicates and is not fixed by guard-abort-only dedupe. The scope is extended
> to cover both paths (Option A chosen). See "Design Divergences from Research"
> below.

## Out of Scope / Follow-ups

- **Symmetric dedupe on `search_issues`** (DP-4 — reversed): ~~the `search_issues`
  full-body sibling has the same anti-loop guard but no current caller relies
  on `result.issues.len()` as a correctness check the way `effective_keys`
  does. Adding dedupe there would be "symmetric for symmetry's sake" and is
  explicitly deferred per the scoped-treatment principle validated in
  `.factory/research/issue-365-design-validation.md` §Q3. A one-liner comment
  at the mirroring guard in `search_issues` records the asymmetry as
  intentional.~~ **REVERSED in v0.1.9 by user decision** — `search_issues`
  dedupe is now IN SCOPE. See "Design Divergences from Research" (DP-4 reversal)
  and the "Implementation Outline — `search_issues`" section. The asymmetry
  comment is replaced by symmetric implementation.

- **Exposing a `dedupe_count` or `had_duplicates` signal on `KeySearchResult`**
  (DP-3, rejected): the existing `has_more = true` on the guard-abort path
  already signals "results may be incomplete." The surveyed production Rust
  SDKs (octocrab, aws-sdk-rust, azure_core, google-cloud-rust) universally
  use preserve-and-document rather than preserve-and-signal-with-a-flag. The
  stderr `JRACLOUD-95368` warning is already the canonical drift signal. No
  current or foreseeable caller would consume a programmatic `dedupe_count`.
  If one materialises, a richer `has_more: Completeness` enum
  (variants `Complete`, `TruncatedByLimit`, `DriftAborted`) is the correct
  future refactor — not tacking on a count field.

- **Lifting the `.min(100)` per-page clamp**: deferred; already listed in the
  parent spec out-of-scope section and unchanged here.

- **A `has_more: Completeness` enum** for fine-grained disambiguation between
  limit-truncation and guard-abort: a reasonable future refactor once a second
  caller needs the distinction. Out of scope here.

- **Engine-level follow-up (PG-365-2) — dark-factory FACTORY.md or
  adversarial-review SKILL.md clarification:** the F1d adversary cannot
  WebFetch external sources (read-only tool profile). When the spec cites a
  research-validated external fact, the adversary can only verify the citation
  exists, not whether the underlying claim is correct. The chain is:
  research-agent (with WebFetch + Perplexity) → spec → adversary (no WebFetch).
  Pass 7 of this cycle's F1d surfaced this as an unresolved trust-boundary
  question. The deeper improvement: research-validation reports SHOULD include
  verbatim quoted blocks from external sources, sized to support adversarial
  verification without re-fetching. Track in dark-factory engine repo as a
  separate issue; this jira-cli spec only flags it.

## Validated API Facts

No new API facts for this feature. The root cause remains JRACLOUD-95368
("nextPageToken pagination is not snapshot-stable under live mutation") as
documented in `docs/specs/2026-05-13-search-issue-keys.md` §Validated API
Facts §5, and the related Atlassian KB article prescribing `ORDER BY key ASC`
as the upstream mitigation.

**Dedupe algorithm comparison** (from `.factory/research/issue-365-design-validation.md` §Q1):

| Candidate | New dependency? | Order-preserving? | Non-consecutive dups? | Alloc at n=1002 |
|-----------|----------------|------------------|-----------------------|-----------------|
| `HashSet` retain (std) | No | Yes | Yes | ~24 KB HashSet + per-elem String clones |
| `indexmap::IndexSet` | Yes (indexmap 2.14, MSRV 1.85) | Yes | Yes | ~26 KB IndexSet + double Vec |
| `itertools::Itertools::unique()` | Yes (itertools 0.14, MSRV 1.63) | Yes | Yes | ~24 KB internal HashSet + Vec |

**`Vec::dedup()` is explicitly wrong** for this case: it is consecutive-only
and JRACLOUD-95368 drift can emit the same key non-consecutively across pages
(e.g., `X-1` on page 1, `X-2` on page 2, `X-1` again from page 2 before the
cursor repeats — this is the load-bearing non-consecutive case demonstrated by
the new `test_search_issue_keys_dedupes_non_consecutive_across_pages`
test introduced below in the Test Diff section).

**Research caveat (noted for adversary):** Perplexity confidently but
incorrectly claimed `itertools::unique()` is consecutive-only (conflation with
`Vec::dedup` / `Itertools::dedup`). This was refuted by direct WebFetch
against docs.rs. The `itertools::Itertools::unique` documentation is
unambiguous: it is a global (non-consecutive) dedupe with `Clone + Eq + Hash`
bounds. See `.factory/research/issue-365-design-validation.md` §Q1 §C.2.
This does not affect the chosen algorithm (incremental HashSet insert wins on
zero-new-dependency grounds regardless) but it is recorded here because it is a live Perplexity
accuracy gap for Rust crate method semantics.

## Design Divergences from Research

**DP-2 literal framing vs. extended scope (v0.1.1):**

Research DP-2 framed the fix as "dedupe only on the guard-abort path" with the
intuition "we're already intervening at the drift-detection point, so that's the
natural insertion site." Adversarial review (F1d Pass 1, CONCERN-2) correctly
identified that this intuition was incomplete: the limit-truncation path
(lines 332–346 of `src/api/jira/issues.rs`) can also produce the same
spurious-truncation-error bug when drift duplicates are present on the first
page.

**Concrete scenario (limit-truncation path bug):**

- Caller passes `limit = effective_max + 1` (e.g., `effective_max = 10`, `limit = 11`)
- Page 1 server response under JRACLOUD-95368 drift returns `[A, A, B, C, D, E, F, G, H, I, J]`
  (11 keys; A appears twice due to live-data drift)
- `all_keys.len() = 11 >= max = 11` → truncation block (lines 332–346) fires BEFORE the guard check
- `all_keys.truncate(11)` → still 11 keys (truncation is a no-op at capacity)
- Control returns to `handle_edit::effective_keys`: `matched_keys.len() = 11 > effective_max = 10`
  → spurious truncation error fires at `cli/issue/create.rs:398–407`
- Same user-visible bug as the guard-abort path. Not fixed by guard-abort-only dedupe.

**Decision: Option A — extend dedupe to both exit paths.**

Dedupe is applied on every loop iteration after extending `all_keys`, before
either break-decision check. This fully closes the spurious-truncation-error
bug on all paths.

Justification:
- The bug is real and reproducible on the truncation path under drift.
- The allocation cost remains negligible (O(N²/page_size) across all iterations,
  ≤ 1001 unique-key insertions for a worst-case N=1001 fetch at 100 items/page,
  because the incremental seen_keys HashSet only stores unique keys — O(N) total
  across all pages; no per-iteration Vec rescan). Per-iteration dedupe (before
  the break-decision checks) is required for **correctness**, not just chosen
  for cost-efficiency. A single post-loop O(N) dedupe would be cheaper but
  cannot fix the spurious-truncation bug because the truncation check would
  already have fired pre-dedupe.
- The behavioral surface is slightly larger than DP-2's literal framing but
  strictly correct: deduplication is a monotone improvement that cannot break
  callers (it produces fewer or equal keys, never more).
- Option B (guard-abort only + defer truncation-path fix) would leave the bug
  half-fixed and require a follow-up issue, adding tracking overhead with no
  design benefit.

**DP-2 is superseded on insertion point.** All other DP points (DP-3, DP-6,
DP-7) are adopted verbatim. DP-4 is reversed as documented below.

**DP-4 reversal — extend dedupe symmetrically to `search_issues` (v0.1.9):**

Research DP-4 framed the fix as scoped to `search_issue_keys` only, with the
justification that "no current caller relies on `result.issues.len()` as a
correctness check." User decision in v0.1.9 reversed this:

1. **Same root cause, same guard structure, should have same fix.** Code parity
   prevents silent divergence in behaviour when both functions share the same
   JRACLOUD-95368 risk.
2. **Latent bug.** If a future caller adopts `+1` over-fetch with `search_issues`,
   the spurious-truncation bug appears silently. Fix now rather than in a
   follow-up PR.
3. **Public contract simplification.** `SearchResult` rustdoc currently warns
   "may contain duplicate issues" — extending dedupe removes that warning,
   aligning `SearchResult` with `KeySearchResult`'s clean contract.
4. **Symmetric test coverage.** Adding `search_issues` dedupe tests in the same
   PR is cheaper than adding them later; the test infrastructure is already in
   place.

Algorithm for `search_issues`: keyed on `issue.key` (`Issue` does not impl
`Hash`, so the HashSet stores cloned key strings, not Issue structs). The per-
iteration insertion point is the same as for `search_issue_keys` — after
`all_issues.extend(page.issues)` and before any break-decision check.

## Behavioral Contract

### `search_issue_keys`

The new contract, replacing the current no-dedupe pin:

**All exit paths:** `all_keys` is deduplicated on every loop iteration via
an incremental `seen_keys: HashSet<String>` maintained outside the loop —
only newly-seen keys are appended (order-preserving, first-occurrence wins),
and the check runs before any break-decision. This means:

- `all_keys` is deduplicated before the limit-truncation check (`all_keys.len()
  >= max`). If drift caused a page to emit duplicates, `all_keys.len()` reflects
  the unique-key count before the truncation check fires. The `+1` over-fetch
  sentinel at the call site (`effective_max + 1`) therefore works correctly.
- `all_keys` is already deduplicated when the guard-abort check fires. No
  additional dedupe call is needed inside the guard block (the per-iteration
  dedupe already ran on that iteration's page).
- `all_keys` is already deduplicated when cursor exhaustion exits via `!page_has_more`.

**Guard-abort path** (inside the `if next_cursor.is_some() && next_cursor
== prev_cursor { ... }` block at `src/api/jira/issues.rs:356`):
- `has_more` remains `true` on this path — deduplication does not change the
  "results may be incomplete" signal.
- The stderr warning text is unchanged — the `JRACLOUD-95368` literal is still
  emitted (and pinned by `test_search_issue_keys_stderr_emits_jracloud_95368_literal`).

**Pure cursor-exhaustion path** (loop exits via `!page_has_more`): the
incremental seen_keys check runs on the final page's data before the loop
exits. No additional action needed. This path cannot produce cross-page
duplicates in normal operation; the dedup insert is a no-op when no
duplicate keys appear.

**Limit-truncation path** (loop exits via `all_keys.len() >= max`): dedupe
runs before the truncation check on each iteration. `all_keys.truncate(max)`
is applied to the already-deduped vec, so the truncation reflects unique keys.

**No new public API field.** `KeySearchResult` retains `{ keys: Vec<String>,
has_more: bool }` unchanged. The caller at `cli/issue/create.rs:386` sees a
correct unique-key count on all paths without any code change.

### `search_issues`

**All exit paths:** `all_issues` is deduplicated on every loop iteration via
an incremental `seen_keys: HashSet<String>` maintained outside the loop —
only newly-seen issues (keyed by `issue.key`) are appended (order-preserving,
first-occurrence wins), and the check runs before any break-decision. Because
`Issue` does not impl `Hash`, the HashSet stores cloned `issue.key` strings,
not Issue structs — the same approach as `search_issue_keys`. This means:

- `all_issues` is deduplicated before the limit-truncation check
  (`all_issues.len() >= max`). Drift-induced issue duplicates do not inflate
  the count seen by the truncation sentinel.
- `all_issues` is already deduplicated when the guard-abort check fires. No
  additional dedupe call is needed inside the guard block.
- `all_issues` is already deduplicated when cursor exhaustion exits via
  `!page_has_more`.

**No new public API field.** `SearchResult` retains `{ issues: Vec<Issue>,
has_more: bool }` unchanged. CLI callers (`list.rs`, `board.rs`, `queue.rs`)
benefit transparently from issue-level dedupe. Note: `cli/sprint.rs` calls
`get_sprint_issues` (Agile API), not `search_issues`, and is not a
`search_issues` caller.

**`SearchResult` rustdoc update:** the existing "may contain duplicate issues"
warning (lines 44–50 of `src/api/jira/issues.rs`) is dropped and replaced with
"duplicates eliminated client-side on all exit paths" — parallel to the
`KeySearchResult` update. See "Doc and Spec Fallout" below for the precise
replacement text.

## Implementation Outline

### Algorithm

Use an incremental `seen_keys: HashSet<String>` maintained **outside** the
loop. On each page, push only keys/issues not yet in `seen_keys` — preserving
first-occurrence order without rescanning the accumulated Vec:

```rust
// BEFORE the loop:
let mut seen_keys: HashSet<String> = HashSet::new();

// INSIDE the loop, after the page fetch:
// For search_issue_keys:
for key in page.issues.into_iter().map(|r| r.key) {
    if seen_keys.insert(key.clone()) {
        all_keys.push(key);
    }
}

// For search_issues (keyed on issue.key; Issue does not impl Hash):
for issue in page.issues {
    if seen_keys.insert(issue.key.clone()) {
        all_issues.push(issue);
    }
}
```

The `use std::collections::HashSet;` import belongs at the top of the file
alongside other `use` declarations.

**Why incremental (not per-iteration retain+rebuild)?** The earlier design
draft used `Vec::retain` with a locally-built `HashSet` after each
`all_keys.extend(...)`. That approach rescans the entire accumulated Vec on
every page — O(N²/page_size) total work. For `search_issue_keys(..., None)`
(unbounded) this grows quadratically with result-set size. The incremental
approach adds each key at most once across all pages — O(N) total (each
unique key stored once; duplicate keys are still hashed on `insert` attempts
but are never appended). Required for correctness: the dedup runs before the
limit-truncation check so `all_keys.len()` reflects unique-key count when the
truncation sentinel fires. `Vec::dedup()` is wrong regardless (consecutive-only;
JRACLOUD-95368 drift can emit the same key non-consecutively across pages).

### Placement within the loop

Revised loop body structure (showing insertion point for `search_issue_keys`):

```
// [NEW — outside the loop, before it begins]
let mut seen_keys: HashSet<String> = HashSet::new();

// INSIDE the loop, after page fetch:
for key in page.issues.into_iter().map(|r| r.key) {
    if seen_keys.insert(key.clone()) {
        all_keys.push(key);
    }
}

if let Some(max) = limit {
    if all_keys.len() >= max as usize {
        // ... truncation block (unchanged) ...
        break;
    }
}

if !page_has_more {
    break;
}

// GUARD: detect repeated cursor token (next == prev) → abort + warn.
if next_cursor.is_some() && next_cursor == prev_cursor {
    eprintln!("[jr] WARNING: ...");
    // [NOTE] all_keys is already deduped by the incremental seen_keys HashSet.
    // No additional dedupe call needed here.
    more_available = true;
    break;
}
```

The guard-abort block's inline comment ("this function does NOT dedupe") must
be replaced with a note that the incremental seen_keys HashSet (maintained
outside the loop) already handled it. See "Doc and Spec Fallout" below for the
precise text changes.

### Implementation Outline — `search_issues`

**Algorithm** (parallel to `search_issue_keys`, keyed on `issue.key`):

```rust
// BEFORE the loop:
let mut seen_keys: HashSet<String> = HashSet::new();

// INSIDE the loop, after page fetch — Issue does not impl Hash, key on issue.key:
for issue in page.issues {
    if seen_keys.insert(issue.key.clone()) {
        all_issues.push(issue);
    }
}
```

**Placement within the `search_issues` loop body:**

```
// [NEW — outside the loop, before it begins]
let mut seen_keys: HashSet<String> = HashSet::new();

// INSIDE the loop, after page fetch:
for issue in page.issues {
    if seen_keys.insert(issue.key.clone()) {
        all_issues.push(issue);
    }
}

if let Some(max) = limit {
    if all_issues.len() >= max as usize {
        more_available = all_issues.len() > max as usize || page_has_more;
        all_issues.truncate(max as usize);
        break;
    }
}

if !page_has_more {
    break;
}

// GUARD: detect repeated cursor token (next == prev) → abort + warn.
if next_cursor.is_some() && next_cursor == prev_cursor {
    eprintln!("[jr] WARNING: ...");
    // [NOTE] all_issues is already deduped by the incremental seen_keys HashSet.
    // No additional dedupe call needed here.
    more_available = true;
    break;
}
```

Source locations confirmed via Read of `src/api/jira/issues.rs`:
- `all_issues.extend(page.issues)` — line 214 (immediately before the
  existing limit-truncation block starting at line 216).
- Guard block: lines 228–254.
- Insertion point: after line 214, before line 216.

**Cost analysis update** (both functions combined): the `search_issues`
HashSet stores cloned key strings (not `Issue` structs — `Issue` would be
enormous), so allocation is in the same magnitude as `search_issue_keys`:
~25 KB worst case for 100 issues (key strings typically 7–12 ASCII chars).
Negligible. Both HashSets are dropped at end of each iteration's block scope.

### No changes required at the call site

`handle_edit::effective_keys` (`src/cli/issue/create.rs:374–409`) checks
`matched_keys.len() > effective_max` for truncation. After this change,
`matched_keys.len()` reflects unique-key count on all paths, so the `+1`
over-fetch truncation sentinel works correctly even when drift was detected
on either the guard-abort or limit-truncation path. No code change at the
call site is required (DP-7 adopted verbatim).

## Test Diff

### Test 13 — rewrite (rename + flip assertion)

**Current name:** `test_search_issue_keys_repeated_cursor_abort_does_not_dedupe`
**New name:** `test_search_issue_keys_repeated_cursor_abort_dedupes`

**Rename justification:** The old name describes the old contract; renaming
makes git-blame trace this test as the load-bearing pin under the new contract.
The CLAUDE.md no-rename convention ("Existing tests with no-prefix names are
NOT renamed") applies only to pre-`test_` prefix legacy tests; this test
already uses the `test_` prefix and is therefore not protected by that
convention.

**Location:** `tests/search_issue_keys.rs` (the block starting at line 307)

**Change:** The mock setup is identical (page 1 returns `["X-1"]` with
`nextPageToken: "loop"`, page 2 returns `["X-1", "X-2"]` with the same
`nextPageToken: "loop"`). Only the assertion changes:

```rust
// Before (no-dedupe pin):
assert_eq!(
    result.keys,
    vec!["X-1".to_string(), "X-1".to_string(), "X-2".to_string()],
    "search_issue_keys MUST NOT dedupe on repeated-cursor abort ..."
);

// After (dedupe contract):
assert_eq!(
    result.keys,
    vec!["X-1".to_string(), "X-2".to_string()],
    "search_issue_keys MUST dedupe on repeated-cursor abort while preserving \
     first-occurrence order. Under JRACLOUD-95368 drift, page 1 emits [X-1] \
     and page 2 emits [X-1, X-2] before the cursor repeats; after dedupe the \
     result must be [X-1, X-2]."
);
assert!(
    result.has_more,
    "repeated-cursor abort must set has_more=true regardless of dedupe"
);
```

Update the leading block comment (lines 291–305) to remove "no-dedupe contract
pin" framing and replace with a description of the new dedupe contract and a
reference to this spec.

### New test — non-consecutive duplicate across pages (correctness pin for Vec::dedup-is-wrong)

**Name:** `test_search_issue_keys_dedupes_non_consecutive_across_pages`

This test is the load-bearing correctness pin: it proves that `Vec::dedup()`
would be wrong and that the `HashSet` retain approach handles non-consecutive
duplicates correctly.

Scenario: page 1 returns `["X-1"]`, page 2 returns `["X-2", "X-1"]` (X-1
appears again, non-consecutively), page 2's response repeats `nextPageToken:
"loop"`. The guard fires after processing page 2. After dedupe, keys must be
`["X-1", "X-2"]` (first-occurrence order). `Vec::dedup()` would leave
`["X-1", "X-2", "X-1"]` unchanged (no adjacent duplicate).

```rust
// Page 1: returns ["X-1"] with nextPageToken "loop"
// Page 2: returns ["X-2", "X-1"] with nextPageToken "loop" (repeated)
// Guard fires. After incremental seen_keys dedupe: ["X-1", "X-2"].
// Vec::dedup() would incorrectly return ["X-1", "X-2", "X-1"].
assert_eq!(result.keys, vec!["X-1".to_string(), "X-2".to_string()]);
assert!(result.has_more);
```

### New test — limit-truncation path dedupes under drift (CONCERN-3, Option A pin)

**Name:** `test_search_issue_keys_limit_truncation_dedupes_under_drift`

This test pins the extended scope (Option A): dedupe applies on the
limit-truncation path, not only the guard-abort path.

Scenario: `effective_max = 10`, `limit = effective_max + 1 = 11`. Page 1 server
response under JRACLOUD-95368 drift returns `["A-1", "A-1", "B-1", "C-1",
"D-1", "E-1", "F-1", "G-1", "H-1", "I-1", "J-1"]` (11 keys, `A-1`
duplicated). Without dedupe: `all_keys.len() = 11 >= max = 11` → truncation
fires → `all_keys.truncate(11)` is a no-op → `keys` contains 11 entries
including the duplicate → caller check `matched_keys.len() = 11 > effective_max
= 10` → spurious truncation error. With dedupe: after per-iteration dedupe,
`all_keys` = `["A-1", "B-1", ..., "J-1"]` (10 unique keys);
`all_keys.len() = 10 < max = 11` → truncation block does NOT fire; loop
continues; no more pages → loop exits via cursor-exhaustion break; caller sees
10 unique keys; `matched_keys.len() = 10 > effective_max = 10` → `false` →
no spurious error. This matches the walked-through example at lines 116–123.

```rust
// effective_max = 10, limit = 11.
// Page 1: returns ["A-1", "A-1", "B-1", ..., "J-1"] (11 keys, A-1 dup)
// with no nextPageToken (next_page_token: None — no more pages).
// After per-iteration dedupe: all_keys = ["A-1","B-1",...,"J-1"] (10 unique).
// Truncation check: 10 < 11 → does NOT fire. No more pages → cursor-exhaustion break.
assert_eq!(
    result.keys,
    vec!["A-1","B-1","C-1","D-1","E-1","F-1","G-1","H-1","I-1","J-1"]
        .iter().map(|s| s.to_string()).collect::<Vec<_>>()
);
// No nextPageToken and len < max → more_available = false.
assert!(!result.has_more);
// Caller check: matched_keys.len() = 10 > effective_max = 10 → false → no spurious error.
```

> **Implementer note:** adjust `more_available` assertions based on the mock's
> `next_page_token` value and whether `all_keys.len() > max` after dedupe. The
> key assertion is that the returned `keys` vec contains exactly 10 unique
> entries with no `"A-1"` duplicate, confirming dedupe ran before the truncation
> check.

### Additional edge-case tests

These may be added as small focused tests or as sub-cases within the existing
test infrastructure at the implementer's discretion:

1. **`test_search_issue_keys_guard_abort_empty_keys_no_panic`** — page 1
   returns `[]` (no keys) with `nextPageToken: "loop"`, page 2 returns `[]`
   with the same token. Guard fires with `all_keys` empty. The incremental
   seen_keys loop over an empty page is a no-op; this test pins that no panic occurs and
   `result.keys == []`, `result.has_more == true`.

2. **`test_search_issue_keys_guard_abort_single_key_no_dup`** — page 1 returns
   `["A-1"]`, page 2 returns `["A-2"]` with the same cursor token. Guard fires;
   `all_keys = ["A-1", "A-2"]` with no duplicates. After dedupe: still
   `["A-1", "A-2"]`. Pins that the dedupe does not disturb the non-duplicate
   case.

3. **`test_search_issue_keys_guard_abort_all_keys_duplicate`** — page 1
   returns `["X-1", "X-1"]` (within-page duplicate — unusual but legal under
   extreme drift), page 2 returns `["X-1"]` with the same cursor. Guard fires;
   `all_keys = ["X-1", "X-1", "X-1"]` before dedupe. After dedupe: `["X-1"]`.
   Pins that the dedupe collapses all duplicates regardless of origin (within-page
   or cross-page).

### Unchanged tests (`search_issue_keys`)

All other tests in `tests/search_issue_keys.rs` (tests 1–12) are unaffected —
they exercise the clean-exhaustion, limit-truncation, error propagation, and
request-shape paths, none of which touch the guard-abort dedupe logic.
`test_search_issue_keys_stderr_emits_jracloud_95368_literal` is also unchanged:
the warning text is identical before and after dedupe.

---

### New tests — `search_issues` mirror set

The three keys-only tests above have direct mirrors for `search_issues`. These
go in `tests/rate_limit_cap_tests.rs` (where the existing `search_issues`
library tests live) or a new dedicated test file `tests/search_issues_dedupe.rs`
at the implementer's discretion.

#### Test: flip the existing `search_issues` no-dedupe assumption

**Name:** `test_search_issues_repeated_cursor_abort_dedupes`

The existing test `test_search_issues_repeated_cursor_abort_sets_has_more_true`
(lines 345–403 of `tests/rate_limit_cap_tests.rs`) uses a stuck-cursor mock
that returns the same issue (`"TEST-1"`) on both page 1 and page 2. That test
currently asserts `has_more == true` and `!result.issues.is_empty()`. It does
NOT assert on whether `result.issues` contains duplicates (so it pins no-dedupe
only implicitly). After this change, add a new companion test that explicitly
asserts dedupe occurred:

```rust
// Mock: page 1 returns [TEST-1], page 2 returns [TEST-1, TEST-2], both with
// nextPageToken "loop". Guard fires after page 2.
// After per-iteration dedupe: issues.len() == 2 (TEST-1, TEST-2).
// Without dedupe: issues.len() == 3 (TEST-1, TEST-1, TEST-2).
assert_eq!(
    result.issues.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(),
    vec!["TEST-1", "TEST-2"],
    "search_issues MUST dedupe on repeated-cursor abort while preserving \
     first-occurrence order. Under JRACLOUD-95368 drift, page 1 emits [TEST-1] \
     and page 2 emits [TEST-1, TEST-2] before the cursor repeats; after dedupe \
     the result must contain TEST-1 once and TEST-2 once."
);
assert!(
    result.has_more,
    "repeated-cursor abort must set has_more=true regardless of dedupe"
);
```

**Mock setup:** two-page mock at `POST /rest/api/3/search/jql`. Page 1 returns
`[{key: "TEST-1", fields: {...}}]` with `nextPageToken: "loop"`. Page 2 returns
`[{key: "TEST-1", fields: {...}}, {key: "TEST-2", fields: {...}}]` with the
same `nextPageToken: "loop"`. Guard fires; dedupe collapses the duplicate
TEST-1.

The full `Issue` fields shape required by `CursorPage<Issue>` deserialization
mirrors the `stuck_response` in `test_search_issues_repeated_cursor_abort_sets_has_more_true`
(lines 349–374). Implementer should copy that shape for the mock bodies.

#### Test: non-consecutive duplicate across pages

**Name:** `test_search_issues_dedupes_non_consecutive_across_pages`

Mirrors `test_search_issue_keys_dedupes_non_consecutive_across_pages`.

Scenario: page 1 returns `[TEST-1]`, page 2 returns `[TEST-2, TEST-1]` (TEST-1
appears again, non-consecutively), page 2's `nextPageToken` repeats. Guard fires.

```rust
// After incremental seen_keys dedupe keyed on issue.key: issues = [TEST-1, TEST-2].
// Vec::dedup() keyed on issue.key would leave [TEST-1, TEST-2, TEST-1]
// unchanged (no adjacent duplicate key).
assert_eq!(
    result.issues.iter().map(|i| i.key.as_str()).collect::<Vec<_>>(),
    vec!["TEST-1", "TEST-2"]
);
assert!(result.has_more);
```

#### Test: limit-truncation path dedupes under drift

**Name:** `test_search_issues_limit_truncation_dedupes_under_drift`

Mirrors `test_search_issue_keys_limit_truncation_dedupes_under_drift`, scaled
to full-body issue returns.

Scenario: `limit = 11`. Page 1 server response under JRACLOUD-95368 drift
returns 11 issues where TEST-1 appears twice. Without dedupe:
`all_issues.len() = 11 >= max = 11` → truncation fires, `has_more` may be
`true`. With dedupe: after per-iteration dedupe `all_issues.len() = 10 < 11`
→ truncation does NOT fire; loop exits via cursor-exhaustion (no
`nextPageToken`); caller receives 10 unique issues.

```rust
// limit = 11.
// Page 1: returns [TEST-1 (dup), TEST-1, TEST-2, ..., TEST-10] (11 issues,
// TEST-1 duplicated). next_page_token = None (no more pages).
// After per-iteration dedupe: all_issues = [TEST-1, TEST-2, ..., TEST-10]
// (10 unique).
// Truncation check: 10 < 11 → does NOT fire. Cursor exhaustion → break.
assert_eq!(result.issues.len(), 10);
assert_eq!(result.issues[0].key, "TEST-1"); // first occurrence preserved
// No nextPageToken and len < max → has_more = false.
assert!(!result.has_more);
```

### Regression-pin test — Risk #5 Apr 2025 overshoot silenced by drift dedupe (CONCERN-D)

**Name:** `test_search_issue_keys_apr2025_overshoot_silenced_by_drift_dedupe`

**Location:** `tests/search_issue_keys.rs`

**Classification: NEGATIVE-PIN test.** This test pins a documented regression
(Risk #5), NOT desired behavior. It asserts `has_more = false` in a corner where
the "correct" answer would be `has_more = true`. The doc-comment in the test must
state this explicitly so a future PR that resolves Risk #5 is forced to update
the test in lockstep.

**Scenario:** `limit = 10`. Page 1 server response returns 11 keys —
`["X-1", "X-1", "A-2", "A-3", "A-4", "A-5", "A-6", "A-7", "A-8", "A-9", "A-10"]`
(11 keys; X-1 duplicated; `next_page_token: None`, i.e. `isLast: true`). This
exercises the triple-collision: Apr 2025 regression (server sends 11 with
`isLast:true` signaling no more pages) + overshoot item is a drift duplicate
(X-1) + `nextPageToken` absent.

**What happens without dedupe:** `all_keys.len() = 11 > max = 10` → truncation
block fires with `more_available = (11 > 10) || false = true`; `all_keys.truncate(10)`.
The `len > max` overshoot detector correctly signals `has_more = true`.

**What happens with dedupe (regression):** After per-iteration dedupe,
`all_keys = ["X-1", "A-2", ..., "A-10"]` (10 unique). `all_keys.len() = 10`.
Truncation check: `10 >= 10` (where `max = limit = 10`) → DOES fire. Inside the
block: `more_available = (10 > 10) || page_has_more (false) = false`;
`all_keys.truncate(10)` is a no-op; loop exits via truncation-block break (not
cursor exhaustion). `more_available = false`. The Apr 2025 overshoot signal is
silenced. Result: `has_more = false` even though real data might exist server-side.

```rust
// NEGATIVE-PIN: this asserts the REGRESSION behavior described in Risk #5.
// has_more = false here is WRONG in the triple-collision scenario (Apr 2025
// server overshoot + drift duplicate + isLast:true). A future PR fixing
// Risk #5 MUST update this test to assert has_more = true.
// limit = 10.
// Page 1: returns ["X-1","X-1","A-2","A-3","A-4","A-5","A-6","A-7","A-8","A-9","A-10"]
// (11 keys, X-1 duplicated). next_page_token = None (server signals isLast:true).
// After per-iteration dedupe: all_keys = ["X-1","A-2",...,"A-10"] (10 unique).
// Truncation check: 10 >= 10 → DOES fire. more_available = (10 > 10) || false = false.
// truncate(10) no-op. Loop exits via truncation-block break (not cursor exhaustion).
assert_eq!(result.keys.len(), 10);
assert_eq!(result.keys[0], "X-1"); // first occurrence preserved
// REGRESSION: has_more is false; the Apr 2025 overshoot was silenced by dedupe.
// Tracked under Risk #5 of docs/specs/2026-05-14-search-issue-keys-dedupe.md.
assert!(!result.has_more, "REGRESSION-PIN: has_more is false because dedupe \
    collapsed the Apr 2025 overshoot duplicate before the truncation sentinel \
    could fire. This is the documented Risk #5 regression — not desired behavior. \
    Update this assertion if Risk #5 is fixed.");
```

**Also add a parallel test for `search_issues`:**

**Name:** `test_search_issues_apr2025_overshoot_silenced_by_drift_dedupe`

Same scenario scaled to full-body issue returns. Page 1 returns 11 issues with
TEST-1 duplicated and `next_page_token: None`. After dedupe: 10 unique issues.
Truncation check: `10 >= 10` (where `max = limit = 10`) → DOES fire. Inside the
block: `more_available = (10 > 10) || page_has_more (false) = false`; `truncate(10)`
no-op; loop exits via truncation-block break (not cursor exhaustion).
`has_more = false` (regression pin).

```rust
// NEGATIVE-PIN: same regression as the keys-only variant (Risk #5).
// limit = 10. Page 1: 11 issues, TEST-1 duplicated, next_page_token = None.
// After per-iteration dedupe: 10 unique issues.
// Truncation check: 10 >= 10 → DOES fire. more_available = (10 > 10) || false = false.
// truncate(10) no-op. Loop exits via truncation-block break (not cursor exhaustion).
assert_eq!(result.issues.len(), 10);
assert!(!result.has_more, "REGRESSION-PIN: see test_search_issue_keys_apr2025_overshoot_silenced_by_drift_dedupe");
```

### Unchanged tests (`search_issues`)

The existing tests in `tests/rate_limit_cap_tests.rs` are unaffected:
- `ac_008_and_ac_new_d_search_jql_cursor_loop_terminates_with_jracloud_warning`
  — subprocess test; asserts loop terminates and JRACLOUD-95368 appears in
  stderr. Dedupe is transparent to this test.
- `test_search_issues_repeated_cursor_abort_sets_has_more_true` — asserts
  `has_more == true` and `!result.issues.is_empty()`. Dedupe does not change
  either assertion (the test uses a single-issue stuck mock; after dedupe the
  single issue is still present, `has_more` is still true). This test remains
  a regression-pin for the `has_more` contract; the new
  `test_search_issues_repeated_cursor_abort_dedupes` test adds the dedupe pin
  on top.

## Caller Migration

None required. `handle_edit::effective_keys` at `src/cli/issue/create.rs:374–409`
is the sole caller of `search_issue_keys`. It already does:

```rust
let matched_keys = search_result.keys;
if matched_keys.len() > effective_max as usize { /* truncation error */ }
```

Before this change: on the guard-abort path AND on the limit-truncation path,
`matched_keys.len()` could be inflated by drift-induced duplicates, spuriously
triggering the truncation error at the limit boundary.

After this change: `matched_keys.len()` reflects unique-key count on all paths.
The truncation check becomes correct without any code change at the call site.
The `+1` over-fetch lookahead (`effective_max + 1`) continues to work correctly.

**Scope of fix:** This PR closes the spurious-truncation-error bug on both the
guard-abort sub-path and the limit-truncation sub-path. Both paths are fully
fixed by the per-iteration dedupe approach adopted in v0.1.1 (Option A). No
follow-up issue is needed for the truncation path.

This is the complete motivation for the feature (DP-7 adopted verbatim; DP-2
insertion point superseded per "Design Divergences from Research").

## Doc and Spec Fallout

### `src/api/jira/issues.rs` — rustdoc update for `SearchResult` (lines 31–63)

The `SearchResult` struct rustdoc at lines 31–63 documents the `has_more`
semantics for `search_issues`. Case 2 currently reads (lines 41–50):

> `and the result set may be **incomplete AND may contain duplicate
> issues**. `search_issues` does not dedupe; under live-data drift the
> server may have emitted the same issue on multiple pages before the
> cursor repeated. Callers that need strict uniqueness should re-issue
> with `key ASC` as a stable secondary sort in the ORDER BY (append
> `, key ASC` to an existing sort, or use `ORDER BY key ASC` if none —
> JQL allows only one ORDER BY clause) — Atlassian's KB mitigation —
> or dedupe locally.`

Replace with:

> `and the result set may be **incomplete** (pagination was aborted before
> all matching issues were fetched). Duplicates are eliminated client-side
> on this path: `search_issues` applies a per-iteration order-preserving
> deduplication keyed on `issue.key`, so each issue appears at most once
> in `issues`, preserving the order of first occurrence. Callers should
> still prefer `key ASC` in the ORDER BY as the upstream prevention
> (Atlassian KB mitigation) to avoid drift in the first place.`

Also add the note parallel to `KeySearchResult`:

> `Note: as of issue #365, `has_more = true` on the guard-abort path no longer
> implies that `issues` contains duplicates — per-iteration deduplication (keyed
> on `issue.key`) is applied before any break check, so `issues` is always
> duplicate-free on return.`

### `src/api/jira/issues.rs` — function-level rustdoc on `search_issues` (lines 159–165)

The function-level rustdoc for `search_issues` (the comment above
`pub async fn search_issues`) does not currently carry a case-by-case
`has_more` breakdown — that is documented on `SearchResult`. No change to
the function-level rustdoc is required beyond the inline guard comment
below. (The function is documented at the "delegates to `SearchResult`
for the full contract" level.)

### `src/api/jira/issues.rs` — inline guard-block comment in `search_issues` (lines 243–253)

The inline comment inside the repeated-cursor guard block at lines 243–253
currently reads:

> `Guard-aborted: signal incomplete results via has_more=true so
> callers can distinguish "clean exhaustion" from
> "repeated-cursor abort". Matches the `KeySearchResult`
> contract for symmetry; otherwise `SearchResult.has_more`
> would silently be `false` despite the explicit
> "Some results may be missing" warning above. Note: under
> JRACLOUD-95368 (live-data drift, the typical cause), earlier
> pages MAY contain issues that the server would emit again
> after the cursor repeats — `search_issues` does not dedupe.`

Replace with:

> `Guard-aborted: signal incomplete results via has_more=true so callers can
> distinguish "clean exhaustion" from "repeated-cursor abort". As of #365,
> all_issues is already deduplicated (keyed on issue.key) by the incremental
> seen_keys HashSet maintained outside the loop. No additional dedupe
> call is needed here.`

### `src/api/jira/issues.rs` — rustdoc update for `KeySearchResult` (lines 65–115)

Per DP-6: update the rustdoc block at lines 65–115 (the `KeySearchResult`
struct comment) to reflect the new dedupe contract on all paths.
Specifically:

- In the case 2 description (lines 79–90), replace:
  > `and the result set may be **incomplete AND may contain duplicate
  > keys**. `search_issue_keys` does not dedupe; under live-data drift the
  > server may have emitted the same key on multiple pages before the cursor
  > repeated. Callers that need strict uniqueness should re-issue with `key
  > ASC` ... or dedupe locally.`

  with:
  > `and the result set may be **incomplete** (pagination was aborted before
  > all matching keys were fetched). Duplicates are eliminated client-side on
  > this path: `search_issue_keys` applies a per-iteration order-preserving
  > deduplication so each key appears at most once in `keys`, preserving the
  > order of first occurrence. Callers should still prefer `key ASC` in the
  > ORDER BY as the upstream prevention (Atlassian KB mitigation) to avoid
  > drift in the first place.`

- Remove the paragraph starting mid-line-95 with "Today's sole caller
  (`handle_edit::effective_keys` in `cli/issue/create.rs`) requests..." through
  line 106 ending with "...to surface 'incomplete-and-possibly-duplicated' via
  the type system." This paragraph describes the old (pre-dedupe) behavior of
  `handle_edit::effective_keys`. After this change the truncation check IS
  dup-tolerant on all paths.

- Retain BOTH disambiguation sentences at lines 92–95 verbatim (Option A —
  do not elide the middle sentence):
  > `When `limit` is set, callers cannot distinguish case 1 from case 2 from
  > `has_more` alone — the stderr warning fires only in case 2. When `limit`
  > is `None`, case 1 cannot trigger, so `has_more = true` unambiguously
  > signals case 2 (repeated-cursor guard abort).`

  and add: `Note: as of issue #365, `has_more = true` on the guard-abort path
  no longer implies that `keys` contains duplicates — per-iteration
  deduplication is applied before any break check, so `keys` is always
  duplicate-free on return.`

### `src/api/jira/issues.rs` — function-level rustdoc on `search_issue_keys` (lines 277–289)

The function-level rustdoc at lines 277–289 currently says in case (2):
> `**keys may be incomplete AND may contain duplicates** — this function does
> NOT dedupe; callers needing uniqueness should re-issue with `key ASC` as a
> stable secondary sort ... or dedupe locally`

Replace with:
> `**keys may be incomplete** (pagination aborted before all matching keys
> were fetched). Duplicates are eliminated client-side via per-iteration
> order-preserving deduplication — each key appears at most once in `keys`,
> preserving first-occurrence order. Callers should still prefer `key ASC`
> in the ORDER BY as the upstream prevention (Atlassian KB mitigation).`

The sentence "See [`KeySearchResult`] for the full contract including the
caller-dedup analysis." may be retained unchanged (it correctly points to the
updated struct rustdoc).

### `src/api/jira/issues.rs` — inline guard-block comment (lines 366–373)

The inline comment inside the repeated-cursor guard block at lines 366–373
currently says:
> `this function does NOT dedupe. Callers needing strict uniqueness should
> re-issue with `key ASC` as a stable secondary sort ... or dedupe locally.`

Replace the entire comment block (lines 366–373) with:
> `Guard-aborted: signal incomplete results via has_more=true so callers can
> distinguish "clean exhaustion" from "repeated-cursor abort". As of #365,
> all_keys is already deduplicated by the incremental seen_keys HashSet
> maintained outside the loop. No additional dedupe call is needed here.`

### `docs/specs/2026-05-13-search-issue-keys.md` — close-out note

Note planned (to be written in F3 when the implementation lands, not in this
spec file): the "New follow-up (deferred)" bullet in the Out of Scope section
of the parent spec will be marked closed with a back-reference to this spec and
to the closing PR. In addition, three further prose references in the parent
spec become stale when PR #365 lands and must be updated in the same PR.

**F3 implementer instruction:** after landing the implementation, apply ALL
FOUR of the following updates to
`docs/specs/2026-05-13-search-issue-keys.md`:

**(a) Deferred follow-up bullet (line 276) — close it out.**

Replace:

> **New follow-up (deferred):** `feat(search): dedupe keys on repeated-cursor
> guard abort` — ...

with:

> ~~**New follow-up (deferred):** `feat(search): dedupe keys on repeated-cursor
> guard abort`~~ **CLOSED by #365** (2026-05-14). See
> `docs/specs/2026-05-14-search-issue-keys-dedupe.md` and the closing PR.

**(b) Test inventory entry #13 (line 243) — update name, assertion, and description.**

The test was renamed and its assertion flipped by this PR. Replace the
existing entry:

> `test_search_issue_keys_repeated_cursor_abort_does_not_dedupe` — page 1 returns `["X-1"]` with `nextPageToken: "loop"`, page 2 returns `["X-1", "X-2"]` with the same `nextPageToken: "loop"` (simulating live-data drift mid-pagination); asserts `keys == ["X-1", "X-1", "X-2"]` and `has_more == true`. Pins the **no-dedupe contract** under JRACLOUD-95368 abort. *(Added by issue #361; if a future PR adds in-function dedupe, this test must be updated in lockstep with the caller-migration plan.)*

with:

> `test_search_issue_keys_repeated_cursor_abort_dedupes` — mock: page 1
> returns `["X-1"]` with `nextPageToken: "loop"`, page 2 returns
> `["X-1", "X-2"]` with the same repeated token; guard fires. Asserts
> `keys == ["X-1", "X-2"]` and `has_more == true`. Pins the dedupe
> contract under JRACLOUD-95368 abort. **Renamed and assertion flipped
> in #365** (2026-05-14); previous no-dedupe contract superseded.

**(c) Risks bullet "Possible duplicate keys on guard-abort under live-data drift" (line 258) — mark resolved.**

Strike the entire bullet and replace with the resolved-by-#365 bullet:

> ~~- **Possible duplicate keys on guard-abort under live-data drift (new finding from #361 research).** Because JRACLOUD-95368 is the typical cause, `search_issue_keys` *may* return duplicate keys in the abort path (live mutation can produce the same key on two pages before the cursor repeats). `search_issue_keys` does NOT dedupe today; the post-abort rustdoc and inline comment now warn callers explicitly and recommend `ORDER BY key ASC` as the upstream prevention. A separate follow-up issue (#365) tracks in-function dedupe. **Not blocking, but user-visible-but-safe** — the existing single caller (`handle_edit::effective_keys`) has two affected paths: (a) the `+1` lookahead truncation check (`keys.len() > effective_max`) is NOT dup-tolerant — a drift-induced duplicate inflates `keys.len()` by 1 and can spuriously trip the "exceeds --max" error when the true unique-key count is exactly at the limit; (b) bulk-edit calls would apply the same field edit to the same key twice. Both are safe: (a) is a recoverable user-visible error (re-run with `ORDER BY key ASC`), (b) is idempotent at the Jira API for most fields (labels server-side dedupe; assignee/priority/etc are point-set operations). An earlier draft of this PR called this "dup-tolerant by construction" — that overclaim was retracted under Copilot review on PR #364.~~

Replace with:

> - **~~Possible duplicate keys on guard-abort under live-data drift~~ RESOLVED by #365** (2026-05-14) — under JRACLOUD-95368, earlier pages would previously collect duplicate keys before the repeated `nextPageToken` was detected and the guard fired. As of #365, `search_issue_keys` deduplicates on every page-fetch iteration, eliminating duplicates on all exit paths (guard-abort, limit-truncation, cursor-exhaustion). The two previously-affected paths (`+1` lookahead truncation check overcounting and bulk-edit redundant calls) are both fixed. See `docs/specs/2026-05-14-search-issue-keys-dedupe.md`.

**(d) Backwards Compatibility paragraph about `--max` spurious error (line 271) — update.**

The sentence "...under guard abort, `effective_keys` may include duplicates
that spuriously trip the `--max` truncation error or generate redundant
bulk-edit calls (both safe-but-user-visible; tracked in #365)" is no longer
true. Replace with:

> ~~...under guard abort, `effective_keys` may include duplicates that spuriously
> trip the `--max` truncation error or generate redundant bulk-edit calls (both
> safe-but-user-visible; tracked in #365).~~
> **RESOLVED by #365** (2026-05-14) — duplicates are now eliminated
> client-side on all exit paths before the truncation check, so the spurious
> `--max` error is no longer possible on any drift-induced path.

**(e) `search_issues` Out-of-Scope bullet (line 28) — close out the carve-out.**

The Out-of-Scope bullet at line 28 of `docs/specs/2026-05-13-search-issue-keys.md`
that reads "Changing the public signature or behavior of `search_issues` — purely
additive..." contains an embedded `search_issues`-dedupe non-decision. The
`search_issues` no-dedupe stance described there ("no current caller relies on
`result.issues.len()` as a correctness check") is reversed by #365. Append to
that bullet:

> **Follow-up (v0.1.9, issue #365):** `search_issues` dedupe was added
> symmetrically by PR #365 (2026-05-14). The "no-dedupe by design" stance
> documented here is superseded. See `docs/specs/2026-05-14-search-issue-keys-dedupe.md`
> §DP-4 reversal.

**(f) Backwards Compatibility caller list (line 266) — remove `sprint.rs`, correct the three-reader claim.**

The Backwards Compatibility bullet on line 266 of
`docs/specs/2026-05-13-search-issue-keys.md` reads:

> "Three CLI readers (`cli/issue/list.rs`, `cli/board.rs`, `cli/sprint.rs`)
> will display the existing 'Showing N of M' truncation hint..."

`cli/sprint.rs` calls `get_sprint_issues` (Agile API), not `search_issues`.
Replace "Three CLI readers (`cli/issue/list.rs`, `cli/board.rs`, `cli/sprint.rs`)"
with "Two CLI readers (`cli/issue/list.rs`, `cli/board.rs`)" in that sentence
(and in the parallel sentence at line 269, which already correctly identifies
the sprint path as `get_sprint_issues`-based and can remain unchanged). Also
note that the parent spec's line 266 caller list omits `cli/queue.rs` (a
pre-existing `search_issues` caller — see `cli/queue.rs:100-102`). The original
three-reader sentence undercounted; add queue.rs to the corrected sentence as a
third (or fourth, after sprint correction) reader. This corrects a pre-existing
parent-spec omission and is unrelated to the v0.1.10 dedupe scope expansion.

### `.factory/specs/prd/bc-2-issue-read.md` — BC-2.6.050 body update + JRACLOUD-94632 rebind + new BC-2.6.051 (ALL in this PR)

**Semantic anchoring decision (CONCERN-E, Option B adopted):** BC-2.6.050 is
titled and scoped exclusively to `search_issue_keys`. Appending `search_issues`
dedupe behavior to BC-2.6.050 would mis-anchor `search_issues` behavior under
a BC that does not describe it. The existing BC-2.6.047 through BC-2.6.050
series contains no BC that covers `search_issues` dedupe specifically (BC-2.2.029
covers cursor `has_more` semantics; BC-2.6.047 covers story-points field
deserialization; none covers the dedupe contract). Therefore: a new sibling
BC-2.6.051 is created for `search_issues` dedupe. The spec frontmatter is
updated from `traces_to: BC-2.6.050` to `traces_to: [BC-2.6.050, BC-2.6.051]`.

---

**Update 1: Append dedupe behavior to BC-2.6.050 Behavior field (`search_issue_keys` only)**

The BC-2.6.050 entry at lines 491–497 of `.factory/specs/prd/bc-2-issue-read.md`
documents `search_issue_keys` behavior but does not mention deduplication. F3
implementer must update the BC-2.6.050 body in the same PR as the implementation.

Append to the **Behavior** field of BC-2.6.050 (after the existing sentence
ending "...clamps `maxResults` per page to `.min(100)` for parity with
`search_issues`."):

> On every page-fetch iteration, `search_issue_keys` appends only newly-seen
> keys to `all_keys` using an incremental `seen_keys: HashSet<String>` (maintained
> outside the loop) — order-preserving, first-occurrence-wins deduplication
> keyed on the key string. All exit paths (guard-abort, limit-truncation,
> cursor-exhaustion) therefore return a duplicate-free `keys` vec. Introduced
> in #365. For the symmetric `search_issues` dedupe contract, see BC-2.6.051.

---

**Update 2: Rebind JRACLOUD-94632 → JRACLOUD-95368 on line 496 (BOTH occurrences)**

BC-2.6.050 at line 496 of `.factory/specs/prd/bc-2-issue-read.md` references
"JRACLOUD-94632" in TWO places (once in the pagination sentence describing the
anti-loop guard, once in the `(b)` clause of the `has_more` definition). The
correct ticket number is JRACLOUD-95368 (rebind established in issue #361).
F3 implementer must replace **both occurrences** of `JRACLOUD-94632` with
`JRACLOUD-95368` on line 496 in the same PR as the dedupe implementation. A
single sed-style substitution targeting only the first match would silently
leave the second occurrence stale; both must be updated. This costs nothing
extra alongside Update 1 above and eliminates an otherwise untracked carry item.

---

**Update 3: Add new BC-2.6.051 for `search_issues` dedupe (`search_issues` only)**

F3 implementer must add BC-2.6.051 immediately after BC-2.6.050 in
`.factory/specs/prd/bc-2-issue-read.md`:

```
#### BC-2.6.051: `client.search_issues(jql, limit, fields)` deduplicates results on all exit paths (JRACLOUD-95368 mitigation)

**Confidence**: HIGH
**Source**: issue #365; spec at `docs/specs/2026-05-14-search-issue-keys-dedupe.md`
**Subject**: Issue read (API layer — full-body JQL search)
**Behavior**: An incremental `seen_keys: HashSet<String>` maintained outside
the pagination loop ensures only newly-seen issues (keyed on `issue.key`;
`Issue` does not impl `Hash` so key strings are cloned) are appended to
`all_issues`. All exit paths (guard-abort, limit-truncation, cursor-exhaustion)
therefore return a duplicate-free `issues` vec in first-occurrence order.
`SearchResult.has_more` semantics are unchanged — `has_more = true` on
guard-abort regardless of dedupe. The `SearchResult` rustdoc "may contain
duplicate issues" warning is dropped; see Doc and Spec Fallout in
`docs/specs/2026-05-14-search-issue-keys-dedupe.md`. Introduced in #365.
For the symmetric `search_issue_keys` dedupe contract, see BC-2.6.050.
**Trace**: `src/api/jira/issues.rs::search_issues` (impl); `cli/issue/list.rs`,
`cli/board.rs`, `cli/queue.rs` (callers); `tests/rate_limit_cap_tests.rs` or
`tests/search_issues_dedupe.rs` (new dedupe tests added by #365)
```

### Update 4: BC catalog count propagation (CRITICAL — pre-merge gate)

**F3 implementer MUST apply ALL of the following BC catalog edits in the same PR
as the implementation, then run `scripts/check-spec-counts.sh` BEFORE pushing.**
The script exits 1 with specific mismatch details if frontmatter counts drift from
body-heading counts (DRIFT-001 mitigation per CLAUDE.md).

All line numbers are approximate — confirm exact positions with the Read tool
before editing.

**`.factory/specs/prd/bc-2-issue-read.md`** — apply alongside adding the
BC-2.6.051 body entry (`#### BC-2.6.051:` heading) to this file:

| Location | Before | After |
|---|---|---|
| Line 4 frontmatter `total_bcs` | `total_bcs: 92` | `total_bcs: 93` |
| Line 5 frontmatter `definitional_count` | `definitional_count: 50` | `definitional_count: 51` |
| Line 511 footer | `## Total BCs in this file: 50 (representative set; BC-INDEX.md carries all 92)` | `## Total BCs in this file: 51 (representative set; BC-INDEX.md carries all 93)` |

**`.factory/specs/prd/BC-INDEX.md`** — apply alongside the above:

| Location | Before | After |
|---|---|---|
| Line 4 frontmatter `total_bcs` | `total_bcs: 546 ... +1 added 2026-05-13 (BC-2.6.050, issue #350)` | append `; +1 added 2026-05-14 (BC-2.6.051, issue #365)` and change `546` → `547` |
| Line 9 `sections:` entry | `bc-2-issue-read.md (92 BCs cumulative; 50 individually-bodied)` | `(93 BCs cumulative; 51 individually-bodied)` |
| Section 2 header | `## Section 2: Issue Read (bc-2-issue-read.md) — 92 BCs cumulative; 50 individually-bodied` | `93/51` |
| 2.6 subsection header | `### 2.6 API Layer (4 BCs: BC-2.6.047..050)` | `(5 BCs: BC-2.6.047..051)` |
| After BC-2.6.050 row | _(no row)_ | New row: `\| BC-2.6.051 \| \`client.search_issues(jql, limit, fields)\` deduplicates results on all exit paths (JRACLOUD-95368 mitigation) \| — (issue #365) \| tests/rate_limit_cap_tests.rs; tests/search_issues_dedupe.rs \| HIGH \|` |
| Totals table `2: Issue Read` row | `92 \| 50` | `93 \| 51` |
| Totals table `\*\*Total\*\*` row | `\*\*546\*\* \| \*\*314\*\*` | `\*\*547\*\* \| \*\*315\*\*` |
| Canonical total note | `Canonical total is \*\*546\*\*` | `\*\*547\*\*` (+ append `+1 BC-2.6.051 added 2026-05-14 via issue #365`) |
| Cumulative ≠ individually-bodied sentence | `Cumulative total (546) ≠ individually-bodied count (314)` | `547 ≠ 315` |

After applying all edits above, run `scripts/check-spec-counts.sh`. It must exit
0 before the PR is pushed.

---

### `CLAUDE.md` — updated

The existing `CLAUDE.md` gotcha for JRACLOUD-95368 was updated in this PR to
reflect that both `search_issue_keys` and `search_issues` now use an incremental
`seen_keys` HashSet to deduplicate on all exit paths (implemented by issue #365).
The gotcha text now states that `search_issue_keys`
and `search_issues` return duplicate-free results and that the `#365` tracking
item is closed. Earlier spec drafts said "no update required" because the dedupe
was initially scoped as an implementation detail of `search_issue_keys` alone;
the scope expanded to `search_issues` as well, and the CLAUDE.md gotcha is the
canonical architectural reference for maintainers — it must stay in lockstep.

### BC-2.6.050 Trace field — pre-existing stale tech debt (NIT-4, process-gap, do not fix here)

The `Trace` field of BC-2.6.050 in `.factory/specs/behavioral-contracts/BC-2.6.050.md`
references a test count that was already stale before this feature and becomes
more stale with the new tests added by #365. This is a **pre-existing open
issue** (parallel to the JRACLOUD-94632 attribution drift handled by Observation-1 above).
The BC Trace field is NOT updated in this spec — it requires a dedicated
maintenance sweep under the VSDD factory process. Tracked as pre-existing tech
debt; do not conflate with the dedupe implementation deliverables.

**Drift Item target (PG-365-1):** "Next BC catalog sweep / BC-2 maintenance
pass". Open-ended deferrals decay; this target gives the deferral an owning
epic so it does not drift further. State-manager will write the STATE.md Drift
Items entry separately.

## Risks

1. **Allocation cost (single HashSet per call, negligible).**
   The incremental `seen_keys: HashSet<String>` is allocated once per
   function call (outside the loop) and grows as unique keys are inserted.
   For typical operation (≤10 pages, ≤100 items/page), the HashSet holds
   at most 1001 entries (BULK_MAX_KEYS=1000 + one-over-fetch) — confirmed:
   `BULK_MAX_KEYS` is defined as `1000` at `src/api/jira/bulk.rs:37`. For
   Jira keys (7–12 ASCII chars), each String clone is a small heap allocation.
   Total allocation is approximately 24 KB in the worst case for
   `search_issue_keys` and ~25 KB for `search_issues` (which clones key
   strings from Issue structs, not Issue structs directly) — negligible on
   any modern system. The HashSet lives for the duration of the function call
   and is dropped after the loop completes. Normal (non-drift) pages produce
   no duplicates so insert attempts on seen keys return false immediately
   (no structural work). Risk: negligible. Accepted.

2. **~~Behavioral asymmetry between `search_issue_keys` (dedupes) and
   `search_issues` (does not).~~** **RESOLVED in v0.1.9.**
   Both functions now apply per-iteration dedupe symmetrically (DP-4
   reversed). The inline asymmetry comment in `search_issues` (which cited
   #365 for intentional no-dedupe) is replaced by the symmetric
   implementation. No asymmetry remains. Risk: closed.

3. **Future caller assumes "guard-abort + has_more=true" implies duplicates.**
   The old rustdoc explicitly warned "may contain duplicate keys" under
   case 2. After this change, the new rustdoc states the opposite: all paths
   guarantee no duplicates. A caller who reads old docs and applies a defensive
   local dedupe will do unnecessary work but not produce wrong results. The
   rustdoc update (see Doc and Spec Fallout) addresses this. Risk: low.

4. **Incremental `seen_keys` insert requires one `clone()` per key candidate.**
   `HashSet<String>::insert` requires ownership, so each key is cloned once when
   passed to `insert`. Unique keys are cloned once (for `seen_keys`) and moved
   into `all_keys`; duplicate keys are cloned once (for the `insert` call) and
   then dropped. An alternative using `HashSet<&str>` would require lifetime
   management across the mutable borrow on `all_keys`. The clone cost per
   iteration is bounded by page size (≤ 100 items). Risk: negligible.

5. **Per-iteration dedupe weakens the Apr 2025 maxResults-overshoot detector
   in a narrow triple-collision corner — applies to both functions.**
   The `all_keys.len() > max` / `all_issues.len() > max` check at
   `issues.rs:332-346` (keys) and the equivalent in `search_issues` was
   originally added to detect the Apr 2025 server-side regression (server
   returns >maxResults rows AND sets `isLast:true` — the overshoot itself
   proves more data existed). Per-iteration dedupe applied before this check
   collapses the overshoot when the extra row happens to be a drift-induced
   duplicate of a prior key/issue. In that triple-collision (Apr 2025
   regression + overshoot item is a drift duplicate + `isLast:true`),
   `has_more` will report `false` even if real additional pages exist
   server-side. The `page_has_more` arm of the OR check still fires whenever
   the server actually issues a `nextPageToken`, so the collision only silences
   the signal in the specific `nextPageToken-absent + drift-duplicate-overshoot`
   corner (the server typically signals this with `isLast:true`, which our
   client does not deserialize — the client relies on `nextPageToken` absence
   via `next_page_token.is_some()`). Mitigation for users hitting this:
   re-issue the query with `, key ASC` appended to the ORDER BY (the standard
   JRACLOUD-95368 mitigation already documented). Accepted trade-off: this
   corner is rarer than the bug Option A closes. Likelihood: low (Apr 2025
   regression is rare; coincidence with a drift-duplicate-overshoot is rarer
   still). The new limit-truncation tests (`test_search_issue_keys_limit_truncation_dedupes_under_drift`
   and `test_search_issues_limit_truncation_dedupes_under_drift`, defined in
   the Test Diff section above) exercise the case where per-iteration dedupe
   brings the accumulated count STRICTLY BELOW `max`, so the truncation block
   is not entered at all — the loop exits via cursor-exhaustion. The `len > max`
   arm of the Apr 2025 overshoot detector is now explicitly exercised by the
   regression-pin test `test_search_issue_keys_apr2025_overshoot_silenced_by_drift_dedupe`
   (see Test Diff section — **NEGATIVE-PIN test**). The `len == max` arm
   (exact capacity without overshoot) remains theoretically described and
   unexercised by any test in this PR. Readers should note that REAL upstream
   data beyond the deduped page would remain undetected in the triple-collision
   scenario. The regression-pin test documents this explicitly so any future
   PR that changes `has_more` behavior in this corner is forced to update the
   test in lockstep.

## Backwards Compatibility

The `search_issue_keys` public API (`KeySearchResult { keys, has_more }`)
is unchanged. Behavioral changes are confined to drift scenarios on all
break-decision paths:

- **`jr issue edit --jql ... --max N`** (`handle_edit::effective_keys`): the
  sole caller. On the guard-abort path AND the limit-truncation path, the user
  previously saw a spurious "JQL matched at least N issues, which exceeds --max
  M" error when the true unique-key count was exactly at the limit but drift
  duplicates inflated `keys.len()`. After this change, the user sees the
  correct behavior: no spurious error, the edit proceeds with the unique set.
  The stderr drift warning (guard-abort path only) remains.

- **No flag, JSON shape, or exit-code changes.** The `--dry-run` path,
  `--output json` path, and all non-drift paths are unaffected.

- **`search_issues` dedupe (v0.1.9 addition).** CLI callers that use
  `search_issues` — `jr issue list --jql` (`list.rs`), `jr board view`
  (`board.rs`), and `jr queue view` (`queue.rs`) — benefit transparently.
  Note: `jr sprint current` uses `get_sprint_issues` (Agile API endpoint
  `/rest/agile/1.0/sprint/{id}/issue`) and is NOT a `search_issues` caller;
  it is unaffected by this dedupe.
  `jr queue view` calls `client.search_issues(&jql, Some(keys.len() as u32), &[])`
  (`cli/queue.rs:100-102`) with `limit = keys.len()` on a `key IN (...)`
  JQL query, then reorders results by queue position. Under JRACLOUD-95368
  drift, drift duplicates can accumulate before the guard fires; dedupe
  collapses them. The `reorder_by_queue_position` call uses HashMap-based
  position lookup with `usize::MAX` for missing keys — it is dup-tolerant
  and benefits transparently. None of these callers performs an
  `issues.len() > effective_max`-style check that would be affected by
  de-inflation of the count. The `has_more` signal is unchanged — it remains
  `true` on guard-abort. The rustdoc "may contain duplicate issues" warning
  is dropped (see Doc and Spec Fallout). No CLI flag, JSON shape, or
  exit-code changes.

- **No other callers exist today.** If a new caller is added after this
  change, it will see the deduped contract on all paths for both functions,
  which is strictly better (fewer or equal items, no duplicates, correct count).

## References

- Research validation report: `.factory/research/issue-365-design-validation.md`
- GitHub issue #365: <https://github.com/Zious11/jira-cli/issues/365>
- Parent feature spec (#350): `docs/specs/2026-05-13-search-issue-keys.md`
- Parent PR (#362, closing #350): <https://github.com/Zious11/jira-cli/pull/362>
  (PR #364 = follow-up `has_more` backport to `search_issues`; #362 = original keys-only method)
- JRACLOUD-95368: <https://jira.atlassian.com/browse/JRACLOUD-95368>
  ("nextPageToken pagination is not snapshot-stable under live mutation")
- Atlassian KB on inconsistent paginated JQL results:
  <https://support.atlassian.com/jira/kb/inconsistent-paginated-api-search-result-while-using-jql/>
- `std::vec::Vec::retain` documentation:
  <https://doc.rust-lang.org/std/vec/struct.Vec.html#method.retain>
- `std::collections::HashSet::insert` documentation (returns `bool`, `true`
  if the value was not present — the semantic `Vec::retain` relies on):
  <https://doc.rust-lang.org/std/collections/struct.HashSet.html#method.insert>
- `search_issue_keys` test file: `tests/search_issue_keys.rs`
- `search_issues` callers: `cli/issue/list.rs`, `cli/board.rs`,
  `cli/queue.rs:100-102` (`jr queue view` — `key IN (...)` JQL with
  `limit = keys.len()`, result reordered by queue position)
- `search_issues` test file: `tests/rate_limit_cap_tests.rs`
  (existing `search_issues` library tests; new `search_issues` dedupe tests
  go here or in a new `tests/search_issues_dedupe.rs` at the implementer's
  discretion)

## Blockquote Audit Table

External-source blockquotes quoted verbatim in this spec. Adversary can verify
citations exist (but cannot re-fetch external URLs — see PG-365-2 in Out of
Scope). Spec-internal paraphrase blocks (`> **Implementer note:**`,
`> **Implementation note:**`) are NOT listed here — they are spec-authored text.

| # | Location in spec | Source type | External source | Verbatim claim | Verifiable without WebFetch? |
|---|-----------------|-------------|-----------------|----------------|------------------------------|
| 1 | Validated API Facts — dedupe algorithm comparison table | Research report | `.factory/research/issue-365-design-validation.md` §Q1 | Alloc sizes and `indexmap`/`itertools` MSRV figures | Yes — local file |
| 2 | Validated API Facts — `Vec::dedup()` is explicitly wrong | Research report | `.factory/research/issue-365-design-validation.md` §Q1 | "is consecutive-only and JRACLOUD-95368 drift can emit the same key non-consecutively across pages" | Yes — local file |
| 3 | Validated API Facts — Research caveat | Research report + docs.rs WebFetch | `docs.rs/itertools` — `Itertools::unique` documentation | "`itertools::Itertools::unique` documentation is unambiguous: it is a global (non-consecutive) dedupe with `Clone + Eq + Hash` bounds" | No — requires WebFetch (PG-365-2) |
| 4 | Doc and Spec Fallout — `SearchResult` rustdoc case 2 (current text) | Source code | `src/api/jira/issues.rs` lines 41–50 | "and the result set may be **incomplete AND may contain duplicate issues**..." | Yes — local file |
| 5 | Doc and Spec Fallout — `search_issues` guard comment (current text) | Source code | `src/api/jira/issues.rs` lines 243–253 | "Guard-aborted: signal incomplete results via has_more=true so callers can distinguish..." | Yes — local file |
| 6 | Doc and Spec Fallout — `KeySearchResult` rustdoc case 2 (current text) | Source code | `src/api/jira/issues.rs` lines 79–90 | "and the result set may be **incomplete AND may contain duplicate keys**..." | Yes — local file |
| 7 | Doc and Spec Fallout — `search_issue_keys` function rustdoc (current text) | Source code | `src/api/jira/issues.rs` lines 280–289 | "**keys may be incomplete AND may contain duplicates** — this function does NOT dedupe..." | Yes — local file |
| 8 | Doc and Spec Fallout — inline guard comment in `search_issue_keys` (current text) | Source code | `src/api/jira/issues.rs` lines 366–373 | "this function does NOT dedupe. Callers needing strict uniqueness should..." | Yes — local file |
| 9 | Test Diff — Test 13 before/after assertions | Source code | `tests/search_issue_keys.rs` line 307 block | `assert_eq!(result.keys, vec!["X-1".to_string(), "X-1".to_string(), "X-2".to_string()], ...)` | Yes — local file |
| 10 | Test Diff — `search_issues` existing test reference | Source code | `tests/rate_limit_cap_tests.rs` lines 345–403 | `test_search_issues_repeated_cursor_abort_sets_has_more_true` mock structure and assertions | Yes — local file |

**Note on row 3:** the `itertools::unique` claim was validated by research-agent WebFetch
against `docs.rs`; the adversary cannot re-verify without WebFetch (read-only tool
profile). The claim is recorded here; its correctness is trusted from research-agent output
(PG-365-2 describes the trust boundary). The claim does NOT affect the chosen algorithm
(incremental HashSet insert wins regardless).
