# `adf.rs` — recursion-depth guard (SEC-001, CWE-674)

**Security item:** SEC-001 (Bundle D triage, item 3)
**CWE:** [CWE-674](https://cwe.mitre.org/data/definitions/674.html) — Uncontrolled Recursion
**Module:** `src/adf.rs`
**Behavioral contract:** new BC-7.2.012 (to be added to `.factory/specs/prd/bc-7-output-render.md`)

---

## Status

**SPEC ONLY — no implementation yet.**

Design decisions are final (human-approved). The one flagged open question
(reverse-path error-vs-warn) is escalated per §8 and must be resolved before
the test-writer begins.

---

## 1. Problem

`src/adf.rs` has six recursive tree-walking functions and no depth guard.
The ADF tree is built from **untrusted user input** (`--description`, `--message`,
`--file` content) via `markdown_to_adf` / `text_to_adf`, and is read back from
**Jira server responses** (issue view, comment list, changelog) via `adf_to_text`.

A pathologically deeply-nested markdown input — for example, thousands of
consecutive `>` blockquote levels or deeply-nested list items — produces a
`serde_json::Value` tree of proportional depth. Every recursive post-processing
pass then walks that tree depth-first, consuming one stack frame per level.
On an 8 MB main-thread stack (the Windows PE-header value set by
`.cargo/config.toml`; also the typical Unix default), a tree several hundred
levels deep is sufficient to reach stack-overflow territory because each frame
on the recursive pass is substantially larger than a raw pointer. The result is
a **process abort / SIGSEGV / STATUS_STACK_OVERFLOW** — a crash, not a clean
error, which is worse than a controlled exit.

**Reachability — confirmed untrusted-input path (forward):**

Any `jr` command that accepts user-authored free text and converts it to ADF
is reachable. A malicious or misconfigured script, an AI agent generating
issues, or a user piping a pathological file via `--description-stdin` can
trigger the condition.

**Reachability — server-response path (reverse):**

`adf_to_text` processes ADF returned by Jira. Under normal conditions Jira
does not produce deeply nested ADF, but a compromised or hostile Jira instance
(possible in combination with `JR_BASE_URL` overrides in debug builds, or
with a real-but-hostile cloud tenant) could return arbitrarily nested ADF to
make `jr issue view` / `jr issue list` crash on render.

**serde_json secondary concern:**

`serde_json`'s JSON serializer also recurses over the tree when `jr` serializes
the ADF body before POSTing it to Jira. Capping the tree depth at construction
protects the serializer too.

**pulldown-cmark parse phase:**

The `AdfBuilder` itself is **not recursive** — it uses an explicit `Vec<PartialNode>`
stack (`start()` / `end()` push/pop). The pulldown-cmark event parser is
largely iterative; its own nesting handling is block-based with a fixed
internal parse stack, not direct Rust call-stack recursion. The risk lies
exclusively in the post-processing passes and the reverse renderer that walk
the already-built `serde_json::Value` tree.

---

## 2. Approved contract (human decision — do not relitigate)

```
const MAX_ADF_DEPTH: usize = 256;
```

- **Limit:** 256 nesting levels, measured as the depth of the `serde_json::Value`
  tree at any container node boundary. A top-level `doc` node is depth 0; its
  direct children are depth 1; their children depth 2, and so on.
- **On exceeding the limit:** return a `JrError::UserError` (exit code 64).
  The error must be clean and informative — **no silent clamp, no truncation**.
- **Applies to both directions:** forward (markdown → ADF build + post-passes)
  AND reverse (ADF → text render).
- **Signature change:** `markdown_to_adf(&str) -> Value` becomes
  `markdown_to_adf(&str) -> Result<Value, JrError>`.
  `text_to_adf` is shallow by construction (one-paragraph output, no recursion)
  and does NOT become fallible — see §4.

---

## 3. Boundary definition

The depth limit is **inclusive**: a tree that reaches depth 256 is an error;
depth 255 is accepted.

| Depth | Result |
|-------|--------|
| 0 – 255 | Ok — processing proceeds normally |
| **256** | **Err(JrError::UserError(...))** — boundary is hit, reject |
| 257+ | Err — would be reached by the same guard at depth 256 |

Rationale for inclusive 256: the guard checks `if current_depth >= MAX_ADF_DEPTH`
(or equivalently `> MAX_ADF_DEPTH - 1`). This is natural in a "depth counter
threaded as a parameter" implementation and easy to test: the 255-level input
must succeed and the 256-level input must fail.

---

## 4. Affected functions — full enumeration

Every recursive function in `src/adf.rs` is listed. The "recursion type" column
classifies whether the function recurses on its own call stack vs. calls another
recursive function.

### 4.1 Forward path (markdown → ADF)

| Symbol | Location | Recursion type | Guard needed? |
|--------|----------|---------------|---------------|
| `normalize_list_item_content` | `src/adf.rs::normalize_list_item_content` | Self-recursive (calls itself on inner `blockquote`/`panel`/`taskList` children) | YES |
| `normalize_panel_content` | `src/adf.rs::normalize_panel_content` | Self-recursive (calls itself on nested `panel`/`blockquote` children) | YES |
| `normalize_blockquote_content` | `src/adf.rs::normalize_blockquote_content` | Self-recursive (calls itself on nested `taskList` children) | YES |
| `assign_local_ids_walk` | `src/adf.rs::assign_local_ids_walk` | Self-recursive (recurses into every node's `content` array) | YES |
| `autolink_bare_urls` | `src/adf.rs::autolink_bare_urls` | Self-recursive (recurses into `content` of non-text, non-code nodes) | YES |
| `is_empty_block_container` | `src/adf.rs::is_empty_block_container` | NOT self-recursive (checks only the immediate `content` array, no descent) | NO — shallow |

`assign_local_ids` is a non-recursive wrapper that calls `assign_local_ids_walk`;
the guard lives in the walk function.

`split_text_node_on_urls` and `find_bare_url_spans` are called BY
`autolink_bare_urls` but are not themselves recursive.

`flatten_table_to_paragraphs` iterates over rows and cells but does not recurse;
it calls `adf_to_text` once per non-paragraph cell block (the only case where
forward and reverse paths touch). This `adf_to_text` call processes a single
cell fragment — in practice a shallow doc — but the reverse guard still applies.

### 4.2 Reverse path (ADF → text)

| Symbol | Location | Recursion type | Guard needed? |
|--------|----------|---------------|---------------|
| `AdfRenderer::render_node` | `src/adf.rs::AdfRenderer::render_node` | Mutually recursive with `render_children` | YES |
| `AdfRenderer::render_children` | `src/adf.rs::AdfRenderer::render_children` | Calls `render_node` on each child (mutual recursion) | via `render_node` |
| `AdfRenderer::render_cell_inline` | `src/adf.rs::AdfRenderer::render_cell_inline` | Calls `render_node` on cell children; non-recursive by itself | via `render_node` |

`render_doc` iterates and calls `render_node` but is not itself recursive.

Total: **7 unique recursion sites** need depth guards (5 forward normalizers,
1 forward tree-walker, 1 forward url-walker, and 1 reverse render entry).
Because `render_children` is always entered through `render_node`, the guard on
`render_node` covers both.

---

## 5. Implementation design

### 5.1 Depth counter mechanism

Thread a `depth: usize` parameter through each recursive function. On every
recursive call, pass `depth + 1`. At the top of each function, check:

```rust
if depth >= MAX_ADF_DEPTH {
    return Err(...);   // or the equivalent for void/Vec-returning fns
}
```

This is straightforward, avoids global mutable state, and makes the guard
visible at every call site. An alternative (a wrapper struct that carries the
counter) is more complex for no gain given the functions are private.

### 5.2 Constant

Define at module scope in `src/adf.rs`:

```rust
/// Maximum ADF nesting depth. Forward (`markdown_to_adf` post-passes) and
/// reverse (`adf_to_text`) tree walkers both reject inputs exceeding this
/// depth to prevent stack overflow (SEC-001, CWE-674).
/// Value 256 provides a large margin over any legitimate human-authored
/// nesting while staying well below the stack-overflow threshold on the
/// configured 8 MB stack.
pub(crate) const MAX_ADF_DEPTH: usize = 256;
```

`pub(crate)` makes the constant available to integration tests without
exposing it in the public API.

### 5.3 Forward path — signature change

`markdown_to_adf` currently returns `Value` (infallible). It must become:

```rust
pub fn markdown_to_adf(markdown: &str) -> Result<Value, JrError>
```

The post-passes (`normalize_list_item_content`, `normalize_panel_content`,
`normalize_blockquote_content`, `assign_local_ids_walk`, `autolink_bare_urls`)
must also return `Result<_, JrError>` to propagate errors. Because
`markdown_to_adf` is the only public entry point that calls these passes,
the fallibility stays internal to `src/adf.rs`; only the public signature
changes.

`text_to_adf` does NOT become fallible. Its output is a flat one-level-deep
doc (`doc > paragraph > [text|hardBreak]*`) regardless of input length. No
recursion is involved; even multi-line inputs produce a single-level content
array with hardBreak nodes. No depth risk exists.

### 5.4 Reverse path — signature change

`adf_to_text` currently returns `String` (infallible). It must become:

```rust
pub fn adf_to_text(adf: &Value) -> Result<String, JrError>
```

The `AdfRenderer::render_node` / `render_children` signatures also become
fallible, propagating the error upward to `render_doc` and then to
`adf_to_text`.

See §7 for the reverse-path nuance and the open question about behavior.

### 5.5 Error message

```
markdown nesting too deep (max 256 levels)
```

This message is used when the forward path exceeds the limit. The `JrError`
variant is `UserError` (exit code 64), because the nesting is caused by
user-supplied content.

For the reverse path the wording differs to distinguish server data from
user input:

```
ADF response nesting too deep (max 256 levels) — the issue data returned
by Jira cannot be rendered
```

The variant is still `JrError::UserError` (exit code 64) — the user can
work around it (e.g. `--output json` to get raw data without rendering).

---

## 6. Call-site impact

### 6.1 Forward call sites (`markdown_to_adf`)

All four forward call sites currently treat `markdown_to_adf` as infallible.
After the signature change they must propagate `Result` with `?`. The `?`
operator in each site already operates inside an `async fn` that returns
`Result<_, anyhow::Error>`, so `JrError` propagates through `anyhow`'s `From`
impl without additional mapping.

| File | Line | Context | Error mapping |
|------|------|---------|---------------|
| `src/cli/issue/create.rs` | ~179 | `issue create --description --markdown` | `?` in `handle_create`, exit 64 |
| `src/cli/issue/create.rs` | ~925 | `issue edit --description --markdown` | `?` in `handle_edit`, exit 64 |
| `src/cli/issue/workflow.rs` | ~1159 | `issue comment --markdown` | `?` in `handle_comment`, exit 64 |
| `src/api/jsm/requests.rs` | ~96 | `JsmRequestBuilder::build()` | `build()` returns `Result<Value, JrError>` |

`JsmRequestBuilder::build()` currently returns `serde_json::Value` (infallible).
It must become `Result<Value, JrError>` and all call sites of `build()` must
add `?` accordingly.

### 6.2 Reverse call sites (`adf_to_text`)

`adf_to_text` is currently called in four locations, all in read-display paths
that render Jira API responses to the terminal.

| File | Line | Context | Error mapping |
|------|------|---------|---------------|
| `src/cli/issue/view.rs` | ~87 | `issue view` description render | `?` propagates to handler, exit 64 |
| `src/cli/issue/comments.rs` | ~33, ~49 | comment body render (two map sites) | `.map(adf::adf_to_text)` → `.map(\|v\| adf::adf_to_text(&v))` with `?` propagation |
| `src/adf.rs` | ~1867 | `flatten_table_to_paragraphs` internal call | internal — propagated via `?` |

The `comments.rs` sites use `.map(adf::adf_to_text)` on `Option<Value>`. After
the signature change these become `.and_then(\|v\| adf::adf_to_text(&v).ok())`
OR the handler collects results and propagates any `Err`. The preferred approach
is to propagate the error (let the handler decide), not swallow it with `.ok()`.

---

## 7. Reverse-path nuance and open question

### Background

The reverse path (`adf_to_text`) processes ADF returned by the **Jira server**,
not user-authored content. Under normal conditions, human-authored Jira issues
never reach 256 nesting levels. The scenario where the guard fires is:

1. A compromised or hostile Jira instance (unlikely in production; more plausible
   with `JR_BASE_URL` override in debug builds).
2. A Jira issue with an ADF body created programmatically by a third-party tool
   that disregards nesting limits (theoretically possible, highly improbable in
   practice).

### Current behavior without a guard

Without a guard, a pathologically nested ADF response causes a stack-overflow
crash (`SIGSEGV` / `STATUS_STACK_OVERFLOW`) when `jr issue view` or `jr issue
list` tries to render the body. A controlled error is strictly better than a
crash.

### Analysis

**Option A — error at 256 (same contract as forward path):**
`jr issue view` exits 64 with an error message. The user cannot read the issue
description from the terminal but can still use `jr issue view --output json`
to get the raw ADF or inspect Jira directly in the browser.

**Option B — clamp and warn (reverse path only):**
Stop recursing at depth 256, emit a `[…truncated: nesting too deep…]` inline
text node, and continue rendering. The user gets a partial render with a
visible warning. The signature stays infallible (`-> String`), avoiding
call-site changes. However: (1) this produces a misleadingly-partial render;
(2) 256 levels is so far beyond any legitimate Jira content that a legitimate
user would never hit it; (3) the partial-render approach silently discards
ADF content and is therefore itself a data-integrity concern.

### Recommendation

Use **Option A** (error at 256) for the reverse path. Rationale:

- 256 levels is unreachable by legitimate content. A real user will never
  hit this error in practice; the guard exists solely to prevent a crash from
  a hostile or malformed payload.
- A clean error + "use `--output json` to see raw data" hint is a better
  user experience than a partial/corrupted render.
- It maintains a uniform contract across both directions: exceed 256 → error.
- It keeps `adf_to_text` under the same `Result<String, JrError>` signature
  as the forward path, making the overall API consistent.

### Open question for the human (must be resolved before implementation)

**Q-1 (reverse path):** Confirm Option A (error at 256) for `adf_to_text`.
If Option B (clamp-and-warn) is preferred, the implementer must keep
`adf_to_text` infallible AND redesign the call sites to not use `?`.

The recommendation above is Option A. Please confirm or override before
the implementer begins.

---

## 8. Edge cases

| Scenario | Expected behavior |
|----------|------------------|
| Forward: depth 255 (one below limit) | `Ok(Value)` — accepted |
| Forward: depth **256** (at limit) | `Err(JrError::UserError("markdown nesting too deep …"))`, exit 64 |
| Forward: depth 257+ | Same error as 256 — guard fires at 256, never reaches 257 |
| Forward: 256 nested blockquotes (`> > > …`) | Error |
| Forward: 256 nested lists (`- \n  - \n    - \n…`) | Error |
| Forward: 256 nested GFM alerts | Error (each alert contributes one panel nesting level after normalization) |
| Forward: mixed node types reaching depth 256 (e.g. blockquote in list in panel) | Error — depth is cumulative across node types |
| Forward: `text_to_adf` with any input | `Ok(Value)` always — no recursion, not guarded |
| Reverse: `adf_to_text` on ADF with depth 255 | `Ok(String)` — accepted |
| Reverse: `adf_to_text` on ADF with depth **256** | `Err(JrError::UserError("ADF response nesting too deep …"))`, exit 64 |
| Reverse: `jr issue view` on a deep-ADF issue | Exit 64, stderr error, no crash |
| Reverse: `jr issue view --output json` on a deep-ADF issue | JSON of the issue is printed (JSON output path does not call `adf_to_text`) |
| Both: `--output json` on `issue create` with deep description | Forward error fires at `markdown_to_adf` before the JSON path is reached |
| Both: proptest depth-5 gen_node (existing test at ~line 8938) | Unaffected — depth 5 is far below 256 |

---

## 9. Test plan (TDD — tests first)

All new tests go in `src/adf.rs::tests` following the `test_<verb>_<subject>_<expected_outcome>` naming convention.

### 9.1 Forward direction tests

**Depth-boundary unit tests (use ADF-builder-bypass helpers that produce a
deep `serde_json::Value` directly, to avoid writing 255+ lines of markdown):**

- `test_markdown_to_adf_depth_255_is_ok` — construct a 255-level nested
  blockquote markdown string (or a JSON value fixture); assert `Ok(_)`.
- `test_markdown_to_adf_depth_256_is_err` — construct a 256-level nested
  blockquote markdown string; assert `Err` with message containing
  `"nesting too deep"` and `"256"`.
- `test_markdown_to_adf_depth_257_is_err` — 257-level; same `Err` check
  (guard fires at 256, early exit, same error).

**Recursive pass isolation tests (test each normalizer separately with a
value fixture at exactly depth 256):**

- `test_normalize_list_item_content_depth_256_is_err`
- `test_normalize_panel_content_depth_256_is_err`
- `test_normalize_blockquote_content_depth_256_is_err`
- `test_assign_local_ids_walk_depth_256_is_err`
- `test_autolink_bare_urls_depth_256_is_err`

**Regression guard — existing behavior unchanged:**

- `test_markdown_to_adf_normal_depth_is_unchanged` — a real-world markdown
  sample (headings, lists, code blocks, tables) must return `Ok` and produce
  the same output as before the guard was added (snapshot or structural check).

### 9.2 Reverse direction tests

- `test_adf_to_text_depth_255_is_ok` — construct a 255-level nested ADF
  value (JSON fixture); assert `Ok(_)`.
- `test_adf_to_text_depth_256_is_err` — 256-level nested ADF; assert `Err`
  with message containing `"nesting too deep"` and `"256"`.
- `test_adf_to_text_normal_depth_is_unchanged` — real-world ADF from existing
  test fixtures; assert same output as before (regression guard).

### 9.3 Call-site integration tests

- `test_issue_create_deep_description_exits_64` — integration test (wiremock):
  `jr issue create ... --description <256-deep-markdown>` asserts exit 64 and
  `"nesting too deep"` in stderr. POST must NOT be called (assert wiremock
  call count = 0).
- `test_issue_comment_deep_body_exits_64` — same pattern for `jr issue comment`.
- `test_issue_view_deep_adf_exits_64` — wiremock returns a deeply-nested ADF
  issue; `jr issue view` exits 64 (or `Ok` if Option B is chosen — revisit
  after Q-1 resolution).

### 9.4 Const-gate test (regression pin)

- `test_max_adf_depth_constant_is_256` — `assert_eq!(MAX_ADF_DEPTH, 256)` —
  pins the constant so a future change requires updating the test explicitly.

---

## 10. Out of scope

- `text_to_adf` — no recursion, no guard needed.
- Iterative rewrite of the recursive functions — disproportionate to the risk
  and would destabilize a large, well-tested body of code.
- pulldown-cmark's internal parse stack — the parser is event-based/iterative;
  its nesting behavior does not use Rust call-stack recursion in a way that
  compounds with our post-processing passes.
- `serde_json::Value` deserialization recursion limit — serde_json's deserializer
  has a default limit of 128 levels; deeply-nested JSON returned from Jira would
  fail at the deserialize step before reaching `adf_to_text`. Not addressed here.
- `flatten_table_to_paragraphs`'s internal `adf_to_text` call for non-paragraph
  cells — this is a single-level call on a leaf doc fragment; the depth guard on
  `adf_to_text` covers it without additional work.
- The `render_node` function at line 8865 in the proptest `GenNode` helper (test
  code only, not in the production `AdfRenderer`) — out of scope; it recurses at
  most to `prop_recursive(5, …)` depth.

---

## 11. References

- `src/adf.rs` — all functions enumerated in §4
- `src/error.rs::JrError` — `UserError` variant, exit code 64
- `src/cli/issue/create.rs` — `handle_create` (~line 179), `handle_edit` (~line 925)
- `src/cli/issue/workflow.rs` — `handle_comment` (~line 1159)
- `src/cli/issue/view.rs` — `adf_to_text` call (~line 87)
- `src/cli/issue/comments.rs` — comment body render (~lines 33, 49)
- `src/api/jsm/requests.rs` — `JsmRequestBuilder::build` (~line 96)
- `.factory/maintenance/2026-06-22/bundle-d-triage.md` — §Item 3 (SEC-001)
- ADR-0004 — per-feature spec convention (this document)
- CWE-674: https://cwe.mitre.org/data/definitions/674.html
- WIN-STACK (CLAUDE.md) — 8 MB PE-header stack on Windows; same 8 MB default on Unix
