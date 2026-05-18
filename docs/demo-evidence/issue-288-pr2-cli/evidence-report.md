---
document_type: demo-evidence-report
story_id: issue-288-pr2-cli
evidence_class: cli-commands (vhs-help + wiremock-integration-tests)
timestamp: 2026-05-18
producer: demo-recorder
recording_strategy: C (VHS --help recordings + wiremock integration test suite as primary evidence)
---

# Demo Evidence: issue-288-pr2-cli

## Recording Strategy: Option C

**Rationale:** `issue-288-pr2-cli` introduces two new CLI commands (`jr requesttype list` and
`jr requesttype fields`) whose behavior is driven by live HTTP interactions with a wiremock
stub server. The commands are read-only and require Atlassian auth credentials + a configured
instance URL to produce meaningful output against a real endpoint.

VHS can demonstrate the static CLI surface (flags, help text, argument structure) without
any credentials. For the substantive behavioral contracts — search-param forwarding, cache
hits, JSON shapes, non-JSM project gating, partial_match resolution, ambiguity errors — the
15 wiremock-based integration tests in `tests/requesttype_commands.rs` are the authoritative
evidence. Each test uses `assert_cmd::Command::cargo_bin("jr")` to run the actual binary
with `JR_BASE_URL` / `JR_AUTH_HEADER` overrides and `expect(N)` constraints that fail if
HTTP endpoint call counts diverge from spec.

**What VHS covers:** CLI surface visible without credentials (AC-001, AC-005 — flag presence
and help text confirming `--search`, `--project`, `--output`, `<NAME|ID>` arg exist).

**What integration tests cover:** All 11 ACs (AC-001 through AC-011) with strict positive +
negative-space assertions. The `expect(1)` / `expect(0)` patterns on wiremock mocks make
cache-hit ACs (AC-008, AC-009) machine-verifiable without any manual observation.

---

## VHS Recordings

| Recording | AC | Description |
|-----------|----|-------------|
| [AC-001-list-command-help.tape](AC-001-list-command-help.tape) | AC-001, AC-002, AC-011 | `jr requesttype list --help` — confirms `--search`, `--project`, `--output` flags exist |
| [AC-001-list-command-help.gif](AC-001-list-command-help.gif) | AC-001, AC-002, AC-011 | GIF embed for PR review |
| [AC-001-list-command-help.webm](AC-001-list-command-help.webm) | AC-001, AC-002, AC-011 | Archival WebM |
| [AC-005-fields-command-help.tape](AC-005-fields-command-help.tape) | AC-005, AC-006, AC-007 | `jr requesttype fields --help` — confirms `<NAME|ID>` positional, `--project`, `--output` flags exist |
| [AC-005-fields-command-help.gif](AC-005-fields-command-help.gif) | AC-005, AC-006, AC-007 | GIF embed for PR review |
| [AC-005-fields-command-help.webm](AC-005-fields-command-help.webm) | AC-005, AC-006, AC-007 | Archival WebM |

---

## AC → Integration Test Mapping

| AC | Acceptance Criterion Summary | Test Function | File | Line |
|----|------------------------------|---------------|------|------|
| AC-001 | `jr requesttype list --project HELP` calls correct endpoint, renders Name + Description table | `test_requesttype_list_returns_types_table` | tests/requesttype_commands.rs | 153 |
| AC-002 | `--search password` sends `?searchQuery=password`; absent when flag not set | `test_requesttype_list_search_forwarded_as_query_param` (positive) + `test_requesttype_list_search_omitted_when_not_set` (negative) | tests/requesttype_commands.rs | 217, 303 |
| AC-003 | Non-JSM project exits 64 with call-site-specific label (NOT "Queue commands require") | `test_requesttype_list_non_jsm_project_exits_64_with_callsite_message` | tests/requesttype_commands.rs | 370 |
| AC-004 | `--output json` returns array with `id`, `name`, `description`, `helpText`, `issueTypeId`, `groupIds` keys | `test_requesttype_list_output_json_shape` | tests/requesttype_commands.rs | 436 |
| AC-005 | `fields "Password Reset"` resolves name → ID via partial_match, calls fields endpoint, shows Field Name/Required/Type columns with uppercase YES/NO | `test_requesttype_fields_resolves_name_and_returns_table` | tests/requesttype_commands.rs | 528 |
| AC-006 | Ambiguous name exits 64 with "Ambiguous request type" + candidates + `Run jr requesttype list` hint; `--no-input` exits cleanly | `test_requesttype_fields_ambiguous_exits_64_with_hint` | tests/requesttype_commands.rs | 634 |
| AC-007 | `fields --output json` returns object with `canRaiseOnBehalfOf`, `canAddRequestParticipants`, `fields` array (not raw `requestTypeFields`) | `test_requesttype_fields_output_json_shape` | tests/requesttype_commands.rs | 988 |
| AC-008 | Second `list` call reads from cache — only ONE HTTP call to list endpoint across two invocations (`expect(1)`) | `test_requesttype_list_cache_hit_no_second_http` | tests/requesttype_commands.rs | 1109 |
| AC-009 | Second `fields` call reads from fields cache — only ONE HTTP call to fields endpoint across two invocations (`expect(1)`) | `test_requesttype_fields_cache_hit_no_second_http` | tests/requesttype_commands.rs | 1217 |
| AC-010 | Queue caller regression: `jr queue list` non-JSM error still contains verbatim BC-X.8.004 phrase "Queue commands (`jr queue`) require a Jira Service Management project" | `test_queue_list_non_jsm_project_emits_canonical_callsite_message` | tests/queue.rs | 445 |
| AC-011 | No `--project` flag + profile has `project = "HELP"` → uses profile project; no flag + no profile project → exits 64 with actionable hint | `test_requesttype_list_uses_profile_project_when_no_flag` (positive) + `test_requesttype_list_errors_when_no_project_flag_or_profile_project` (negative) | tests/requesttype_commands.rs | 1428, 1495 |

### Additional tests beyond AC-001..011

These tests pin edge cases and adversary-discovered defects:

| Test Function | Adversary Finding | File | Line |
|---------------|-------------------|------|------|
| `test_requesttype_fields_case_variant_duplicates_lists_all_ids` | H-1 (pass-03): ExactMultiple case-insensitive filter — all three case-variant IDs must appear | tests/requesttype_commands.rs | 767 |
| `test_requesttype_fields_not_found_error_includes_cache_deletion_hint` | BC-X.12.008 §Stale-cache: not-found error includes cache deletion path | tests/requesttype_commands.rs | 902 |
| `test_requesttype_fields_numeric_id_bypasses_list_resolution` | M-2 (pass-04): numeric `<NAME|ID>` skips list endpoint (`expect(0)` guard) | tests/requesttype_commands.rs | 1331 |

---

## Regression-Guard Evidence

| Test Suite | Tests | Exit | File |
|-----------|-------|------|------|
| `cargo test --test requesttype_commands` | 15 passed, 0 failed | 0 | tests/requesttype_commands.rs |
| `cargo test --test queue` | 12 passed, 0 failed | 0 | tests/queue.rs |
| `cargo test --test project_meta` | 3 passed, 0 failed | 0 | tests/project_meta.rs |

**Key regression tests for BC-X.8.004 (queue call-site label unchanged):**
- `tests/queue.rs:445` — `test_queue_list_non_jsm_project_emits_canonical_callsite_message`
  pins the verbatim BC-X.8.004 phrase in queue's stderr after the `require_service_desk`
  signature change.
- `tests/project_meta.rs:100` — `require_service_desk_errors_for_software_project` verifies
  the require_service_desk mechanism itself at the API layer.

---

## Adversarial Convergence Evidence

Three consecutive CLEAN passes achieved per BC-5.39.001 per-story adversarial review policy:

| Pass | File | Verdict |
|------|------|---------|
| Pass 09 | `.factory/code-delivery/issue-288-pr2-cli/adversary-pass-09.md` | CLEAN — counter 1/3. No CRITICAL/HIGH/MEDIUM findings. All 12 ACs traced. |
| Pass 10 | `.factory/code-delivery/issue-288-pr2-cli/adversary-pass-10.md` | CLEAN — counter 2/3. Full BC-by-BC re-derivation confirmed. |
| Pass 11 | `.factory/code-delivery/issue-288-pr2-cli/adversary-pass-11.md` | CLEAN → 3/3 CONVERGED. All 9 BCs (X.12.001..008 + X.8.004) verified green. |

Pass 11 explicitly verified:
- All 8 BCs (X.12.001..008) traced to impl + tests with verbatim string pins and negative-space guards
- BC-X.8.004 call-site label: both queue + requesttype callers pass canonical phrases
- L-288-pr1-01 test-precision audit: zero `||` accept-either disjunctions in positive assertions
- Cross-profile cache isolation: direct isolation + corrupt-cache self-heal tests for both new cache families
- Numeric-bypass edge case regression-pinned by `expect(0)` on list endpoint mock

---

## BC Coverage Summary

| BC ID | Summary | Pinning Tests |
|-------|---------|---------------|
| BC-X.12.001 | `list` → GET `.../servicedesk/{id}/requesttype`, Name + Description columns | AC-001, AC-008 tests |
| BC-X.12.002 | `--search` → `searchQuery` server param; absent when omitted | AC-002 positive + negative tests |
| BC-X.12.003 | `--project` override; non-JSM error is call-site-specific | AC-003, AC-011 tests |
| BC-X.12.004 | `--output json` returns array with required camelCase keys | AC-004 test |
| BC-X.12.005 | `fields` → partial_match resolution, Field Name/Required/Type columns, YES/NO | AC-005, M-2 numeric-bypass test |
| BC-X.12.006 | Ambiguity error + ExactMultiple error + hints; `--no-input` no prompt | AC-006, H-1 case-variant test |
| BC-X.12.007 | `fields --output json` → BC-mandated `fields` key (not `requestTypeFields`) | AC-007 test |
| BC-X.12.008 | Cache per `(profile, sid)` and `(profile, sid, rtId)`; TTL 7d; stale-cache hint | AC-008, AC-009, not-found hint test |
| BC-X.8.004 | `require_service_desk` call-site label; queue path unchanged | AC-010 (queue.rs:445), project_meta.rs:100 |
