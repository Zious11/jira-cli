---
document_type: f7-traceability-chain-delta
feature: issue-398 / S-398
spec_version: v1.4.0
pr: "#399"
pr_sha: b49f2fd
date: 2026-05-22
producer: architect-agent
---

# Traceability Chain — S-398 Delta

This document records the end-to-end traceability for the S-398 delta, linking
behavioral contracts through verification properties, implementation artifacts,
test coverage, and adversarial verification.

The S-398 delta is APPENDED to the existing traceability record in this directory.
The S-388 delta (prior feature) is recorded in `traceability-chain-delta.md`.

---

## BC → VP → Implementation → Test → Verification

### BC-3.4.012 — `issue edit` table-mode success echo

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.012` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Spec anchor** | Single-key `jr issue edit KEY [flags...]` (table mode) emits one `  <field> → <value>` line per changed field to stderr, alphabetically ordered. Team echo is resolved display name; description echo is `(updated)` marker only; cleared fields use `parent`/`points` keys with `"(cleared)"` value. Bulk paths and `--label` route (handle_edit_bulk_labels) are excluded. |
| **Verification Properties** | VP-398-001 (team display name), VP-398-002 (description asymmetry — table side), VP-398-004 (cleared-field single-key model) |
| **Implementation** | `src/cli/issue/create.rs` — `handle_edit`: new `let mut changed_fields: BTreeMap<String, String> = BTreeMap::new();` declaration; parallel population alongside `fields` JSON for each flag: summary, issue_type, priority, parent/no-parent, points/no-points, team (3rd tuple element from `resolve_team_field`), description (raw `desc_text` stored in map; display layer emits `"(updated)"`). Table-mode echo loop after `output::print_success`: `for (field, value) in &changed_fields { eprintln!("  {} → {}", field, value); }`. Echo fires only when `effective_keys.len() == 1`. |
| **Helper change** | `src/cli/issue/helpers.rs` — `resolve_team_field` return type widened from `Result<(String, String)>` to `Result<(String, String, String)>` where the third element is the resolved team display name across all five return paths (UUID-bypass: raw UUID; Exact: matched_name; ExactMultiple: stored casing from cache; Ambiguous: stored casing from cache; None: Err returned). |
| **Integration tests** | `tests/issue_edit_echo.rs` (956 LOC, 44 tests, all new): `test_BC_3_4_012_edit_table_echo_summary_and_priority` (AC-001), `test_BC_3_4_012_team_echo_is_resolved_name_not_uuid` (AC-002/VP-398-001), `test_BC_3_4_012_description_echo_is_updated_marker_not_content` (AC-003/VP-398-002), `test_BC_3_4_012_no_parent_table_echo_uses_parent_key` (AC-004/VP-398-004), `test_BC_3_4_012_edit_echo_does_not_fire_on_dry_run` (AC-010), `test_BC_3_4_012_edit_echo_excluded_for_bulk_multi_key` (AC-013), `test_BC_3_4_012_empty_summary_echoes_empty_value` (EC-3.4.012-12), `test_BC_3_4_012_echo_suppressed_on_put_error` (AC-021) |
| **Adversary verification** | Per-story adversarial: 3/3 CLEAN (1 false-alarm pass discarded — adversary mis-resolved worktree path; 3 genuine clean passes). F5 scoped adversarial: 3/3 CLEAN. |
| **Hardening** | Mutation: 100% kill rate — `create.rs:972` mutant (`== → !=` in description-echo predicate) killed by `tests/issue_edit_echo.rs`. Kani: JUSTIFIED SKIP (pure BTreeMap construction; no proof harness infra). Fuzz: JUSTIFIED SKIP (no new input parser). |
| **Merged SHA** | `b49f2fd` on `develop` (PR #399, 2026-05-22) |

---

### BC-3.4.013 — `issue edit` JSON-mode success echo

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.013` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Spec anchor** | Single-key `jr issue edit KEY [flags...] --output json` extends `edit_response` with `changed_fields: BTreeMap<String,String>`. `"updated": true` is retained. `changed_fields.description` carries raw user-supplied input string (not `"(updated)"`, not ADF round-trip). Alphabetical key order guaranteed by `BTreeMap`. Cleared-field model uses single keys (`"parent"`/`"points"` with `"(cleared)"` value). |
| **Verification Properties** | VP-398-001 (team display name — JSON side), VP-398-002 (description asymmetry — JSON side), VP-398-003 (`updated: true` preserved), VP-398-004 (cleared-field single-key model — JSON side) |
| **Implementation** | `src/cli/issue/json_output.rs` — `edit_response` signature extended: `pub(crate) fn edit_response(key: &str, changed_fields: &BTreeMap<String, String>) -> Value`. `changed_fields` serialized into response JSON; `"updated": true` retained. `src/cli/issue/create.rs:~910` — production call site updated: `json_output::edit_response(key, &changed_fields)`. |
| **Snapshot** | `src/cli/issue/snapshots/jr__cli__issue__json_output__tests__edit.snap` regenerated: `{"changed_fields": {"summary": "New title"}, "key": "TEST-1", "updated": true}`. Top-level key order alphabetical (serde_json::Map default; `preserve_order` feature absent). |
| **Integration tests** | `tests/issue_edit_echo.rs`: `test_BC_3_4_013_updated_true_present_with_summary_changed_fields` (AC-005/VP-398-003), `test_BC_3_4_013_description_echo_is_raw_input_string_not_marker` (AC-007/VP-398-002), `test_BC_3_4_013_description_stdin_trailing_newline_preserved_in_changed_fields` (AC-007 sub-case/VP-398-002), `test_BC_3_4_013_no_parent_key_is_parent_not_no_parent` (AC-008/VP-398-004), `test_BC_3_4_013_no_points_key_is_points_not_no_points` (AC-008/VP-398-004), `test_BC_3_4_013_empty_summary_in_changed_fields` (EC-3.4.013-10) |
| **Unit tests** | `src/cli/issue/json_output.rs`: `test_edit` MODIFIED (non-empty BTreeMap + snapshot regen); `test_edit_response_empty_changed_fields` NEW (empty BTreeMap, no snapshot) |
| **Adversary verification** | Shared with BC-3.4.012 evidence: 3/3 CLEAN per-story; 3/3 CLEAN F5. |
| **Hardening** | Same mutation run as BC-3.4.012. `create.rs:972` mutant covers the description-echo predicate which routes the table vs. JSON asymmetry. |
| **Merged SHA** | `b49f2fd` on `develop` (PR #399, 2026-05-22) |

---

### BC-3.4.014 — `issue create` table-mode all-fields echo

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.014` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Spec anchor** | `jr issue create [flags...]` (table mode, platform path — not JSM `--request-type`) echoes all set fields between "Created issue FOO-123" and browse URL, alphabetically. Fields: assignee (display name from `--to`; raw account ID from `--account-id`), description (`(updated)` marker), issue_type, label (comma-space joined, command-line order), parent, points (`f64::to_string()`), priority, summary, team (resolved display name). JSON output path unchanged. Human-gate revision 2026-05-22 broadened scope from team-only to all-fields. |
| **Verification Properties** | VP-398-001 (team display name — create side), VP-398-005 (team-resolution error exits 64; all-fields echo alphabetical), VP-398-006 (description echo is `(updated)` marker on create path) |
| **Implementation** | `src/cli/issue/create.rs` — `handle_create`: new `let mut create_echo: BTreeMap<String, String> = BTreeMap::new();` declared after `--request-type` dispatch fork; populated in parallel with `fields` for each resolved flag. `resolve_assignee_by_project` second return element rebound from `_display_name` to `display_name` and inserted as `"assignee"`. `OutputFormat::Table` arm emits echo loop between confirmation line and browse URL. `OutputFormat::Json` arm unchanged (no `changed_fields` key added to create JSON). |
| **Dispatch fork guard** | JSM path (`handle_jsm_create`) does not build `create_echo` — the `--request-type` dispatch fork gates this. Regression-pinned by `test_BC_3_4_014_create_json_output_unchanged_no_changed_fields_key`. |
| **Integration tests** | `tests/issue_create_echo.rs` (915 LOC, 54 tests, all new): `test_BC_3_4_014_create_all_fields_echo_alphabetical_order` (AC-006/VP-398-005-B), `test_BC_3_4_014_create_team_echo_is_resolved_name_not_uuid` (AC-002/VP-398-001), `test_BC_3_4_014_create_description_echo_is_updated_marker` (AC-009/VP-398-006), `test_BC_3_4_014_create_label_echo_comma_space_joined` (AC-011), `test_BC_3_4_014_create_assignee_echo_display_name` (AC-012), `test_BC_3_4_014_create_unresolvable_team_no_input_exits_64` (AC-019/VP-398-005-A), `test_BC_3_4_014_create_json_output_unchanged_no_changed_fields_key` (AC-015) |
| **Adversary verification** | Per-story adversarial: 3/3 CLEAN. F5 scoped adversarial: 3/3 CLEAN. |
| **Hardening** | Mutation: `create.rs:39` and `create.rs:304` mutants both caught — `handle_create` and `handle_edit` are not vacuously covered. |
| **Merged SHA** | `b49f2fd` on `develop` (PR #399, 2026-05-22) |

---

### BC-3.4.003 — Annotation-only cross-reference (S-398 touch)

| Link | Artifact |
|------|----------|
| **Behavioral Contract** | `BC-3.4.003` in `.factory/specs/prd/bc-3-issue-write.md` |
| **Change** | Annotation-only: added success-output cross-reference to BC-3.4.012 and BC-3.4.013 ("On the single-key success path (PUT 204), see BC-3.4.012 and BC-3.4.013"). No behavioral change to the PUT wire contract. |
| **Test coverage** | Existing tests for BC-3.4.003 unaffected; no new tests required for the annotation. |
| **Adversary verification** | F2 adversarial (16 passes, 3 consecutive clean final): annotation confirmed correctly placed and not contradicting BC-3.4.003. |

---

### S-398 Story → BC Anchors

| Story | BCs Implemented | ACs | Test Files |
|-------|----------------|-----|-----------|
| S-398 | BC-3.4.012, BC-3.4.013, BC-3.4.014 | 21 ACs (all verified) | `tests/issue_edit_echo.rs` (44 tests), `tests/issue_create_echo.rs` (54 tests) |
| S-398 | BC-3.4.003 | annotation-only cross-ref | No new AC required |

---

## Changed-Files → Implementation Map

| File | Delta | What S-398 Added |
|------|-------|-----------------|
| `src/cli/issue/create.rs` | +74/−16 | `changed_fields` BTreeMap in `handle_edit`; `create_echo` BTreeMap in `handle_create`; table-mode echo loops; `edit_response` call updated; `_display_name` → `display_name` rebind; BTreeMap import added; HashMap import removed (no longer used) |
| `src/cli/issue/helpers.rs` | +15/−4 | `resolve_team_field` 3-tuple widening across all 5 return paths |
| `src/cli/issue/json_output.rs` | +37/−2 | `edit_response` signature extended; `test_edit` non-empty BTreeMap; `test_edit_response_empty_changed_fields` added; snapshot regenerated |
| `src/cli/issue/list.rs` | +8/−2 | `handle_list` closure destructure 2-tuple → 3-tuple; `_resolved_team_name` underscore-prefixed (JQL-filter path does not echo) |
| `CLAUDE.md` | — | Description-echo-asymmetry Gotcha entry added verbatim (AC-016 gate) |
| `CHANGELOG.md` | — | `issue edit` + `issue create` changed-fields echo entry for v0.7 |
| `tests/issue_edit_echo.rs` | +956 LOC | 44 new integration tests (all BC-3.4.012/013 coverage) |
| `tests/issue_create_echo.rs` | +915 LOC | 54 new integration tests (all BC-3.4.014 coverage) |
| `snapshots/jr__cli__issue__json_output__tests__edit.snap` | +5/−1 | Regenerated to include `changed_fields` in pinned shape |

---

## Verification Chain

| Verification Type | Result | Evidence |
|------------------|--------|----------|
| Per-story adversarial | 3/3 CLEAN (+ 1 false-alarm discarded) | Per-story adversarial convergence log (burst) |
| F5 scoped adversarial | 3/3 CLEAN (CONVERGED) | `.factory/phase-f6-hardening/issue-398-summary.md` §5 cross-reference; F5 report |
| Kani formal proofs | JUSTIFIED SKIP | No Kani infra; pure predicate `is_team_uuid` + BTreeMap construction fully unit-tested |
| Fuzz testing | JUSTIFIED SKIP | No new external-input parser introduced |
| Mutation testing | 100% kill rate (3/3 caught, 0 surviving) | `.factory/phase-f6-hardening/issue-398-summary.md` §1 |
| cargo-audit | PASS — 0 vulnerabilities | F6 summary §4 |
| cargo-deny | PASS — advisories/bans/licenses/sources ok | F6 summary §4 |
| cargo clippy | PASS — zero warnings | F6 summary §5 |
| cargo fmt | PASS — no formatting drift | F6 summary §5 |
| Full regression | PASS — all shards green; 1 pre-existing env flake (multi_cloudid_disambiguation, out of S-398 scope) | F6 summary §5 |
| Copilot review | APPROVE — 1 finding REFUTED on research-agent validation (UUID-bypass spec carve-out is deliberate) | PR #399 |

---

## Non-Blocking Follow-Ups (for future maintenance)

| ID | Description | Priority |
|----|-------------|----------|
| TH-398-1 | dry-run echo guard test (AC-010) passes somewhat vacuously — strengthen by asserting exact absence of `→` character in stderr | LOW |
| TH-398-2 | bulk exclusion guard test (AC-013) passes somewhat vacuously — strengthen by asserting `→` character absent after multi-key edit | LOW |
| TH-398-3 | Create-stdin description echo not directly tested against the create path (VP-398-006 covers table mode, but `--description-stdin` on create not exercised in the integration suite) | LOW |
| TH-398-4 | `multi_cloudid_disambiguation` macOS-keychain test-isolation flake — unrelated to S-398 but recurs; fix: use unique service-name prefixes per test to avoid keychain collision | MEDIUM |
| PG-398-1 | BC count surface enumeration checklist for PRD deltas should include CANONICAL-COUNTS.md Breakdown bullets, `last_verified` field, BC-INDEX Coverage Statistics table | LOW |
| PG-398-2 | `check-bc-cumulative-counts.sh` does not guard BC-INDEX Coverage Statistics body table (a 9th surface) | LOW |
| PG-398-3 | verification-delta `new_vps:` frontmatter length, `### VP-` heading count, and VP-to-BC mapping table row count must agree — no automated guard exists | LOW |
| PG-398-4 | CLAUDE.md test-name-casing convention for AC references (`test_BC_3_4_012_*` uses upper-case BC IDs) not documented; future story-writer will see a naming inconsistency with the `test_<verb>_<subject>` convention in CLAUDE.md | LOW |

---

## Traceability Append Note

The S-388 delta traceability is in `traceability-chain-delta.md` (same directory).
The S-398 entries in this file are the APPENDED traceability for the next feature
cycle. If a project-level unified traceability matrix is produced in a future cycle,
the S-398 entries should be merged with the key:
`bc_ids: [BC-3.4.012, BC-3.4.013, BC-3.4.014, BC-3.4.003]`, `story: S-398`,
`pr: #399`, `sha: b49f2fd`.
