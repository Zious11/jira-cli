# Evidence Report — S-2.03: BC-4 assets/CMDB Regression Holdout Suite

**Story:** S-2.03  
**Branch:** `test/S-2.03-bc-4-asset-enrichment-holdout-suite`  
**Test file:** `tests/asset_holdouts.rs`  
**Activation HEAD:** `dea1664`  
**Evidence recorded:** 2026-05-07  
**VHS available:** Yes (`/opt/homebrew/bin/vhs`)

---

## Coverage Summary

| AC | Holdout | BC | Test Name | Status | Evidence Type |
|----|---------|----|-----------|--------|---------------|
| AC-001 | H-037 | BC-4.2.001 | `test_s_2_03_h_037_bc_4_2_001_workspace_id_cached_after_first_call` | PASS | Transcript |
| AC-002 | H-038 | BC-4.3.002 | `test_s_2_03_h_038_bc_4_3_002_enrich_assets_skips_already_resolved` | PASS | Transcript |
| AC-003 | H-039 | BC-4.2.006 | `test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64` | PASS | Transcript + VHS |

---

## AC-001 / H-037 / BC-4.2.001 — Workspace ID cached after first call

**Behavioral contract:** The workspace ID is cached after the first discovery call.
Subsequent calls within the 7-day TTL window return the cached ID without making
any HTTP request to the workspace discovery endpoint.

**Test:** `test_s_2_03_h_037_bc_4_2_001_workspace_id_cached_after_first_call`

**Verification command:**
```
cargo test --test asset_holdouts test_s_2_03_h_037_bc_4_2_001_workspace_id_cached_after_first_call -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running tests/asset_holdouts.rs (target/debug/deps/asset_holdouts-3e93ee528c8a4e20)

running 1 test
test test_s_2_03_h_037_bc_4_2_001_workspace_id_cached_after_first_call ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.70s
```

**Why it passes:** Two consecutive `jr assets search` invocations share the same
`XDG_CACHE_HOME`; wiremock's `expect(1)` on the workspace discovery endpoint confirms
that only the first invocation fires an HTTP call — the second serves the workspace ID
from the on-disk cache (`~/.cache/jr/v1/default/workspace.json`).

**What behavior it pins:** `src/api/assets/workspace.rs::get_or_fetch_workspace_id`
cache-hit branch. Removing or bypassing the cache check would cause two workspace
discovery requests instead of one, burning an extra API round-trip on every command.

---

## AC-002 / H-038 / BC-4.3.002 — enrich_assets skips already-resolved assets

**Behavioral contract:** In `enrich_assets`, assets that already have both `key` and
`name` populated skip the GET fetch step. Only assets with id-only data are enriched
via HTTP.

**Test:** `test_s_2_03_h_038_bc_4_3_002_enrich_assets_skips_already_resolved`

**Verification command:**
```
cargo test --test asset_holdouts test_s_2_03_h_038_bc_4_3_002_enrich_assets_skips_already_resolved -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s
     Running tests/asset_holdouts.rs (target/debug/deps/asset_holdouts-3e93ee528c8a4e20)

running 1 test
test test_s_2_03_h_038_bc_4_3_002_enrich_assets_skips_already_resolved ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s
```

**Why it passes:** The test calls `enrich_assets` directly (library-level; `enrich_assets`
is declared `pub` in `src/api/assets/linked.rs`). Asset A (id-only) hits the GET endpoint
(`expect(1)`); asset B (id + key + name already set) does not (`expect(0)`). wiremock
`server.verify()` confirms exactly 1 GET request was made. After enrichment, asset A has
`key = "A-001"` and `name = "Resolved Asset A"`; asset B's fields are unchanged.

**What behavior it pins:** The `a.key.is_none() && a.name.is_none()` filter in
`src/api/assets/linked.rs::enrich_assets`. Removing this guard would cause unnecessary
GET requests for all assets, potentially overwriting already-enriched data.

---

## AC-003 / H-039 / BC-4.2.006 — assets tickets ambiguous --status exits 64

**Behavioral contract:** `jr assets tickets OBJ-1 --status <substring>` that matches
multiple status strings rejects with exit code 64, "Ambiguous status" in stderr, and
all matching candidates listed in stderr.

**Test:** `test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64`

**Verification command:**
```
cargo test --test asset_holdouts test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64 -- --nocapture
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.09s
     Running tests/asset_holdouts.rs (target/debug/deps/asset_holdouts-3e93ee528c8a4e20)

running 1 test
test test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.69s
```

**Why it passes:** The test spawns `jr assets tickets OBJ-1 --status PROG` against a
wiremock server with two connected tickets whose statuses are "In Progress" and
"Progressing". Both match the substring "PROG" (case-insensitive), so
`partial_match::partial_match` returns `MatchResult::Ambiguous`. The `filter_tickets`
function in `src/cli/assets.rs` surfaces this as a `JrError::UserError` (exit code 64)
with both candidate names in stderr.

**What behavior it pins:** The `Ambiguous` branch in `src/cli/assets.rs::filter_tickets`.
Removing or loosening this check (e.g., auto-selecting the first match) would silently
return incorrect filtered results rather than rejecting the ambiguous input.

**VHS recording:**
- `AC-003-ambiguous-status.tape` — VHS script source
- `AC-003-ambiguous-status.gif` — terminal recording (184 KB)
- `AC-003-ambiguous-status.webm` — archival recording (422 KB)

The recording runs the AC-003 test via `cargo test --test asset_holdouts ... --nocapture`,
which spawns `jr` through wiremock and captures the disambiguated error path.

---

## Combined Run

**Verification command:**
```
cargo test --test asset_holdouts
```

**Captured output (verbatim):**
```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.12s
     Running tests/asset_holdouts.rs (target/debug/deps/asset_holdouts-3e93ee528c8a4e20)

running 3 tests
test test_s_2_03_h_038_bc_4_3_002_enrich_assets_skips_already_resolved ... ok
test test_s_2_03_h_039_bc_4_2_006_assets_tickets_ambiguous_status_exits_64 ... ok
test test_s_2_03_h_037_bc_4_2_001_workspace_id_cached_after_first_call ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s
```

Full transcript also available at: `docs/demo-evidence/S-2.03/combined-transcript.txt`

---

## Files in This Directory

| File | Description |
|------|-------------|
| `evidence-report.md` | This report — AC coverage, transcripts, rationale |
| `combined-transcript.txt` | Verbatim `cargo test --test asset_holdouts` output |
| `AC-003-ambiguous-status.tape` | VHS script for AC-003 recording |
| `AC-003-ambiguous-status.gif` | VHS-generated terminal recording (PR embed) |
| `AC-003-ambiguous-status.webm` | VHS-generated terminal recording (archival) |

Note: AC-001 and AC-002 use transcript-only evidence. These are wiremock + library-level
integration tests with no interactive CLI surface; a `cargo test` transcript is more
informative than a VHS recording of the same text. AC-003 has a user-facing CLI error
path (`Ambiguous status`) and is additionally evidenced with a VHS recording.
