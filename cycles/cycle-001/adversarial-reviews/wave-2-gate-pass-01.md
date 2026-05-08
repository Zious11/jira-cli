---
document_type: adversarial-review
wave: 2
pass: 01
producer: adversary
date: 2026-05-08
diff_range: ab19783..ca22be0
files_changed: 72
insertions: 5889
deletions: 17
verdict: CONCERNS
findings_count: 12
blocking: 3
concern: 5
nit: 4
---

# Adversarial Review — Wave 2 Gate Pass 01

## Verdict: CONCERNS — 3 BLOCKING + 5 CONCERN + 4 NIT

## Top 3 by leverage

1. **WV2-ADV-01 — BC-7.3.004 semantic mis-anchor permeates S-2.07** (story + 11 test docstrings) — single canonical-BC error multiplied across the test corpus; orthodoxy says mis-anchoring blocks convergence
2. **WV2-ADV-02 — nfr-catalog.md not swept; 10 of 11 claimed-closed NFRs still show open routing** — biggest blast radius; future readers + downstream specs will trust the catalog
3. **WV2-ADV-04 — Snapshot tests don't pin runtime field order; S-2.07 spec misstates the json! macro behavior** — silent regression risk that bypasses both the snapshot suite and integration tests

## Recommendation: needs 2 fix-PRs before integration gate close

- **Fix-PR A (mis-anchors)**: Re-anchor S-2.07 + S-2.06 spec text and test docstrings (BC-7.3.004 → BC-7.1.001+BC-7.4.0NN; BC-6.2.013 → BC-6.2.006). ~15 files, ~30 LOC.
- **Fix-PR B (NFR sweep)**: Update nfr-catalog.md body rows + Summary Table for the 10 closed-but-not-marked NFRs. 1 file, ~30 lines.
- 5 CONCERN findings: defer to Wave 3 OR address inline during gate (orchestrator's risk appetite call).
- 4 NIT findings: track in STATE.md drift; do not block.
- 3 process-gap findings (WV2-ADV-06, WV2-ADV-10, WV2-ADV-12): codify in `rules/lessons-codification.md`.

---

## BLOCKING Findings

### WV2-ADV-01 — BC-7.3.004 semantic mis-anchor pervades S-2.07 (story spec + 5 tests + snapshot tests)

- Severity: BLOCKING
- Category: cross-story-consistency / wave-pivot-risk
- Tag: `[content-defect]`
- Confidence: HIGH

**Description**: S-2.07 spec line 110-114 declares `BC-7.3.004` as the postcondition "All commands that support --output json return structured JSON on stdout. Auth subcommands must participate." But the canonical BC-7.3.004 in `.factory/specs/prd/bc-7-output-render.md:163-167` is: "Empty `errorMessages[]` and empty `errors{}` fall through to raw body (no early exit)" — about `extract_error_message` body fall-through, NOT JSON output participation. BC-INDEX.md:465 confirms. The actual JSON-success-shape contract is BC-7.1.001 (`--output json emits structured JSON`) and the per-shape pins live in BC-7.4.001..012. Per the agent SOUL "Mis-anchoring is NEVER an Observation or deferred post-v1 — it ALWAYS blocks convergence."

**Evidence**:
- `.factory/stories/wave-2/S-2.07-json-output-policy-and-test-naming.md:110-114` (story spec)
- `tests/auth_output_json.rs:99,141,184,235,298` (5 test docstrings cite BC-7.3.004 as success-shape postcondition)
- `src/cli/auth.rs:2126,2156,2198,2210,2222,2234` (6 inline test docstrings cite BC-7.3.004 invariant)
- `.factory/specs/prd/bc-7-output-render.md:163-167` (canonical BC-7.3.004 definition — extract_error_message)
- `.factory/specs/prd/BC-INDEX.md:465` (index confirms canonical)

**Recommendation**: Fix-PR before integration gate close. Either (a) re-anchor S-2.07 spec + 11 test docstrings to BC-7.1.001 + BC-7.4.013..016 (new sub-BCs for the 4 auth shapes), OR (b) add a new BC-7.3.0NN that captures "auth subcommands participate in --output json" and re-anchor everything to it. Option (a) is more orthodox because BC-7.4 is already the JSON-shapes section.

### WV2-ADV-02 — nfr-catalog.md NFR Summary Table not updated for 8 of 11 closures

- Severity: BLOCKING
- Category: cross-story-consistency
- Tag: `[content-defect]`
- Confidence: HIGH

**Description**: The prompt claims Wave 2 closed 11 NFRs. Reading `.factory/specs/prd/nfr-catalog.md` NFR Summary Table (lines 144-186): only NFR-R-C is marked RESOLVED (line 151). The other 10 still show `POLICY-DECISION` / `DOCUMENT-AS-IS` / `FIX-IN-PHASE-3` routings. Even the body section rows (lines 93-115) still describe the routings as open work.

**Evidence**:
- `.factory/specs/prd/nfr-catalog.md:144-186` (Summary Table — only NFR-R-C marked RESOLVED)
- `.factory/specs/prd/nfr-catalog.md:93,94,95,96,97,99,108,112,115` (body rows for NFR-O-F/J/L/M/O/W/H/R/V show open routing)
- `.factory/STATE.md` (asserts S-2.05/S-2.06/S-2.07 closed all 11 NFRs)

**Recommendation**: Sweep nfr-catalog.md (body rows + Summary Table + Phase 3 routing summary at line 188-193). Mark each closed NFR with `RESOLVED — <date> via S-N.NN` and recompute the routing-summary counts.

### WV2-ADV-03 — BC-6.2.013 mis-anchor in S-2.06 Part B (story spec + 2 holdout tests)

- Severity: BLOCKING
- Category: cross-story-consistency
- Tag: `[content-defect]`
- Confidence: HIGH

**Description**: S-2.06 spec lines 95-97 declare `BC-6.2.013` as "When `cmdb_fields.json` contains a format that fails deserialization → cache miss". But BC-INDEX.md:423 says BC-6.2.013 = "`write_object_type_attr_cache` MERGES into existing per-type map; same corruption recovery pattern". The semantically-correct anchor for the CMDB-fields-legacy-format-graceful-miss invariant is BC-6.2.006 (BC-INDEX.md:416: "`cmdb_fields.json` stores (id, name) tuples; old ID-only format → cache miss (graceful)") or BC-6.2.011 ("Corrupt cache files... return `Ok(None)`").

**Evidence**:
- `.factory/stories/wave-2/S-2.06-worklog-duration-and-cmdb-cache-tuple.md:95-97`
- `tests/worklog_duration_holdouts.rs:467` (test_s_2_06_ac_005_bc_6_2_013_legacy_id_only_cmdb_cache_graceful_miss) — test name embeds wrong BC ID
- `tests/worklog_duration_holdouts.rs:524` (test_s_2_06_ac_006_bc_6_2_013_valid_tuple_cache_hits_no_api_call) — same mis-anchor
- `.factory/specs/prd/BC-INDEX.md:416,421,423` (canonical definitions)

**Recommendation**: Re-anchor S-2.06 spec + 2 test names to BC-6.2.006 (most precise match for legacy-ID-format graceful miss). Test names already shipped via PR #308; suggest follow-up rename PR.

---

## CONCERN Findings

### WV2-ADV-04 — Snapshot tests use alphabetical key sort, NOT insertion order; runtime stdout field order is unpinned

- Severity: CONCERN; Confidence: HIGH; Tag: `[content-defect]`

**Description**: S-2.07 spec lines 291-296 claim "json! macro preserves insertion order, making field order deterministic by construction". This is wrong. Examined snapshot file `src/cli/snapshots/jr__cli__auth__tests__auth_login_json_shape.snap` lines 6-10: keys are alphabetically sorted (action, ok, profile), NOT insertion order (profile, action, ok). Confirmed in `.factory/specs/prd/bc-7-output-render.md:222`: "Keys are sorted alphabetically in insta output." Integration tests in `tests/auth_output_json.rs:131-134` compare via `serde_json::Value` equality (key-set, also order-insensitive). A future change reordering fields in runtime stdout `println!` would silently pass BOTH the snapshot tests AND the integration tests.

**Recommendation**: Either (1) add explicit "field order" assertion using `serde_json::to_string(&value)` to pin exact byte order, or (2) document in `docs/specs/json-output-shapes.md` that field order is intentionally unstable. Either way, S-2.07 spec lines 291-296 should be corrected.

### WV2-ADV-05 — `auth_json_response` helper has zero direct unit tests

- Severity: CONCERN; Confidence: HIGH; Tag: `[content-defect]`

**Description**: Helper at `src/cli/auth.rs:269-275` is invoked by 4 handlers. The 4 snapshot tests at `src/cli/auth.rs:2200-2244` construct `serde_json::json!({"profile":...,"action":...,"ok":true})` INLINE — they do NOT call `auth_json_response("testprof", "login")`. Helper is never exercised by a test. A regression that adds a 4th field, drops `ok`, or renames `action` to `verb` would NOT be caught by the snapshot suite.

**Recommendation**: Add `test_auth_json_response_returns_canonical_shape` calling `auth_json_response("foo", "bar")` and asserting `serde_json::json!({"profile": "foo", "action": "bar", "ok": true})` equality.

### WV2-ADV-06 — Test naming convention spec is internally inconsistent

- Severity: CONCERN; Confidence: HIGH; Tag: `[content-defect]` and `[process-gap]`

**Description**: `docs/specs/test-naming-convention.md:8` lists `test_auth_switch_returns_json_ok` as a canonical example, but the convention rule is `test_<verb>_<subject>_<expected>` which would dictate `test_switch_auth_returns_json_ok`. The shipped tests follow the example, not the rule. The 4 snapshot tests `test_auth_<verb>_json_shape` and the 5 integration tests `test_auth_<verb>_returns_<expected>` all use `<subject>_<verb>_<expected>` ordering.

**Recommendation**: [process-gap] Either amend the rule to `test_<subject>_<verb>_<expected>` (to match shipped tests + example), OR correct the example. The convention spec contradicts itself.

### WV2-ADV-07 — Auth login `--output json` test is OS-gated, may silently pass on Linux CI

- Severity: CONCERN; Confidence: MEDIUM; Tag: `[content-defect]`

**Description**: `tests/auth_output_json.rs:317-363` (test_auth_login_emits_json_when_output_json_set) docstring acknowledges keychain may fail on Linux CI but the test does NOT skip on Linux. No `#[ignore]`, no `#[cfg(target_os = "macos")]`, no env-gate visible.

**Recommendation**: Verify CI runs this test on all 3 OSes. If only on macOS, gate explicitly. If on Linux too, document keychain availability.

### WV2-ADV-08 — `parse_duration` calculator `pub` visibility wider than necessary

- Severity: CONCERN; Confidence: MEDIUM; Tag: `[content-defect]`

**Description**: `src/duration.rs:81-125` defines `pub fn parse_duration` with SUPERSEDED-BY comment. Only caller is the proptest at lines 208-229 in same module (uses `super::*`). `pub` is structurally unnecessary.

**Recommendation**: Demote to `pub(crate)` or `pub(super)` or just `fn`. Or add `#[deprecated]` attribute so the compiler warns any new caller.

---

## NIT Findings

### WV2-ADV-09 — STATE.md S-2.06-DEFER-01 marked RESOLVED but cleanup is queued (S-3.10), not delivered

- Severity: NIT; Confidence: HIGH; Tag: `[process-gap]`

**Description**: STATE.md line 144 marks `S-2.06-DEFER-01` RESOLVED, but the deprecated calculator is STILL in `src/duration.rs:81-125` and the `format_roundtrip` proptest STILL depends on it. The cleanup is queued in S-3.10. RESOLVED status conflates "decided" with "done".

**Recommendation**: Either rename to `S-2.06-DEFER-01-DECIDED` or add sub-status `decision: RESOLVED, cleanup: PENDING (S-3.10)`.

### WV2-ADV-10 — CLAUDE.md `list.rs` description lags split (S-2.05-DEFER-01 already deferred)

- Severity: NIT; Confidence: HIGH; Tag: `[process-gap]`

**Description**: CLAUDE.md:17 still says `list.rs # list + view + comments`. After S-2.05 added separate view.rs and comments.rs rows, this is inaccurate.

**Recommendation**: Already deferred (S-2.05-DEFER-01). No action at gate; confirm deferral is sticky.

### WV2-ADV-11 — H-018 BC-X.5.005 body text vs. index summary not verified

- Severity: NIT; Confidence: MEDIUM; Tag: `[content-defect]`

**Description**: BC-INDEX.md:557 describes BC-X.5.005 as a compound BC (deprecated calculator + new validator). H-018 references BC-X.5.005. The BC body file (cross-cutting.md or wherever bodied) was NOT verified during this review pass.

**Recommendation**: Consistency-validator step should verify the BC-X.5.005 body in `cross-cutting.md` reflects the post-S-2.06 dual-meaning.

### WV2-ADV-12 — Holdout test naming pattern is undocumented (process-gap)

- Severity: NIT; Confidence: MEDIUM; Tag: `[process-gap]`

**Description**: Tests in holdout suites follow `test_s_<wave>_<story>_<bc>_<purpose>` but `docs/specs/test-naming-convention.md` only describes `test_<verb>_<subject>_<expected>`. Two conventions coexist; only one documented.

**Recommendation**: [process-gap] Amend `docs/specs/test-naming-convention.md` to acknowledge the holdout-pin pattern as a second convention.

---

## File Paths Referenced

- `.factory/stories/wave-2/S-2.06-worklog-duration-and-cmdb-cache-tuple.md`
- `.factory/stories/wave-2/S-2.07-json-output-policy-and-test-naming.md`
- `.factory/specs/prd/bc-7-output-render.md`
- `.factory/specs/prd/BC-INDEX.md`
- `.factory/specs/prd/nfr-catalog.md`
- `.factory/specs/prd/holdout-scenarios.md`
- `.factory/STATE.md`
- `CLAUDE.md`
- `docs/specs/json-output-shapes.md`
- `docs/specs/test-naming-convention.md`
- `src/cli/auth.rs`
- `src/cli/issue/json_output.rs`
- `src/duration.rs`
- `src/cli/snapshots/jr__cli__auth__tests__auth_login_json_shape.snap`
- `tests/auth_output_json.rs`
- `tests/issue_read_holdouts.rs`
- `tests/issue_write_holdouts.rs`
- `tests/asset_holdouts.rs`
- `tests/boards_sprints_holdouts.rs`
- `tests/worklog_duration_holdouts.rs`
