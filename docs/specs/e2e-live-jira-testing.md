# Feature Spec: Live-Jira E2E Testing in CI

**Status:** Draft
**Author:** jr maintainers (brainstormed 2026-05-28)
**Research:** `.factory/research/e2e-real-jira-ci.md`
**Tracking:** TBD (GitHub issue to be filed)

## 1. Overview

Add an end-to-end (E2E) test suite that exercises the compiled `jr` binary against a
**real Jira Cloud instance**, wired into GitHub Actions. Today all 64 integration test
files are hermetic (wiremock / `JR_BASE_URL` mocks); nothing verifies `jr` against real
Jira API response shapes, pagination, eventual consistency, or error bodies. This feature
closes that gap with a small, high-value, **read-heavy + minimal-write** suite that runs on
push to `develop`/`main`, nightly, and on demand.

## 2. Goals / Non-Goals

### Goals
- Verify `jr`'s read commands against a live Jira Cloud site (real data shapes, real JQL,
  real pagination).
- Verify one happy-path write flow (create → verify → mutate → close) against the live site.
- Run automatically on push to `develop`/`main` + nightly schedule + manual dispatch.
- Be **non-blocking**: E2E failures surface but do not gate branches or PRs.
- Keep the Jira credential safe on a **public** repo (never reachable from fork PRs).
- Guarantee cleanup of every run's artifacts even when tests fail.

### Non-Goals
- **Not** testing the OAuth 3LO / keychain login flows (interactive/local; can't run in CI;
  already covered by unit/integration tests). E2E authenticates via the debug-only
  `JR_AUTH_HEADER` Basic seam.
- **Not** testing Assets/CMDB (`assets`, linked-asset lookup) live — Assets requires a **paid**
  plan (Standard+ as of Feb 2026; ~5k objects on Standard) and is **not available on Free**, so a
  free CI site cannot exercise it. These stay under wiremock.
- **Not** exhaustive command coverage — smoke-level happy paths per command family, not every
  flag combination.
- **Not** adding a `jr issue delete` command (teardown is close-only; see §7).

## 3. Authentication seam

CI runs `cargo test`, which builds in **debug** mode. The `JR_BASE_URL` and `JR_AUTH_HEADER`
env seams are gated to `#[cfg(debug_assertions)]` (release binaries ignore them — pinned by
`tests/base_url_release_gate.rs` and `tests/auth_header_release_gate.rs`). The E2E suite reuses
this exact seam — the same one the existing wiremock tests use — pointed at the real site:

- `JR_BASE_URL` = real site URL (e.g. `https://<site>.atlassian.net`), from a secret.
- `JR_AUTH_HEADER` = `Basic <base64(email:api-token)>`, composed in-workflow from secrets.

Atlassian **email + API-token Basic auth** is current and works non-interactively for REST v3
and the Agile API (only password Basic auth was deprecated, in 2019). This exercises everything
downstream of auth against real Jira. **Token caveat:** API tokens created after Dec 2024 expire
in ≤ 1 year — see §9 (Maintenance).

## 4. Test suite — `tests/e2e_live.rs`

A new gated integration test file.

### Gating
Mirrors `tests/oauth_embedded_login.rs`: every test is `#[ignore]` and early-returns unless
`JR_RUN_E2E=1`. A normal `cargo test` (local or in the existing `ci.yml`) never touches Jira.
The E2E workflow runs `cargo test --test e2e_live -- --include-ignored --test-threads=1`.

### Harness helper
```
fn e2e_cmd() -> Command   // Command::cargo_bin("jr") with:
  // - JR_BASE_URL    = env JR_E2E_BASE_URL
  // - JR_AUTH_HEADER = env JR_AUTH_HEADER (pre-composed Basic header)
  // - XDG_CONFIG_HOME / XDG_CACHE_HOME = per-process tempfile::tempdir()
  // - --no-input (non-interactive)
```
Plus helpers: `run_label()` → `e2e-<run_id>` (from `GITHUB_RUN_ID`, falling back to a process
timestamp locally); `project()` → env `JR_E2E_PROJECT`; `poll_view(key)` → **bounded retry** of
`jr issue view <KEY> --output json` until exit 0 (a few attempts with short backoff). The
working assumption is that GET-issue-by-key is read-after-write consistent — sidestepping the
search-index lag of seconds-to-minutes that affects JQL search — but **this is a reasonable
inference, not vendor-documented** (Atlassian documents only that *search* is eventually
consistent). The bounded retry in `poll_view` is the real guarantee; do not rely on a single
GET succeeding immediately.

### Read coverage (assert exit 0 + JSON shape)
| Command family | E2E assertion |
|---|---|
| `issue list --jql "project=<E2E>" --output json` | valid JSON array — **also the auth-seam validator** (first real network call; a 401 means the `JR_AUTH_HEADER` seam/credential is broken). `auth status` is intentionally NOT tested: it emits no JSON and makes no API call (see story AC-004-v2). |
| `issue search` / list with JQL | filters apply |
| `issue view <seed-or-created-key> --output json` | issue fields present |
| `board list --output json` | the Scrum board appears |
| `sprint list` / `sprint current` | sprints enumerate (Scrum project) |
| `worklog list <key>` | valid (possibly empty) list |
| `user search <self> --output json` | service account resolves |
| `project fields --project <E2E> --output json` | JSON **object** with `issue_types` + `statuses_by_issue_type` keys (types/priorities/statuses are surfaced inside `project fields`; there are no separate `project types`/`project statuses` subcommands) |

### Write flow (one happy path, run-scoped)
1. `issue create --project <E2E> --type Task --summary "[e2e <run_id>] ..." --label e2e-<run_id> --output json` → capture `key`.
2. `poll_view(key)` until consistent.
3. `issue edit <key>` (e.g. add/update summary or a second label).
4. `issue comment <key> ...`.
5. `worklog add <key> 5m ...`.
6. `issue move <key> $JR_E2E_STATUS_IN_PROGRESS` then `$JR_E2E_STATUS_DONE` (single-key `move` is idempotent). Status names are configurable via env vars (see §8); defaults are `"In Progress"` and `"Done"` respectively.
7. Best-effort in-test close; **guaranteed** close handled by the workflow teardown (§5).

### Optional / feature-flagged
- **JSM** (gated on `JR_E2E_JSM_PROJECT`; value `EJ`; skip cleanly when unset): seven test
  functions covering queue list/view, requesttype list/fields, comment visibility, create
  round-trip, and the non-JSM guard. All tests use dynamic fixture discovery from list output
  (no hardcoded queue/RT id env vars). Scenarios 5 and 6 create JSM requests and self-close in
  the test body (see §6-teardown note). Added in S-JSM-E2E-1 (spec `docs/specs/jsm-e2e-coverage.md`).
  - `test_e2e_jsm_queue_list_shape` — queue list shape; per-item `id`+`name` assertions
  - `test_e2e_jsm_requesttype_list_shape` — requesttype list shape; per-item `id`+`name` assertions
  - `test_e2e_jsm_queue_view` — exercises by-name and `--id` routing; asserts issue-array shape on both paths (`queue view` returns the queue's issues, not a queue identity object)
  - `test_e2e_jsm_requesttype_fields` — requesttype fields shape; top-level `"fields"` key; numeric-bypass pin
  - `test_e2e_jsm_comment_visibility` — create request; add public + `--internal` comment; assert `sd.public.comment` property round-trip; self-close
  - `test_e2e_jsm_create_request_roundtrip` — `issue create --request-type` (ADR-0014 fork); assert `{"key":"EJ-N"}`; `poll_view`; self-close
  - `test_e2e_jsm_non_jsm_guard` — `queue list --project <ES>` exits 64; stderr contains `"Jira Service Management project"` (does NOT require `JR_E2E_JSM_PROJECT`)
- **Sprint mutation** (`sprint add/remove`): only if `JR_E2E_BOARD_ID` is set.

## 5. CI workflow — `.github/workflows/e2e.yml`

```yaml
on:
  push:           { branches: [develop, main] }
  schedule:       [ { cron: "0 6 * * *" } ]   # 06:00 UTC nightly; also keeps the site warm
  workflow_dispatch:

concurrency: { group: jira-e2e, cancel-in-progress: false }   # serialize on shared site

jobs:
  e2e:
    # Combined gate: never on PRs AND require JR_E2E_ENABLED repo variable.
    # JR_E2E_ENABLED MUST be a repository variable (not environment-scoped):
    # environment variables are not available in jobs.<id>.if at scheduling time.
    # Forks without this variable set receive empty string → job skips cleanly.
    if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'
    runs-on: ubuntu-latest
    environment: jira-e2e                      # secrets gated to this env + branch policy
    timeout-minutes: 20
    permissions: { contents: read }
    steps:
      - harden-runner (egress-policy: block, allowlisted)
          # block is fail-closed: an unlisted host aborts the job rather than leaking
          # the credential. api.atlassian.com is intentionally omitted — the E2E suite
          # uses Basic auth against *.atlassian.net directly and makes no OAuth/cloudId
          # calls (those are the OAuth 3LO path, not exercised here).
      - checkout
      - install Rust
      - name: Preflight — assert required E2E config
        # Collects ALL missing required values before failing, so an operator fixing
        # a fresh environment sees every gap at once rather than one re-run at a time.
        # env: sources each var from its secret/variable (using ${{ secrets.X }} /
        #      ${{ vars.X }} expressions — shown as bare names here for readability).
        env: { JR_E2E_BASE_URL: ${{ secrets.JR_E2E_BASE_URL }},
               JR_E2E_PROJECT: ${{ vars.JR_E2E_PROJECT }},
               JR_E2E_EMAIL: ${{ secrets.JR_E2E_EMAIL }},
               JR_E2E_API_TOKEN: ${{ secrets.JR_E2E_API_TOKEN }} }
        # Canonical implementation: see .github/workflows/e2e.yml and
        # docs/specs/e2e-fork-safe-ci-enablement.md §2.4 for the normative form.
        run: |
          missing=()
          [ -z "${JR_E2E_BASE_URL:-}" ]  && missing+=("JR_E2E_BASE_URL (secret, jira-e2e environment)")
          [ -z "${JR_E2E_PROJECT:-}" ]   && missing+=("JR_E2E_PROJECT (variable)")
          [ -z "${JR_E2E_EMAIL:-}" ]     && missing+=("JR_E2E_EMAIL (secret)")
          [ -z "${JR_E2E_API_TOKEN:-}" ] && missing+=("JR_E2E_API_TOKEN (secret)")
          if [ ${#missing[@]} -gt 0 ]; then
            printf '::error::E2E preflight failed — missing required config:\n'
            printf '  - %s\n' "${missing[@]}"
            exit 1
          fi
          echo "E2E preflight OK — all required config present."
      - compose JR_AUTH_HEADER from secrets:
          JR_AUTH_HEADER="Basic $(printf '%s:%s' "$EMAIL" "$TOKEN" | base64 | tr -d '\n')"
      - cargo test --test e2e_live -- --include-ignored --test-threads=1
          env: JR_RUN_E2E=1, JR_BASE_URL, JR_AUTH_HEADER, JR_E2E_PROJECT, GITHUB_RUN_ID, (optional JSM/board vars)
      - name: Teardown (close-only)
        if: always()
        run: |
          # close every issue this run created, by label
          # JR_E2E_STATUS_DONE defaults to "Done" if unset
          STATUS_DONE="${JR_E2E_STATUS_DONE:-Done}"
          jr issue list --jql "project=$JR_E2E_PROJECT AND labels=e2e-${GITHUB_RUN_ID} AND statusCategory != Done" --output json \
            | jq -r '.[].key' \
            | while read -r KEY; do jr issue move "$KEY" "$STATUS_DONE" || true; done
```

- **Non-blocking:** this workflow is separate from `ci.yml` and is not added to branch
  protection's required checks.
- **Teardown is close-only:** transitions this run's issues to `Done` (no DELETE, no new
  command, no delete permission). Accumulated closed issues are acceptable on a throwaway site
  and remain selectable by label.

## 6. Secret safety (public repo)

- GitHub already withholds repo/environment secrets from `pull_request` runs triggered by
  **forks** (only `GITHUB_TOKEN`, downgraded to read-only, is passed). Fork PRs therefore can
  never read the Jira credential.
- **Primary fork-safety mechanism — `JR_E2E_ENABLED` repository variable:** the `e2e:` job
  is gated at scheduling time by `vars.JR_E2E_ENABLED == 'true'`. This variable must be set
  as a **repository-level variable** (not an environment variable scoped to `jira-e2e`).
  Environment-scoped variables are not available in `jobs.<id>.if` expressions, which are
  evaluated before the job starts. On any fork that has not explicitly created this repository
  variable, `vars.JR_E2E_ENABLED` evaluates to an empty string → condition false → job is
  skipped (not failed). This prevents fork contributors from seeing red CI runs they cannot
  fix. See `docs/specs/e2e-fork-safe-ci-enablement.md §2.3` for the full architectural
  rationale. The canonical repo (`Zious11/jira-cli`) sets `JR_E2E_ENABLED=true` as a
  repository variable at `https://github.com/Zious11/jira-cli/settings/variables/actions`.
- **Defense-in-depth:** the job is bound to a GitHub **Environment** named `jira-e2e` whose
  **deployment-branch policy** is restricted to `develop` and `main`. Even a same-repo feature
  branch cannot read the environment's secrets. Optionally add **required reviewers** on the
  environment for an approval gate.
- The workflow is `push`/`schedule`/`workflow_dispatch` only and guarded by
  `if: github.event_name != 'pull_request' && vars.JR_E2E_ENABLED == 'true'`. **`pull_request_target` is never used.**

## 7. Isolation & cleanup

- **Dedicated project** `<E2E>` used by nothing else — a botched teardown never touches real work.
- **Run-scoped label** `e2e-<run_id>` (+ summary prefix `[e2e <run_id>]`) on every created
  artifact, so teardown selects exactly this run's issues and concurrent/serialized runs don't
  interfere.
- **Guaranteed teardown** via the `if: always()` close-only step (§5).
- **Read-after-write consistency:** poll `jr issue view <KEY>` (GET-by-key, *assumed* consistent —
  see §4) with bounded retry instead of searching (JQL search is documented eventually consistent,
  lag of seconds-to-minutes).
- **Rate limits:** rely on `jr`'s built-in 429/`Retry-After` retry (`api/rate_limit.rs`,
  `api/client.rs`); the suite does not add a competing retry layer and keeps request volume modest.

## 8. Configuration inventory (GitHub Environment `jira-e2e`)

| Name | Kind | Where Set | Required | Notes |
|---|---|---|---|---|
| `JR_E2E_ENABLED` | Repository variable (`vars.*`) | **Repository level** (not environment-scoped) | Yes — canonical repo; forks default OFF | `vars.JR_E2E_ENABLED == 'true'` gates **both** the `e2e:` job (e2e.yml) and the `sweep:` job (e2e-sweeper.yml) at scheduling time. MUST be a repository variable, not an environment variable (environment variables are not available in `jobs.<id>.if:`). Forks with this variable unset receive empty string → both jobs skip cleanly. Create via Settings → Secrets and variables → Actions → Variables → Repository variables (see §10 step 7). See `docs/specs/e2e-fork-safe-ci-enablement.md §2.3`. |

| Name | Kind | Example | Notes |
|---|---|---|---|
| `JR_E2E_BASE_URL` | secret | `https://<site>.atlassian.net` | real URL never in repo |
| `JR_E2E_EMAIL` | secret | `ci@example.com` | service-account login |
| `JR_E2E_API_TOKEN` | secret | `<365-day token>` | annual rotation (§9) |
| `JR_E2E_PROJECT` | variable | `E2E` | Scrum project key |
| `JR_E2E_BOARD_ID` | variable (optional) | `1` | enables sprint mutation |
| `JR_E2E_JSM_PROJECT` | variable (optional) | `EJ` | enables JSM tests (7 test functions added in S-JSM-E2E-1). Teardown for write tests (Scenarios 5+6) is `jsm_self_close(key, &h)` in the test body (S-JSM-E2E-2) — discovers a closing transition dynamically via `jr issue transitions --output json` (`statusCategory.key == "done"`) rather than hardcoding `"Done"`, because the EJ JSM workflow has no transition named "Done". NOT the label-based CI sweeper (labels do not propagate from `servicedeskapi` to Jira issue labels). Orphan risk LOW and accepted (spec `docs/specs/jsm-e2e-coverage.md §6.3`). Activate post-merge by setting `JR_E2E_JSM_PROJECT=EJ` as an environment variable in the `jira-e2e` GitHub Environment. |
| `JR_E2E_STATUS_DONE` | variable (optional) | `Done` | workflow status name for "closed/done"; default `"Done"`. Set if the provisioned Scrum project uses a different status name (e.g. `"Closed"`). Used in write-flow step 6 and teardown. |
| `JR_E2E_STATUS_IN_PROGRESS` | variable (optional) | `In Progress` | workflow status name for "in progress"; default `"In Progress"`. Set if the provisioned Scrum project uses a different status name. Used in write-flow step 6. |

## 9. Maintenance

- **Annual API-token rotation:** Atlassian caps API tokens at 1 year. The owner mints a new
  365-day token from the CI service account before expiry and updates `JR_E2E_API_TOKEN`. When
  a token expires, the nightly run fails with HTTP 401 (loud signal). A runbook section in the
  repo documents the steps.
- **Keep-warm (data-loss safeguard):** the nightly run prevents ~120-day idle deactivation of the
  free site. This matters beyond convenience — after deactivation there is only a **~15–60 day
  reactivation grace window before the site's data is permanently deleted**, so the nightly job is
  effectively a data-retention guard, not just a latency optimization.

## 10. Provisioning runbook (one-time, manual — documented for the maintainer)

1. Create a free Jira Cloud site (e.g. `<site>.atlassian.net`) dedicated to E2E.
2. Create a CI service account (a real Atlassian account, member of the site).
3. Create a **Scrum** software project, key `E2E`, and a board (note the board id).
4. (Optional) Create a JSM project for JSM read tests.
5. Mint a 365-day API token for the service account.
6. In the repo: create Environment `jira-e2e`, set deployment-branch policy to `develop`/`main`,
   add the secrets/variables from §8.
7. **Create the repository variable to enable the E2E workflows:**
   Navigate to Settings → Secrets and variables → Actions → **Variables** tab →
   **Repository variables** section → click "New repository variable".
   Name: `JR_E2E_ENABLED`, Value: `true`.
   **This is a repository-level variable, NOT an environment variable.** Do not create it
   inside the `jira-e2e` Environment settings page — environment-scoped variables are not
   available in `jobs.<id>.if:` at scheduling time (see §6). Without this step, both the
   `e2e:` and `sweep:` jobs will skip on every trigger (green-but-skipped), and the README
   badge will reflect a skipped run rather than real test results.
8. Verify: trigger a `workflow_dispatch` run on `develop`; confirm the `e2e:` job starts
   (not skipped) and the preflight step passes.

## 11. Testing strategy

- The E2E tests themselves are the verification artifact; they assert exit codes + JSON shapes.
- A short local dry-run path: a maintainer with creds can run
  `JR_RUN_E2E=1 JR_E2E_BASE_URL=... JR_AUTH_HEADER=... JR_E2E_PROJECT=E2E cargo test --test e2e_live -- --include-ignored`.
- The gate (`JR_RUN_E2E`) is itself verified by the always-run `test_e2e_gate_disabled_when_env_unset`
  (a pure-function assertion over literal inputs) and `test_every_ignored_test_has_gate_guard`
  (a source meta-guard ensuring every `#[ignore]` test early-returns via `e2e_enabled()`);
  `ci.yml` never passes `--include-ignored`, so gated tests are inert there regardless of
  `JR_RUN_E2E`.

## 12. Open items (resolve during implementation)

1. **Exact read assertions** per command — finalize the minimal JSON-shape checks that are
   stable across a fresh free site (avoid over-fitting to seed data).
2. ~~**Transition names** — `move` targets depend on the project's workflow status names
   (`In Progress`, `Done`); confirm against the provisioned Scrum project, or make them
   configurable via vars.~~ **RESOLVED (F2, 2026-05-29):** Status names are now configurable
   via `JR_E2E_STATUS_DONE` (default `"Done"`) and `JR_E2E_STATUS_IN_PROGRESS` (default
   `"In Progress"`) env vars (see §8). The write-flow step 6 and teardown both use these
   vars, falling back to the defaults when unset. Hard-coded names are eliminated.
3. **JSM free-tier coverage** — confirm which JSM read commands work on free; keep behind the
   `JR_E2E_JSM_PROJECT` flag so the suite passes if a feature is unavailable.
4. **Token-expiry early warning** — optional: a scheduled step that warns ~30 days before
   expiry (requires storing the mint/expiry date as a variable). Minor; loud 401 on expiry is
   the baseline.

## 13. References

- Research report: `.factory/research/e2e-real-jira-ci.md` (primary-source-anchored).
- Atlassian Basic auth, API-token expiry, Search-and-Reconcile (eventual consistency).
- GitHub Actions: secrets withheld from fork PRs; Environments + deployment-branch policies.
- Peer signal: `ankitpokhrel/jira-cli` runs **mock-only** CI (no live-Jira E2E) — live E2E is a
  deliberate addition, not table stakes.
