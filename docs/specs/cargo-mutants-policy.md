# cargo-mutants Policy

## Purpose

Mutation testing as a meta-verification layer on the bulk + create modules.
Reference: F6 hardening review of PR #110-pr2 (2026-05-10); closes audit-followup #346.

Mutation testing catches a class of defect that line-coverage metrics miss: tests that
pass even when the implementation is silently broken by small code mutations (negated
conditions, removed returns, swapped operators). The three modules designated below had
high line coverage but untested assertion strength at the time of the F6 review.

## Scope

`cargo-mutants` runs against:
- `src/cli/issue/create.rs` — `handle_edit_bulk_labels`, `handle_edit_bulk_fields`, `handle_jsm_create`, `parse_field_kv`
- `src/api/jira/bulk.rs` — `await_bulk_task`, polling loop, deadline propagation
- `src/types/jira/bulk.rs` — serde structs for bulk API responses
- `src/api/jsm/requests.rs` — `JsmRequestBuilder::build` (JSM POST body construction) (added S-288-pr4)
- `src/api/jsm/request_types.rs` — `list_request_types`, `get_request_type_fields` (added S-288-pr4)
- `src/cli/requesttype.rs` — `handle_list`, `handle_fields`, `resolve_request_type_id` (added S-288-pr4)

Configured in `.cargo/mutants.toml::examine_globs`. The CI job relies on this
configuration alone (no `--file` CLI flags) for scope enforcement; `--in-diff` further
narrows to lines changed in the PR diff.

Note: cargo-mutants v27+ reads its config from `.cargo/mutants.toml` (not `.mutants.toml`
at repo root). This is the canonical config location for this project.

## Kill-Rate Target

**90% on the PR diff scope.** The CI `mutants` job fails if the kill rate is below 90%.

Rationale: with the inline proptest from S-345 (BC-3.4.006) and the integration tests
in `tests/issue_bulk_pr2.rs` and `tests/issue_bulk.rs`, the bulk + create paths have
strong existing coverage. Mutation testing surfaces gaps where assertions are too loose.

The 90% threshold lives in the CI YAML `Check kill rate` step (not in `.cargo/mutants.toml`)
for CI-artifact visibility: reviewers can read the threshold without parsing TOML.

## Whitelist Convention

When a mutant cannot reasonably be killed — defensive guard, unreachable code, or a
performance-only change with identical observable behavior — annotate the function with
`#[mutants::skip]` AND include a justification comment IMMEDIATELY ABOVE the attribute.

Required format:

```rust
// mutants::skip: <one-line reason>
// Example: "defensive guard against impossible state; debug_assert! covers runtime invariant"
#[mutants::skip]
fn some_guard(...) { ... }
```

**Rules:**
- Bare `#[mutants::skip]` without a justification comment is **forbidden**. Code review
  MUST reject any PR that adds a bare whitelist attribute.
- The justification comment must be on the line(s) immediately preceding `#[mutants::skip]`.
- Valid justification categories:
  - Defensive guard for unreachable state (e.g., error branch that cannot be triggered
    through the public API under test)
  - Performance-only optimization (e.g., `with_capacity` hint) where the observable
    behavior is identical whether the hint is present or not
  - Debug-only assertion (e.g., `debug_assert!`) that does not run in release builds

Invalid justifications:
- "Tests don't cover this" — that is a gap to close, not a reason to skip
- "It's hard to test" — that is a refactoring opportunity, not a reason to skip

## Deferral Policy

The initial baseline PR (S-346) MUST NOT block on achieving 90% kill-rate on first run.
The intent is to land the CI gate; incremental improvement follows.

When the baseline reveals surviving mutants below 90%:

1. **Whitelist clearly-defensive mutants** per the convention above with justification comments.
2. **File one follow-up GitHub issue per uncovered-region cluster** (not per individual
   mutant). Title pattern: `chore(mutants): close surviving-mutant gap in <module> — N mutants`
3. **Track deferred follow-ups** in `docs/demo-evidence/S-346/deferred-followups.md` with
   issue numbers, links, and surviving mutant descriptions.
4. **Subsequent PRs** incrementally close the gap by tightening assertions, adding
   targeted test cases, or whitelisting genuinely unkillable mutants.

The CI `mutants` job enforces 90% on the PR diff scope going forward. A PR that touches
the scoped files and scores below 90% on changed lines will fail CI.

## Local Invocation

Install (one-time):

```bash
cargo install cargo-mutants --locked
```

Full baseline on scoped files (uses `.cargo/mutants.toml` automatically):

```bash
cargo mutants --jobs 4
```

PR-diff-equivalent run (matches CI scope):

```bash
DIFF_FILE=$(mktemp -t pr.diff.XXXXXX)
trap 'rm -f "$DIFF_FILE"' EXIT
git diff origin/develop...HEAD > "$DIFF_FILE"
cargo mutants --in-diff "$DIFF_FILE" --jobs 4
```

Note: the `--file` flags are omitted above because `.cargo/mutants.toml` already
scopes via `examine_globs`. The `--in-diff` flag further narrows to lines changed in
the diff. Using both is redundant (CI uses `--in-diff` only).

Single-file inspection:

```bash
cargo mutants --file src/api/jira/bulk.rs --jobs 4
```

Results land in `mutants.out/` (excluded from git via `.gitignore`).

## CI Integration

The `mutants` job in `.github/workflows/ci.yml` runs on PRs only (not pushes to
`develop` / `main`). This is consistent with the `security` job pattern and keeps
mutation testing cost bounded to the PR review phase.

The job uses `--in-diff "$DIFF_FILE"` (where `DIFF_FILE` is a `mktemp`-created path
under `${{ runner.temp }}`) to scope mutations to lines changed in the PR.
This means:
- Only mutants in code **changed by the PR** AND **in the three scoped files** are tested
  (`.cargo/mutants.toml::examine_globs` provides the file-scope; `--in-diff` narrows to
  changed lines within those files).
- PRs that do not touch the scoped files generate zero mutants; the kill-rate check
  exits 0 ("no killable mutants — skip threshold check").

See `.factory/cicd-setup.md` §1.1a for the canonical CI job specification.
