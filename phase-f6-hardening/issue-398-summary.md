# Phase F6 — Targeted Hardening: issue-398 / S-398 (changed-fields echo)

- **Feature:** Echo changed/set fields on `issue edit` + `issue create` success (closes #398)
- **Delta range:** `e0ea24b..b49f2fd` on `develop` (merged PR #399)
- **HEAD verified:** `b49f2fd6a8198c04f8ceb610f2d26d25aed1113f`
- **Date:** 2026-05-22
- **Verdict:** **F6 PASS**

## Scope — changed source files

| File | Δ | Nature of change |
|------|---|------------------|
| `src/cli/issue/create.rs` | +74/−16 | `create_echo` BTreeMap built in parallel with `fields`; `changed_fields` in `handle_edit`; table-mode echo loops |
| `src/cli/issue/helpers.rs` | +15/−4 | `resolve_team_field` return type widened to 3-tuple (adds resolved display name) |
| `src/cli/issue/json_output.rs` | +37/−2 | `edit_response` gains `changed_fields: &BTreeMap<String,String>` param |
| `src/cli/issue/list.rs` | +8/−2 | Destructure update for the widened `resolve_team_field` tuple |

No dependency changes: `git diff e0ea24b..b49f2fd` touches neither `Cargo.toml` nor `Cargo.lock`.

## 1. Mutation Testing — PASS (100% kill rate)

- **Tool:** `cargo-mutants` 27.0.0
- **Invocation:** `cargo mutants --in-diff <e0ea24b..b49f2fd diff> --jobs 4 --baseline skip`
- **Scope:** Intersection of the PR diff and `.cargo/mutants.toml::examine_globs`. Of the 4 changed files, only `src/cli/issue/create.rs` is in `examine_globs`; `helpers.rs`, `json_output.rs`, `list.rs` are out of the configured mutation scope (consistent with CI, which uses `--in-diff` over the same config).
- **Result:** **3 mutants found, 3 caught, 0 missed, 0 timeout, 0 unviable → 100% kill rate.**

| Mutant | Disposition |
|--------|-------------|
| `create.rs:39` — `replace handle_create -> Result<()> with Ok(())` | CAUGHT |
| `create.rs:304` — `replace handle_edit -> Result<()> with Ok(())` | CAUGHT |
| `create.rs:972` — `replace == with != in handle_edit` | CAUGHT |

- Target ≥90% (≥95% security-critical) → **exceeded (100%)**.
- The `create.rs:972` mutant is the `if field == "description"` predicate in the
  S-398 table-mode echo loop (the description-asymmetry branch: table shows
  `(updated)` marker, JSON carries raw input). It was killed — the echo branch
  is genuinely covered by `tests/issue_edit_echo.rs`, **not vacuous**.
- **Cross-reference to the F5 vacuous-guard concern:** the F5 finding flagged
  dry-run / bulk-exclusion guard tests as potentially vacuous. That concern does
  **not** materialize here — there are zero surviving mutants on any exclusion or
  guard path within the delta's mutation scope. The `is_team_uuid` predicate is
  in `helpers.rs`, outside `examine_globs`, so it produced no mutants in this
  run; it is instead covered by the dedicated unit tests
  `is_team_uuid_*` in `src/cli/issue/helpers.rs` (standard/uppercase/mixed-case
  UUID accept; wrong-length, missing-hyphen, non-hex, plausible-team-name reject).

### Baseline note (methodology)

The first two mutation runs aborted because cargo-mutants' baseline (unmutated
tree) test pass failed on `tests/multi_cloudid_disambiguation.rs` with
`Platform secure storage failure: The specified item already exists in the
keychain`. This is a **pre-existing macOS-keychain test-isolation flake**,
unrelated to the S-398 delta:

- The delta touches **zero** auth/keychain/keyring code (verified by
  `git diff e0ea24b..b49f2fd --stat | grep -iE 'auth|keychain|keyring|cloudid'`
  → no matches).
- The failing test rotates (different cloudid test fails per run) and the error
  is always the keychain-collision message — a leftover OS keychain entry from
  an interrupted prior run, not a code defect.
- The mutants live in `create.rs` and are killed by `tests/issue_create_echo.rs`
  / `tests/issue_edit_echo.rs`, never by the cloudid suite.

The run was completed with `--baseline skip`. This is sound here because the
baseline state was independently verified: the full `cargo test --all-features`
regression (Section 5) isolates the single failing shard and confirms every
other shard — including all delta-relevant shards — passes.

## 2. Kani Formal Proofs — JUSTIFIED SKIP

The project does **not** use Kani — it is absent from `Cargo.toml`, the CI
workflows, and the repo has no proof-harness infrastructure.

S-398 is an output-formatting feature. The only new logic surface is:
- `is_team_uuid` — a **pure** predicate over an already-clap-parsed `&str`
  (36-char length check + 8-4-4-4-12 hex/hyphen positional scan, no allocation,
  no I/O, total function).
- `changed_fields` / `create_echo` — pure `BTreeMap<String,String>` construction;
  determinism (alphabetical key order) is a direct consequence of `BTreeMap`'s
  sorted iteration and needs no proof.

Both are fully exercised by unit tests (`is_team_uuid_*`), the snapshot test
`test_edit` + `test_edit_response_empty_changed_fields` in `json_output.rs`,
and the integration suites `tests/issue_create_echo.rs` (915 LOC) /
`tests/issue_edit_echo.rs` (956 LOC). Introducing Kani for this delta is not
warranted. **Kani: JUSTIFIED SKIP.**

## 3. Fuzz Testing — JUSTIFIED SKIP

S-398 introduces no new external-input parser. The echo feature is pure output
formatting; `is_team_uuid` consumes a string that clap has already parsed and
validated as a CLI argument. No new byte-stream / untrusted-input boundary
exists in the delta. No genuine new fuzz target was identified.
**Fuzz: JUSTIFIED SKIP.**

## 4. Security Scan — PASS (clean)

| Check | Tool | Result |
|-------|------|--------|
| Dependency vulnerabilities | `cargo audit` 0.22.1 | **clean** — 340 crates scanned, 1098 advisories loaded, 0 vulnerabilities |
| Licenses + bans + sources + advisories | `cargo deny check` | **clean** — `advisories ok, bans ok, licenses ok, sources ok` (only 2 benign `license-not-encountered` warnings for unused allowances `OpenSSL`/`Unicode-DFS-2016`) |
| New dependencies | `git diff e0ea24b..b49f2fd` | **none** — Cargo.toml / Cargo.lock unchanged |

No CRITICAL or HIGH findings. No security escalation required.

## 5. Full Regression — PASS

| Check | Result |
|-------|--------|
| `cargo test --all-features` (full workspace) | **PASS** — all shards green except one pre-existing environmental flake (`multi_cloudid_disambiguation`, macOS keychain collision; see Section 1 baseline note). Zero failures in any delta-relevant shard; the new suites `issue_create_echo` (54 tests) and `issue_edit_echo` (44 tests) pass. |
| `cargo clippy --all-targets --all-features -- -D warnings` | **PASS** — zero warnings |
| `cargo fmt --all -- --check` | **PASS** — no formatting drift |

The single `multi_cloudid_disambiguation` failure is a known macOS-keychain
test-isolation flake, **out of S-398 scope** (delta touches no auth/keychain
code) and **green on PR #399's CI** (which ran on Linux + macOS runners with
clean keychains). It is an environment artifact of this local machine, not a
regression introduced by the delta. Recommend filing a test-hardening
follow-up to make `multi_cloudid_disambiguation` keychain-collision-resistant
(pre-clean unique service names before write), but it does not block F6.

## F6 Gate Summary

| Gate | Status |
|------|--------|
| Mutation kill rate ≥90% on changed files | PASS — 100% (3/3 caught) |
| Kani proofs (or justified skip) | PASS — justified skip |
| Fuzz testing (or justified skip) | PASS — justified skip |
| No unresolved CRITICAL/HIGH security findings | PASS — cargo audit + cargo deny clean |
| Full regression suite | PASS — clippy/fmt clean; only a pre-existing out-of-scope env flake |
| Hardening summary written | PASS — this document |

**F6 VERDICT: PASS.** Mutation kill rate meets target with zero surviving
mutants, security scans are clean with no new dependencies, and regression is
clean modulo a documented pre-existing environmental flake outside the delta
scope.
