# S-3.02 Demo Evidence Report

Story: Refactor `cli/assets.rs` — shard-split into 5 module files
Story ID: S-3.02
Branch: refactor/S-3.02-cli-assets-shard-split
Base SHA: fba47ad (develop at story branch-off)
Mode: strict/refactor (pure refactor — zero behavioral changes)
Recorded: 2026-05-09

---

## What was delivered

S-3.02 is a pure refactor story. The original `src/cli/assets.rs` (1,055 LOC monolith)
was split into 5 module files, all under `src/cli/assets/`:

| Shard | LOC | Contents |
|---|---|---|
| `mod.rs` | 65 | Dispatch + re-exports |
| `search.rs` | 158 | `handle_search` |
| `view.rs` | 91 | `handle_view` |
| `tickets.rs` | 285 | `handle_tickets` + `filter_tickets` (incl. `--open` colorName filter) |
| `schemas.rs` | 490 | `handle_schemas` + `handle_types` + `handle_schema` + `resolve_schema` |
| **Total** | **1,089** | (new total including module boilerplate) |

All shards are under the 600 LOC cap. The largest is `schemas.rs` at 490 LOC.
Zero behavioral changes: same CLI surface, same test counts, same exit codes.

Commits: `2f20052..fb7af06` (6 commits) on top of develop@fba47ad.

---

## Per-AC Evidence

### AC-001 — All tests pass (612 unit tests + H-037/H-038/H-039 holdouts)

**Spec claim:** Refactor does not break any existing tests. Unit test count stays at 612.
H-037/H-038/H-039 holdout tests pass.

| Artifact | Description |
|---|---|
| `AC-001-all-tests-green.gif` | `cargo test --test asset_holdouts` (tail -5) then `cargo test --lib` (tail -5) |
| `AC-001-all-tests-green.webm` | Same recording, archival format |
| `AC-001-all-tests-green.tape` | VHS script source |

**Confirmation:** Recording shows `test result: ok. 612 passed; 0 failed; 10 ignored` for
`--lib` and H-037/H-038/H-039 passing in `asset_holdouts`.

**Note:** The `auth_login_emits_json_when_output_json_set` integration test fails on macOS
with a pre-existing keychain `item already exists` flake. This is NOT caused by S-3.02
(that test is entirely unrelated to the assets module). The `--lib` (unit test) run is
unaffected; the integration test flake predates this branch and exists on develop too.

---

### AC-002 — Release build exits 0

**Spec claim:** `cargo build --release` succeeds (exit 0) after the shard-split. No
regressions in the compilation unit graph.

| Artifact | Description |
|---|---|
| `AC-002-release-build-green.gif` | `cargo build --release --quiet && echo "release build: OK"` |
| `AC-002-release-build-green.webm` | Same recording, archival format |
| `AC-002-release-build-green.tape` | VHS script source |

**Confirmation:** Recording shows `release build: OK` on stdout. Exit 0.

---

### AC-003 — All shards under 600 LOC

**Spec claim:** Every file under `src/cli/assets/` is strictly less than 600 LOC. The
largest shard (schemas.rs) is 490 LOC.

| Artifact | Description |
|---|---|
| `AC-003-shard-loc-under-600.gif` | `wc -l src/cli/assets/*.rs` then PASS echo |
| `AC-003-shard-loc-under-600.webm` | Same recording, archival format |
| `AC-003-shard-loc-under-600.tape` | VHS script source |

**Confirmation:** Recording shows the 5 file sizes:
- `mod.rs`: 65 LOC
- `schemas.rs`: 490 LOC
- `search.rs`: 158 LOC
- `tickets.rs`: 285 LOC
- `view.rs`: 91 LOC

All are < 600. Echo confirms `AC-003 PASS: largest shard 490 LOC < 600 cap`.

---

### AC-004 (bonus) — CLI assets subcommand surface unchanged

**Spec claim:** The public CLI dispatch is identical after the shard-split. The `assets`
subcommand still exposes: `search`, `view`, `tickets`, `schemas`, `types`, `schema`.

| Artifact | Description |
|---|---|
| `AC-004-cli-help-unchanged.gif` | `cargo run --release --quiet -- assets --help` (head -30) |
| `AC-004-cli-help-unchanged.webm` | Same recording, archival format |
| `AC-004-cli-help-unchanged.tape` | VHS script source |

**Confirmation:** Recording shows the full `assets --help` output with all 6 subcommands
visible. CLI surface is identical to pre-refactor.

---

### AC-005 (bonus) — `--open` filter survived the move into `tickets.rs`

**Spec claim:** The `--open` filter (client-side `color_name != "green"` check) that lived
in the original `cli/assets.rs` was correctly moved to `src/cli/assets/tickets.rs` with
no logic changes.

| Artifact | Description |
|---|---|
| `AC-005-open-filter-intact.gif` | `grep -nE "color_name" src/cli/assets/tickets.rs` then PASS echo |
| `AC-005-open-filter-intact.webm` | Same recording, archival format |
| `AC-005-open-filter-intact.tape` | VHS script source |

**Confirmation:** Recording shows `color_name` references at lines 27 and 169 in
`tickets.rs`, confirming the filter is present and in the correct shard.

---

## LOC Delta Summary

| File | Pre-refactor | Post-refactor |
|---|---|---|
| `src/cli/assets.rs` | 1,055 LOC | deleted (replaced by module) |
| `src/cli/assets/mod.rs` | — | 65 LOC |
| `src/cli/assets/search.rs` | — | 158 LOC |
| `src/cli/assets/view.rs` | — | 91 LOC |
| `src/cli/assets/tickets.rs` | — | 285 LOC |
| `src/cli/assets/schemas.rs` | — | 490 LOC |
| **Total (new)** | — | **1,089 LOC** |

The 34-LOC increase over the original 1,055 is from module boilerplate (`use` re-exports
in `mod.rs`, `pub use` statements, and one additional `#[cfg(test)] mod tests` block).

---

## Reproduction Commands

```bash
cd /Users/zious/Documents/GITHUB/jira-cli/.worktrees/S-3.02

# AC-001: All tests pass
cargo test --test asset_holdouts 2>&1 | tail -5
cargo test --lib 2>&1 | tail -5

# AC-002: Release build
cargo build --release --quiet && echo "release build: OK"

# AC-003: Shard LOC
wc -l src/cli/assets/*.rs

# AC-004: CLI surface unchanged
cargo run --release --quiet -- assets --help 2>&1 | head -30

# AC-005: --open filter intact
grep -nE "color_name" src/cli/assets/tickets.rs | head -5
```

---

## Caveats

None. This is a pure refactor. All outputs are deterministic and reproducible offline
(no network calls required for any of the 5 demos).

The `auth_login_emits_json_when_output_json_set` integration test keychain flake on macOS
is a pre-existing issue unrelated to S-3.02. It does not affect unit tests or this story's
acceptance criteria.
