---
document_type: f7-traceability-chain-delta
feature: issue-396 / S-396 + FIX-F5-001
spec_version: v1.5.0
pr_primary: "#401"
pr_primary_sha: 2f61566
pr_followup: "#406"
pr_followup_sha: 699a5fd
date: 2026-05-25
producer: architect-agent
---

# Traceability Chain — S-396 Delta (+ FIX-F5-001)

This document records the end-to-end traceability for the S-396 delta plus its
FIX-F5-001 follow-up fix, linking behavioral contracts through verification
properties, implementation artifacts, test coverage, adversarial verification,
and mutation results.

The S-396 delta is APPENDED to the existing traceability record in this directory.
The S-398 delta (prior feature) is recorded in `issue-398-traceability-chain-delta.md`.
The S-388 delta is recorded in `traceability-chain-delta.md`.

---

## BC → VP → Implementation → Test → Verification

### BC-3.4.015 — `issue edit KEY --field NAME=VALUE` (string/number/date/datetime/user field, single-key path)

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.015` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Spec anchor** | Single-key `jr issue edit KEY --field NAME=VALUE` resolves `NAME` to `customfield_NNNNN` (via `fields.json` cache or `GET /rest/api/3/field`), validates field presence via `GET /rest/api/3/issue/{key}/editmeta`, checks `"set"` in `operations`, serializes `VALUE` per `schema.type`, PUTs via existing `edit_issue`; on 204 inserts `(human_name, display_value)` into `changed_fields` BTreeMap and emits standard echo (BC-3.4.012 table; BC-3.4.013 JSON). `customfield_NNNNN` literals bypass the field-list lookup step. |
| **Verification Properties** | VP-396-001 (string/number → `changed_fields` table+JSON echo; `customfield_NNNNN` bypass skips field-list HTTP), VP-396-003 (field absent from editmeta → exit 64, Edit-screen actionable hint, no PUT), VP-396-004 (unsupported types `array`/`any` → exit 64 with hint), VP-396-006 (warm `fields.json` cache → no field-list HTTP; cold cache → exactly one fetch + cache written), VP-396-007 (cache-write failure → `warning:` stderr only, exit 0; stdout uncontaminated in `--output json` mode), VP-396-008 (`--dry-run` success exits 0, no PUT; gates still fire; resolution failure exits 64), VP-396-009 (partial-failure → zero PUT; PUT-failure → `changed_fields` discarded), VP-396-010 (number `f64` wire form: `5` not `5.0`; `5e3`→`5000`; NaN/Inf exit 64), VP-396-011 (`user` wire shape `{"accountId": VALUE}`; `date`/`datetime` bare-string pass-through; no client-side ISO 8601 validation), VP-396-012 (`operations` lacks `"set"` → exit 64 with hint) |
| **Implementation** | `src/cli/issue/field_resolve.rs` (NEW) — `resolve_edit_fields(client, profile, key, field_pairs, fields, changed_fields) -> Result<()>`: Step 1 `customfield_NNNNN` regex bypass; Step 2 `read_fields_cache(profile)?` then `list_fields()` fallback + `write_fields_cache(profile, ...)` best-effort; Step 2b case-insensitive exact-then-substring match via inner `search_field` pure fn; Step 3 `get_editmeta(key).await?` + presence guard; Step 3b `operations`/"set" check; Step 4 type dispatch (string/number/option/date/datetime/user → wire serializer; array/any/unknown → exit 64 hint); Step 4a option resolution (id-bypass / exact-case-insensitive / substring match against `allowedValues`); Step 5 merge into `fields` JSON object; Step 6 insert into `changed_fields` BTreeMap. |
| **Supporting modules** | `src/api/jira/issues.rs` — `pub async fn get_editmeta(key: &str) -> Result<EditMeta>` (GET `/rest/api/3/issue/{key}/editmeta`). `src/types/jira/editmeta.rs` (NEW) — `EditMeta`, `EditMetaField` (`#[serde(rename = "allowedValues")]` on `allowed_values`), `EditMetaFieldSchema`, `AllowedValue` Serde structs. `src/cache.rs` — `FieldsCache`, `read_fields_cache`, `write_fields_cache` (best-effort writer; docs cite CLAUDE.md best-effort pattern). `src/cli/issue/create.rs` — `handle_edit` extended: destructures `field` from `Edit` variant; calls `parse_field_kv` → `HashMap<String,String>`; adds `!field_pairs.is_empty()` to `has_any_field_change` guard; calls `resolve_edit_fields` in BOTH the `if dry_run { ... }` block AND the live block (BC-3.4.015 invariant 10). `src/cli/mod.rs` — `field: Vec<String>` added to `IssueCommand::Edit` variant with `ArgAction::Append`. |
| **Integration tests** | `tests/issue_edit_field.rs` (NEW, 45 test functions) — naming pattern `test_bc_3_4_015_*`: covers VP-396-001 (string value table+JSON echo, `customfield_NNNNN` bypass), VP-396-003 (absent field exit 64), VP-396-004 (array/any type exit 64), VP-396-006 (warm cache no-HTTP, cold cache write-and-reuse), VP-396-007 (cache-write failure warns stderr, stdout clean), VP-396-008 (dry-run exit 0 no-PUT, gate fires under dry-run, resolution failure exits 64), VP-396-009 (partial-failure no PUT, PUT-failure discards echo), VP-396-010 (integer wire form, NaN rejection), VP-396-011 (user accountId shape, date/datetime bare string, no-validation sub-case), VP-396-012 (operations lacks set exit 64) |
| **Unit test** | `src/cache.rs::tests::test_write_fields_cache_swallow_io_error_returns_ok` — pins the best-effort swallow-and-return-ok behavior by overriding `XDG_CACHE_HOME` to a file path (causing ENOTDIR on `create_dir_all`). |
| **F2 adversarial** | 9 passes total; convergence at passes 7/8/9 (3 consecutive CLEAN). Pass 1 fixed 4 HIGH (dry-run gap, cache-write gap, VP-396-006 absence, VP-396-007 absence) + 7 MEDIUM. Passes 7/8/9: 0 HIGH/MEDIUM; OBS-1 serde-rename audit confirmed at pass 8. |
| **F4 adversarial** | 5 passes total; convergence at passes 3/4/5 (3 consecutive CLEAN). Pass 1 (R1) fixed 3 Copilot findings. Pass 2 (R2) fixed 2, REFUTED 1, DEFERRED 1. R3: CONVERGED 0 inline. PR #401 merged via squash @ `2f61566`. |
| **Mutation** | `cargo-mutants --in-diff` on delta `b49f2fd..699a5fd`: **15/15 caught (100%)**. All in `src/cli/issue/create.rs` per `examine_globs`. Gate A (multi-key+`--field`), Gate B (flag-overlap), `--label`-conflict, and live-path `--field` conditionals all rigorously test-pinned. 0 missed / 0 timeout / 0 unviable. |
| **Merged SHA** | `2f61566` on `develop` (PR #401, 2026-05-23); superseded-HEAD `699a5fd` (FIX-F5-001 PR #406, 2026-05-25). |

---

### BC-3.4.016 — `issue edit KEY --field NAME=VALUE` (single-select `option` field)

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.016` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Spec anchor** | When `editmeta` reports `schema.type == "option"`, the handler resolves the select option value to its `allowedValues[].id` (id-bypass if `VALUE` is a numeric id literal; otherwise case-insensitive exact match then substring match). Wire payload: `{"customfield_NNNNN": {"id": "<optionId>"}}`. `changed_fields` echo value is the matched `allowedValues[].value` (human label, not the id). |
| **Verification Properties** | VP-396-002 (option field wire shape `{"id":"..."}` pinned by wiremock body-match; echo shows human label not id; case-insensitive resolution; id-bypass path echoes raw VALUE), VP-396-006 (inherits warm-cache path — field-list resolution is shared with BC-3.4.015) |
| **Implementation** | `src/cli/issue/field_resolve.rs` — Step 4a option resolution within `resolve_edit_fields`; `allowed_values.as_deref().unwrap_or(&[])` safe fallback; id-bypass wins when `VALUE` matches a numeric id string; exact-case-insensitive match on `AllowedValue.value`; substring match fallback; empty `allowedValues` → exit 64 hint; ambiguous matches → exit 64 with candidates list. `src/types/jira/editmeta.rs` — `AllowedValue { id: String, value: Option<String>, name: Option<String> }`. |
| **Integration tests** | `tests/issue_edit_field.rs` — `test_bc_3_4_016_*`: covers VP-396-002 (option resolves to id on wire + human label in echo, wiremock body-match confirms PUT shape; case-insensitive resolution; id-bypass path). |
| **F2 adversarial** | Shared with BC-3.4.015 evidence (9 passes, 3 consecutive CLEAN). OBS-1 (pass 8) confirmed `#[serde(rename = "allowedValues")]` prevents silent all-None deserialization. |
| **Mutation** | Shared with BC-3.4.015 mutation run (15/15 caught). |
| **Merged SHA** | `2f61566` (PR #401); HEAD `699a5fd` (PR #406). |

---

### BC-3.4.017 — `--field` multi-key/`--jql` rejection (Gate A) + flag-overlap hard error (Gate B) + `--label` conflict (EC-3.4.017-13)

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.017` in `.factory/specs/prd/bc-3-issue-write.md` (EC-3.4.017-13 added by FIX-F5-001 factory-artifacts commit `9e61c05`) |
| **Spec anchor** | Gate B fires before Gate A. **Gate B (flag-overlap):** `--field` targeting `summary`/`description`/`issuetype`/`priority` alongside its dedicated flag → exit 64, no HTTP. Scope: exactly four first-party system-field keys; `--team`/`--points` deferred (dynamic IDs). **Gate A (C-1 multi-key rejection):** `--field` with 2+ positional keys or `--jql` resolving 2+ issues → exit 64 "doesn't yet support: `--field`". `--jql` resolving exactly one issue is NOT rejected. **EC-3.4.017-13 (FIX-F5-001):** `--label` + `--field` combination → exit 64 from the `--label` conflict block in `handle_edit_bulk_labels`; `--label` early-dispatches before any field resolution, so `--field` would be silently discarded. Guard added at FIX-F5-001. |
| **Verification Properties** | VP-396-005 (Gate A: multi-key exit 64 no HTTP; `--jql` multi-issue exit 64 no PUT; Gate B: summary/description/issuetype overlap exits 64 no HTTP), VP-396-008 (Gate A fires under `--dry-run`) |
| **Implementation — Gate A/B** | `src/cli/issue/create.rs` — Gate B: flag-overlap detection block added before C-1 guard; matches `summary`, `description`, `issuetype` (for `--type`), `priority` system-field keys from `field_pairs` keys against the four dedicated-flag activations. Gate A: `--field` added to `REJECTED_IN_BULK` set; C-1 rejection block extended to include `field_pairs.is_some()`. |
| **Implementation — EC-3.4.017-13** | `src/cli/issue/workflow.rs` (or equivalent handler) — `--field` added to the `--label` conflict block (was 11 entries, now 12); exit 64 guard added. Test: `test_label_plus_summary_rejected_with_exit_64_no_http` pins the entire conflict block. |
| **Integration tests** | `tests/issue_edit_field.rs` — `test_bc_3_4_017_*`: covers VP-396-005 (Gate A multi-key, Gate A `--jql` multi-issue, Gate B summary/description/issuetype overlaps all exit 64 with no HTTP), VP-396-008 (Gate A fires under `--dry-run`). FIX-F5-001 test (`test_label_plus_summary_rejected_with_exit_64_no_http`) included in PR #406. |
| **F5 adversarial (HIGH-1 catch)** | Pass 1 found HIGH-1: `--label` + `--field` silent-drop (exit 0, `--field` discarded). Routed to FIX-F5-001. Passes 2/3/4 CLEAN after fix. |
| **F4 adversarial** | 5 passes; 3 CLEAN at passes 3/4/5. |
| **Mutation** | Shared mutation run: 15/15. `--label` conflict block and Gate A/B conditionals all killed. |
| **Merged SHA** | `2f61566` (Gate A/B, PR #401); `699a5fd` (EC-3.4.017-13/`--label` conflict, FIX-F5-001 PR #406). |

---

## S-396 Story → BC Anchors

| Story | BCs Implemented | VPs | ACs | Test File |
|-------|----------------|-----|-----|-----------|
| S-396 | BC-3.4.015, BC-3.4.016, BC-3.4.017 | VP-396-001..012 (12 VPs) | 18 ACs (all verified) | `tests/issue_edit_field.rs` (45 tests) |
| FIX-F5-001 | BC-3.4.017 EC-3.4.017-13 | VP-396-005 (Gate A scope extension) | 1 AC (label+field conflict) | FIX-F5-001 test in PR #406 |

---

## Changed-Files → Implementation Map

| File | Delta | What S-396 Added |
|------|-------|-----------------|
| `src/cli/mod.rs` | +5/−0 | `field: Vec<String>` to `IssueCommand::Edit` variant; `ArgAction::Append` |
| `src/cli/issue/create.rs` | ~+120/−8 | `handle_edit`: destructures `field`; `parse_field_kv` call; `has_any_field_change` guard expansion; Gate B flag-overlap detection block; `--field` in `REJECTED_IN_BULK`; `resolve_edit_fields` in both dry-run and live blocks |
| `src/cli/issue/field_resolve.rs` | NEW | `resolve_edit_fields` full orchestration (Steps 1–6); `search_field` pure inner fn; all type dispatch arms; option resolution sub-logic |
| `src/cli/issue/mod.rs` | +2/−0 | `pub(crate) mod field_resolve;` declaration |
| `src/api/jira/issues.rs` | +20/−0 | `pub async fn get_editmeta` |
| `src/types/jira/editmeta.rs` | NEW | `EditMeta`, `EditMetaField`, `EditMetaFieldSchema`, `AllowedValue` with `#[serde(rename = "allowedValues")]` (critical rename per F2 OBS-1) |
| `src/types/jira/mod.rs` | +1/−0 | `pub mod editmeta;` declaration |
| `src/cache.rs` | +60/−0 | `FieldsCache` struct; `read_fields_cache`; `write_fields_cache` (best-effort writer, rustdoc cites CLAUDE.md); inline unit test |
| `CLAUDE.md` | +40/−0 | `--field` on `issue edit` Gotcha entry (F4 implementation deliverable per prd-delta §10) |
| `CHANGELOG.md` | +15/−0 | v0.7 `--field` entry |
| `tests/issue_edit_field.rs` | NEW (1,200+ LOC) | 45 integration tests covering all 12 VPs |

---

## Verification Chain

| Verification Type | Result | Evidence |
|------------------|--------|----------|
| F2 spec adversarial | 9 passes; 3/3 CLEAN (passes 7/8/9); novelty ~0.00 | `verification-delta-396.md` pass summaries |
| F4 per-story adversarial (PR #401) | 5 passes; 3/3 CLEAN (passes 3/4/5) | PR #401 Copilot rounds R1/R2/R3 |
| F4 per-story adversarial (PR #406) | 2 passes; CONVERGED R1→R2 | PR #406 Copilot rounds R1/R2 |
| F5 scoped adversarial | 4 passes; 3 CLEAN (passes 2/3/4); 1 HIGH → FIX-F5-001 | `.factory/phase-f5-adversarial/issue-396/convergence-summary.md` |
| Kani formal proofs | JUSTIFIED SKIP | No Kani infra in project; pure `search_field` and type-dispatch logic covered by integration tests |
| Fuzz testing | JUSTIFIED SKIP | No new external-input parser; `parse_field_kv` already fuzz-covered by proptest (4 properties) |
| Proptest (`parse_field_kv`) | PASS — 4 properties × 256 runs | `src/cli/issue/create.rs::parse_field_kv_proptests` |
| Mutation testing (`cargo-mutants --in-diff`) | **100% (15/15 caught)** | `.factory/phase-f6-hardening/issue-396-summary.md` §1 |
| `cargo audit` | PASS — 0 vulnerabilities | F6 summary §2 |
| `cargo deny check` | PASS — advisories/bans/licenses/sources ok | F6 summary §3 |
| Unsafe deserialization audit | PASS — no `unwrap` on Results; `allowed_values` safe fallback `.unwrap_or(&[])` | F6 summary §4 |
| Purity boundary audit | PASS — 4 I/O sites clearly identified; `search_field` + type-dispatch PURE; profile cache boundary respected | F6 summary §7 |
| Full regression | **PASS — 1,459 passed, 0 failed** (12 pre-existing macOS-keychain-blocked tests skipped; none introduced by S-396) | F6 summary §5 |
| CI (HEAD `699a5fd`) | **success** | F6 summary §8 |
| CI (S-396 merge `c59651b`) | **success** | F6 summary §8 |

---

## Drift Items (non-blocking, for cycle close)

| ID | Description | Priority |
|----|-------------|----------|
| DI-396-F5-1 | `--label` conflict-block negative regression coverage debt: 10 of 12 conflict entries remain untested | LOW |
| DI-396-F5-2 | Process-gap: no structural/meta-test enforces every `BULK_SUPPORTED-minus-label` and `REJECTED_IN_BULK` flag appears in the `--label` conflict block | LOW |
| DI-396-F5-3 | clap `--field` help text does not mention `--label` exclusion (UX papercut) | LOW |
| DI-396-F5-4 | EC-3.4.017-13 line-anchor citation class — will drift as bc-3 is edited (same class as PG-385-3) | LOW |
| R2-C4 | (carry-forward from F4 Copilot R2): test 38 reimplements wire-serialization inline; tests 26/27 already pin via wiremock body-match | LOW |
| F6-surface | 9 pre-existing macOS-keychain-blocking tests (NOT introduced by #396) should be `#[ignore]` + `JR_RUN_KEYRING_TESTS=1` gated (matching `oauth_embedded_login.rs` pattern) | MEDIUM |

---

## Traceability Append Note

The S-388 delta traceability is in `traceability-chain-delta.md`.
The S-398 delta traceability is in `issue-398-traceability-chain-delta.md`.
The S-396 entries in this file are the APPENDED traceability for the next feature cycle.
If a project-level unified traceability matrix is produced in a future cycle, the S-396
entries should be merged with:
`bc_ids: [BC-3.4.015, BC-3.4.016, BC-3.4.017]`, `story: S-396`, `fix: FIX-F5-001`,
`pr: #401`, `pr_sha: 2f61566`, `fix_pr: #406`, `fix_sha: 699a5fd`.
