# ADR-0008: Asset Enrichment HashMap Key Correctness

## Status
Accepted

## Context

A HIGH-severity correctness bug was discovered in the asset enrichment 3-pass dataflow
in `src/cli/issue/list.rs`. The bug is a workspace-qualifier mismatch between the
deduplication map and the result map.

**Bug anatomy:**

The enrichment flow has three passes:

- **Pass 1 (extract):** deduplication key is `(workspace_id, object_id)` — a qualified
  2-tuple. `to_enrich: HashMap<(String, String), ()>`. Correct.
- **Pass 2 (resolve):** N concurrent `client.get_asset(wid, oid)` calls via
  `futures::future::join_all`. Results deposited into `resolved: HashMap<String, (key, name, type)>`
  keyed by `oid` alone. **The workspace qualifier is silently dropped.**
- **Pass 3 (redistribute):** redistribution loop uses `resolved.get(oid)` — the
  single-key lookup. For multi-workspace tenants where two distinct assets share an `oid`
  across workspaces, the second insertion in Pass 2 overwrites the first; Pass 3 silently
  misattributes enrichment data.

**Impact:** Single-workspace tenants are completely unaffected — `oid` is unique within a
workspace. Multi-workspace tenants with overlapping `oid` values across workspaces get
silently wrong asset names and keys in issue lists.

**Note:** The implementation in `src/api/assets/linked.rs::enrich_assets` is correct.
The bug is exclusively in `src/cli/issue/list.rs` at the 3-pass composition level.

## Decision

Change the `resolved` HashMap key type from `String` (oid only) to `(String, String)`
(workspace_id, oid) at all affected sites in `src/cli/issue/list.rs`. The redistribution
lookup in Pass 3 changes from `resolved.get(oid)` to
`resolved.get(&(wid.clone(), oid.clone()))`.

## Rationale

- This is the minimal correct fix. The deduplication key in Pass 1 already carries the
  workspace qualifier; extending it to the result map in Pass 2 and the lookup in Pass 3
  restores semantic consistency across all three passes.
- No architectural refactoring is needed. The 3-pass structure is correct; only the key
  type at one data structure is wrong.
- The fix is backward-compatible: single-workspace tenants are unaffected because their
  workspace_id is constant, so `(wid, oid)` deduplicates identically to `oid` alone.

## Consequences

- **Fix scope:** ~5 LOC change at 3 sites in `src/cli/issue/list.rs`.
- **Regression risk:** LOW for single-workspace tenants (no behavior change). NONE for
  multi-workspace tenants (they currently see wrong data; the fix makes it correct).
- **Test requirement:** Add a multi-workspace fixture integration test that presents two
  assets with the same `oid` in different workspaces and verifies that both enrichment
  results appear correctly in issue list output.
- **`src/api/assets/linked.rs::enrich_assets`** (the concurrent enrichment helper for
  standalone `jr assets` commands) is already correct and is not affected by this ADR.
- **BC anchor:** BC-4.3.001.

## See Also

- `src/cli/issue/list.rs` — 3-pass enrichment composition (affected)
- `src/api/assets/linked.rs::enrich_assets` — standalone assets enrichment (correct, unaffected)
- ADR-0005 — GraphQL org discovery (multi-workspace context)
