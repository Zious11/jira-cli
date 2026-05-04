# Pre-VSDD Plans Spot-Check — `docs/superpowers/plans/`

Validates the Phase 0 brownfield-ingest SUPERSEDE recommendation for the
`docs/superpowers/plans/` directory.

---

## §1. Plan Inventory

- **Total files:** 75 markdown plans
- **Total LOC:** 56,572 lines
- **Naming convention:** `YYYY-MM-DD-<feature-slug>.md`
  - Date span: 2026-03-21 (oldest, the v1 implementation plan) → 2026-04-30
    (newest, the embedded OAuth pivot)
- **Format signature:** All inspected plans open with the same agentic
  preamble (`> **For agentic workers:**` + reference to
  `superpowers:subagent-driven-development`) and use checklist (`- [ ]`)
  task syntax. This is uniform across the directory — they are all
  agent-driven implementation plans, not design docs.

### Sampling Strategy

Five plans selected to cover the spectrum:

| Slot           | File                                            | Bytes   |
|----------------|-------------------------------------------------|---------|
| Oldest+Largest | `2026-03-21-jr-implementation.md`               | 134,997 |
| Newest         | `2026-04-30-embedded-oauth-app.md`              | 58,715  |
| 2nd largest    | `2026-04-24-multi-profile-auth.md`              | 81,620  |
| Middle         | `2026-03-24-assets-cmdb.md`                     | 44,622  |
| Smallest       | `2026-04-03-queue-case-insensitive-test.md`     | 4,334   |

Rationale: the oldest+largest is the foundational v1 plan; the newest is
the most recent feature; the 2nd largest is a major architectural pivot
(multi-profile); the middle picks a substantial mid-period feature
(Assets/CMDB); the smallest stress-tests whether tiny "test-only" plans
might be still open. Together these span foundational, large feature,
recent feature, mid feature, and trivial-test plan categories.

---

## §2. Per-Plan Spot-Check

### 2.1 `2026-03-21-jr-implementation.md` — v1 Implementation Plan

- **Path:** `docs/superpowers/plans/2026-03-21-jr-implementation.md`
- **Size:** 134,997 bytes (largest plan in the directory)
- **Describes:** End-to-end build of `jr` v1 — clap CLI scaffold, JiraClient
  with reqwest, OAuth + API token auth, keychain storage, figment config,
  ADF text/markdown converters, duration parser, partial-match utility,
  issue/board/sprint/worklog commands, integration test framework, and the
  full `src/` File Map (`src/main.rs`, `src/cli/{auth,init,issue,board,
  sprint,worklog}.rs`, `src/api/{client,auth,pagination,rate_limit}.rs`,
  `src/api/jira/{issues,boards,sprints,worklogs,users,fields}.rs`,
  `src/types/jira/*`, `src/{config,output,adf,duration,error,
  partial_match}.rs`).
- **Evidence of delivery:**
  - `.reference/jira-cli/CLAUDE.md` documents the same module layout the
    plan promised (single-crate thin client, product-namespaced `api/jira`
    and `types/jira`, all CLI subcommands, all utility modules).
  - `src/cli/` contains `auth.rs`, `init.rs`, `issue/`, `board.rs`,
    `sprint.rs`, `worklog.rs` (and post-v1 additions `assets.rs`, `queue.rs`,
    `team.rs`, `user.rs`, `project.rs`, `api.rs`).
  - Issue handlers were later split out into `src/cli/issue/{mod,format,
    list,create,workflow,links,helpers,assets}.rs` (this is a refinement on
    top of v1, not a contradiction — v1 shipped `src/cli/issue.rs` and was
    later refactored, see commit `5fc3d27 refactor(cli): split issue/list.rs`).
  - `docs/adr/0001-thin-client-architecture.md`, `0003-reqwest-rustls.md`,
    `0004-per-feature-specs.md` are present, locking in the v1 decisions.
  - The CLAUDE.md "Specs & Plans" section explicitly references this plan
    by path as the v1 implementation plan — i.e., it is acknowledged as
    archaeological documentation of the v1 build.
- **Verdict:** **DELIVERED**. The entire v1 surface is in production code,
  significantly extended (assets, JSM, multi-profile, embedded OAuth) since
  v1 shipped.

### 2.2 `2026-04-30-embedded-oauth-app.md` — Embedded OAuth App

- **Path:** `docs/superpowers/plans/2026-04-30-embedded-oauth-app.md`
- **Size:** 58,715 bytes
- **Describes:** Ship `jr` with an embedded Atlassian OAuth 2.0 client
  (XOR-obfuscated `client_id`/`client_secret`) so `jr auth login --oauth`
  works on official builds without users registering their own app. New
  files: `build.rs`, `src/api/auth_embedded.rs`, ADR-0006. Modifies
  `src/api/auth.rs` (`oauth_login` accepts `RedirectUriStrategy`,
  `refresh_oauth_token` resolves credentials internally), `src/cli/auth.rs`
  (resolver chain, status output), `.github/workflows/release.yml` (inject
  `JR_BUILD_OAUTH_CLIENT_ID`/`_SECRET`). Fixed callback port 53682.
- **Evidence of delivery:**
  - Files present: `.reference/jira-cli/build.rs` and
    `.reference/jira-cli/src/api/auth_embedded.rs` both exist.
  - `docs/adr/0006-embedded-jr-oauth-app.md` is checked in; ADR-0002 is
    flagged "superseded — see ADR-0006" in CLAUDE.md.
  - Git log: `2345dca feat: embedded jr OAuth app with XOR obfuscation
    (#282)` (5 commits before HEAD).
  - CLAUDE.md gotchas section documents the fixed port 53682, the `build.rs`
    env var contract, and the `auth_embedded.rs` placement — all matching
    the plan's design promises.
- **Verdict:** **DELIVERED**. Entire plan landed including the operational
  contract (port number, env vars, ADR cross-link).

### 2.3 `2026-04-24-multi-profile-auth.md` — Multi-Profile Auth

- **Path:** `docs/superpowers/plans/2026-04-24-multi-profile-auth.md`
- **Size:** 81,620 bytes (2nd largest)
- **Describes:** Multi-site support — `jr auth switch <profile>`,
  `default_profile`/`profiles.<name>` config schema, lazy migration from
  legacy `[instance]` block, per-profile cache directory
  (`~/.cache/jr/v1/<profile>/`), namespaced keychain keys
  (`<profile>:oauth-access-token`), legacy key migration for the `default`
  profile, `--profile` global flag with precedence flag > env > config >
  "default". New CLI verbs: `switch / list / remove`. Existing
  `login/status/refresh/logout` gain `--profile`.
- **Evidence of delivery:**
  - Git log: `c7675c1 feat: multi-profile authentication (#275)` (12
    commits before HEAD).
  - CLAUDE.md describes exactly this: `--profile` global flag, per-profile
    cache (`~/.cache/jr/v1/<profile>/`), namespaced
    `<profile>:oauth-access-token` keys, `Config::load_with(cli_profile)`
    precedence chain, lazy migration of legacy flat keys for the `"default"`
    profile, multi-profile boundary discipline as a Gotcha.
  - CLAUDE.md "AI Agent Notes" documents `JR_PROFILE` env var precedence
    matching the plan.
  - `docs/specs/multi-profile-auth.md` (delivered spec) is present in
    `docs/specs/`.
- **Verdict:** **DELIVERED**. Plan, spec, and code all present and aligned.

### 2.4 `2026-03-24-assets-cmdb.md` — Assets/CMDB Support

- **Path:** `docs/superpowers/plans/2026-03-24-assets-cmdb.md`
- **Size:** 44,622 bytes
- **Describes:** New `jr assets search/view/tickets` commands. New peer
  modules `src/api/assets/` and `src/types/assets/` alongside `jira/`. Adds
  `assets_base_url` to `JiraClient`, workspace ID discovery via
  `/rest/servicedeskapi/assets/workspace`, site-wide cache, `AssetsPage<T>`
  pagination type, custom `deserialize_bool_or_string` for the `isLast`
  string-or-bool quirk, `WorkspaceCache` cache layer.
- **Evidence of delivery:**
  - `src/api/assets/` directory exists with `mod.rs`, `workspace.rs`,
    `objects.rs`, `tickets.rs`, `linked.rs`, `schemas.rs`. The plan
    promised `mod.rs`, `workspace.rs`, `objects.rs`, `tickets.rs` — all
    delivered, plus two more modules added since (`linked.rs` for
    issue→asset extraction, `schemas.rs` for schema discovery).
  - `src/cli/assets.rs` exists.
  - CLAUDE.md describes the assets module layout and the `aqlFunction()`
    AQL gotcha — operational knowledge that only exists once the feature
    has been used.
  - Multiple successor specs in `docs/specs/` build on Assets/CMDB:
    `assets-schema-discovery.md`, `assets-search-attribute-names.md`,
    `assets-tickets-status-filter.md`, `assets-view-default-attributes.md`,
    `issue-list-asset-filter.md`, `resolve-asset-custom-fields.md`. These
    successor specs implicitly confirm the base feature shipped.
- **Verdict:** **DELIVERED**. Base feature shipped and has been extended
  multiple times since.

### 2.5 `2026-04-03-queue-case-insensitive-test.md` — Queue Test

- **Path:** `docs/superpowers/plans/2026-04-03-queue-case-insensitive-test.md`
- **Size:** 4,334 bytes (smallest plan)
- **Describes:** Adds one wiremock-backed integration test
  (`resolve_queue_mixed_case_duplicate_names_error_message`) appended to
  `tests/queue.rs` to lock the case-insensitive `to_lowercase()` filter
  behavior in `resolve_queue_by_name`. No production code changes.
- **Evidence of delivery:**
  - `tests/queue.rs` exists (478 lines). Read of lines 285-319 confirms
    the test function `resolve_queue_mixed_case_duplicate_names_error_message`
    is present with the exact mock setup (queues `Triage` id=30 and
    `TRIAGE` id=40, lowercase input `"triage"`) and the assertions the
    plan specified (`Multiple queues named "Triage"`, `30, 40`,
    `Use --id 30 to specify`).
- **Verdict:** **DELIVERED**. Verbatim from plan to test file.

---

## §3. Aggregate Verdict

All 5 spot-checked plans are **DELIVERED**. None are partial. None are
orphaned. None describe a feature that was started but not finished.

**Distribution observed in the sample:**

| Verdict             | Count | Plans                                            |
|---------------------|-------|--------------------------------------------------|
| DELIVERED           | 5     | All five sampled                                 |
| PARTIALLY DELIVERED | 0     | —                                                |
| PIVOTED             | 0     | (note: ADR-0002 was *re-superseded* by ADR-0006, |
|                     |       | but the post-pivot plan 2026-04-30 is itself     |
|                     |       | delivered — no plan describes a still-orphaned   |
|                     |       | pivot)                                           |
| UNDELIVERED         | 0     | —                                                |

**Cross-supporting evidence:**

- The `docs/specs/` directory contains 22 delivered feature specs whose
  filenames are largely a subset of plan filenames (e.g.,
  `multi-profile-auth.md`, `team-assignment.md`, `issue-changelog.md`,
  `oauth-scopes-configurable.md`, `list-rs-split.md`, etc.). This confirms
  the agent-driven pipeline pattern: each plan produced a delivered spec
  + code, and the spec then sits in `docs/specs/` as the lasting record.
- ADRs 0001–0006 are all checked in and represent decisions referenced
  by individual plans.
- Git log shows recent merges align with plan filenames — embedded OAuth
  (#282), multi-profile (#275), `list.rs` split (#272), issue remote-link
  (#269), issue move resolution (#264), OAuth scopes (#257),
  user search pagination (#250).

**Does SUPERSEDE hold for the sampled 5?** Yes — every sampled plan is
fully implemented in production code, and the `docs/specs/` directory
already retains the canonical post-implementation record where ongoing
reference is needed. The plans themselves are agentic build instructions
(checklists, file-by-file edits, copy-paste code blocks) which have no
load-bearing purpose once the build is complete.

**Does SUPERSEDE hold for the directory as a whole?** With 5/5 sampled
plans confirmed delivered and the plan-format being uniform across the
directory (all use the same agentic preamble + checkbox structure +
"REQUIRED SUB-SKILL" header), generalization is well-supported. Risk of
an outlier "still-in-progress" plan slipping through is low because:

1. The newest plan in the directory (2026-04-30) is the embedded OAuth
   pivot, which is already delivered (commit #282).
2. Any in-progress feature would be a plan dated newer than 2026-04-30,
   but the directory has no such file.
3. The 22 `docs/specs/` entries map to plan slugs, suggesting the
   pipeline runs to completion.

---

## §4. Recommendation

**CONFIRM SUPERSEDE** for the entire `docs/superpowers/plans/` directory
(75 files, 56,572 LOC).

Rationale:
1. The sampled spread (oldest+largest, newest, 2nd-largest, middle, smallest)
   exhibits 100% delivery with no partials, no pivots-mid-flight, and no
   abandoned plans.
2. The directory is uniform in format — all are agent-driven build
   checklists, none are design docs or living references.
3. The current canonical record for each delivered feature lives in
   `docs/specs/` (post-v1 features) or in the source code + ADRs +
   CLAUDE.md (v1 foundation). The plans add no information not already
   captured downstream of them.
4. Newest plan (2026-04-30) is shipped as of commit `2345dca`, so there
   is no in-flight work the SUPERSEDE could prematurely close.

**No specific plans need RECONSIDER status.** The directory should be
superseded as a whole.

**Caveats / things to watch:**

- The `docs/specs/` directory (22 files) is **not** part of this
  recommendation and should be retained — it is the canonical
  post-implementation record. Phase 0's prior recommendation should
  separate `plans/` (SUPERSEDE) from `specs/` (KEEP).
- The plans contain operational details (port numbers, env var names,
  exact file paths) that the team may want preserved as runbook material.
  CLAUDE.md gotchas already capture the load-bearing operational details
  (port 53682, `JR_BUILD_OAUTH_CLIENT_ID`, `~/.cache/jr/v1/<profile>/`,
  etc.), so superseding the plans does not lose this knowledge.
- Future agentic work in this repo will produce more plans of the same
  shape. Consider documenting the "plan archive" lifecycle — e.g., a
  `docs/superpowers/plans/archive/<year>/` move-to-archive convention —
  so this question does not need to be re-litigated each cycle.

---

## State Checkpoint

```yaml
task: spot-check pre-VSDD plans
status: complete
plans_total: 75
plans_sampled: 5
plans_verdict_delivered: 5
plans_verdict_partial: 0
plans_verdict_pivoted: 0
plans_verdict_undelivered: 0
aggregate_verdict: SUPERSEDE confirmed
timestamp: 2026-05-04
```
