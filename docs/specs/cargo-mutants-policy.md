# cargo-mutants Policy

## Purpose

Mutation testing as a meta-verification layer on the bulk, create, ADF, and supporting modules.
Reference: F6 hardening review of PR #110-pr2 (2026-05-10); closes audit-followup #346.

Mutation testing catches a class of defect that line-coverage metrics miss: tests that
pass even when the implementation is silently broken by small code mutations (negated
conditions, removed returns, swapped operators). The scoped modules designated below had
high line coverage but untested assertion strength at the time of the F6 review.

## Scope

`cargo-mutants` runs against:
- `src/adf.rs` — ADF conversion core (`markdown_to_adf`, `adf_to_text`, `text_to_adf`); largest
  behavior-dense module with high weak-assertion surface across node normalization, pruning,
  mark deduplication, and the Algorithm B HTML block path (added F6 hardening)
- `src/cli/issue/create.rs` — `handle_create` (platform-path `issue create` logic) and `parse_field_kv`
- `src/cli/issue/edit.rs` — `handle_edit`, `handle_edit_bulk_labels`, `handle_edit_bulk_fields` (extracted from `create.rs` by ADR-0012 Seam B, PR #558); bulk routing forks, C-1 guard, label endpoint fork, type-change path; ~99 mutants (added DEC-149)
- `src/cli/issue/jsm_create.rs` — `handle_jsm_create` (extracted from `create.rs` by ADR-0012 Seam A, PR #556); JSM POST body dispatch, RT-id resolution, scope-hint; ~9 mutants (added DEC-149)
- `src/api/jira/bulk.rs` — `await_bulk_task`, polling loop, deadline propagation
- `src/types/jira/bulk.rs` — serde structs for bulk API responses
- `src/api/jsm/requests.rs` — `JsmRequestBuilder::build` (JSM POST body construction) (added S-288-pr4)
- `src/api/jsm/request_types.rs` — `list_request_types`, `get_request_type_fields` (added S-288-pr4)
- `src/cli/requesttype.rs` — `handle_list`, `handle_fields`, `resolve_request_type_id` (added S-288-pr4)
- `src/api/jira/issues.rs` — `search_issues`, `search_issue_keys` (anti-loop guard, seen_keys dedup,
  has_more sentinel, cursor-vs-offset pagination branch); `list_comments` (added MAINT-MUTANTS-GLOBS-01)
- `src/cache.rs` — TTL logic, per-profile path construction, model-a vs model-b error-handling split
  (`write_cmdb_fields_cache` / `write_object_type_attr_cache` swallow errors; others propagate)
  (added MAINT-MUTANTS-GLOBS-01)
- `src/cli/issue/interactions.rs` — `handle_comment_add`, `handle_comment_delete`, `handle_comment_edit`, `handle_comment_view` (comment CRUD handlers: ADF conversion fork, internal/public visibility flags, stdin/file/positional source resolution; extracted from `workflow.rs` by ADR-0012 Seam, S-577-1/PF-017) (backfilled S-MUTANTS-SCOPE-1; originally added S-577-1)
- `src/cli/issue/attachments.rs` — `handle_attachment_list`, `display_sanitize_filename` (CWE-116 display-safety sanitization), `sanitize_attachment_filename` (CWE-22 disk-path sanitization), `handle_single_download`, `handle_batch_download` (backfilled S-MUTANTS-SCOPE-1; originally added S-576-1)
- `src/api/jira/attachments.rs` — `get_attachment_metadata`, `upload_attachments`, `delete_attachment`, `delete_attachment_targeted`, `list_attachments`, `get_attachment_content` (backfilled S-MUTANTS-SCOPE-1; originally added S-576-1/S-576-3/S-576-4)
- `src/api/jsm/attachments.rs` — `attach_temporary_file`, `post_request_attachment` (JSM two-step upload; SEC-576-005 `X-Atlassian-Token`, SEC-576-006 stale-ID self-heal, BC-3.9.006 step-2 error taxonomy) (backfilled S-MUTANTS-SCOPE-1; originally added S-576-5)
- `src/api/jsm/servicedesks.rs` — `get_or_fetch_project_meta`, `resolve_service_desk_id`, `require_service_desk` (TTL logic, project_id string-equality match, service-desk-id resolution chain) (backfilled S-MUTANTS-SCOPE-1; originally added S-576-5)
- `src/cli/queue.rs` — `handle`, `handle_list`, `handle_view`, `resolve_queue_by_name`, `extra_fields_allow_list`, `is_customfield_token`, `reorder_by_queue_position`, `collapse_and_truncate` (F6-hardened 200-char truncation boundary, PR #700; partial-match queue-name resolution) (added S-MUTANTS-SCOPE-1)
- `src/main.rs` — `init_tracing`, `run`, `run_until_shutdown` (previously-zero-coverage `tokio::select!` ctrl_c/SIGINT fork, now covered by VP-MUTANTS-SCOPE-1-001/002; `InvalidSubcommand` intercept) (added S-MUTANTS-SCOPE-1)
- `src/cli/field.rs` — `handle` (`jr field options <field>` entry point, S-580-1), `resolve_field_context`/`resolve_m2_project` (M1/M2/M3 field-context-mechanism resolution), `normalize_from_allowed_values`/`normalize_from_valid_values` (the normalized `FieldOption` model), `filter_options` (`--value` filter), `render_option_rows`, `resolve_field_id`; ~91 mutants — previously omitted from the examine_globs scope (added FIX-F6-MUTANTS-SCOPE)
- `src/cli/issue/field_resolve.rs` — `resolve_edit_fields`/`dispatch_field_value` (shared `--field` resolution/dispatch hub for both `issue edit --field` and `issue create --field`, S-578-2/S-578-4), `detect_flag_field_overlap` (D2 collision guard), `resolve_against_createmeta`/`resolve_against_editmeta`, `compose_option_hint`/`compose_id_hint`/`compose_name_hint`/`compose_asset_hint` (wire-value composers); ~45 mutants — previously omitted from the examine_globs scope despite backing two command families (P22-001/DEC-149/S-MUTANTS-SCOPE-1 drift class) (added FIX-F6-MUTANTS-SCOPE)
- `src/output.rs` — `sanitize_env_display`/`strip_control_and_ansi` (security-relevant display-sanitization for `ProfileConfig.env`, terminal-escape/control-char injection; same class as the CWE-116 display-safety sanitizer in `attachments.rs`), plus `render_table`/`render_json` in the same file (whole-file scope, no sub-file targeting — same tradeoff as `main.rs`/`queue.rs`) (added S-cycle3-env-tag, per pr-reviewer BLOCKING-1 on PR #752)
- `src/api/jira/tenant.rs` — `fetch_cloud_id` (S-cycle4-cloud-id-correctness API-token cloud_id acquisition), `validate_and_trim_site_url`, `is_plausible_cloud_id`, and the `MAX_TENANT_INFO_RESPONSE_BYTES` response-body size-cap guards (both the Content-Length fast-path check and the streamed-read check). Fully default-CI-testable — no keyring or Windows-cfg boundary in this file — 21 mutants, 100% kill after the FIX-F6-1 body-cap boundary tests (see `fetch_cloud_id`'s test coverage in `tests/cloud_id_tenant_info.rs`, plus the inline constant regression pin in this file's own unit test module). Was omitted from the examine_globs scope at file-creation time (S-cycle4-cloud-id-correctness) — the same P22-001/DEC-149/S-MUTANTS-SCOPE-1 "new security-relevant file → add to mutants.toml at creation" drift class documented above, confirmed by cycle-004's F6 mutation pass (`.factory/phase-f6-hardening/cycle-004/mutation-results.md` §0) to have slipped for the ENTIRE cycle-004 auth cluster, not just this file (added FIX-F6-1)

Configured in `.cargo/mutants.toml::examine_globs`. The CI job relies on this
configuration alone (no `--file` CLI flags) for scope enforcement; `--in-diff` further
narrows to lines changed in the PR diff.

Note: cargo-mutants v27+ reads its config from `.cargo/mutants.toml` (not `.mutants.toml`
at repo root). This is the canonical config location for this project.

Current `examine_globs` count: 22 entries (verify against `.cargo/mutants.toml` before citing
this number elsewhere — it has drifted before and will drift again as scope changes).

**FIX-F6-1 deferred, not added:** `src/api/auth.rs` and `src/cli/auth/login.rs` are NOT added
to `examine_globs` by this change, despite being part of the same cycle-004 security-critical
delta as `tenant.rs`. Both have a large majority of their mutants (29/48 for `auth.rs`, 5/19 for
`login.rs`) unreachable under default `cargo test` — they observe only real-OS-keychain state
(the VP-AUTHDX-005/006/007 keyring-gated boundary, `#[ignore]` + `JR_RUN_KEYRING_TESTS=1`) or are
`#[cfg(windows)]`-only. Adding either file as-is would flood default CI with un-actionable
"survivor" noise on every future diff touching them. Closing this gap needs either an in-CI
keychain-injection seam or a broad, carefully-scoped `exclude_re` allowlist for the keyring-gated
functions — both out of scope for FIX-F6-1; tracked as a follow-up (mutation-results.md §5,
FIX-F6-A).

**FIX-F6-1 `src/api/auth_windows_store.rs` — attempted, SKIPPED (not forced):** the mutation-
results.md §3 partial run (53/71 processed before being intentionally stopped) reported only ONE
non-cfg survivor — the documented-equivalent `fsync_parent_dir_best_effort with ()` mutant (a
`()`-returning best-effort durability fsync; unobservable in any functional test by construction)
— alongside 35 caught and 14 TIMEOUTs confined to the `#[cfg(windows)]` real-DPAPI region. This
FIX-F6-1 pass attempted a fresh, clean (non-contended) scoped re-run
(`--file src/api/auth_windows_store.rs`, `--jobs 2 --timeout 240`, restricted to the file's own
inline unit tests plus its Windows/DPAPI-seam integration test binaries) to confirm that single-
equivalent-mutant result before adding a narrow `exclude_re` for it. The re-run's OWN incremental
build/test cycle per mutant proved far more expensive than the `tenant.rs` run (this file pulls in
`keyring`/DPAPI-adjacent code paths that trigger a much larger dependency-recompilation graph per
mutant) — observed throughput was roughly 1 mutant per ~60-90 seconds even with zero external
contention, i.e. a full 71-mutant pass would run well over an hour, impractical to complete and
verify within one hardening session. Rather than add the file with an `exclude_re` pinned to a
result that was NOT independently re-confirmed end-to-end in this pass, **the addition is
SKIPPED** — per this story's own explicit instruction not to force a scope addition that cannot be
cleanly verified. This is a scheduling/runtime-cost deferral, not a discovered flooding problem:
the partial-run evidence (35 caught / 1 equivalent / 14 windows-only-TIMEOUT, zero genuine
default-CI survivors) is consistent with the file being a clean, single-`exclude_re` candidate —
a future pass should re-run it to completion (ideally with a longer time budget or CI-side, where
wall-clock cost is less constraining than an interactive session) and add it with the
`fsync_parent_dir_best_effort` `exclude_re` once that full run is independently reconfirmed.

### Sibling Candidates Considered and Deferred (MAINT-MUTANTS-GLOBS-01)

These files were evaluated when `issues.rs` and `cache.rs` were added. Their dispositions
are recorded here so future reviewers know they were considered, not overlooked.

| File | Disposition | Rationale |
|------|-------------|-----------|
| `src/api/pagination.rs` | EXCLUDE | Simple serde structs + `items()` field accessor. No conditional logic or error-handling branches worth mutating; survivors would be caught by the broad integration test suite. Low payoff relative to baseline cost. |
| `src/jql.rs` | EXCLUDE | Already property-tested inline with proptest. Mutation survivors in JQL escaping/validation would almost certainly be caught by existing proptest strategies. |
| `src/api/jira/users.rs` | DEFER | Contains the `USER_PAGE_SIZE`-advance pagination workaround (JRACLOUD-71293 fix). Good candidate in principle, but test coverage via `tests/user_commands.rs` is limited — adding it without targeted pagination tests risks a noisy first-run kill rate. Revisit in a dedicated "users pagination hardening" cycle. |

### Out of Scope by Design: `tests/`, `scripts/`, and CI YAML (S-626-1 pass-56, ADV-P56-LOW-004)

`examine_globs` intentionally contains no entry from `tests/` or `scripts/` — `cargo-mutants`
mutates `src/` production code and re-runs the test suite against each mutant; it has no
mechanism to mutate a *test file* (there is no "implementation" for a test to verify) or a
standalone shell script (`cargo-mutants` operates on the Rust compilation unit, never invoking
bash scripts as a mutation target). Two consequences worth stating explicitly, since a reader
skimming `examine_globs` could otherwise assume test/script code is either covered elsewhere or
simply forgotten:

- `.github/workflows/ci.yml` and `scripts/check-ci-gate.sh` are structurally unreachable by
  any Rust mutation tool. Both are covered instead by `scripts/check-ci-gate.sh --self-test`'s
  own fixture suite (a fixed-denominator, `EXPECTED_FIXTURES`-pinned set — see that script's
  self-test header) and by `tests/ci_gate_completeness.rs`'s line-based structural pins, which
  read `ci.yml` directly and assert over its text. Neither is a cargo-mutants concern.
- The line-based YAML extractor and pin-comparison helper functions in
  `tests/ci_gate_completeness.rs` itself (`extract_key_name_at_indent`,
  `extract_and_normalize_if_expr`, `extract_and_normalize_sole_run_line`,
  `extract_and_normalize_sole_needs_line`, `collect_mapping_key_set`, `parse_needs_set`,
  `list_job_ids_in_workflow`, `extract_job_display_name`, and roughly a dozen more of the same
  shape — ~15 functions in total at time of writing, an approximate count that will drift as the
  file grows) are themselves ordinary Rust functions a mutation tool COULD in principle mutate,
  but `tests/` is outside `examine_globs` by the same rule as any other test file, so none of
  them are. Their correctness is instead established by the PR-review discipline this file's own
  20+ adversarial-review rounds document: manual RED-proof construction (deliberately mutate the
  extractor or plant an adversarial YAML payload, confirm the expected test fails, then revert)
  plus human code review of the diff. This is a real, accepted gap relative to `src/` code, which
  gets both test coverage AND mutation coverage — it is documented here so a future reviewer
  evaluating "is this helper actually tested" does not go looking for a cargo-mutants report that
  will never exist for it.

## Kill-Rate Target

**90% on the PR diff scope.** The required `ci-gate.needs` member `mutants-aggregate` fails
the gate if the POOLED kill rate across all 8 shards is below 90%.

Rationale: with the inline proptest from S-345 (BC-3.4.006) and the integration tests
in `tests/issue_bulk_pr2.rs` and `tests/issue_bulk.rs`, the bulk + create paths have
strong existing coverage. Mutation testing surfaces gaps where assertions are too loose.

**Location (cycle-006 correction):** there is no `Check kill rate` CI YAML step and no
single deciding `mutants` job today — both were retired by cycle-006's sharded design
(see **Sharded Mutation Gate (cycle-006)** below). The 90% threshold now lives in
`scripts/mutants-aggregate.sh`'s Step 6 (`kill_rate=$(( (caught_total * 100) / killable ))`;
`if [ "${kill_rate}" -lt 90 ]`), invoked from `ci.yml`'s `mutants-aggregate` job's
"Evaluate sharded mutation gate" step — a shell script, not inline CI YAML, for the same
CI-artifact-visibility rationale: reviewers can read the threshold without parsing YAML
`env:`/`with:` blocks either way.

## Timeout Parameters (MUTATION-CI-TIMEOUT, 2026-06-28; corrected F5 adversarial pass)

**Partially superseded by cycle-006 (I-MED, F4 review round 8) — read before relying on
the CI-topology claims below.** This section predates cycle-006's sharded design (see
**Sharded Mutation Gate (cycle-006)** below) and describes a single `mutants` CI job
running the whole PR diff at `--jobs 4` with a 240-minute job timeout. That topology is
retired. What remains CURRENT from this section: the `--timeout 240` per-mutant ceiling
itself and its full derivation (measured baseline, runner-variance headroom, the
floor-vs-ceiling correction for `minimum_test_timeout`/`timeout_multiplier`) — the shard
job's "Run mutation tests on this shard" step passes this exact same `--timeout 240` value
today. What is now HISTORICAL, describing the pre-cycle-006 topology only: **F-2:
Cancelled Job Semantics** (below — the shard job is no longer a direct `ci-gate.needs`
member and `ALLOWED_SKIPS` no longer contains an entry for it; see that subsection's own
correction), the **CI Budget Model** table and its `--jobs 4` / 240-minute-single-job
scale (the shard job's own `timeout-minutes` is 60, not 240 — "a stuck SHARD fails fast,"
per `ci.yml`'s own comment on that job — and 8 shards run in parallel at `--jobs 2` each,
not one job at `--jobs 4`), the **Oversized-Diff Signal** subsection (below — see its own
correction pointing at the current `>120`-mutant escalation threshold), and the
**Flakiness Risk Assessment** subsection (below — its `--jobs 4` and "200+ mutants" figures
are pre-sharding scale).

### CONFIRMED CRITICAL — Previous Config Was Inverted

The F5 adversarial review pass identified a CRITICAL error in the previous version of this
section: `minimum_test_timeout` is a **floor** (lower bound), not a ceiling. Setting it to
120 could only *lengthen* timeouts for fast baselines — it cannot cap them. The corrective
analysis is in `.factory/research/cargo-mutants-timeout-keys-verification-2026-06-28.md`.

Verified facts from cargo-mutants 27.x source (verbatim doc-comment):
- `minimum_test_timeout` — *"Minimum test timeout, in seconds, as a floor on the autoset
  value."* Default is **20s**. It is a FLOOR, not a ceiling.
- The ONLY absolute per-mutant ceiling is the **`--timeout <SECS>` CLI flag**. There is
  NO `.cargo/mutants.toml` key for it (`test_timeout`/`timeout` do not exist as toml keys
  in 27.x; the field `test_timeout` in `Options` is populated solely from `args.timeout`).
- `--timeout` **supersedes** `timeout_multiplier` entirely (book: *"The multiplier only
  has an effect if the baseline is not skipped and if `--timeout` is not specified."*).
  Once `--timeout` is in the CI invocation, `timeout_multiplier` is dead config.
- Per-mutant timeout WITHOUT `--timeout`, with baseline measured:
  `effective = max(baseline × timeout_multiplier, minimum_test_timeout)` — unbounded above.

### Corrected Configuration

The fix removes the dead/misleading config keys and moves the ceiling to the CI invocation:

**`.cargo/mutants.toml`** — REMOVE both `minimum_test_timeout` and `timeout_multiplier`.
Both become dead config once `--timeout` is added to the invocation. Removing them avoids
misleading future readers into thinking either key controls a ceiling.

**CI invocation (`.github/workflows/ci.yml`, `Run mutation tests on PR diff` step)** —
ADD `--timeout 240` to the `cargo mutants` command line:

```
cargo mutants --in-diff "${DIFF_FILE}" --jobs 4 --timeout 240
```

**Local invocation (`CLAUDE.md` Build & Test section)** — add `--timeout 240` to match:

```
cargo mutants --in-diff "$DIFF_FILE" --jobs 4 --timeout 240
```

### Root Cause: Real Wall-Clock Sleeps in `bulk.rs` Scope

The `mutants` CI job for PR #553 (SEC-001, ADF recursion guard) was cancelled at exactly
60 minutes after evaluating 36 mutants from `src/adf.rs`. The root cause was **not** the
mutant count — it was the interaction between `src/api/jira/bulk.rs` being in
`examine_globs` and `tests/bulk_deadline_propagation.rs` using real wall-clock sleeps.

Key facts:
- `tests/bulk_deadline_propagation.rs` is a subprocess test (`assert_cmd::Command`) that
  drives `jr issue edit` against a wiremock server returning `HTTP 429 Retry-After: 60`
  indefinitely. It does this to test deadline propagation across a process boundary. It
  deliberately cannot use `tokio::time::pause` because `time::pause` is incompatible with
  subprocess + wiremock (tokio #4522, documented in the test file's module-level comment).
- The test's wall-clock budget is approximately 30–40 seconds per run.
- The per-mutant cost is **a full baseline test suite run**, not just the slowest single
  test. cargo-mutants runs the entire test suite per mutant (with `--all-features`).
- With a ~90s baseline and old `timeout_multiplier = 3.0` (the S-346 original config; note
  this was transiently set to 2.0 in the initial MUTATION-CI-TIMEOUT pass-1 before being
  removed entirely in pass-2 in favor of the `--timeout` CLI flag), the auto-derived
  per-mutant ceiling was ~270s. With no `--timeout`, the multiplier result was unbounded
  above as the baseline grows.

Detail: `.factory/phase-f1-delta-analysis/MUTATION-CI-TIMEOUT-delta-analysis.md` §2.1.

### Absolute Timeout Ceiling: `--timeout 240`

`--timeout 240` is passed on the `cargo mutants` command line as the absolute per-mutant
test ceiling.

**Value derivation (measured, not assumed — F5 fix, 2026-06-28):**
- Measured `cargo test --all-features` on ubuntu-latest from 5 recent green develop runs:
  - Run 28324668568: 133s
  - Run 28302021132: 145s
  - Run 28300391929: 135s
  - Run 28298946264: 145s
  - Run 28297473119: 135s
  - **Measured range: 133–145s. The prior ~90s assumed baseline was materially wrong.**
- GitHub Actions ubuntu-latest runner performance variability adds ~10–20%: worst-case
  legitimate run ≈ 145s × 1.2 = ~174s.
- `--timeout 240` gives ~38% headroom over the worst-case legitimate run (~174s),
  which is adequate to avoid false-timeout flakiness on a now-REQUIRED gate.
- 240s still kills genuine async hangs well below the previous uncapped scenarios (~270s+).

**Why not 180?**
The previously used 180s value was derived from an assumed 90s baseline. With the real
measured baseline of 133–145s and worst-case runner variance of ~174s, 180s gives only
~3–6% headroom — dangerously close to producing false timeouts on any slow runner day.

**Why not a larger value (e.g., 300)?**
300s is the `--baseline=skip` fallback, which was the uncapped state we are trying to
escape. 240s provides a meaningful cap while remaining conservative enough not to false-
timeout on the bulk deadline propagation test.

**Calibration note:** This PR itself touches NO examine_globs files, so its own mutants
run will hit the "0 mutants" path and will NOT exercise the timeout ceiling. The first PR
that touches a scoped file provides the real calibration. Watch for `timeout` outcomes in
the `Check kill rate` step — if any appear on otherwise-healthy mutants, bump `--timeout`
further. If the job consistently finishes under 30 minutes for typical PRs, a tighter
value (e.g., 200) could reduce average job time.

### Multiplier Decision: REMOVE `timeout_multiplier` (Was 3.0 in S-346 original; transiently 2.0 in MUTATION-CI-TIMEOUT pass-1; removed in pass-2)

`timeout_multiplier` is removed from `.cargo/mutants.toml` because:
- It is dead config once `--timeout 240` is in the CLI invocation (book: superseded).
- Retaining it creates a documentation debt: readers see a multiplier and assume it has
  effect, but it does not. Future maintainers may then reason incorrectly about the
  timeout model (the exact failure mode that caused the original CRITICAL).
- There is no "baseline-proportional fallback" value to preserve: the fallback when
  `--timeout` is absent is `--baseline=skip`'s 300s default, not `timeout_multiplier`.

If `--timeout` is ever removed from the invocation (e.g., during a Path B sharding
migration), reinstate `timeout_multiplier` at that time with a comment explaining its
role and its interaction with `--timeout`.

### Floor Decision: REMOVE `minimum_test_timeout` (Was 120)

`minimum_test_timeout = 120` is removed from `.cargo/mutants.toml` because:
- It is a FLOOR, not a ceiling. Setting it to 120 can only *lengthen* per-mutant timeouts
  when the baseline is tiny — a 120s floor is actively harmful on a fast-baseline project.
- The default floor (20s) is correct for this project: no legitimate test takes fewer than
  20s to fail a mutant, and we do not need to raise the floor.
- The combination of `minimum_test_timeout = 120` (floor) and no `--timeout` (no ceiling)
  was precisely the config that allowed the original unbounded hang: every mutant was
  guaranteed at least 120s to run, with no upper bound.

### CI Budget Model

With `--timeout 240`, `--jobs 4`, and the `--in-diff` PR diff scope:

Per-mutant cost is approximately `min(actual_suite_duration, 240)` seconds. For normal
non-hanging mutants the suite finishes in ~140s (measured median); hanging mutants are
capped at 240s.

| PR scenario | Estimated mutants | Estimated wall-clock |
|-------------|-------------------|----------------------|
| SEC-001 scale (adf.rs recursion guards) | ~36 | ~21 min |
| Typical adf.rs PR | ~80 | ~47 min |
| Large adf.rs + bulk.rs PR | ~120 | ~70 min |
| Very large / multiple scoped files | ~200 | ~117 min — well within the 240-min ceiling |
| Extreme scale (rare) | ~400+ | ~235+ min — approaches/exceeds the 240-min ceiling; job cancelled → split the PR |

Formula: `mutants / 4 jobs × ~140s avg` — the 140s average is the measured median
baseline (133–145s range from 5 green develop runs, 2026-06-28); hanging mutants add up
to 240s each (capped), so a PR with many async hangs will skew toward the upper bound.
Most mutants in the adf.rs + cache.rs scope do not produce async hangs.

The CI job `timeout-minutes` is set to **240 minutes** (raised from 90 in a later
hardening pass — see `ci.yml :: mutants`). A PR generating ~400+ mutants that
approaches or exceeds this budget is a signal to split the PR. See **Oversized-Diff
Signal** below.

### F-2: Cancelled Job Semantics on a Required Gate

A PR that generates ~400+ mutants and causes the 240-minute job to be cancelled by GitHub
Actions produces a `cancelled` job status. `ci-gate`'s pass/fail decision is fail-closed
(`scripts/check-ci-gate.sh`, S-CIGATE-2): every job in `needs` must report `success`
except jobs named in the script's `ALLOWED_SKIPS` allowlist (which may additionally
report `skipped`) — any other result, including `cancelled` and `failure`, fails the
gate by default. `mutants` is in `ALLOWED_SKIPS` only for its designed-in `skipped`
result on push events (see **Push-Event Safety** below); a `cancelled` result is not
covered by that allowlist entry and **blocks merge**.

This is **intentional and correct** for a REQUIRED gate. A `cancelled` outcome means:
- The mutation run was incomplete.
- The 90% kill rate was never verified against the full diff scope.
- Merging would allow unverified mutations to reach `develop`.

The correct response to a `cancelled` mutants job is:
1. **Split the PR** into smaller, more targeted changes that generate fewer mutants.
2. **Admin bypass** (for emergency or release-branch situations) — the admin can merge
   over the ci-gate with the GitHub "Require approvals" bypass, acknowledging the
   incomplete mutation run explicitly in the PR description.

Do NOT increase `timeout-minutes` beyond 240 to accommodate oversized diffs. Do NOT
treat a budget-exceeded cancellation as a flaky check.

**Corrected for cycle-006 (I-MED, F4 review round 8):** the paragraphs above describe the
pre-cycle-006 single-`mutants`-job design, retained as history. Under the sharded design
(see **Sharded Mutation Gate (cycle-006)** below), `mutants` (the 8-shard matrix) is
**not** itself a `ci-gate.needs` member — `mutants-aggregate` is, and its `if: always()`
means it always runs and never reports `skipped`, so `ALLOWED_SKIPS` contains **no** entry
for either job today (`&[]` in `tests/ci_gate_completeness.rs::PINNED_ALLOWED_SKIP_IF_
EXPRESSIONS`, per CLAUDE.md's CI Gate scope summary). A single shard job being
`cancelled` (its own `timeout-minutes: 60` is far tighter than the old single job's 240,
specifically so "a stuck SHARD fails fast," per `ci.yml`'s own comment on that job) does
not, by itself, propagate to `ci-gate` via job-status inheritance the way it used to — the
`mutants-aggregate` job still runs (`if: always()`) and Step 2's sentinel-based
completeness check (INV-COMPLETE) is what fails the gate closed: a cancelled or crashed
shard never uploads its status sentinel (or uploads one whose `run_outcome` is not
`success`), and Step 2 treats a missing-or-failed sentinel as a hard FAIL, not a silent
skip. **The end result is the same as this section's original conclusion** — an
incomplete mutation run blocks merge, not a flaky pass — but the MECHANISM is now
sentinel-based completeness accounting inside `mutants-aggregate.sh`, not `ci-gate`
directly observing a `cancelled` job status on `mutants` itself. The two admin-bypass /
split-the-PR remedies below are unchanged and still apply; the escalation threshold
(`>120` in-diff mutants, INV-ESCALATE, see **Sharded Mutation Gate (cycle-006)** below) is
now the primary forcing function for an oversized diff, reached well before any shard
would plausibly hit its own 60-minute budget.

### `--baseline=skip` and Path B

When cargo-mutants runs with `--baseline=skip` (required for sharding), `timeout_multiplier`
is silently ignored and the test timeout falls back to **300s** per mutant (book verbatim:
*"The multiplier timeout options cannot be used when the baseline is skipped ... the test
timeout default of 300 seconds will be used."*).

**This Path-A design (retained baseline run) MUST NOT use `--baseline=skip`.** The
baseline run is retained so the suite is proven green before any mutant is scored;
`--timeout 240` applies as the per-mutant ceiling unconditionally — it supersedes the
`timeout_multiplier` and is independent of whether `--baseline=skip` is used.

A future sharding effort (Path B) MUST pass `--timeout 240` (or a tuned value) explicitly
on every shard command, since `timeout_multiplier` is not available under `--baseline=skip`.
See research: `.factory/research/mutation-ci-perf-2026-06-28.md` §4.

## CI Gate: Required Check (MUTATION-CI-TIMEOUT, 2026-06-28)

**Superseded by cycle-006 (I-MED, F4 review round 8) — "Promotion to Hard-Required" and
"Push-Event Safety" immediately below describe the pre-cycle-006 single-`mutants`-job
design and are retained as history, not current fact.** See **Sharded Mutation Gate
(cycle-006)** below for the current, authoritative `ci-gate.needs` membership and
skip-tolerance configuration; the corrected summary: `ci-gate.needs` today ends in
`mutants-aggregate`, not a bare `mutants` entry — the bare `mutants` job (the 8-shard
matrix) is a dependency of `mutants-aggregate` (`needs: [mutants-plan, mutants]`), not a
direct `ci-gate.needs` member itself — and `ALLOWED_SKIPS` is EMPTY (`&[]`) today, since
`mutants-aggregate` runs `if: always()` and resolves a push-event no-op as an ordinary
`exit 0` inside its own Step 0 rather than via a job-level `if:` short-circuit, so it
never reports `skipped` to GitHub Actions at all. There is currently no `ci-gate.needs`
member that needs an `ALLOWED_SKIPS` entry.

### Promotion to Hard-Required

The `mutants` job is now **HARD-REQUIRED** via `ci-gate.needs`. Per DEC-096/097 and the
convention in CLAUDE.md, new required jobs are added to `ci-gate.needs` — never wired
directly into branch protection. This prevents the matrix-rename fragility class.

`ci-gate.needs` now includes `mutants`:

```yaml
needs: [fmt, clippy, test, msrv, deny, spec-guard, check-signing-workflow-injection, mutants]
```

### Push-Event Safety

The `mutants` job has `if: github.event_name == 'pull_request'`. On a push event to
`develop` or `main`, the job does not run and its result is `skipped`. Since S-CIGATE-2,
`scripts/check-ci-gate.sh`'s `evaluate_needs()` is fail-closed by default — an unlisted
job's `skipped` result FAILS the gate. `mutants`' push-event `skipped` result passes
ci-gate ONLY because `mutants` is named in that script's restrictive `ALLOWED_SKIPS`
allowlist, with a matching human-reviewed entry in
`tests/ci_gate_completeness.rs::PINNED_ALLOWED_SKIP_IF_EXPRESSIONS` pinning its `if:`
expression (`github.event_name == 'pull_request'`) so a future change to that condition
is caught rather than silently trusted. This is a deliberate, reviewed opt-in — not the
retired "checks for `failure`/`cancelled` only" mechanism this section previously
described, which was the exact false-green S-CIGATE-2 closed. Push-to-develop behavior
is unchanged.

### Oversized-Diff Signal

**Superseded by cycle-006 (I-MED, F4 review round 8).** The paragraph below describes the
pre-cycle-006 single-job budget (a ~400+ mutant PR timing out a single 240-minute job).
Under the sharded design, the primary forcing function for an oversized diff is the
`>120`-in-diff-mutant escalation threshold (`ESCALATION_THRESHOLD=120`, INV-ESCALATE —
see **Sharded Mutation Gate (cycle-006)** below) — the comparison is `-gt 120`, so
escalation begins at 121+ in-diff mutants (120 itself does not escalate), not ~400 — and
resolved by `mutants-aggregate`'s Step 1 as an ordinary CI failure BEFORE any shard job is
even scheduled — not by a job timing out after running. A single shard job's own
`timeout-minutes: 60` budget being exceeded (a genuinely slow shard, independent of the
escalation threshold) is covered by the corrected **F-2: Cancelled Job Semantics** above,
not by this subsection. The retired framing below is kept for historical context only:

A PR that generates ~400+ mutants and times out the 240-minute job **is not a flakiness
event** — it is a forcing function to keep PR diffs focused, consistent with the
`--in-diff` philosophy. The correct response is to split the PR into smaller, more
targeted changes. Do not increase `timeout-minutes` beyond 240 to accommodate oversized
diffs; do not treat a budget-exceeded cancellation as a flaky check.

### Timeout Semantics: Timeouts Count as Survived

Per cargo-mutants v27 convention, `timeout` outcomes are counted as survived mutants in
the kill-rate denominator. A 90%-kill-rate gate under this convention means: if async
hangs cause many timeouts on a large PR, the kill rate may fall below 90% even if all
reachable mutants are caught. This is the correct and intended behavior — it provides an
incentive to resolve async hang mutations (via `#[mutants::skip]` with justification, or
by refactoring the code to be mutation-testable) rather than silently ignoring them.

### F-3: Positive-Coverage Assertion (IMPLEMENTED, corrected by F5 adversarial pass)

**Superseded by cycle-006 (I-MED, F4 review round 8) — the shell excerpts below describe
a retired, single-job "Check kill rate" step that no longer exists.** The problem
statement, the F5 false-RED correction, and the underlying PRINCIPLE (test the OVERALL
diff size, not a per-file scoped count) are all still accurate and still the reasoning
behind the current implementation. What changed is WHERE and HOW: this guard now runs as
`scripts/mutants-aggregate.sh`'s Step 5 (see **Sharded Mutation Gate (cycle-006)** and
bullet 5 of **Future Path: Job Sharding (Path B) — LANDED** below for the full account of
what changed vs. the original sharding proposal), and its discriminator is `total_scored`
(the shards' own pooled sum, already reconciled against `MUTANT_COUNT` by Step 4) being
`== 0` — NOT a direct `[ ! -f mutants.out/outcomes.json ]` check on a single file, since
there is no longer a single `outcomes.json` to check for absence. `OVERALL_DIFF_LINES` is
still consulted, but only to explain WHY `total_scored` is legitimately zero, never as the
primary completeness signal (INV-COMPLETE's sentinel-presence check and INV-AGG's
`MUTANT_COUNT` reconciliation already rule out a shard crash or plan/execution mismatch by
the time Step 5 runs). Current mechanism, byte-accurate:
```bash
if [ "${total_scored}" -eq 0 ]; then
  if [ "${OVERALL_DIFF_LINES:-0}" -eq 0 ]; then
    echo "FAIL: 0 mutants scored (MUTANT_COUNT=0, already reconciled at Step 4) AND overall diff is EMPTY."
    echo "      Possible base-ref drift — same F-3 signature as the"
    echo "      pre-sharding single-job design."
    return 1
  fi
  echo "OK: 0 mutants scored — MUTANT_COUNT=0 (reconciled at Step 4; this is the only way to reach 0 scored mutants under the restored hard fail) — non-empty diff produced no mutable lines in examine_globs files (comment-only, whitespace, docs-only, or non-scoped-file PR)."
  return 0
fi
```
The pre-cycle-006 problem statement, F5 correction, and single-job shell excerpts below
are retained as history.

**Problem (F-3 MEDIUM [process-gap]):** When the PR diff resolves empty via base-ref
drift (e.g., the feature branch was rebased but `git diff origin/...HEAD` produces an
empty diff against the resolved merge base), `--in-diff` generates 0 mutants, `cargo
mutants` exits 0 with no `outcomes.json`, and the current gate logic returns "OK: 0
mutants — clean PR". This is a **false-green** on a now-REQUIRED gate.

**F5 adversarial correction (HIGH false-RED):** The initial F-3 implementation used
`SCOPED_DIFF_LINES` (per-file scoped line count) to detect drift. This introduced a
HIGH false-RED: a comment-only, whitespace-only, or reformat edit to a scoped file
(e.g. a rustdoc line in `src/cache.rs`) yields `SCOPED_DIFF_LINES > 0` but legitimately
0 mutants and no `outcomes.json` — the old guard would FALSELY FAIL a correct PR on a
now-required gate.

**Corrected implementation:** The guard tests the OVERALL diff size, not the scoped line
count. Real base-ref drift signature is an EMPTY `DIFF_FILE`. A non-empty diff that
produces 0 mutants is always legitimate (comment-only, docs-only, whitespace, or changes
to non-scoped files within scoped files).

**Implemented gate logic (ci.yml `Run mutation tests on PR diff` step):**

```bash
# Compute overall diff size for the base-ref drift guard in Check kill rate.
OVERALL_DIFF_LINES=$(wc -l < "${DIFF_FILE}" | tr -d ' ')
echo "Overall diff lines: ${OVERALL_DIFF_LINES}"
echo "OVERALL_DIFF_LINES=${OVERALL_DIFF_LINES}" >> "${GITHUB_ENV}"
```

**In the `Check kill rate` step, zero-`outcomes.json` branch:**

```bash
if [ ! -f mutants.out/outcomes.json ]; then
  if [ "$run_outcome" = "success" ]; then
    if [ "${OVERALL_DIFF_LINES:-0}" -eq 0 ]; then
      echo "FAIL: cargo-mutants exited 0 with no outcomes.json AND overall diff is EMPTY."
      echo "      Possible base-ref drift (git diff produced an empty file)."
      exit 1
    fi
    echo "OK: 0 mutants — non-empty diff produced no mutable lines in examine_globs files"
    echo "    (comment-only, whitespace, docs-only, or non-scoped-file PR)."
    exit 0
  else
    echo "FAIL: cargo-mutants exited non-zero AND outcomes.json missing — harness crash."
    exit 1
  fi
fi
```

This check adds zero network calls and negligible time. It fires only on the degenerate
case of genuine base-ref drift (empty diff file). All legitimate zero-mutant PRs pass.

**Maintenance:** No file-list maintenance required — the guard tests the overall diff
size, not a per-file enumeration. If the examine_globs scope changes, no update to this
guard is needed.

### Flakiness Risk Assessment

**Partially superseded by cycle-006 (I-MED, F4 review round 8):** items 3 and 4 below
describe pre-sharding scale/parallelism and are retained as history — see the corrected
figures inline. Items 1 and 2 are unaffected by sharding and remain current as written.

The flakiness risk of the required mutation gate is moderate:

1. **GitHub Actions runner performance variability:** ubuntu-latest runners vary in CPU
   speed by ~10–20%. The `--timeout 240` absolute cap provides adequate headroom for the
   measured 133–145s baseline on the slowest plausible runner (145s × 1.2 = ~174s, well
   under 240s). See **Absolute Timeout Ceiling** above for the full derivation. This
   ceiling is unchanged by sharding — every shard's "Run mutation tests on this shard"
   step passes the same `--timeout 240`.
2. **crates.io download reliability:** `taiki-e/install-action` downloads cargo-mutants
   (now pinned to the exact release `cargo-mutants@27.1.0` — see **cargo-mutants Version
   Pin** above); `Swatinem/rust-cache` caches the binary after first install, limiting
   exposure. Each of the 8 shard jobs, plus `mutants-plan`, runs its own install step.
3. **Parallel wiremock port contention:** parallel mutant runs start their own test
   processes and wiremock servers. Corrected scale: each of the 8 shard jobs runs at
   `--jobs 2` (not a single job at `--jobs 4`); no new risk introduced by making the gate
   required.
4. **Very large diffs (>120 in-diff mutants):** corrected threshold — the sharded design's
   `ESCALATION_THRESHOLD=120` (INV-ESCALATE) routes an oversized diff to an ordinary,
   actionable CI failure at `mutants-plan`, before any shard job is even scheduled — not a
   budget-exceeded cancellation after running (that pre-sharding framing, "200+ mutants,"
   is retired; see the corrected **Oversized-Diff Signal** above). Treat escalation as the
   split-PR signal today.

## Sharded Mutation Gate (cycle-006)

**This is the authoritative, current description of the mutation gate's CI topology and
decision logic.** It supersedes the single-`mutants`-job design described in **CI
Integration** below (retained there as history) and in the retired `Check kill rate` step
referenced in the pre-cycle-006 text elsewhere in this document. Every claim below is
verified directly against `scripts/mutants-aggregate.sh` and `.github/workflows/ci.yml` as
shipped, not carried forward from planning documents.

### Topology

Three jobs, always in this order, all PR-only:

1. **`mutants-plan`** (`if: github.event_name == 'pull_request'`) — computes
   `git diff origin/<base_ref>...HEAD` ONCE, uploads it as the `mutants-diff-file`
   artifact (shared, byte-identical, across every shard), and pre-counts in-diff mutants
   via `cargo mutants --list --in-diff <diff>` (a listing-only invocation — it does not
   execute the PR's code). Outputs: `escalated` (bool), `mutant_count` (the pre-count),
   `overall_diff_lines`.
2. **`mutants`** (matrix, `shard: [0..7]`, N=8; `if: github.event_name == 'pull_request'
   && needs.mutants-plan.outputs.escalated != 'true'`) — each shard runs `cargo mutants
   --in-diff <diff> --shard <k>/8 --sharding slice --jobs 2 --baseline skip --timeout 240`
   with `continue-on-error: true` (a shard reporting missed/timeout mutants is routine
   output, not a shard-job failure). Every shard unconditionally (`if: always()`) writes
   and uploads a status sentinel (`{"shard_index", "run_outcome", "has_outcomes"}`, keyed
   off `steps.run-mutants.outcome` — NOT `.conclusion`, which always reads `success` under
   `continue-on-error: true`) and its `mutants.out/outcomes.json`.
3. **`mutants-aggregate`** (`needs: [mutants-plan, mutants]`, `if: always()` — this job
   NEVER reports `skipped` to GitHub Actions; a push-event no-op is resolved as an
   ordinary `exit 0` INSIDE `scripts/mutants-aggregate.sh`'s Step 0, not via a job-level
   `if:` short-circuit) — the sole pass/fail arbiter, invoked as `bash
   scripts/mutants-aggregate.sh`. This is the required `ci-gate.needs` member that
   replaces the pre-sharding single `mutants` job.

### Pooled kill-rate computation (sum, not average)

`mutants-aggregate.sh`'s Step 3 sums `caught`/`missed`/`timeout`/`unviable` from every
shard's `outcomes.json` into pooled totals (`caught_total`, `missed_total`,
`timeout_total`, `unviable_total`) — a POOLED sum across all 8 shards, never an average of
8 per-shard percentages (which would let a small shard's extreme kill rate skew the
result). Step 6 then computes:

```
killable   = caught_total + missed_total + timeout_total   # excludes unviable
kill_rate  = (caught_total * 100) / killable                # integer division
FAIL if kill_rate -lt 90                                    # integer comparison, not <=89
```

`unviable_total` is excluded from the `killable` denominator (an unviable mutant was never
a real chance to catch a regression); if `killable` is 0 (every scored mutant was
unviable), the gate passes unconditionally (`OK: N mutant(s) generated, all unviable.`).

### Sentinel-based completeness (INV-COMPLETE) — every shard must report, or fail closed

Step 2 checks, by exact shard index (0-7), that a status sentinel file exists at
`${STATUS_DIR}/mutants-shard-status-<i>/shard-status-<i>.json` for every one of the 8
expected shards — a crashed, cancelled, or never-scheduled shard job means its mutants
were never verified, and the gate FAILS CLOSED rather than silently excluding that shard
from the denominator. Step 3 then reads each present sentinel's `run_outcome`/
`has_outcomes` fields to distinguish "this shard legitimately produced 0 mutants"
(`run_outcome=success`, `has_outcomes=false` — contributes 0, not a failure) from "this
shard's `run-mutants` step genuinely crashed before producing any `outcomes.json`"
(anything else with `has_outcomes=false` — a hard FAIL). A sentinel claiming
`has_outcomes=true` whose `outcomes.json` is missing from the download, or is malformed
JSON, is also a hard FAIL (sentinel/data desync), never a silent skip.

### Exact-equality `MUTANT_COUNT` reconciliation

Step 4 requires the pooled `total_scored` (the shards' own summed
`caught+missed+timeout+unviable`, folded in Step 3 from their OWN `outcomes.json` data)
to equal `MUTANT_COUNT` (`mutants-plan`'s independent `cargo mutants --list --in-diff`
pre-count) EXACTLY — both an under-count (a dropped, possibly-surviving mutant) and an
over-count (the shard matrix examined more than was planned) are hard fails, not just the
under-count direction. Step 6 (the kill-rate decision) is unreachable whenever Step 4
finds a mismatch. See **Sharded Gate: Residual Trust Boundary** below for what this
reconciliation does and does not protect against adversarially.

### `>120`-mutant escape hatch (INV-ESCALATE) and escalation

`mutants-plan`'s Step (`Compute diff and mutation plan`) compares its own
`MUTANT_COUNT` pre-count against a human-reviewed literal, `ESCALATION_THRESHOLD=120`
(update in the SAME commit as any deliberate change to shard count N or per-shard
`--timeout`). When `MUTANT_COUNT > 120`, `escalated=true` is set as a job output; the
`mutants` shard matrix's job-level `if:` then skips the entire matrix
(`needs.mutants-plan.outputs.escalated != 'true'`), and `mutants-aggregate`'s Step 1
short-circuits to an ordinary, actionable CI FAILURE — never a silent skip or a silent
pass — naming two ways forward: split the PR into smaller changes, or (if genuinely large
and reviewed) a repo admin merges via GitHub's branch-protection "Require approvals"
bypass, explicitly acknowledging the unverified mutation coverage in the PR description.
An advisory nightly full-scope workflow (`.github/workflows/mutants-nightly.yml`, N=16,
non-blocking) still exercises the full mutation surface regardless of how an escalated PR
is merged.

## Whitelist Convention

When a mutant cannot reasonably be killed — defensive guard, unreachable code, or a
performance-only change with identical observable behavior — annotate the function with
`#[mutants::skip]` AND include a justification comment IMMEDIATELY ABOVE the attribute.

Required format:

```rust
// mutants::skip: <one-line reason>
// Example: "defensive guard against impossible state; debug_assert! covers runtime invariant"
#[mutants::skip]
fn some_guard(...) { ... }
```

**Rules:**
- Bare `#[mutants::skip]` without a justification comment is **forbidden**. Code review
  MUST reject any PR that adds a bare whitelist attribute.
- The justification comment must be on the line(s) immediately preceding `#[mutants::skip]`.
- Valid justification categories:
  - Defensive guard for unreachable state (e.g., error branch that cannot be triggered
    through the public API under test)
  - Performance-only optimization (e.g., `with_capacity` hint) where the observable
    behavior is identical whether the hint is present or not
  - Debug-only assertion (e.g., `debug_assert!`) that does not run in release builds

Invalid justifications:
- "Tests don't cover this" — that is a gap to close, not a reason to skip
- "It's hard to test" — that is a refactoring opportunity, not a reason to skip

## Exclusions

`#[mutants::skip]` (above) marks a mutant unkillable at the *code* site. A small, distinct
class of mutant is unkillable only in the *test-harness execution model*: the mutated code
would run forever, and cargo-mutants runs the whole test binary as one process against each
mutant — so no assertion can ever observe the hang before `--timeout` kills the run and the
mutant is classified TIMEOUT (counted as survived, identically to MISSED). No amount of
additional test coverage changes this outcome, because the failure mode is the wall-clock
timeout itself, not a missing assertion. For this class, `#[mutants::skip]` on the enclosing
function is too broad (it would also skip mutants on adjacent, genuinely-testable lines), so
the exclusion is expressed instead as a single-mutant `exclude_re` entry in
`.cargo/mutants.toml`, anchored to the mutant's exact `file:line:col` + function name so it
cannot accidentally widen to cover any other mutant (including a same-shaped mutant in a
sibling function).

**Rules**, mirroring the Whitelist Convention above:
- Every `exclude_re` entry MUST be preceded by a comment block in `.cargo/mutants.toml`
  explaining (a) why the mutant is uncatchable-as-anything-but-timeout, not merely hard to
  test, and (b) where the mutated behavior's *correctness* is positively verified by other
  means (so the exclusion is not silently removing coverage of a real invariant).
- The regex MUST be anchored precisely enough to match only the intended mutant — never a
  bare function-name or file-name substring that could also match sibling mutants.
- Record the exclusion here with the same justification, kept in sync with the config
  comment.

**Current exclusions:**

- **`src/api/jira/issues.rs:374:16: delete ! in JiraClient::search_issues_with_fields`**
  (S-575-1). This is the `if !page_has_more { break; }` terminal-page guard in
  `search_issues_with_fields`'s cursor-pagination loop. Deleting the `!` makes the loop
  fail to break on a terminal page; since a terminal page always has
  `next_page_token == None`, the JRACLOUD-95368 repeated-cursor anti-loop guard
  (`next_cursor.is_some() && next_cursor == prev_cursor`) never fires (`is_some()` is false
  on `None`), so the mutant re-fetches the same terminal page **forever**. Every realistic
  pagination test reaches a terminal page — that is how pagination is supposed to end — so
  every such test hangs under this mutant, the whole `cargo-mutants` test-binary invocation
  is killed by `--timeout 240`, and the outcome is TIMEOUT (survived) before any assertion
  can run. No test can be added that catches this any other way — the infinite loop, not a
  missing assertion, is what defeats every candidate test. The break's *correctness* is
  positively verified by the multi-page termination tests already in the suite (e.g.
  `test_search_issues_with_fields_two_page_pagination_terminates_at_terminal_page` in
  `src/api/jira/issues.rs`, and the pagination-termination tests in
  `tests/rate_limit_cap_tests.rs`), which assert the loop DOES terminate — exactly the
  property this mutant would violate. The sibling `JiraClient::search_issues`
  (`issues.rs:273`) has the identical `if !page_has_more { break; }` construct and is
  deliberately **not** excluded — only this one, precisely-anchored mutant is.

## Deferral Policy

The initial baseline PR (S-346) MUST NOT block on achieving 90% kill-rate on first run.
The intent is to land the CI gate; incremental improvement follows.

When the baseline reveals surviving mutants below 90%:

1. **Whitelist clearly-defensive mutants** per the convention above with justification comments.
2. **File one follow-up GitHub issue per uncovered-region cluster** (not per individual
   mutant). Title pattern: `chore(mutants): close surviving-mutant gap in <module> — N mutants`
3. **Track deferred follow-ups** via GitHub issues labeled `audit-followup` with
   issue numbers, links, and surviving mutant descriptions in the issue body.
4. **Subsequent PRs** incrementally close the gap by tightening assertions, adding
   targeted test cases, or whitelisting genuinely unkillable mutants.

The CI `mutants` job enforces 90% on the PR diff scope going forward. A PR that touches
the scoped files and scores below 90% on changed lines will fail CI.

## Local Invocation

**Note (cycle-006):** the commands below reproduce a single (non-sharded) run
locally — they remain valid for local iteration and are what an `examine_globs`
file's own `--file`-scoped run uses. They do NOT reproduce the sharded CI topology
(`mutants-plan` → 8-shard matrix → `mutants-aggregate`) itself; the pooled kill-rate
computation, the sentinel-based completeness accounting, and the escalation
threshold are CI-only behaviors implemented in `scripts/mutants-aggregate.sh` (see
**Sharded Mutation Gate (cycle-006)** above) and are not exercised by a local single-process run.
To reproduce one shard's slice locally, add `--shard <k>/8 --sharding slice
--baseline skip` to the PR-diff-equivalent command below.

Install (one-time):

```bash
cargo install cargo-mutants --locked
```

Full baseline on scoped files (uses `.cargo/mutants.toml` automatically):

```bash
cargo mutants --jobs 4 --timeout 240
```

PR-diff-equivalent run (matches CI scope):

```bash
DIFF_FILE=$(mktemp -t pr.diff.XXXXXX)
trap 'rm -f "$DIFF_FILE"' EXIT
git diff origin/develop...HEAD > "$DIFF_FILE"
cargo mutants --in-diff "$DIFF_FILE" --jobs 4 --timeout 240
```

Note: the `--file` flags are omitted above because `.cargo/mutants.toml` already
scopes via `examine_globs`. The `--in-diff` flag further narrows to lines changed in
the diff. Using both is redundant (CI uses `--in-diff` only).

The `--timeout 240` flag sets the absolute per-mutant test ceiling to 240 seconds.
This is the same value used in CI. See **Timeout Parameters** above for the derivation.

Single-file inspection:

```bash
cargo mutants --file src/api/jira/bulk.rs --jobs 4 --timeout 240
```

Results land in `mutants.out/` (excluded from git via `.gitignore`).

## CI Integration

**Superseded by cycle-006's sharded pipeline — see Sharded Mutation Gate (cycle-006)
above for the current job names/shapes.** The description below (a single `mutants` job) is
retained as history; the required `ci-gate.needs` member today is
`mutants-aggregate`, fed by `mutants-plan` and the 8-shard `mutants` matrix, all
three PR-only (`mutants-plan`/`mutants`: `if: github.event_name == 'pull_request'`;
`mutants-aggregate`: `if: always()`, resolving push-event no-ops internally rather
than via a job-level `if:`). This remains consistent with the `security` job
pattern and keeps mutation testing cost bounded to the PR review phase.

The `mutants` job in `.github/workflows/ci.yml` runs on PRs only (not pushes to
`develop` / `main`). This is consistent with the `security` job pattern and keeps
mutation testing cost bounded to the PR review phase.

The canonical `cargo mutants` invocation is:

```
cargo mutants --in-diff "${DIFF_FILE}" --jobs 4 --timeout 240
```

- `--in-diff` scopes mutations to lines changed in the PR diff.
- `--jobs 4` runs four mutants in parallel.
- `--timeout 240` sets the absolute per-mutant test ceiling to 240 seconds (CLI-only; no
  equivalent `.cargo/mutants.toml` key exists for this parameter in cargo-mutants 27.x).

The job also includes a base-ref drift guard (`OVERALL_DIFF_LINES` check) that
guards against base-ref drift producing a false-green zero-mutant result: the gate
FAILs only when the computed `DIFF_FILE` is empty (overall diff is zero lines); a
non-empty diff that yields 0 mutants passes. See
**F-3: Positive-Coverage Assertion** above for the exact gate logic.

Only mutants in code **changed by the PR** AND **in the scoped files** are tested
(`.cargo/mutants.toml::examine_globs` provides the file-scope; `--in-diff` narrows to
changed lines within those files). PRs that do not touch the scoped files generate zero
mutants; the kill-rate check exits 0 provided the positive-coverage assertion also passes.

The job `timeout-minutes` is set to **240** (increased from 60 in MUTATION-CI-TIMEOUT,
2026-06-28, then raised again from 90 to 240 in a later hardening pass — see `ci.yml ::
mutants`). See **CI Budget Model** above.

The live workflow `.github/workflows/ci.yml` is the source of truth for the current job
specification. The reference to `.factory/cicd-setup.md §1.1a` is historical — that
artifact-branch file records the pre-MUTATION-CI-TIMEOUT spec (60-min/no-`--timeout`/
advisory) and is pending refresh on the factory-artifacts branch. Do NOT use it as
authoritative for the current gate configuration.

## Schema-Drift and False-Green Guards

Beyond the base-ref drift guard (F-3) and kill-rate threshold, `scripts/mutants-
aggregate.sh`'s per-shard loop (Step 3 of `evaluate_mutants_aggregate`) implements
several additional guards. These are documented here because they are load-bearing
correctness invariants of the required gate, and the policy doc must match the
implemented behavior.

**Location corrected (I-MED, cycle-006 F4 review round 8):** every guard in this section
previously described a single "Check kill rate" CI step reading one, unqualified
`mutants.out/outcomes.json` via a bare `jq` invocation — that step, that file layout, and
that bare-`jq` usage were all retired by cycle-006's sharded design (see **Sharded
Mutation Gate (cycle-006)** above). Each guard below now runs once PER SHARD inside
`scripts/mutants-aggregate.sh`, reading that shard's own downloaded `outcomes.json`
(`"${f}" = "${SHARD_DIR}/mutants-shard-outcomes-${i}/outcomes.json"`, one per shard index
`${i}`, 0–7) through the shared, PATH-shim-resistant `"${jq_bin}"` resolver
(`scripts/lib/trusted-jq.sh::resolve_trusted_jq`), never a bare `jq`. The underlying
PURPOSE of each guard is unchanged from the single-job design; only the CI-topology
prose and code excerpts below are corrected to match where they actually run today.

### cargo-mutants Version Pin (`cargo-mutants@27.1.0`)

**Corrected (I-MED, cycle-006 F4 review round 8):** this section previously described a
major-version-only pin, `cargo-mutants@27`. As of cycle-006 (see the Changelog's
2026-09-07 entry), the pin was TIGHTENED to the exact release, `cargo-mutants@27.1.0` —
both `mutants-plan`'s and the `mutants` shard job's `taiki-e/install-action` steps in
`.github/workflows/ci.yml` pin `tool: cargo-mutants@27.1.0` verbatim (the shard job's
install step comment reads "same exact pin as mutants-plan"), and
`scripts/mutants-aggregate.sh`'s own schema-drift FAIL message cites the same exact
string (`"Pin: cargo-mutants@27.1.0"`, see **Runtime Schema-Drift Guard (H-1)** below).
The rationale for pinning AT ALL (below) is unchanged; only the pin's precision — major
version vs. exact release — is corrected here to match what is actually shipped.

**Trigger:** `taiki-e/install-action` install steps in `.github/workflows/ci.yml`
(`mutants-plan` and the `mutants` shard job each carry their own install step, both
pinned identically).

**Mechanism:** Both install steps pin to `cargo-mutants@27.1.0` (exact release, not a
major-version range).

**Rationale:** `scripts/mutants-aggregate.sh`'s per-shard loop makes specific assumptions
about cargo-mutants v27 behavior that could change silently across major (or, given the
exact pin, any) versions:
- **outcomes.json top-level summary keys:** `caught`, `missed`, `timeout`, `unviable`,
  `total_mutants` — all present as top-level integer fields in v27. If a future version
  moves these into a nested object (e.g. `summary.caught`), the `// 0` fallbacks in the
  `"${jq_bin}"` extraction would all fire silently, giving a 0-mutant false-green per
  shard (closed at runtime by the **Runtime Schema-Drift Guard (H-1)** below, not by the
  pin alone).
- **Exit-code semantics:** `0` means all mutants caught or none generated; non-zero means
  missed mutants, timeouts, or harness errors. The `(run_outcome, has_outcomes)`
  sentinel-based completeness logic in `scripts/mutants-aggregate.sh`'s Step 2 depends on
  this invariant.
- **`--timeout` semantics:** `--timeout` is a CLI-only flag (no `.cargo/mutants.toml`
  equivalent) that supersedes `timeout_multiplier` entirely when present.

**Evidence basis:** v27 top-level schema empirically confirmed in S-346 Pass 5 F1
refutation (`.factory/cycles/cycle-001/S-346/implementation/red-gate-log.md`, Pass 5 F1
empirical refutation section, showing `caught`/`missed`/`timeout`/`unviable`/
`total_mutants` as top-level integer keys). Exit-code and `--timeout` semantics confirmed
via source-code analysis in `.factory/research/cargo-mutants-timeout-keys-verification-2026-06-28.md`.

**Impact:** Pinning to an exact release means a silent upstream release of cargo-mutants
(major OR minor/patch) with incompatible schema or exit-code changes cannot break the
required gate without an explicit pin-bump that surfaces the change for review — strictly
tighter than the original major-version-only pin this section previously (and
inaccurately) described as still current.

### Malformed-JSON Guard

**Trigger:** a shard's `outcomes.json` exists but fails `"${jq_bin}" empty` parseability
check.

**Mechanism (byte-accurate, `scripts/mutants-aggregate.sh`'s per-shard loop):** before
extracting any fields, the loop runs:
```bash
if ! "${jq_bin}" empty "${f}" 2>/dev/null; then
  echo "FAIL: shard ${i}'s outcomes.json exists but is malformed JSON."
  return 1
fi
```

**Rationale:** cargo-mutants writes `outcomes.json` incrementally. An OOM-kill or
runner crash mid-write can produce a truncated, syntactically invalid file. Without this
guard, the subsequent `"${jq_bin}" '.caught // 0'` extractions would all return `0` via
the `// 0` fallback — yielding a false-green zero-mutant result for that shard even
though mutants were scored. The `"${jq_bin}" empty` check FAILs the shard (and,
transitively, the whole gate) rather than silently passing.

### Integer Validation

**Trigger:** Any `"${jq_bin}"`-extracted summary field, for a given shard, contains a
non-integer value.

**Mechanism:** After extracting `caught`, `missed`, `timeout`, `unviable`, and
`total_mutants` via `"${jq_bin}"` (per shard, inside `scripts/mutants-aggregate.sh`'s
per-shard loop), each variable is validated with a regex guard:
```bash
[[ "${caught}"        =~ ^[0-9]+$ ]] || caught=0
[[ "${missed}"        =~ ^[0-9]+$ ]] || missed=0
[[ "${timeout}"       =~ ^[0-9]+$ ]] || timeout=0
[[ "${unviable}"      =~ ^[0-9]+$ ]] || unviable=0
[[ "${total_mutants}" =~ ^[0-9]+$ ]] || total_mutants=0
```

**Rationale:** A future schema change that emits a string, float, or object in any of
these fields would survive `jq`'s `// 0` fallback (the fallback only fires on `null`, not
on a wrong type) and would then cause the `$(( ))` arithmetic to fail under `set -e`,
producing a false-RED (job failure on an otherwise healthy PR). Coercing to `0` on any
non-integer makes the gate predictably non-crashing — the schema-drift guard or the
kill-rate calculation will then surface the anomaly in a controlled way.

### Runtime Schema-Drift Guard (H-1)

**Location corrected (cycle-006, I-MED, F4 review round 8):** this guard no longer lives
in a single "Check kill rate" CI step reading one `mutants.out/outcomes.json` via bare
`jq` — that step was retired by cycle-006's sharded design (see **Sharded Mutation Gate
(cycle-006)** above). It now runs once PER SHARD, inside `scripts/mutants-aggregate.sh`'s
per-shard loop (Step 3 of `evaluate_mutants_aggregate`), reading each shard's own
downloaded `outcomes.json` (`"${f}"`, one per shard index `${i}`) through the shared,
PATH-shim-resistant `"${jq_bin}"` resolver (`scripts/lib/trusted-jq.sh`), not a bare `jq`
invocation. The guard's underlying PURPOSE — detect a cargo-mutants schema migration that
silently zeroes out the extracted summary fields — is unchanged; only the trigger
CONDITION and its surrounding mechanics were previously misdescribed here (see
**Condition corrected** below).

**Trigger (as shipped):** a shard's `outcomes.json` is valid JSON, its four scored-summary
keys (`caught`, `missed`, `timeout`, `unviable`) all extracted as `0` (`_sum_check == 0`),
AND EITHER its `.outcomes` array is non-empty OR its `total_mutants` field is non-zero —
either signal alone, independent of the other, is sufficient to fire.

**Mechanism (byte-accurate, `scripts/mutants-aggregate.sh`'s per-shard loop):**
```bash
_outcomes_len=$("${jq_bin}" '(.outcomes // []) | length' "${f}" 2>/dev/null || echo 0)
[[ "${_outcomes_len}" =~ ^[0-9]+$ ]] || _outcomes_len=0
_sum_check=$((caught + missed + timeout + unviable))
if [ "${_sum_check}" -eq 0 ] && { [ "${_outcomes_len}" -gt 0 ] || [ "${total_mutants}" -ne 0 ]; }; then
  echo "FAIL: shard ${i}'s outcomes.json schema drift detected (non-empty outcomes/total_mutants but all summary keys sum to 0). Pin: cargo-mutants@27.1.0"
  return 1
fi
```

**Rationale:** This is the fingerprint of a schema migration in which summary keys move
from the top level into a nested object (e.g. `summary.caught`). When that happens, the
`"${jq_bin}" '.caught // 0'`-style extractions all return `0` silently (the key does not
exist at the top level), giving `_sum_check = 0` → without this guard, the shard would
contribute nothing to the pooled kill-rate denominator as if it legitimately scored zero
mutants (false-green). The guard detects this by cross-checking: if EITHER the `outcomes`
array is non-empty OR `total_mutants` is non-zero — i.e. cargo-mutants clearly did produce
real per-mutant data for this shard, by at least one of the two available signals — while
all four scored-summary keys nonetheless sum to zero, the schema has changed. The guard
then FAILs the shard (and, transitively, the whole gate) with an actionable message
referencing the `cargo-mutants@27.1.0` pin.

**Condition corrected (I-MED, cycle-006 F4 review round 8):** an earlier revision of this
section showed the trigger as a 3-way logical AND —
`_outcomes_len -gt 0 AND _sum_check -eq 0 AND total_mutants -eq 0` — requiring
`total_mutants` to ALSO read `0` before firing. That is NOT what the shipped script does,
and the difference is not cosmetic: under the AND-shaped condition, a schema migration
that moved `caught`/`missed`/`timeout`/`unviable` into a nested object while LEAVING
`total_mutants` at the top level (so it still extracts correctly, non-zero) would leave
`total_mutants -eq 0` false and the guard would never fire — exactly the false-green this
guard exists to close, undetected. The shipped condition ORs the two positive signals
(`_outcomes_len -gt 0` OR `total_mutants -ne 0`) instead of requiring both a positive
signal AND a zeroed `total_mutants`, so either signal alone is sufficient — closing that
gap. This section previously stated the AND-shaped condition as fact; it did not describe
a real, then-later-fixed defect in the script itself (the script's `||` was not one of the
findings' subjects) — it was this documentation that was wrong relative to the code from
the moment this section was written for the sharded design.

**Why it cannot produce false-REDs on legitimate runs:**
- A genuine zero-mutant shard produces **no** `outcomes.json` at all — this branch is
  never reached (handled separately by Step 2's sentinel-based completeness check).
- A genuine all-unviable shard has `unviable > 0`, so `_sum_check > 0` — the condition
  does not fire.
- A genuine empty `.outcomes` array (no mutants scored) AND `total_mutants == 0` has both
  disjuncts false — the condition does not fire.

### `total_mutants` Reconciliation Warning (M-2)

**Trigger:** for a given shard, `caught + missed + timeout + unviable != total_mutants`
(and `total_mutants != 0`).

**Mechanism (byte-accurate, per-shard):**
```bash
if [ "${total_mutants}" -ne 0 ] && [ "${_sum_check}" -ne "${total_mutants}" ]; then
  echo "::warning::Schema mismatch on shard ${i}: total_mutants=${total_mutants} but sum of known categories=${_sum_check}."
fi
```

**This emits a `::warning::` annotation — it does NOT hard-fail the gate.** As of
cycle-006 (see `scripts/mutants-aggregate.sh`'s own inline comment at this call site),
this per-shard warning is effectively MOOT under Step 4's exact-equality `MUTANT_COUNT`
reconciliation (see **Sharded Mutation Gate (cycle-006)** above) — a future outcome
category would eventually make the pooled `total_scored` fail to equal `MUTANT_COUNT`
regardless of whether this warning fired, so it now functions as an early diagnostic
pointing at WHY Step 4 is about to fail, not as an independent escape hatch in its own
right. The "why warning-only, not hard-fail" rationale below predates that observation and
remains the reason THIS check itself was never promoted to a hard-fail.

**Rationale for warning-only:** The `total_mutants` field accounts for ALL outcomes,
including any new outcome categories added in future cargo-mutants versions that this
script does not yet enumerate. A mismatch means the denominator in the kill-rate
calculation may be understated (some mutants fell into an unrecognized category and are
not counted in `missed`). However, promoting this to a hard-fail was considered and
rejected for the following reasons:

1. **False-RED risk:** If cargo-mutants adds a new outcome category (e.g. `skipped`),
   the sum would legitimately diverge from `total_mutants` — hard-failing would block
   every PR until the script is updated, even if the kill rate is healthy.
2. **Accepted residual:** The `cargo-mutants@27.1.0` exact-release pin (I-MED: corrected
   from an earlier, less precise `@27` major-version-only description — see
   **cargo-mutants Version Pin** above) protects against undiscovered schema changes in
   the current CI. If the pin is deliberately bumped to accommodate a new version that
   adds an outcome category, the reconciliation mismatch will surface in CI logs at that
   time — making it an observable, actionable signal rather than a silent drift. The
   warning-in-logs posture is sufficient because defeating it requires bypassing the pin
   AND the change being visible in job logs.
3. **Defense-in-depth:** The H-1 schema-drift guard (above) already FAILs the gate when
   all summary keys are zero despite non-empty outcomes — the most dangerous false-green
   class. The reconciliation warning catches the residual case of a partial-key move.

## Spec Anchor

The mutation gate is governed solely by this policy document (`docs/specs/cargo-mutants-policy.md`).
There is no dedicated BC (Behavioral Contract) for the mutation gate. The MUTATION-CI-TIMEOUT
drift item in STATE.md tracked the promotion-to-required change.

A BC could be authored — e.g., `BC-X.14.001: mutation-gate-required-check` — but the
policy spec provides sufficient governance for a CI-only behavior. The human explicitly
chose not to author a BC for this cycle (F1 §8, Q3 resolution: policy-doc-only). If the
mutation gate invariants need formal traceability in a future cycle, author a BC at that
time. For F7 traceability: the governing artifact is this file at the `docs/specs/` path,
not a PRD BC.

**cycle-006 (S-cycle6-mutants-ci-sharding) continues this precedent.** The sharded
gate's governing invariants — INV-AGG, INV-COMPLETE, INV-ESCALATE (see **Sharded
Mutation Gate (cycle-006)** above) — are also policy-doc-only, per DEC-348 (F1 approval) and
DEC-349 (F2 gate approval). No PRD BC exists for the sharded gate either; this file,
plus `.factory/phase-f2-spec-evolution/cycle-006/mutants-sharding-invariants.md` (the
invariant statements and their adversarial-review history) and
`.factory/cycles/cycle-006/phase-f3-stories/S-cycle6-mutants-ci-sharding.md` (the
delivering story), are the governing artifacts.

## Guards

Two static-analysis guards protect §Scope integrity (DEC-150):

- **Guard 2 — `scripts/check-cargo-mutants-policy-citations.sh` (CI-MUTANTS-CITE-001):**
  Parses the §Scope bulleted list, extracts every (file, fn) pair, and verifies each
  against source definitions via definition-anchored grep. Exits 1 with an offender list
  if any citation is stale. Runs in the spec-guard CI job after `check-bc-cumulative-counts`.
  `--self-test` flag runs 12 offline fixtures. `--policy-doc` / `--src-root` flags provide
  seams for fixtures. Reproduce locally: `bash scripts/check-cargo-mutants-policy-citations.sh`
  (canonical) or `bash scripts/check-cargo-mutants-policy-citations.sh --self-test` (offline
  fixture run). On failure: fix the stale citation in §Scope or, for an intentional
  relocation, update the bullet to the new file/function.

- **Guard 3 — `tests/mutants_glob_existence.rs`:**
  Validates every `examine_globs` entry in `.cargo/mutants.toml` resolves to ≥1 real file
  via `glob::glob()`. Panics with `MUTANTS-GLOBS-KEY-MISSING` if the key is absent or empty;
  panics with `MUTANTS-GLOBS-COVERAGE-FLOOR` if the entry count falls below 11. Runs as
  part of the always-run `cargo test` suite. Reproduce locally:
  `cargo test --test mutants_glob_existence`. On failure: fix the dead examine_globs entry
  or update it for the file move.

## Sharded Gate: Residual Trust Boundary (documented, not fixed) — cycle-006 F4 review round 1, F-PC-MED-001

`mutants-aggregate.sh`'s kill-rate decision reads `caught`/`missed`/`timeout`/`unviable`
from each shard's `outcomes.json` (Step 3 of `evaluate_mutants_aggregate`). Each
`outcomes.json` is produced inside the `mutants` shard job's own "Run mutation tests on
this shard" step, which runs `cargo mutants --in-diff … --shard <k>/8 …` — and `cargo
mutants` itself compiles and runs the PR's OWN code (`cargo build`/`cargo test` under the
hood) to determine which mutants are caught. A PR that achieves code execution somewhere
in that build/test loop (a malicious `build.rs`, a proc-macro, or a test-time side effect)
runs with the ability to overwrite `mutants.out/outcomes.json` before the shard job's
"Upload shard outcomes" step picks it up.

**The attack:** `MUTANT_COUNT` — the value `mutants-aggregate.sh`'s Step 4 reconciles the
pooled shard total against — is deterministically computable offline via the exact same
`cargo mutants --list --in-diff <diff>` command `mutants-plan` runs (this command only
lists mutants; it does not execute the PR's code). A PR author can therefore precompute
`MUTANT_COUNT` for their own diff and hand-fabricate an `outcomes.json` whose
`caught`/`missed`/`timeout`/`unviable` sum to exactly that value while reporting a 100%
pooled kill rate — satisfying both Step 4's exact-equality reconciliation and Step 6's
kill-rate threshold trivially, with no real mutant ever executed.

**Blast radius: mutation-QUALITY signal only.** None of the three `mutants-plan`,
`mutants`, or `mutants-aggregate` jobs in `ci.yml` reference any repository secret or
`GITHUB_TOKEN` (grep-verified) — a PR that exploits this residual can force the mutation
gate green without earning it, but it gains no path to secret exfiltration or write access
through this mechanism.

**This is not a regression.** The pre-sharding single `mutants` job had this identical
trust boundary — it also produced `outcomes.json` inside the same job that built and
tested the PR's own code, with **zero** reconciliation against anything computed
independently. This diff *narrows* that pre-existing boundary rather than introducing it:
Step 4's exact-equality `MUTANT_COUNT` cross-check (pooled shard total vs. an
independently-computed pre-count) did not exist before cycle-006 and forces a forger to at
minimum reproduce the correct total, not merely report a plausible-looking pass.

**Correction (cycle-006 F4 review round 2, F-PE-LOW-002): reconciliation is not an
adversarial control.** The paragraph above is accurate about the reconciliation's origin,
but its "narrows the boundary" framing was previously read as if it also raised the bar for
a DELIBERATE forger — it does not. `mutants-plan` computes `MUTANT_COUNT` by compiling and
running `cargo mutants --list --in-diff` against the SAME PR checkout the `mutants` shard
jobs later build and test — so an attacker who has already achieved code execution in one
job (a malicious `build.rs`, a proc-macro, or a test-time side effect) has code execution in
the other too, via the identical mechanism. That attacker controls BOTH sides of Step 4's
exact-equality check simultaneously and can make them agree on any number it likes while
fabricating a 100% pooled kill rate. Stated plainly: **reconciliation constrains accidental
divergence only** (a bug, a partial-run artifact, an off-by-one in the sharding math
producing a `MUTANT_COUNT`/pooled-total mismatch by mistake) — **it offers no adversarial
protection against an attacker who already has code execution**, because that attacker
controls both sides of the equation it checks.

**Currency (same round): the TRIVIAL, no-code-execution variant of this bypass is now
CLOSED.** Before this round, a plaintext `ci.yml` edit to `mutants-plan`'s "Compute diff and
mutation plan" step — replacing its `run:` body with hardcoded `echo "escalated=false"` /
`echo "mutant_count=0"` / `echo "overall_diff_lines=1"` output lines and no real `git
diff`/`cargo mutants --list` invocation at all — satisfied every structural pin that existed
at the time (job/step presence, `outputs:` key set, `if:` value) while requiring **zero**
code execution and zero real mutation testing, for any PR diff size. Two new tests in
`tests/ci_gate_completeness.rs` close this: `test_mutants_plan_compute_step_content_is_pinned`
asserts the compute step's `run:` body contains ten required fragments, in order, tying each
of `mutant_count` / `overall_diff_lines` / `escalated` to a real `git diff` / `cargo mutants
--list --in-diff`-derived shell variable rather than a literal; `test_mutants_plan_job_and_
step_key_sets_are_pinned` closes the job/step key-set gap that content pin sits inside (a
smuggled job-level `defaults:`/`continue-on-error:`, or a smuggled step-level
`if:`/`shell:`/`continue-on-error:`, on `mutants-plan` itself). The residual documented in
this section is therefore narrower than at initial cycle-006 landing — but it was NOT, as
an earlier revision of this paragraph claimed, narrowed all the way down to "specifically
the code-execution-required path, not a plaintext `ci.yml` edit": that overclaimed
closure. **Correction (cycle-006 F4 review round 4, A-F1):** a SECOND plaintext,
no-code-execution `ci.yml` vector remained open on the sibling `mutants` shard job at the
time that sentence was written — appending shell to the `run-mutants` step's `run:` body,
AFTER the real `cargo mutants` invocation inside the SAME step, that launders the shard's
own `mutants.out/outcomes.json` (moving `missed`/`timeout` counts into `caught`,
sum-preserving so Step 4's exact-equality reconciliation still passes). AC-033's
presence-only ordered-substring search over the whole raw `mutants` job block proved the
real invocation was present, but never inspected what came after it. This is now CLOSED by
`assert_run_mutants_step_content_is_honest` (scoped to the `run-mutants` step's PARSED
`run:` scalar, mirroring this section's own `mutants-plan` fix) and its production pin
`test_mutants_shard_run_step_content_is_pinned` in `tests/ci_gate_completeness.rs` — see
"A second forgery path" below for a third, DIFFERENT no-code-execution variant
(spoofed sibling-job artifacts) that remains open and tracked, separate from the one this
paragraph now correctly states is closed. What remains open, accurately stated: the
**code-execution-required** path (a malicious `build.rs`/proc-macro/test-time side effect
reachable from a `cargo mutants`/`cargo build`/`cargo test` invocation) and the
spoofed-sibling-artifact / canceling-errors / set-identity residuals described below — not
a trailing-shell-append inside the shard's own `run-mutants` step, which this fix closes.

**Correction (cycle-006 F4 review round 6, finding D-HIGH): the round-4 fix above was
itself a substring DENYLIST, not default-deny, and a THIRD no-code-execution variant on
the sibling upload step was never checked at all — all three are now CLOSED.**
`assert_run_mutants_step_content_is_honest`'s ban on the CONTIGUOUS LITERAL
`outcomes.json` appearing after the invocation was bypassable by any construction that
references the file without spelling that exact literal: (1) shell variable indirection
(`n=outcomes; ... > "mutants.out/${n}.json"`), or (2) a glob/loop that names the file only
via a wildcard or loop variable (`for f in mutants.out/*.json; do … "$f" …; done`) — neither
contains the substring `outcomes.json` anywhere in the scanned text. (3) A THIRD variant
needed no change to `run-mutants` at all: repointing the sibling "Upload shard outcomes"
step's `with.path`/`with.name` to a forged file — `PINNED_MUTANTS_SHARD_STEP_KEY_SETS` pins
that step's own key set (`if`/`name`/`uses`/`with`) but has no visibility into `with:`'s
CHILDREN, and nothing else in this file checked them either. All three were sum-preserving
launders (or, for variant 3, bypassed the reconciliation entirely by uploading a
fully-fabricated file), so `mutants-aggregate.sh`'s Step 4 exact-equality reconciliation and
Step 6 kill-rate check both still passed. **Fixed** by replacing the substring-denylist call
in `test_mutants_shard_run_step_content_is_pinned` with a FULL byte-for-byte comparison of
the `run-mutants` step's entire parsed `run:` scalar against a reviewed literal
(`PINNED_MUTANTS_SHARD_RUN_BODY` in `tests/ci_gate_completeness.rs`, via
`extract_and_normalize_mutants_shard_run_body` — also rejecting a YAML anchor, tag, or any
scalar style other than the real `Literal` (`|`) block form), and by adding a TARGETED,
justified exception to this file's general "`with:` block CONTENTS are out of scope"
boundary for this one security-critical step: `PINNED_MUTANTS_SHARD_UPLOAD_OUTCOMES_WITH_
PATH`/`_WITH_NAME` pin the "Upload shard outcomes" step's `with.path`/`with.name` values
byte-for-byte, via `extract_and_normalize_upload_outcomes_with_value`. `assert_run_mutants_
step_content_is_honest` (the OLD substring-denylist function) is retained, unchanged, only
so three standing RED regression tests — `test_mutants_shard_run_step_rejects_variable_
indirection_launder`, `test_mutants_shard_run_step_rejects_glob_loop_launder`, and
`test_mutants_shard_upload_outcomes_pin_rejects_repointed_path` — can demonstrate what it
used to accept; its own doc comment states plainly that it is superseded and is no longer
called by the production pin. What remains open, accurately stated, is narrower than
before this round: the **code-execution-required** path and the spoofed-sibling-artifact /
canceling-errors / set-identity residuals described below — not any of the three
plaintext-`ci.yml`-edit, no-code-execution variants this round closes.

### A second forgery path: spoofed sibling-job artifacts (cycle-006 F4 review round 2, F-PF-LOW-002)

A second, arguably *easier* forgery mechanism exists alongside the build.rs/proc-macro path
above, and it needs no code execution in the `mutants` shard build/test loop at all.
`mutants-aggregate`'s two download steps (`ci.yml`'s "Download all shard status sentinels"
and "Download all shard outcomes") pull artifacts by **name pattern** —
`mutants-shard-status-*` and `mutants-shard-outcomes-*` — with no binding to the `mutants`
matrix job's identity, run, or provenance. `scripts/mutants-aggregate.sh`'s completeness
check (Step 2's sentinel-presence sweep, `EXPECTED_SHARDS=8`) verifies only that exactly one
status sentinel exists per shard index 0–7; it has no way to tell a sentinel uploaded by the
legitimate `mutants` matrix job apart from one uploaded by any OTHER job in the same
workflow run. A PR that adds (or repurposes) a sibling job in the same workflow run — one
with no relationship to `cargo mutants` at all — to upload 8 well-formed
`mutants-shard-status-{0..7}` sentinels plus 8 well-formed `mutants-shard-outcomes-{0..7}`
files (each summing to the `MUTANT_COUNT` `mutants-plan` computed, at a 100% kill rate)
would be indistinguishable from a genuine shard run to every check `mutants-aggregate.sh`
performs: the duplicate/stray-artifact guard (`actual_sentinel_count -ne EXPECTED_SHARDS`)
only catches a *miscount*, not a well-formed spoof landing at exactly the expected count.

This path is **not** narrowed by the `MUTANT_COUNT` reconciliation discussed above — a
spoofing PR author has the same offline access to `cargo mutants --list --in-diff`'s output
that `mutants-plan` itself uses (it is, after all, just a `cargo mutants --list` invocation
against the PR's own diff, requiring no code execution privilege beyond what any PR author
already has to open the PR), so a spoofed submission can make its fabricated totals
reconcile exactly against Step 4's check.

**Same blast radius as the build.rs/proc-macro path: mutation-quality signal only.** No
repository secret or `GITHUB_TOKEN` is read or referenced by this mechanism. **Same
mitigation, too: this requires a visible `ci.yml` diff** (a new or repurposed job,
fabricated JSON payloads) — exactly the shape of change code review of a `ci-gate`-touching
PR is expected to catch (see CLAUDE.md's "CI Gate — SCOPE SUMMARY" "Review scope" note).
Documented and reasoned about, not fixed in this round; tracked alongside the
code-execution path as an accepted residual.

**Longer-term mitigation options (tracked as a future direction, not opened as a story by
this round):**
1. Independent per-mutant re-derivation — reconcile the actual *set* of mutant IDs each
   shard reports against the diff-derived set `mutants-plan` computes, not just the
   summary counts, closing the "right total, wrong contents" forgery this residual
   currently allows.
2. Sandboxed/isolated shard execution — run each shard's `cargo mutants` invocation in an
   environment with no write access to the artifact the aggregator later trusts, so a
   compromised build/test loop cannot influence its own scoring.
3. Provenance-bound artifact download — verify the uploading job's identity (e.g. via the
   GitHub API's run/job metadata) rather than trusting artifact NAME PATTERNS alone, closing
   the spoofed-sibling-artifact path documented immediately above without requiring either
   of the two options above.

Same documentation posture as this repo's other accepted CI-gate residuals (the sudo bound
and the two unpinned `uses:` values on the `ci-gate` decision path — see CLAUDE.md's
"CI Gate — SCOPE SUMMARY"): documented and reasoned about, not fixed in this round.

## Future Path: Job Sharding (Path B) — LANDED (cycle-006, 2026-09-07)

**This is no longer a future path — it is the current design.** Path A's 240-minute
single-job budget did prove insufficient in practice (PR #778, 281 mutants), and
cycle-006 (S-cycle6-mutants-ci-sharding) implemented the sharded design this section
originally proposed. See **Sharded Mutation Gate (cycle-006)** above for the current,
authoritative topology, invariants, and job shapes. This section is retained,
unedited below, as the historical proposal — cross-referenced against what actually
landed:

1. ~~Run a dedicated `mutants-baseline` job first (`cargo test --locked`) to prove
   the suite is green; shards then run with `--baseline=skip`.~~ **Landed
   differently:** no dedicated `mutants-baseline` job was added. The shard jobs rely
   on the pre-existing `test` job (already a `ci-gate.needs` member, already proving
   the suite green on every PR) instead of duplicating that proof — see `ci.yml`'s
   `mutants` shard job's own comment: `--baseline skip` is "legitimate here because
   `test` (ci-gate.needs member) already proves the suite green before mutants ever
   runs."
2. **Landed as proposed.** `--timeout 240` is passed explicitly on every shard
   command (`cargo mutants --in-diff "$DIFF_FILE" --shard <k>/8 --sharding slice
   --jobs 2 --baseline skip --timeout 240`) — neither `timeout_multiplier` nor
   `minimum_test_timeout` is relied upon under `--baseline skip`, per this
   document's own **`--baseline=skip` and Path B** section above.
3. **Landed as proposed, by name.** `mutants-aggregate` (`needs: [mutants-plan,
   mutants]`) is the single shard-aggregator job wired into `ci-gate.needs` per
   DEC-096/097 — not any individual shard matrix job.
4. **Landed as proposed.** `mutants-plan` computes `DIFF_FILE` exactly once and
   uploads it as the `mutants-diff-file` artifact; every one of the 8 shards
   downloads and uses the SAME artifact bytes.
5. **Landed with a refinement, not exactly as proposed.** The base-ref drift guard
   does run in the aggregator job, not per-shard, as proposed — but its
   discriminator changed from the proposed `OVERALL_DIFF_LINES`-only check to
   `MUTANT_COUNT == 0` (established independently by `mutants-plan`, consulted only
   after INV-COMPLETE's sentinel-presence/interpretation and INV-AGG's
   `MUTANT_COUNT` reconciliation have already ruled out a shard-level crash or
   plan/execution mismatch) — see **INV-COMPLETE** above for why this is stronger
   than the original proposal.

## Changelog

| Date | Cycle | Change |
|------|-------|--------|
| 2026-09-07 | S-cycle6-mutants-ci-sharding | **Sharded mutation gate:** replaced the single `mutants` job with a three-job pipeline (`mutants-plan` → 8-shard `mutants` matrix → `mutants-aggregate`) plus an advisory nightly full-scope workflow (`.github/workflows/mutants-nightly.yml`, N=16). `mutants-aggregate` (extracted to `scripts/mutants-aggregate.sh`) replaces `mutants` as the `ci-gate.needs` member and computes a POOLED sum-not-average kill rate across all 8 shards (INV-AGG), with exact-equality `MUTANT_COUNT` reconciliation as a hard fail (both over- and under-count directions). Fail-closed, sentinel-based shard-completeness accounting (INV-COMPLETE) replaces the old artifact-count proxy, closing an all-shards-crash false-green and an empty-shard false-red the single-job design was never exposed to. A `>120`-in-diff-mutant escape hatch (`ESCALATION_THRESHOLD=120`, INV-ESCALATE) routes oversized PRs to an ordinary, actionable CI failure — never a silent skip or pass — resolved by splitting the diff or an admin branch-protection bypass. `cargo-mutants` pin tightened from major-only `@27` to the exact release `@27.1.0`. Both `scripts/check-ci-gate.sh` and the new `scripts/mutants-aggregate.sh` now source a shared `scripts/lib/trusted-jq.sh`. See **Sharded Mutation Gate (cycle-006)** above for the full account; governed by this policy doc per DEC-348/DEC-349 (policy-doc-only, no new PRD BC), mirroring the MUTATION-CI-TIMEOUT precedent below. No `src/` (product-code) changes — CI/CD infrastructure only. |
| 2026-08-31 | FIX-F6-MUTANTS-SCOPE | Scope-gap fix: added `src/cli/field.rs` (~91 mutants, S-580-1's `jr field options <field>` M1/M2/M3 resolution) and `src/cli/issue/field_resolve.rs` (~45 mutants, shared `--field` resolution/dispatch hub for `issue edit --field` and `issue create --field`) to `examine_globs` (18 → 20 entries). Both files had been omitted since creation across all field-dx PRs (S-580-1, #578 parts 1-5) — same P22-001/DEC-149/S-MUTANTS-SCOPE-1 drift class ("new CLI handler file → add to mutants.toml at creation"), meaning the required CI `mutants` gate generated zero mutants for either file across every field-dx PR to date. |
| 2026-08-21 | S-575-1 | Added new "Exclusions" section (distinct from `#[mutants::skip]` Whitelist Convention) and a single `exclude_re` entry in `.cargo/mutants.toml` for `src/api/jira/issues.rs:374:16: delete ! in JiraClient::search_issues_with_fields` — an infinite-loop mutant uncatchable-as-anything-but-TIMEOUT under the whole-binary test-harness execution model. Termination correctness remains verified by existing multi-page pagination tests. |
| 2026-08-14 | S-MUTANTS-SCOPE-1 | Scope widening + drift backfill: added `src/cli/queue.rs` and `src/main.rs` to `examine_globs` (16 → 18 entries). Backfilled §Scope bullets for 5 previously-undocumented `examine_globs` members: `src/cli/issue/interactions.rs`, `src/cli/issue/attachments.rs`, `src/api/jira/attachments.rs`, `src/api/jsm/attachments.rs`, `src/api/jsm/servicedesks.rs`. Closes drift item MUTANTS-SCOPE-GAP-QUEUE-MAIN. |
| 2026-07-02 | DEC-149 / S-MUTANTS-EXAMINE-GLOBS-1 | Scope widening: added `src/cli/issue/edit.rs` (~99 mutants) and `src/cli/issue/jsm_create.rs` (~9 mutants) to `examine_globs`. Root cause: ADR-0012 Seam A (PR #556) and Seam B (PR #558) relocated `handle_edit`, `handle_edit_bulk_labels`, `handle_edit_bulk_fields` → `edit.rs` and `handle_jsm_create` → `jsm_create.rs` from `create.rs`, but `examine_globs` was not updated. Total scope: 594 → ~702 mutants (+18%). Corrected `create.rs` entry to reflect remaining functions (`parse_field_kv`, thin dispatcher) only. |
| 2026-06-28 | MUTATION-CI-TIMEOUT (F5 doc-completeness, pass 3) | Added "Schema-Drift and False-Green Guards" section documenting @27 pin rationale + evidence basis, malformed-JSON guard, integer-validation guard, H-1 runtime schema-drift guard, and M-2 total_mutants reconciliation warning-only design decision. Disambiguated timeout_multiplier history (3.0 in S-346 original; 2.0 in pass-1; removed in pass-2). Softened .factory/cicd-setup.md reference from "canonical" to "historical/pending refresh." F5 final blocker F1 (HIGH) + O1 + O3. |
| 2026-06-28 | MUTATION-CI-TIMEOUT (F5 adversarial correction, pass 2) | HIGH false-RED fix: replaced SCOPED_DIFF_LINES-based drift guard with OVERALL_DIFF_LINES check. Old guard incorrectly failed comment-only/whitespace/reformat edits to scoped files. New guard: FAIL only when overall diff is EMPTY (genuine base-ref drift); PASS for any non-empty diff that yields 0 mutants. Grounded --timeout in measured baseline (133–145s on ubuntu-latest, 5 green develop runs 2026-06-28). Bumped --timeout 180 → 240 (old 180s gave only 3–6% headroom over worst-case 174s; 240s gives 38% headroom). Updated all --timeout references in policy doc, CLAUDE.md, and CI YAML. |
| 2026-06-28 | MUTATION-CI-TIMEOUT (F5 adversarial correction, pass 1) | CRITICAL: corrected inverted timeout-mechanism documentation. `minimum_test_timeout` is a FLOOR not a ceiling; it and `timeout_multiplier` are REMOVED from `.cargo/mutants.toml` (dead config once `--timeout` is set). Moved the absolute per-mutant ceiling to `--timeout 180` on the CLI invocation. Derived 180s value with explicit reasoning (baseline ~90s assumed + runner variance headroom). Documented F-2 (cancelled = blocking, intentional). Added F-3 positive-coverage assertion (in-scope: gate is now required; base-ref drift false-green is a correctness hole). Corrected budget model to use `--timeout 180` / `~90s avg`. Corrected Path B sharding guidance to remove `minimum_test_timeout` references. Updated Local Invocation commands to add `--timeout 180`. |
| 2026-06-28 | MUTATION-CI-TIMEOUT | Promoted `mutants` job to hard-required via `ci-gate.needs`. Raised job `timeout-minutes: 60 → 90`. Added (incorrectly) `minimum_test_timeout = 120` and `timeout_multiplier = 2.0` in `.cargo/mutants.toml` — both superseded by F5 correction above. |
| 2026-05-10 | F6 / S-346 | Initial policy established. Scope: bulk + create modules. Kill-rate target: 90%. `timeout_multiplier = 3.0`. Non-required (advisory) CI job. |
