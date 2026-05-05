---
title: "BC-04: Assets & CMDB"
version: "1.0.0"
snapshot_sha: "dea166471e22eff55974d7675593469b37048c5f"
traces_to: "README.md"
source_passes: "Pass 2 broad §2a.2 Assets + R1 §3.2 T-03 + Pass 8 §2.2 BC#4,5"
entity_count: 18
invariant_count: 18
bc_count: 32
risk_level: HIGH
---

# BC-04: Assets & CMDB

The Assets/CMDB bounded context covers two surfaces:
1. **Standalone assets** (`jr assets *`) — AQL search, object view, schemas, types, tickets.
2. **Issue-linked CMDB enrichment** (`jr issue list --assets`, `jr issue assets KEY`) — extraction of CMDB objects referenced in issue custom fields.

The workspace-ID discovery uses a JSM REST endpoint, making this context architecturally dependent on JSM.

---

## §1 Ubiquitous Language

| Term | Definition |
|------|-----------|
| **Workspace ID** | UUID for an Assets/CMDB workspace. Discovered via `GET /rest/servicedeskapi/assets/workspace` (JSM endpoint). Cached per profile. |
| **AQL** | Assets Query Language. Used for `jr assets search`. Distinct from JQL. |
| **Object key** | Validated string `<alphanumeric>-<digits>` (e.g., `CUST-5`). The `objectKey` JSON field on API responses. The AQL attribute is capitalized `Key` (not `objectKey`). |
| **CMDB field** | A Jira custom field of type `com.atlassian.jira.plugins.cmdb:cmdb-object-cftype`. Discovered via `client.list_fields()` + schema.custom filter. |
| **`aqlFunction()`** | The Jira JQL function that bridges issue search to AQL. LHS must be the field NAME (not `cf[ID]` or `customfield_NNNNN`). |
| **`LinkedAsset`** | Display-side serialize-only struct bridging Jira and Assets contexts. Has `display()` with graceful `"run jr init"` fallback when only `id` is present. |
| **3-pass enrichment** | Pass 1: extract + workspace-qualified dedup. Pass 2: concurrent `join_all` fetch. Pass 3: redistribute. |
| **Multi-workspace bug** | NFR-R-E: `resolved` HashMap in `issue list` keyed by `oid` alone, dropping workspace qualifier. Affects multi-workspace tenants. |
| **`colorName`** | The `TicketStatus.color_name` field carrying Jira status-category colour tokens (`green`/`yellow`/`blue-gray`). Only place colour appears as first-class data (Jira issues use `statusCategory.key` instead). |
| **`--open` (assets)** | Connected-tickets `--open` filters `status.colorName != "green"` (client-side, colour-based). Different from issue-list `--open` (JQL `statusCategory != Done`). |

---

## §2 Entities

Source: Pass 2 broad §2a.2 Assets context.

| Entity | Module | Key Fields | Notes |
|--------|--------|-----------|-------|
| `AssetObject` | `types/assets/object.rs:3-15` | `id: String`, `label: String`, `object_key: String`, `object_type: ObjectType`, `created`, `updated`, `attributes: Vec<AssetAttribute>` | Aggregate root for Assets context. Read-only. |
| `ObjectType` | `types/assets/object.rs:17-22` | `id: String`, `name: String`, `description: Option<String>` | Inline on `AssetObject`. Also fetched standalone. |
| `AssetAttribute` | `types/assets/object.rs:24-31` | `id: String`, `object_type_attribute_id: String`, `values: Vec<ObjectAttributeValue>` | Inline on AQL search results. Has only numeric `objectTypeAttributeId`; name requires enrichment via `get_object_type_attributes`. |
| `ObjectAttribute` | `types/assets/object.rs:43-52` | `id: String`, `object_type_attribute_id: String`, `object_type_attribute: ObjectTypeAttributeDef`, `values: Vec<ObjectAttributeValue>` | "Fat" attribute shape returned by `GET /object/{id}/attributes` — name resolution baked in. |
| `ObjectTypeAttributeDef` | `types/assets/object.rs:55-81` | `id`, `name`, `system`, `hidden`, `label`, `position`, `default_type`, `reference_type`, `reference_object_type`, `minimum_cardinality`, `maximum_cardinality`, `editable`, `description`, `options` | Schema metadata. Most fields default → old JSON without them deserializes cleanly. |
| `DefaultType` | `types/assets/object.rs:84-88` | `id: i32`, `name: String` | E.g., `{0, "Text"}`, `{10, "Select"}`. |
| `ReferenceType` | `types/assets/object.rs:91-95` | `id: String`, `name: String` | E.g., "Depends on", "References". |
| `ReferenceObjectType` | `types/assets/object.rs:98-102` | `id: String`, `name: String` | Target type a reference attribute points at. |
| `ObjectAttributeValue` | `types/assets/object.rs:33-38` | `value: Option<String>`, `display_value: Option<String>` | Single attribute cell. |
| `ObjectSchema` | `types/assets/schema.rs:5-15` | `id: String`, `name: String`, `object_schema_key: String`, `description`, `object_count`, `object_type_count` | Top-level CMDB schema container. Read-only. |
| `ObjectTypeEntry` | `types/assets/schema.rs:18-33` | `id`, `name`, `description`, `position`, `object_count`, `object_schema_id`, `inherited`, `abstract_object_type` | Returned by `/objectschema/{id}/objecttypes/flat`. Distinct from `ObjectType` (inline reference inside `AssetObject`). |
| `LinkedAsset` | `types/assets/linked.rs:4-17` | `key: Option<String>`, `name: Option<String>`, `asset_type: Option<String>`, `id: Option<String>`, `workspace_id: Option<String>` | Serialize-only (no `Deserialize`). All fields `Option` — partial extraction is normal. Created by extraction from `IssueFields::extra`. |
| `ConnectedTicketsResponse` | `types/assets/ticket.rs:3-9` | `tickets: Vec<ConnectedTicket>`, `all_tickets_query: Option<String>` | `all_tickets_query` carries opaque `issueFunction in assetsObject(...)` JQL for follow-up. |
| `ConnectedTicket` | `types/assets/ticket.rs:11-23` | `key`, `id`, `title`, `reporter`, `created`, `updated`, `status: Option<TicketStatus>`, `issue_type: Option<TicketType>`, `priority: Option<TicketPriority>` | Distinct from `Issue` — uses `title` not `summary`; `TicketStatus` carries `color_name` not `statusCategory`. |
| `TicketStatus` | `types/assets/ticket.rs:25-30` | `name: String`, `color_name: Option<String>` | ONLY place colour names appear as first-class data. Used for `--open` filter in `cli/assets.rs`. |
| `TicketType` | `types/assets/ticket.rs:32-35` | `name: String` | — |
| `TicketPriority` | `types/assets/ticket.rs:37-40` | `name: String` | — |
| `WorkspaceCache` | `cache.rs:175-185` | `workspace_id: String`, `fetched_at: DateTime<Utc>` | Whole-file `workspace.json`, 7-day TTL. |

---

## §3 Value Objects & Enums

- **`AssetsPage<T>`**: pagination shape for Assets API. `is_last` accepts both bool and string (`"true"`/`"false"`) via custom deserializer.
- **`MAX_ASSETS_PAGE = 25`** (`api/assets/objects.rs:26`): cap on AQL search page count. NEW-INV-06.
- **`parse_cmdb_value`**: requires at least one of `{label, objectKey, objectId}` in the raw JSON array element. Missing all three → skip (NEW-INV-05).
- **CMDB field type discriminator**: `schema.custom == "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"` — schema-based (not name-heuristic). BC-1137a.

---

## §4 Operations

| Command | HTTP | Notes |
|---------|------|-------|
| `assets search <aql>` | `POST /jsm/assets/workspace/<wid>/v1/object/aql` | Auto-paginated (up to MAX_ASSETS_PAGE=25). `AssetsPage::is_last` tolerance. |
| `assets view <key>` | `GET /object/<id>?includeAttributes=...` | Numeric input → use as object-id. Non-numeric → AQL key→id resolution. |
| `assets tickets <key>` | Connected-tickets endpoint + optional status filter | `--open` filters `colorName != "green"` (client-side). |
| `assets schemas` | `GET /jsm/assets/workspace/<wid>/v1/objectschema/list` | — |
| `assets types [--schema X]` | Per-schema: `GET /objectschema/<id>/objecttypes/flat` | — |
| `assets schema <name>` | `GET /objecttype/<id>/attributes` | Cached as `ObjectTypeAttrCache`, 7-day TTL. |
| `issue list --assets` | (see BC-02) | 3-pass dedup-and-concurrent enrichment. |
| `issue assets <key>` | `GET /issue/<key>` + CMDB field discovery + per-field `GET object/<key>` | — |
| Workspace discovery | `GET /rest/servicedeskapi/assets/workspace` | JSM REST endpoint (not Assets-native). Cached. 403/404 → "JSM Premium/Enterprise required" user error. |

---

## §5 Business Rules & Invariants

| ID | Invariant | Source |
|----|----------|--------|
| INV-ASSETS-001 | AQL `aqlFunction()` in Jira JQL uses the human-readable field NAME (not `cf[ID]` or `customfield_NNNNN`). The `(id, name)` tuple's `id` is destructured-and-ignored at the call site. | `jql.rs:67-74`, BC-308/309 |
| INV-ASSETS-002 | AQL attribute for asset key is `Key` (capital K), NOT `objectKey`. Hardcoded literal in `jql.rs:70`. | `jql.rs:70` |
| INV-ASSETS-003 | Workspace discovery uses `GET /rest/servicedeskapi/assets/workspace` — a JSM endpoint, not an Assets endpoint. The cross-context dependency is by Atlassian API design. | `api/assets/workspace.rs` |
| INV-ASSETS-004 | `LinkedAsset::display()` falls back to `#<id> (run "jr init" to resolve asset names)` when only `id` is present. Serialize-only — never deserialized from API. | `types/assets/linked.rs:19-44` |
| INV-ASSETS-005 | `parse_cmdb_value` requires at least one of `{label, objectKey, objectId}` in raw JSON. Missing all three → skip silently. | `api/assets/linked.rs:87-89`, NEW-INV-05 |
| INV-ASSETS-006 | `AssetsPage::is_last` tolerates both bool and string "true"/"false" via custom serde deserializer. The max page count is 25 regardless. | `api/assets/objects.rs:26`, NEW-INV-06 |
| INV-ASSETS-007 | 3-pass enrichment dedup key is `(workspace_id, object_id)`. Pass 1 correctly workspace-qualifies. Pass 2 (`join_all`) resolves M unique pairs concurrently. | `cli/issue/list.rs:398-445`, NEW-INV-227,228 |
| INV-ASSETS-008 | NFR-R-E: `resolved` HashMap in Pass 3 is keyed by `oid` alone (NOT `(wid, oid)`). Last-write-wins on colliding oid across workspaces. Single-workspace tenants unaffected. | `cli/issue/list.rs:446,449,456`, NEW-INV-229 |
| INV-ASSETS-009 | Connected-tickets `--open` filters `status.colorName != "green"` (client-side, colour-based). Tickets with no status are INCLUDED by `--open` (`.unwrap_or(true)`). | `cli/assets.rs:303-321` |
| INV-ASSETS-010 | Connected-tickets `--status` filters via `partial_match` on status names. Tickets with no status are EXCLUDED by `--status`. | `cli/assets.rs:303-321` |
| INV-ASSETS-011 | `--open` in assets context (colour-based) vs `--open` in issue context (JQL `statusCategory != Done`) — two distinct mechanisms. Same flag name, different implementation. | — |
| INV-ASSETS-012 | CMDB field discovery uses schema type discriminator: `schema.custom == "com.atlassian.jira.plugins.cmdb:cmdb-object-cftype"`. NOT a name-based heuristic. | `api/assets/linked.rs` (BC-1137a) |
| INV-ASSETS-013 | Workspace ID is fetched lazily — only when enrichment targets are non-empty. Zero-cost for issues without CMDB fields. | `cli/issue/list.rs:415`, NEW-INV-230 |
| INV-ASSETS-014 | `CmdbFieldsCache` stores `Vec<(String, String)>` (id, name tuples). Old format (ID-only) causes deserialization failure → cache miss. If cache format changes again, bump `v1/` → `v2/`. | `cache.rs:237-247` |
| INV-ASSETS-015 | `ObjectTypeAttrCache` is a map cache keyed by object-type id, with file-level TTL (not per-entry TTL). | `cache.rs:278-282` |
| INV-ASSETS-016 | `assets view <key>` distinguishes numeric input (use as object-id directly) from alphanumeric (AQL key→id resolution). Input heuristic is `str::parse::<u64>()`. | `cli/assets.rs` |
| INV-ASSETS-017 | `ObjectTypeAttributeDef` most fields default → old JSON without them deserializes cleanly. Backward-compatibility by design. | `types/assets/object.rs:55-81` |
| INV-ASSETS-018 | Workspace 403/404 is mapped to a user-facing "Assets not available on this Jira site (requires JSM Premium/Enterprise)" error, not a generic API error. | `api/assets/workspace.rs`, Pass 8 §2.2 BC#4 |

---

## §6 Aggregate Boundaries

- **`AssetObject`** is the Assets-context aggregate root. Owns `ObjectType` (1:1) and `AssetAttribute[]`.
- **`ObjectSchema → ObjectTypeEntry`** is the discovery aggregate (one schema, many types).
- **`LinkedAsset`** is a leaf-only display projection with no aggregate behaviour.
- **`ConnectedTicket`** is distinct from `Issue` — different struct (uses `title` not `summary`, carries `colorName`).

---

## §7 Cross-Context Dependencies

| Depends on | Reason |
|-----------|--------|
| **Issue Read (BC-02)** | `IssueFields::extra` (the flatten container) is the source of CMDB field values. |
| **JSM** (not a full BC; folded here) | Workspace ID discovery uses `GET /servicedeskapi/assets/workspace`. |
| **Cache (BC-06)** | `workspace.json`, `cmdb_fields.json`, `object_type_attrs.json` per profile. |
| **Cross-cutting** | `jql.rs` for AQL clause building (`build_asset_clause`, `validate_asset_key`). |
| **Output (BC-07)** | Table/JSON rendering. |
