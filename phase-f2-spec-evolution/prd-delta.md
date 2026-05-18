---
document_type: f2-prd-delta
phase: phase-f2-spec-evolution
issue: 288
producer: product-owner
timestamp: 2026-05-18
status: complete
---

# F2 PRD Delta — Issue #288

JSM request type support: `jr issue create --request-type` + `jr requesttype list/fields`.

---

## Summary

17 new BCs minted (F2) + 1 new BC (F1d: BC-3.8.010) = 18 total new BCs. 2 existing BCs modified (F2) + 6 existing BCs modified (F1d) = 8 total modified BCs. 3 holdout scenarios added (F2) + 1 holdout added (F1d: H-NEW-JSM-RT-004) + 1 holdout added (F1d pass-02: H-NEW-JSM-RT-005) = 5 total new holdouts.
Counts updated across BC-INDEX.md and CANONICAL-COUNTS.md.

---

## New BCs

### BC-3.8.001..010 — JSM Request Create (bc-3-issue-write.md)

| BC ID | Summary |
|-------|---------|
| BC-3.8.001 | `--request-type` dispatches to `POST /rest/servicedeskapi/request`; platform path unchanged when absent |
| BC-3.8.002 | JSM body uses `requestFieldValues`; `serviceDeskId` resolved via `require_service_desk` |
| BC-3.8.003 | Name resolution via `partial_match`; errors clean on Ambiguous/None with `jr requesttype list` hint |
| BC-3.8.004 | Numeric `--request-type <ID>` bypasses name resolution |
| BC-3.8.005 | `--summary` → `requestFieldValues.summary` (required) |
| BC-3.8.006 | `--description` → ADF; `--markdown` → `markdown_to_adf`; plain → `text_to_adf`; `isAdfRequest: true` |
| BC-3.8.007 | `--priority`/`--label` → `requestFieldValues.priority`/`requestFieldValues.labels` |
| BC-3.8.008 | `--field NAME=VALUE` (repeatable); first `=` splits; `customfield_NNNNN` bypasses lookup; duplicate last-wins |
| BC-3.8.009 | `--on-behalf-of <accountId>` → `raiseOnBehalfOf`; non-accountId rejected with `jr user search` hint |

### BC-X.12.001..008 — JSM Request Type Discovery (cross-cutting.md)

| BC ID | Summary |
|-------|---------|
| BC-X.12.001 | `jr requesttype list` → `GET .../servicedesk/<id>/requesttype` (paginated) |
| BC-X.12.002 | `--search <QUERY>` → `searchQuery` server-side param |
| BC-X.12.003 | `--project` overrides profile; `require_service_desk` errors clean on software project |
| BC-X.12.004 | `--output json` returns `[{id, name, description, helpText, issueTypeId, groupIds}]`; table: Name + Description |
| BC-X.12.005 | `jr requesttype fields <NAME\|ID>` → `GET .../requesttype/<rtId>/field` |
| BC-X.12.006 | Partial-name resolution via `partial_match`; ambiguity errors with hint |
| BC-X.12.007 | `--output json` for `fields` returns `{canRaiseOnBehalfOf, canAddRequestParticipants, fields: [...]}` |
| BC-X.12.008 | Request types cached per `(profile, serviceDeskId)`; TTL 7d; key `v1/<profile>/request_types_<sid>.json` |

---

## Modified BCs

| BC ID | File | Change |
|-------|------|--------|
| BC-1.3.023 | bc-1-auth-identity.md | `write:servicedesk-request` added to `DEFAULT_OAUTH_SCOPES`. Full scope string updated. Developer Console coordination note added. Pin test `default_oauth_scopes_pins_the_full_set_with_offline_access` update requirement documented. |
| BC-3.3.001 | bc-3-issue-write.md | Routing clause added: platform endpoint applies only when `--request-type` absent; JSM path when present (see BC-3.8.001). |
| BC-X.8.004 | cross-cutting.md | [UPDATED F1d] "Queue commands require…" string made caller-supplied context label; queue-specific error message now defined in BC-X.8.004; issue-create and requesttype call sites use different call-site-specific messages (see BC-3.8.002 and BC-X.12.003). |

---

## New Holdout Scenarios

Group 9 added to holdout-scenarios.md (50 → 55):

| ID | Title | BCs |
|----|-------|-----|
| H-NEW-JSM-RT-001 | JSM request creation routes to servicedeskapi; `expect(0)` on platform endpoint | BC-3.8.001, BC-3.8.002, BC-3.8.008 |
| H-NEW-JSM-RT-002 | `issue create --request-type` on software project exits 64 clean, zero POST | BC-3.8.002, BC-X.8.004 |
| H-NEW-JSM-RT-003 | 401 scope-mismatch surfaces `write:servicedesk-request` in recovery hint | BC-3.8.009, BC-X.3.005, BC-1.6.042, BC-1.3.023 |
| H-NEW-JSM-RT-004 | `--type` flag ignored with stderr warning when `--request-type` is set | BC-3.8.010, BC-3.8.001 |
| H-NEW-JSM-RT-005 | `jr requesttype fields` uses cache on second call — no extra HTTP | BC-X.12.005, BC-X.12.008 |

---

## Count Bumps

| File | Field | Before | After |
|------|-------|--------|-------|
| bc-3-issue-write.md | `total_bcs` | 78 | 88 |
| bc-3-issue-write.md | `definitional_count` | 49 | 59 |
| cross-cutting.md | `total_bcs` | 130 | 138 |
| cross-cutting.md | `definitional_count` | 64 | 72 |
| BC-INDEX.md | `total_bcs` | 548 | 566 |
| BC-INDEX.md | bc-3 section | 78 cumulative; 49 bodied | 88 cumulative; 59 bodied |
| BC-INDEX.md | cross-cutting section | 130 cumulative; 64 bodied | 138 cumulative; 72 bodied |
| CANONICAL-COUNTS.md | bc-3 definitional | 49 | 59 |
| CANONICAL-COUNTS.md | cross-cutting definitional | 64 | 72 |
| CANONICAL-COUNTS.md | total individually-bodied | 315 | 334 |
| CANONICAL-COUNTS.md | bc-3 total_bcs | 78 | 88 |
| CANONICAL-COUNTS.md | cross-cutting total_bcs | 130 | 138 |
| CANONICAL-COUNTS.md | grand total sum | 547 | 566 |
| holdout-scenarios.md | `total_holdouts` | 50 | 55 |

---

## Reviewers' Map

| File | Change Type | Lines (approx) |
|------|-------------|-----------------|
| `.factory/specs/prd/bc-3-issue-write.md` | Frontmatter updated; BC-3.3.001 body modified (routing clause); BC-3.8.001..010 section added; footer updated | BC-3.8 section: ~180 new lines |
| `.factory/specs/prd/cross-cutting.md` | Frontmatter updated; BC-X.12.001..008 section added before Key Invariants; BC-X.8.004 modified (call-site label contract); BC-X.12.003 modified (call-site-specific error message); BC-X.12.005 §Caching subsection added; BC-X.12.008 stale-cache-window + recovery hint extended | ~130 new lines |
| `.factory/specs/prd/bc-1-auth-identity.md` | BC-1.3.023 body updated (scope expansion, Developer Console note, update marker) | ~10 lines modified |
| `.factory/specs/prd/BC-INDEX.md` | Frontmatter counts; section 1.3 BC-1.3.023 title; section 3 header; BC-3.3.001 title; new 3.8 subsection; new X.12 subsection; coverage stats table | ~30 lines modified/added |
| `.factory/specs/prd/CANONICAL-COUNTS.md` | Per-file definitional table; per-file total_bcs table; grand total; holdout count; last_verified | ~20 lines modified |
| `.factory/specs/prd/holdout-scenarios.md` | `total_holdouts` frontmatter; Group 9 section with 5 new holdouts appended (H-NEW-JSM-RT-001..005) | ~80 new lines |
| `.factory/phase-f2-spec-evolution/prd-delta.md` | This file (new) | — |

---

## Open Questions / Scope Flags

None remaining. All F2 open questions were resolved by F1d passes 01–04 (see §Validated).

---

## Validated (F1d adversary pass-01, 2026-05-18)

The following Open Questions from the original F2 delta have been resolved via Perplexity validation and are now closed:

1. **RESOLVED — BC-3.8.007 labels wire shape**: Atlassian docs confirm `labels` is a plain string array `["alpha","beta"]` for both `POST /rest/api/3/issue` and `POST /rest/servicedeskapi/request` `requestFieldValues`. The `[{"name":"..."}]` object-array concern was unwarranted. BC-3.8.007 Confidence promoted MEDIUM→HIGH. Source: https://developer.atlassian.com/cloud/jira/platform/rest/v3/api-group-labels/

2. **RESOLVED — BC-3.8.009 accountId format validation**: Regex `[a-zA-Z0-9]{24}` removed from BC-3.8.009. Atlassian accountIds are not documented as fixed-format; migrated accountIds use colon-separated forms (e.g., `557058:abc...`). Implementation uses pass-through (matching `--account-id` pattern at BC-3.1.001). Invalid accountIds rejected server-side by JSM 400. BC-3.8.009 updated to reflect pass-through behavior.

3. **RESOLVED — BC-X.12.001 cross-cutting vs. bc-3**: The `jr requesttype` commands are placed in cross-cutting (X.12) rather than bc-3, consistent with the BA input rationale (discovery commands parallel `jr project` and `jr queue`). This grouping is stable. Closed at F2 authoring; confirmed by adversary passes 01–04.

4. **RESOLVED — CANONICAL-COUNTS.md total discrepancy**: Updated at F1d adversary pass-01 — CANONICAL-COUNTS now shows 566 as canonical total. BC-INDEX.md header updated to match. No residual discrepancy.

---

## Modified BCs (F1d adversary pass-01, 2026-05-18)

In addition to the F2 modifications above:

| BC ID | File | Change |
|-------|------|--------|
| BC-3.8.002 | bc-3-issue-write.md | Errors field: call-site-specific error message required (NOT "Queue commands require…"). |
| BC-3.8.007 | bc-3-issue-write.md | Labels wire shape hardened to plain string array; priority JSDSERVER-4564 caveat added; Confidence MEDIUM→HIGH. |
| BC-3.8.009 | bc-3-issue-write.md | Regex `[a-zA-Z0-9]{24}` removed; pass-through behavior specified; Errors field references corrected (BC-X.3.005 + BC-1.6.042 + H-NEW-JSM-RT-003). |
| BC-3.8.010 | bc-3-issue-write.md | NEW (F1d): `--type` ignored with stderr warning when `--request-type` is set. Warning fires on success path only (after flag parsing + request-type resolution); early-exit paths (BC-3.8.005, BC-3.8.003) do not require warning. |
| BC-X.8.004 | cross-cutting.md | "Queue commands require…" string made caller-supplied; call-site-specific error messages specified for each invocation site. |
| BC-X.12.003 | cross-cutting.md | Non-JSM error message updated to call-site-specific form (mirrors BC-X.8.004 change). |
| BC-X.12.008 | cross-cutting.md | Stale-cache window, manual recovery path, and cache-not-found error message added. |
| BC-1.3.023 | bc-1-auth-identity.md | Release gate enforcement added (PR template checklist; story S-288-C). |

---

## Validated (F1d adversary pass-02, 2026-05-18)

Propagation fixes applied in pass-02:
- BC-X.12.005 Caching section hoisted from BC-X.12.008 "Fields cache note" into BC-X.12.005 body (F16)
- BC-X.12.008 "Fields cache note" replaced with canonical reference to BC-X.12.005 §Caching (F16)
- BC-X.8.004 implementation contract appended (call-site `&'static str` parameter convention) (F18)
- BC-3.8.010 warning semantics clarified: fires on success path only; early-exit paths exempt (F19)
- ADR-0014 `related` frontmatter expanded to include BC-3.8.010 + BC-1.3.023 (F20)
- Holdout H-NEW-JSM-RT-005 added (cache hit pin for `jr requesttype fields`) (F16)
- Architecture cross-cutting.md §10.1 updated to list `request_type_fields_<sid>_<rtId>.json` (F16)
- prd-delta.md count tables updated to reflect final F1d totals (F14)
- CANONICAL-COUNTS.md stale breakdown prose removed/replaced (F15)
- bc-1-auth-identity.md `last_updated` bumped (pass-02 cosmetic sweep)

---

## Drift Items Spawned by This Delta (F1d)

F13 (process gap — cross-doc arithmetic check `scripts/check-spec-counts.sh` should validate CANONICAL-COUNTS sum against per-file frontmatter): this is a CI-tooling improvement, NOT a BC change. Deferred to STATE.md Drift Items table for state-manager to record. Do NOT fix in any BC body.
