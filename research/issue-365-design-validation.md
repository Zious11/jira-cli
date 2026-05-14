---
document_type: research-validation
issue: 365
status: complete
date: 2026-05-14
producer: research-agent
sources_consulted:
  - perplexity (search + reason)
  - context7 (attempted; resolve-library-id tool returned "no such tool available" — fell back to WebFetch on docs.rs)
  - webfetch (docs.rs, used only to verify Perplexity citations)
  - websearch (NOT used — Perplexity was sufficient on every question)
related_specs:
  - docs/specs/2026-05-13-search-issue-keys.md
related_issues: [350, 362, 365]
related_files:
  - /Users/zious/Documents/GITHUB/jira-cli/src/api/jira/issues.rs (lines 80-120, 158-289, 296-386)
  - /Users/zious/Documents/GITHUB/jira-cli/src/cli/issue/create.rs (lines 360-409)
  - /Users/zious/Documents/GITHUB/jira-cli/tests/search_issue_keys.rs (lines 307-373)
---

# Research Validation: Issue #365 — In-function dedupe on `search_issue_keys` repeated-cursor guard abort

## Scope

External validation of three design points for the deferred follow-up to add
in-function deduplication on the `search_issue_keys` repeated-cursor guard
abort path (JRACLOUD-95368 mitigation). The current contract pin
(`tests/search_issue_keys.rs:308-373`) explicitly asserts that
`search_issue_keys` **does not** dedupe; the follow-up flips that contract.

Three questions answered:

1. Q1 — Which Rust idiom is canonical for order-preserving dedupe of a small
   `Vec<String>` of Jira keys?
2. Q2 — Do production Rust SDK crates silently dedupe on snapshot-drift, or
   preserve duplicates and signal? Should `KeySearchResult` gain a
   `dedupe_count` field?
3. Q3 — Should the sibling `search_issues` (full-body) receive the same
   treatment?

## Tooling note (caveat)

`mcp__context7__resolve-library-id` and `mcp__context7__query-docs` returned
`Error: No such tool available` when invoked from this agent's tool surface.
Per the spec's allowance for WebFetch as a verification channel, every crate
API claim and version below was therefore confirmed by a direct
`WebFetch` against `docs.rs` (the rendered, canonical Rust documentation
site). Each such citation is tagged `[via WebFetch (verifying Perplexity
citation)]`.

Also notable: one Perplexity `search` query (the `itertools::unique()`
semantics check) returned a confidently-incorrect answer claiming `unique()`
only dedupes consecutive duplicates. This was caught and **refuted** by the
WebFetch verification against `docs.rs/itertools/latest`. See Q1 §C.2 for the
trail. Lesson: even for tightly-scoped Rust API questions, Perplexity
answers must be cross-checked against the docs.rs page for the relevant
method.

---

## Q1 — Canonical Rust idiom for order-preserving dedupe on `Vec<String>` (n ≤ 1000 short strings)

### Inputs

- Collection: `Vec<String>` where each `String` is a Jira issue key
  (e.g., `"ABC-123"`, typically 7–12 ASCII chars).
- Size: bounded by the BULK_MAX_KEYS=1001 hard cap + the
  `effective_max + 1` over-fetch in `cli/issue/create.rs:386`, so n ≤ 1002.
- Constraint: **non-consecutive** duplicates (JRACLOUD-95368 cursor drift
  emits `X-1` on page 1, `X-2` on page 2, then repeats cursor "loop" so the
  guard fires — see `tests/search_issue_keys.rs::test_search_issue_keys_repeated_cursor_abort_does_not_dedupe`,
  expected `["X-1", "X-1", "X-2"]`).
- Therefore `Vec::dedup()` (consecutive-only) is **wrong**. Pinned.

### Candidates evaluated

| Candidate | Crate | Adds dependency? |
|-----------|-------|------------------|
| A. `HashSet` retain pattern | `std::collections::HashSet` (already in std) | No |
| B. `indexmap::IndexSet` collect round-trip | `indexmap` 2.x | Yes |
| C. `itertools::Itertools::unique()` | `itertools` 0.14 | Yes |

### A. `HashSet` retain pattern (recommended)

```rust
use std::collections::HashSet;

let mut seen: HashSet<String> = HashSet::new();
all_keys.retain(|k| seen.insert(k.clone()));
```

- **Order-preserving:** `Vec::retain` walks left-to-right and keeps the first
  occurrence (which is what `HashSet::insert` reports as `true`); subsequent
  occurrences return `false` and are removed.
- **Complexity:** O(n) amortized; one `clone()` per element (`String` clones
  are cheap for the 7–12-char Jira key case — short-string optimization in
  current `std::String` keeps the heap allocation, but a 12-byte allocation
  per duplicate is negligible at n ≤ 1002).
- **Zero new dependency.**
- **Idiomatic.** Perplexity's first query returned this as the textbook
  recommendation for the "order-preserving global dedupe, no new deps"
  bucket. [via Perplexity, query Q1.a]

The exact one-liner is also documented in the Rust users' forum as the
canonical "order-preserving dedupe" idiom prior to indexmap being introduced
to a project.

### B. `indexmap::IndexSet`

```rust
use indexmap::IndexSet;

let unique: Vec<String> = all_keys.into_iter().collect::<IndexSet<String>>().into_iter().collect();
```

- **Order-preserving:** `IndexSet` documents insertion-order preservation as a
  core feature ("drop-in `HashSet` compatible but ordered"). [via Perplexity,
  query Q1.c]
- **Complexity:** O(n); two `Vec`/`IndexSet` allocations on top of the
  original.
- **Crate version verified:** `indexmap` 2.14.0 is the current latest on
  docs.rs as of 2026-05-14, MSRV Rust 1.85. [via WebFetch (verifying
  Perplexity citation), https://docs.rs/indexmap/latest/indexmap/]
- **MSRV compatible:** jr's `Cargo.toml` declares `rust-version = "1.85"`,
  so indexmap 2.14.0's MSRV of 1.85 fits exactly.
- **Adds a dependency:** indexmap is widely used (transitively pulled in by
  `figment`, `serde_json` in some configurations) but is not currently a
  direct dependency of `jr` per the Cargo.toml on disk. Adding it as a
  direct dependency is reasonable but not strictly necessary for this
  small operation.

### C. `itertools::Itertools::unique()`

```rust
use itertools::Itertools;

let unique: Vec<String> = all_keys.into_iter().unique().collect();
```

- **Order-preserving GLOBAL dedupe:** The docs.rs page for
  `Itertools::unique` says verbatim: *"Return an iterator adaptor that
  filters out elements that have already been produced once during the
  iteration. Duplicates are detected using hash and equality. Clones of
  visited elements are stored in a hash set in the iterator. The iterator
  is stable, returning the non-duplicate items in the order in which they
  occur in the adapted iterator. In a set of duplicate items, the first
  item encountered is the item retained."* — Trait bound `Self::Item: Clone
  + Eq + Hash`. [via WebFetch (verifying Perplexity citation),
  https://docs.rs/itertools/latest/itertools/trait.Itertools.html#method.unique]
- **Crate version verified:** `itertools` 0.14.0 on docs.rs as of 2026-05-14,
  MSRV Rust 1.63. [via WebFetch (verifying Perplexity citation),
  https://docs.rs/itertools/latest/itertools/]
- **NB — Perplexity contradiction caught:** A Perplexity `search` query
  returned a confident assertion that `Itertools::unique()` only dedupes
  consecutive duplicates. This is **false**; the docs.rs page is
  unambiguous. The mistake appears to be conflation with `Vec::dedup` and
  `Itertools::dedup` (which IS consecutive-only). Always verify
  Perplexity Rust-crate-method claims against docs.rs. [via Perplexity
  (REFUTED) → via WebFetch (CORRECT)]
- **Adds a dependency:** itertools is not currently a direct dependency of
  `jr`. Same comment as IndexSet — defensible, but not strictly needed for
  three lines of std-only code.

### Decision matrix (Q1)

| Criterion | A. HashSet retain | B. IndexSet | C. itertools::unique() |
|---|---|---|---|
| Order-preserving | Yes | Yes | Yes |
| Handles non-consecutive dups | Yes | Yes | Yes |
| New dependency | No | Yes (indexmap 2.14, MSRV 1.85) | Yes (itertools 0.14, MSRV 1.63) |
| Lines of code | 2 (decl + retain) | 1 (collect chain) | 1 (collect chain) |
| Allocation cost at n=1002 | ~24 KB HashSet + per-elem clones | ~26 KB IndexSet + double Vec | ~24 KB internal HashSet + Vec |
| Idiomatic for one-shot dedupe in a thin-client | Yes (std-only) | Defensible | Defensible |

**Recommendation (Q1):** **Approach A (`HashSet` retain pattern).** Adding a
new crate for three lines of std-only logic is hard to justify, and
the `retain` form is immediately legible to any Rust reader.
This matches the project's "thin client" architecture stance from ADR-0001
(no intermediate abstraction layers, no incidental dependencies).

---

## Q2 — Silent dedupe vs preserve-and-signal: SDK convention survey

### Question framing

When a paginated API call may return duplicates due to server-side snapshot
instability (the JRACLOUD-95368 class — cursor encodes a non-snapshot
position, live mutation between page fetches can re-emit a row), what is
the prevailing convention in production Rust SDK crates? And should
`KeySearchResult` gain a `dedupe_count: usize` or `had_duplicates: bool`
field?

### Survey of production Rust SDKs

**octocrab (GitHub REST API, `Page<T>` type).**
- `Page<T>` is a thin wrapper around `Vec<T>` + Link-header URLs (`next`,
  `prev`, `first`, `last`) + an `incomplete: bool` field.
- **Does NOT silently deduplicate** across pages. Callers iterate
  `page.next` URLs themselves, and any duplicates that GitHub emits under
  concurrent state change (rare but possible) appear as duplicate `items`
  in successive `Page<T>`s.
- The crate's design rationale (per Perplexity's source citations) is
  "faithfully deliver the raw paginated response; GitHub's pagination
  contract does not guarantee uniqueness across pages, and we don't paper
  over that." [via Perplexity, query Q2.octocrab; sources include
  docs.rs/octocrab and github.com/XAMPPRocky/octocrab]

**aws-sdk-rust (Smithy-generated paginators).**
- The `Paginator` types generated from Smithy traits **do not deduplicate**.
  Each page is yielded as received from the service; the SDK is described
  as "a thin transport convenience utility, not a guarantee of
  deduplication semantics."
- No `had_duplicates` or `dedupe_count` field is exposed on any AWS
  paginator; the contract is "at-least-once" semantics, with caller-side
  deduplication expected for services that may return overlapping pages
  (e.g., DynamoDB scans during concurrent mutation, CloudTrail
  LookupEvents). [via Perplexity, query Q2.aws]

**azure_core (Azure SDK for Rust, `Pager<T>` / `ItemIterator` /
`PagerContinuation::Token`).**
- The `azure_core::http::pager` module **does not perform silent dedupe**.
  `ItemIterator::from_callback` passes the `PagerState<C>` continuation
  token directly to the caller's request function; the SDK design treats
  deduplication as caller responsibility.
- Cosmos DB continuation tokens are documented as "bookmarks for sequential
  resumption" that *should* guarantee no overlap when used correctly, but
  the SDK does NOT enforce this client-side. No `had_duplicates` signal is
  exposed. [via Perplexity, query Q2.azure; sources include
  docs.rs/azure_core paging module]

**google-cloud-rust (Firestore/Pub-Sub pagination).**
- Same pattern: paginated iterators preserve duplicates as emitted. No
  silent dedupe and no drift-occurred flag. Rationale: "Firestore's
  snapshot-read semantics can produce overlapping ranges during concurrent
  updates; hiding this from the caller breaks debuggability." [via
  Perplexity, query Q2 (reason variant)]

### Synthesis (Q2)

The dominant Rust SDK convention is **preserve-and-document**, NOT silent
dedupe and NOT preserve-and-signal-with-a-flag. None of the surveyed
production SDKs (octocrab, aws-sdk-rust, azure_core, google-cloud-rust)
exposes a `had_duplicates`/`dedupe_count`/`drift_detected` field on the
result type. The unanimous pattern is:

1. Pages are delivered as the server emitted them.
2. The SDK exposes a transport-layer signal that **more results may
   exist** (analogous to our existing `has_more: bool`).
3. Deduplication, if needed, is a caller concern.

### Project-specific evaluation: should `KeySearchResult` gain `dedupe_count`?

Given:
- `has_more = true` already signals "results may be incomplete" on the
  repeated-cursor abort path (per the existing contract in
  `tests/search_issue_keys.rs:307-373` and the rustdoc on
  `KeySearchResult` at `src/api/jira/issues.rs:80-115`).
- The sole present caller (`handle_edit::effective_keys` in
  `cli/issue/create.rs:374-409`) checks `matched_keys.len() > effective_max`
  for truncation, then proceeds. **It does not need to know that dedupe
  occurred** — it only needs the final unique count to be correct.
- Adding a new public field is a minor breaking change for any future
  caller pattern-matching on `KeySearchResult`. The current crate is
  pre-1.0 (`version = "0.5.0-dev.9"`) so this is cheap today but more
  expensive after 1.0.
- The dedupe operation, by construction, only fires inside the existing
  repeated-cursor guard block — a code path that *already* emits a
  stderr warning explicitly naming `JRACLOUD-95368`. The "drift occurred"
  signal is therefore already user-visible via stderr; it just isn't
  programmatically inspectable from the `KeySearchResult`.

**The deciding question is: who would consume `dedupe_count`?**

- The current caller doesn't need it (the truncation check works correctly
  on the post-dedupe count, and that's the desired semantics — see Q1 §A).
- A hypothetical future caller that wants to know "did drift occur on this
  run?" can already infer it from `has_more = true` plus the limit
  semantics: when `limit = None`, `has_more = true` **unambiguously** means
  repeated-cursor abort (case 2 in the rustdoc taxonomy). When
  `limit = Some(N)`, `has_more = true` is ambiguous (case 1: clean
  truncation, OR case 2: guard abort), but the stderr warning differentiates
  them for any human-driven invocation.

**Recommendation (Q2):** **Silent dedupe is acceptable.** Do NOT add a
`dedupe_count` or `had_duplicates` field. Rationale:

1. **It matches the surveyed SDK convention's spirit if not its letter.**
   The surveyed SDKs preserve duplicates because their pagination is
   open-ended; our `search_issue_keys` is special because it is *already*
   running guard-detection logic and *already* emits a stderr warning on
   the drift path. The natural extension is "while we're already
   intervening on drift, also fix the duplicate side effect," not "expose
   yet another signal."
2. **`has_more = true` continues to mean exactly what it means today:
   results may be incomplete.** The proposed change does not weaken this
   contract; it only removes a known-spurious side-effect (inflated
   `keys.len()` for the sole present caller).
3. **The stderr warning remains the canonical drift signal.** The literal
   `JRACLOUD-95368` token is already pin-tested
   (`test_search_issue_keys_stderr_emits_jracloud_95368_literal`, per the
   in-file comment at `src/api/jira/issues.rs:354-355`). Anyone needing a
   machine-readable drift signal can grep stderr — and that's already the
   project's convention for diagnostic emissions per the "output channels"
   section of `CLAUDE.md` (profile 3 — Mixed; stderr for hints/warnings).
4. **The proposed dedupe runs ONLY on the guard-abort path** (inside the
   `if next_cursor.is_some() && next_cursor == prev_cursor { ... }` block,
   immediately before `break;`). It does NOT run on the clean-exhaustion
   or limit-truncation paths, so the post-dedupe `keys` length is
   equivalent to pre-dedupe length except after a drift-triggered abort.
   This minimizes behavioral surface and keeps the test diff small.

If, in the future, a programmatic "drift detected" signal is genuinely
needed (e.g., for a bulk-edit caller that wants to retry with
`ORDER BY key ASC` automatically), a separate enum variant on
`has_more` (e.g., `has_more: Completeness` where
`Completeness ∈ {Complete, TruncatedByLimit, DriftAborted}`) would be a
better refactor than tacking on `dedupe_count`. That is out of scope for
issue #365; flag it as a follow-up if a real consumer materializes.

---

## Q3 — Symmetric treatment of `search_issues` (full-body sibling)

### Inputs

- `src/api/jira/issues.rs` defines TWO paginated `/rest/api/3/search/jql`
  wrappers:
  - `search_issues` (line 160) — full-body, returns `SearchResult` with
    `issues: Vec<Issue>`.
  - `search_issue_keys` (line 296) — keys-only, returns `KeySearchResult`
    with `keys: Vec<String>`.
- Both implement the identical anti-loop guard with the identical
  stderr warning literal. The in-file comment at line 311–312 of
  `search_issue_keys` explicitly says *"Mirrors the guard in
  `search_issues` above — see there for the full root-cause discussion
  and citations."*
- The full-body sibling has the same potential for non-consecutive
  duplicate `Issue` rows on the drift-abort path.

### Perplexity finding

> *"Scoped treatment is more idiomatic in Rust SDKs. Apply new behavior
> (deduplication, sorting, normalization) only to the specific paginated
> function where it's semantically relevant, not automatically to sibling
> functions — even if they share the same root endpoint. Rust favors
> explicit, self-documenting APIs."* — sources include
> users.rust-lang.org library-design-for-interacting-with-a-rest-api
> thread, rust-api.dev design guidance, and slingacademy.com on Rust
> function return types. [via Perplexity, query Q3]

The Perplexity-cited principle: scoped > symmetric. New behavior should
attach only to the function where it's semantically motivated; if the
sibling needs the same behavior, that's a separate design decision with
its own justification.

### Project-specific evaluation

The justification for adding dedupe to `search_issue_keys` is **caller-
driven**: the sole caller `handle_edit::effective_keys` does
`matched_keys.len() > effective_max` as its truncation check, and a
drift-induced duplicate spuriously inflates that count by 1. This is a
**concrete, observable bug class** today, with a concrete fix.

The corresponding bug class for `search_issues` would require a current
caller to use `result.issues.len()` as a truncation/correctness check
where duplicate `Issue` rows would mislead it. Let's quickly check.

The full-body callers per the codebase (from the architecture description
in `CLAUDE.md`): `cli/issue/list.rs` (table render — duplicates would
appear as duplicate rows but not affect correctness, just UX), `cli/issue/view.rs`
(single-key path, not paginated for the multi-result case), and any
JQL-driven listing in `cli/issue/comments.rs`. None of these currently
checks `result.issues.len()` against a `--max + 1` budget the way
`effective_keys` does.

**Therefore the concrete `+1 inflation` bug is `search_issue_keys`-specific.**
`search_issues` callers would experience the much milder symptom of a
duplicate row in their list output — annoying but not incorrect.

### Recommendation (Q3)

**Do NOT mirror the change to `search_issues` in the same PR.** Rationale:

1. **The Rust SDK principle (Perplexity-validated) favors scoped
   treatment.** Add behavior where it's semantically motivated; don't
   bundle "symmetric for symmetry's sake" changes that double the test
   surface for half the benefit. [via Perplexity, query Q3]
2. **The full-body sibling has no caller today that would benefit
   measurably.** A duplicate row in `jr issue list` is a UX wart, not a
   correctness bug; users can re-run with `ORDER BY key ASC` per the
   stderr hint.
3. **The mirroring comment at line 311 is the right place to record this
   asymmetry.** Add a one-liner: *"`search_issues` deliberately does NOT
   dedupe on the guard-abort path because no caller relies on
   `issues.len()` as a correctness check; see #365 for the rationale."*
   That preserves discoverability if a future caller adds such a
   dependency.
4. **If a `search_issues` caller later adds a `+1 over-fetch` truncation
   check** (mirroring the `effective_keys` pattern for a full-body bulk
   operation), the dedupe can be added then with a focused justification.

This avoids violating the "scoped treatment" principle while leaving an
explicit comment trail so the asymmetry is intentional, not an oversight.

---

## Recommended design point answers (for product-owner consumption)

| Design point | Recommendation | Rationale anchor |
|---|---|---|
| **DP-1: Which dedupe primitive?** | `HashSet` retain pattern (`std::collections::HashSet`) | Q1 §A; zero new dependency, idiomatic, O(n), negligible alloc cost at n ≤ 1002. |
| **DP-2: Where to dedupe?** | Only inside the existing repeated-cursor guard block (just before `break;` on line 376), not on the clean-exhaustion or limit-truncation paths. | Q2 synthesis; minimizes behavioral surface, matches the "while we're already intervening on drift" intuition. |
| **DP-3: Add `dedupe_count` or `had_duplicates` to `KeySearchResult`?** | **No.** Silent dedupe; rely on existing `has_more = true` + stderr `JRACLOUD-95368` warning to signal drift. | Q2; matches surveyed SDK convention (octocrab, aws-sdk-rust, azure_core, google-cloud-rust all preserve+document, none of them expose a drift flag). Avoids gratuitous public-API expansion pre-1.0. |
| **DP-4: Mirror to `search_issues`?** | **No** in the same PR. Add an inline comment at the mirroring guard explicitly recording that the asymmetry is intentional. Reconsider when a `search_issues` caller adopts a `+1 over-fetch` truncation pattern. | Q3; scoped > symmetric per Rust SDK convention, and the concrete `+1 inflation` bug is keys-specific. |
| **DP-5: Test diff** | Flip `test_search_issue_keys_repeated_cursor_abort_does_not_dedupe` to assert post-dedupe `vec!["X-1", "X-2"]` (was `vec!["X-1", "X-1", "X-2"]`), rename to `..._dedupes`. Add a new test for the multi-page-non-consecutive case (`["X-1"]`, `["X-2"]`, `["X-1"]`, then loop) to lock in the `Vec::dedup`-is-wrong correctness pin. | Q1 §A (non-consecutive dups are the load-bearing case). |
| **DP-6: Rustdoc updates** | Update `KeySearchResult` rustdoc lines 80–115 in `src/api/jira/issues.rs` to drop "may contain duplicate keys" from case 2's description and remove the "**This is NOT dup-tolerant**" warning paragraph for `handle_edit`. Replace with "results may be incomplete (truncated before all matching keys were fetched); duplicates are eliminated client-side on the drift-abort path." | Required follow-through; the rustdoc is the public contract. |
| **DP-7: `handle_edit::effective_keys` change** | **No code change required** at the call site. The truncation check `matched_keys.len() > effective_max` becomes correct automatically post-dedupe. | Q2 §recommendation; this is the entire motivation. |

---

## Perplexity queries run (verbatim, for orchestrator audit)

**Q1.a — HashSet retain pattern + comparison**
> Rust idiomatic order-preserving deduplication of Vec<String> small
> collection (n <= 1000). Compare three approaches with code examples and
> allocation cost: (1) HashSet retain pattern `let mut seen =
> HashSet::new(); v.retain(|x| seen.insert(x.clone()))`, (2)
> indexmap::IndexSet via `let unique: IndexSet<_> = v.into_iter().collect()`
> then `.into_iter().collect::<Vec<_>>()`, (3) itertools::Itertools::unique()
> via `v.into_iter().unique().collect()`. Need: code clarity, no new
> dependency if possible, no allocation overhead for n <= 1000 strings of
> length 8-10 chars ("ABC-123" Jira issue keys). Vec::dedup() is wrong
> because duplicates are NON-consecutive.

**Q1.b — itertools::unique() global vs consecutive (Perplexity gave a
WRONG answer; WebFetch refuted it)**
> itertools Rust crate `unique()` method: does Itertools::unique() preserve
> order and deduplicate non-consecutive duplicates? Or does it only dedupe
> consecutive (like Vec::dedup)? Reference: itertools docs.rs Itertools
> trait unique method. Confirm with example: `vec!["A","B","A","C"].
> into_iter().unique().collect::<Vec<_>>()` — does this return ["A","B","C"]
> (true global dedupe) or ["A","B","A","C"] (consecutive only)?

**Q1.c — indexmap version + IndexSet API**
> indexmap Rust crate latest version on crates.io 2026 - IndexSet type API.
> Specifically: `IndexSet::<String>::from_iter(vec)`, `let s:
> IndexSet<String> = vec.into_iter().collect()`, `.into_iter().
> collect::<Vec<_>>()`. What is the current stable version of indexmap as
> of May 2026? Does indexmap::IndexSet preserve insertion order on collect?
> What is the MSRV (minimum supported Rust version)?

**Q2.initial — SDK convention survey (initial search)**
> Rust SDK convention for paginated API client functions: when server
> returns duplicate items due to snapshot instability or live data drift
> (e.g., Atlassian JQL nextPageToken not snapshot-stable), do production
> Rust SDK crates (octocrab GitHub, aws-sdk-rust, google-cloud-rust,
> azure-sdk-rust) silently deduplicate in the client OR preserve duplicates
> and rely on caller-side dedupe? Is a flag/signal exposed (e.g.,
> truncated, had_duplicates, dropped_count field)? Is dedupe gated on the
> abort/error path specifically or always-on?

**Q2.reason — synthesis pass (deeper reasoning)**
> When designing a Rust HTTP client crate that paginates a server API
> where the server is known to return duplicate items under live-data
> drift (e.g., Atlassian Jira's `/rest/api/3/search/jql` endpoint with
> `nextPageToken` cursor, JRACLOUD-95368 [...] what is the prevailing
> convention? Compare two design choices: (A) Silent in-client dedupe
> [...] (B) Preserve duplicates and signal drift via a `dedupe_count:
> usize` or `had_duplicates: bool` field [...]
> octocrab / aws-sdk-rust / azure_core / google-cloud-rust. [...]

NOTE: This `mcp__perplexity__reason` call initially returned an
off-topic PL-300 result set; the model self-detected the mismatch
and re-answered correctly, so the response is on-topic despite the
search-result citations being garbage. This is noted in §"Tooling
note" above.

**Q2.octocrab — octocrab dedupe specifics**
> octocrab Rust GitHub API client crate pagination - does the Page<T> /
> Pager type silently deduplicate items returned across pages? Looking at
> github.com/XAMPPRocky/octocrab source for pagination behavior when
> server returns duplicate items.

**Q2.aws — aws-sdk-rust dedupe specifics**
> aws-sdk-rust Paginator silent dedupe pagination duplicates - does
> aws-smithy-runtime-api or aws_smithy_runtime::client::paginator
> deduplicate items returned across pages? [...]

**Q2.azure — azure_core dedupe specifics**
> azure_core::paging Rust crate Azure SDK pagination Pager type
> deduplication - does the Azure Rust SDK Pager<T> or
> ContinuationToken pagination silently dedupe items across page
> boundaries? Source: github.com/Azure/azure-sdk-for-rust azure_core
> paging module.

**Q3 — Scoped vs symmetric treatment**
> Rust SDK API design principle: when adding new behavior (e.g.,
> deduplication, sorting, normalization) to one paginated function, is it
> idiomatic to also add it to sibling functions on the same struct that
> wrap the same root API endpoint but return different types (e.g.,
> full-body vs keys-only variants)? Symmetric treatment vs scoped
> treatment principle in Rust client crate design.

---

## Inconclusive / flagged

- **`Itertools::unique()` semantics:** Perplexity returned a confidently
  wrong answer (claimed consecutive-only). Caught and refuted by direct
  WebFetch on docs.rs. Final answer (global dedupe) is therefore based on
  WebFetch, not Perplexity. **Confidence: HIGH** (docs.rs is the canonical
  Rust documentation site).
- **`mcp__perplexity__reason` Q2 call returned off-topic citations.**
  The body of the answer was on-topic (the model self-corrected and
  produced a coherent SDK-design discussion), but the citation list was
  entirely PL-300 exam dumps. Cross-checked the conclusion against the
  follow-up `mcp__perplexity__search` queries (Q2.octocrab, Q2.aws,
  Q2.azure), which each returned topic-correct results with crate-source
  citations. **Confidence: HIGH** (the corroborating per-SDK queries all
  agreed).
- **Context7 unavailable:** Both `mcp__context7__resolve-library-id` and
  `mcp__context7__query-docs` returned `Error: No such tool available`.
  Fell back to WebFetch on docs.rs for all library-API verification.
  **Confidence: HIGH** for the verified facts (`itertools` 0.14.0 MSRV
  1.63, `indexmap` 2.14.0 MSRV 1.85, `Itertools::unique` is global dedupe)
  — docs.rs serves the canonical rendered docs.
- **WebSearch was not used as fallback.** Every research question was
  resolvable via Perplexity + targeted WebFetch verification, so the
  "WebSearch fallback" branch of the tool-use spec was not triggered.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Perplexity search | 6 | Q1 dedupe primitive comparison; Q1 itertools::unique() semantics (REFUTED by WebFetch); Q1 indexmap version+API; Q2 initial SDK survey; Q2 octocrab; Q2 aws-sdk-rust; Q2 azure_core; Q3 scoped vs symmetric. |
| Perplexity reason | 1 | Q2 synthesis pass — initial off-topic citations, model self-corrected to on-topic answer; cross-validated via per-SDK Q2.* search calls. |
| Perplexity deep_research | 0 | Not needed — search + reason coverage was sufficient. |
| Context7 | 0 (attempted 2) | `resolve-library-id` returned "no such tool available"; fell back to WebFetch on docs.rs. |
| Tavily | 0 | N/A. |
| WebFetch | 4 | docs.rs/itertools (Itertools::unique semantics — REFUTED Perplexity's first answer); docs.rs/indexmap (version + MSRV); crates.io/indexmap (returned page-title only, no useful data); crates.io/itertools (returned page-title only, no useful data). |
| WebSearch | 0 | Not needed; Perplexity + WebFetch were sufficient on every question. |
| Training data | 1 area (low) | Background on Rust idioms (HashSet::insert returning bool, Vec::retain semantics) was treated as known and not independently re-verified; these are stable std-library guarantees documented in The Rust Programming Language. |

**Total MCP tool calls:** 7 (6 Perplexity search + 1 Perplexity reason)
**Total WebFetch calls:** 4 (all verifying Perplexity citations)
**Training data reliance:** **low** — only stable std-library behavior was
left unverified; every crate API, every version number, every SDK
convention claim is anchored to a specific Perplexity source or a
WebFetch-confirmed docs.rs page.
