# ADR-0012: Module Shard Rule Codification

## Status
Accepted

## Context

The codebase has an implicit "shard at ~1,000 LOC" rule that was applied once formally
via `docs/specs/list-rs-split.md`, which tracked the refactor of the monolithic
`src/cli/issue/list.rs` into `list.rs` + `view.rs` + `comments.rs`. The rule was never
codified explicitly. At the time this ADR was written, three `src/cli/` files violated
the threshold:

| File | LOC | Shard status |
|------|----:|--------------|
| `src/cli/auth.rs` | 1,998 | Violated — 2× threshold |
| `src/cli/issue/list.rs` | 1,083 | Violated — over threshold post-split |
| `src/cli/assets.rs` | 1,055 | Violated — over threshold |

**Why the rule exists:**
- Files over ~1,000 LOC have higher branch density and more undocumented edge cases
- Large files are harder to review in a PR
- Clippy's `too_many_arguments` lint and similar signals appear more often in large files
- AI agents reading the codebase have a limited context budget (per ADR-0004's
  token-economy rationale)

**Why exceptions exist:**
- `src/adf.rs` (large LOC) is a self-contained DSL translator with complex but coherent
  logic. Sharding it artificially would split coherent transformation functions across
  files without a natural boundary.
- `src/api/auth.rs` (large LOC) contains a tightly coupled state machine (OAuth flow +
  keychain namespacing + legacy migration + refresh coordinator). Cohesion is high;
  sharding is possible but not urgent.

## Decision

**Codify the shard rule as follows:**

1. **Threshold:** any source file in `src/cli/` that reaches or exceeds 1,000 LOC is a
   shard candidate.
2. **Trigger:** when a file hits 1,000 LOC, the contributor must either (a) create a
   feature spec in `docs/specs/` for the shard plan, or (b) document explicitly in the
   PR why deferral is appropriate.
3. **Exception list:** `src/adf.rs` (coherent DSL, no natural split boundary);
   `src/api/auth.rs` (tight state machine cohesion).
4. **Shard targets:**
   - `src/cli/auth.rs` → `src/cli/auth/{login,switch,list,status,refresh,logout,remove,keychain}.rs`
   - `src/cli/assets.rs` → `src/cli/assets/{search,view,tickets,schemas}.rs`
5. **`src/cli/issue/list.rs`:** no further sharding beyond the already-extracted `view.rs`
   and `comments.rs`. The content is unified JQL composition + asset integration + all
   filter clauses; a natural boundary does not exist without artificial decomposition.
   Documented in CLAUDE.md "Known Size Deviations" as a known exception.

## Rationale

- Making the rule explicit prevents the "shard once, then violate" pattern from repeating
  silently.
- The exception list is important — not all large files should be sharded. `src/adf.rs`
  is large because it translates a complex format; sharding would create artificial
  dependencies between closely related transformation functions.
- The shard targets (`src/cli/auth.rs`, `src/cli/assets.rs`) have natural split
  boundaries (one subcommand handler per file) following the precedent established by the
  `src/cli/issue/` directory.

## Consequences

- All future PRs that push a `src/cli/` file past 1,000 LOC must acknowledge the rule in
  the PR description with either a shard plan or a deferral justification.
- The `src/cli/auth/` and `src/cli/assets/` shards are first-class delivery stories,
  not afterthoughts.
- The CLAUDE.md "Architecture" section must be updated to reflect the sharded module
  layout after each shard operation.
- `src/config.rs` (large LOC) is in `src/` (not `src/cli/`) and is a single-concern
  module. It is outside the rule's scope but should be monitored.

## See Also

- `docs/specs/list-rs-split.md` — precedent for the `src/cli/issue/` shard
- `src/cli/auth/` — `src/cli/auth.rs` after the shard landed
- `src/cli/assets/` — `src/cli/assets.rs` after the shard landed
- CLAUDE.md "Known Size Deviations" — list of documented LOC exceptions
