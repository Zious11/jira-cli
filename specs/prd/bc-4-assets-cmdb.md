---
context: bc-4
title: "Assets & CMDB"
total_bcs: 32   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 22   # count of `#### BC-` headings in this file
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-04-assets-cmdb.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.4
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.3
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.7
---

# BC-4 — Assets & CMDB

32 behavioral contracts across 4 subdomains: AQL / CMDB field resolution (4.1),
Asset search & view (4.2), Asset enrichment — MUST-FIX (4.3), Error handling (4.4).

---

## Subdomains

### 4.1 AQL / CMDB Field Resolution

#### BC-4.1.001: `find_cmdb_fields()` filters by `schema.custom == "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"`

**Confidence**: HIGH
**Source**: `tests/cmdb_fields.rs:50-83`
**Subject**: Assets/CMDB
**Behavior**: Returns `Vec<(String, String)>` of (id, name) tuples matching ONLY CMDB custom fields. Story points and summary are filtered out. Filter is schema.custom string, NOT name-based heuristic.
**Edge cases**: empty result when no CMDB fields exist.
**Trace**: Pass 3 BC-301; BC-1137a (R4)

---

#### BC-4.1.002: `build_asset_clause` for single CMDB field emits `"<NAME>" IN aqlFunction("Key = \"<KEY>\"")`  (NO outer parens)

**Confidence**: HIGH
**Source**: `src/jql.rs:61-82`
**Subject**: Assets/CMDB
**Behavior**: Single-field branch returns `clauses.into_iter().next().unwrap()` — no outer parens. For field `("customfield_10191", "Client")` and key `"CUST-5"` → `"Client" IN aqlFunction("Key = \"CUST-5\"")`. LHS is field NAME not id. AQL attribute is capital `Key` (NOT `objectKey`).
**Trace**: Pass 3 BC-306, BC-306-R (R1); CLAUDE.md gotcha

---

#### BC-4.1.003: `build_asset_clause` uses `escape_value` for BOTH field name AND asset key

**Confidence**: HIGH
**Source**: `src/jql.rs:67-74`
**Subject**: Assets/CMDB
**Behavior**: Both name and key go through `escape_value`. JQL injection via field names or keys is structurally prevented.
**Trace**: Pass 3 BC-307, BC-307-R (R1)

---

#### BC-4.1.004: Two CMDB fields → parenthesized OR-join: `("X" IN aqlFunction(...) OR "Y" IN aqlFunction(...))`

**Confidence**: HIGH
**Source**: `src/jql.rs:77-81`
**Trace**: Pass 3 BC-308, BC-308-R (R1)

---

#### BC-4.1.005: `validate_asset_key("CUST-5")` → Ok; `"CUST"` → Err; `"5-CUST"` → Err

**Confidence**: HIGH
**Source**: `src/jql.rs:39-54`
**Subject**: Assets/CMDB
**Behavior**: ASCII alphanumeric prefix + `-` + ASCII digit suffix, both nonempty.
**Trace**: Pass 3 BC-309

---

#### BC-4.1.006: `extract_linked_assets` reads `[{label, objectKey}]` shape → `LinkedAsset{key, name}`

**Confidence**: HIGH
**Source**: `tests/cmdb_fields.rs:86-118`
**Subject**: Assets/CMDB
**Behavior**: `"customfield_10191": [{"label": "Acme Corp", "objectKey": "OBJ-1"}]` → `LinkedAsset { name: Some("Acme Corp"), key: Some("OBJ-1"), ... }`.
**Trace**: Pass 3 BC-302; BC-1137c (R4)

---

#### BC-4.1.007: `extract_linked_assets` returns empty Vec for null custom field value

**Confidence**: HIGH
**Source**: `tests/cmdb_fields.rs:120-146`
**Trace**: Pass 3 BC-303; BC-1137d (R4); BC-324 (R1)

---

### 4.2 Asset Search & View

#### BC-4.2.001: `assets search` discovers workspace ID first (cache or API)

**Confidence**: HIGH
**Source**: `tests/assets.rs:1489-1496`; `src/api/assets/workspace.rs`
**Subject**: Assets/CMDB
**Behavior**: GET `/rest/servicedeskapi/assets/workspace` → `{values: [{workspaceId: "ws-123"}]}`. Cached as `WorkspaceCache` with 7d TTL. Cache hit → no HTTP.
**Trace**: Pass 3 BC-310; BC-322 (R1)

---

#### BC-4.2.002: `client.search_assets(workspace_id, aql, limit, include_attrs)` POSTs to `/jsm/assets/workspace/<id>/v1/object/aql`

**Confidence**: HIGH
**Source**: `tests/assets.rs:39-80, 238-295`
**Subject**: Assets/CMDB
**Behavior**: Query params: `startAt=0`, `maxResults=25` (asset-specific page size, NOT 50), `includeAttributes=false|true`. Pagination advances `startAt` by 25 per page (offset, not cursor).
**Trace**: Pass 3 BC-316 (R1)

---

#### BC-4.2.003: `AssetsPage::is_last` accepts both bool and string-encoded bool `"true"`

**Confidence**: HIGH
**Source**: `tests/assets.rs:140-170`
**Subject**: Assets/CMDB
**Behavior**: Custom deserializer handles `"isLast": true` AND `"isLast": "true"`.
**Trace**: Pass 3 BC-317 (R1)

---

#### BC-4.2.004: `client.get_asset(workspace_id, id, include_attrs=true)` GETs `/jsm/assets/workspace/<id>/v1/object/<oid>?includeAttributes=true`

**Confidence**: HIGH
**Source**: `tests/assets.rs:172-203`
**Trace**: Pass 3 BC-318 (R1)

---

#### BC-4.2.005: `client.get_connected_tickets(workspace_id, oid)` GETs `/jsm/assets/workspace/<id>/v1/objectconnectedtickets/<oid>/tickets`

**Confidence**: HIGH
**Source**: `tests/assets.rs:205-236`
**Behavior**: Returns `{tickets: [...], allTicketsQuery: Option<String>}`. `tickets[].status.colorName` present.
**Trace**: Pass 3 BC-319 (R1)

---

#### BC-4.2.006: `assets tickets <KEY> --status PROG` ambiguous → exit 64 `Ambiguous status` + both candidates

**Confidence**: HIGH
**Source**: `tests/assets.rs:1579-1684`
**Behavior**: Workspace → resolve_object_key → connected_tickets endpoint. `partial_match` returns `Ambiguous` on two-match. Literal stderr `"Ambiguous status"`, `"In Progress"`, `"Progressing"`.
**Trace**: Pass 3 BC-320 (R1)

---

#### BC-4.2.007: `assets schema <TYPE-SUBSTR>` ambiguous → exit 64 `Ambiguous type` + NO per-type attribute fetch

**Confidence**: HIGH
**Source**: `tests/assets.rs:1695-1799`
**Behavior**: `Mock::expect(0)` on per-type attribute endpoints. Short-circuit before expensive fetch.
**Trace**: Pass 3 BC-321 (R1)

---

#### BC-4.2.008: `assets tickets --open` filters `status.colorName != "green"` (client-side)

**Confidence**: MEDIUM
**Source**: `src/cli/assets.rs:303-321`; unit tests
**Behavior**: Tickets with no status are included under `--open`, excluded under `--status`. Client-side color filter.
**Trace**: Pass 3 BC-314

---

#### BC-4.2.009: `assets tickets --open` and `--status` clap conflict

**Confidence**: HIGH
**Source**: `tests/cli_smoke.rs:51-58`
**Trace**: Pass 3 BC-315

---

### 4.3 Asset Enrichment (MUST-FIX: NFR-R-E)

#### BC-4.3.001: Asset enrichment `resolved` HashMap MUST be keyed by `(workspace_id, oid)` not `oid` alone [MUST-FIX: NFR-R-E]

**Confidence**: HIGH
**Source**: `src/cli/issue/list.rs:440, 446, 449, 456` (BUG SITES)

> **MUST-FIX (HIGH — NFR-R-E):** Current code at line 446 creates `resolved: StdHashMap<String, _>` keyed
> by `oid` alone. Multi-workspace tenants sharing `oid` values across workspaces experience
> last-write-wins mis-attribution. The separate `api/assets/linked.rs::enrich_assets` is CORRECT
> (uses composite key). This contract describes the FIXED behavior.

**Spec contract (fixed behavior):**
- `to_enrich: HashMap<(String, String), ()>` — correctly uses composite `(wid, oid)` key ✓ (already correct at line 398)
- `resolved: HashMap<(String, String), (String, String, String)>` — MUST ALSO use composite key (currently broken)
- Line 449: `resolved.insert((wid.clone(), oid.clone()), ...)` — workspace preserved
- Line 456: `resolved.get(&(wid.clone(), oid.clone()))` — workspace-qualified lookup

**Effects**: Multi-workspace tenants see correct asset names. Single-workspace tenants unaffected.
**Trace**: Pass 3 BC-147 (R1); NFR-R-E; Pass 4 R4 §1.4

---

#### BC-4.3.002: `enrich_assets(client, &mut [LinkedAsset])` resolves ONLY assets with `id.is_some() && key.is_none() && name.is_none()`

**Confidence**: HIGH
**Source**: `tests/cmdb_fields.rs:148-189`
**Subject**: Assets/CMDB
**Behavior**: Only id-only assets are re-fetched. Assets with name/key already populated skip the GET. After enrichment: `key = Some("OBJ-88")`, `name = Some("Acme Corp")`, `asset_type = Some("Client")`.
**Trace**: Pass 3 BC-304; BC-323 (R1); BC-1137e (R4)

---

#### BC-4.3.003: `LinkedAsset::display()` falls back to `#<id> (run 'jr init' to resolve asset names)` when only id present

**Confidence**: HIGH
**Source**: `src/types/assets/linked.rs::tests::display_id_fallback_with_hint`
**Trace**: Pass 3 BC-305

---

### 4.4 Asset Error Handling

#### BC-4.4.001: `assets search` 5xx → exit 1 + `API error (500)` + no panic

**Confidence**: HIGH
**Source**: `tests/assets_errors.rs:21-64`; `tests/assets_errors.rs:20-153` (BC-1136 R4)
**Subject**: Assets/CMDB
**Behavior**: Workspace discovery is first call; errors there propagate same as direct-issue endpoints.
**Trace**: Pass 3 BC-311; BC-1136 (R4)

---

#### BC-4.4.002: `assets search` 401 → exit 2 + `Not authenticated` + `jr auth login`

**Confidence**: HIGH
**Source**: `tests/assets_errors.rs:67-113`
**Trace**: Pass 3 BC-312

---

#### BC-4.4.003: `assets search` network drop → exit 1 + `Could not reach`

**Confidence**: HIGH
**Source**: `tests/assets_errors.rs:116-153`
**Trace**: Pass 3 BC-313

---

## Key Invariants

- AQL attribute for object key: capital `Key` (NOT `objectKey`) — CLAUDE.md gotcha
- `aqlFunction()` LHS: field NAME, not `cf[ID]` or `customfield_NNNNN` — CLAUDE.md gotcha
- Asset page size: 25 (NOT 50 like Jira)
- `is_last` is bool-or-string (tolerance via custom deserializer)
- Workspace ID: discovered via JSM REST, cached 7d per profile
