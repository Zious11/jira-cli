# Fork-Friendly Release Ops — opt-in signing, backfill, and fork sync

## Problem

`jr` is developed in this repo and republished by downstream forks that add
platform packaging — today, a fork that codesigns/notarizes macOS builds with
an Apple Developer ID and publishes them through a Homebrew tap. Carrying
those pipelines only in the fork created two recurring costs:

1. **Sync churn.** Every fork-local edit to shared files risks merge
   conflicts on the next upstream sync, requiring manual intervention and
   risking the fork's additions being erased.
2. **Drift.** The fork's copies of release machinery silently fall behind
   upstream conventions (action pins, runner hardening, comment style).

## Approach

Host the release-ops workflows in the canonical repo, gated on repository
variables so they are **no-ops by default**. The canonical repo never needs
an Apple Developer Program account, signing secrets, or a tap repo — with no
variables set, nothing runs and CI is byte-for-byte unaffected. A fork opts
in by setting variables/secrets; the workflow files themselves stay identical
in both repos, so syncs are conflict-free.

## Components

| File | Purpose | Inert unless |
|---|---|---|
| `.github/workflows/sign-and-publish.yml` | Sign + notarize macOS binaries on five channels (alpha/dev/beta/rc/stable), publish to a Homebrew tap | `vars.SIGNING_ENABLED == 'true'` |
| `.github/workflows/backfill-release.yml` | Build + release an existing tag that has no GitHub Release (manual dispatch); optionally sign/publish | always manual; sign job needs `SIGNING_ENABLED`, homebrew job needs `HOMEBREW_TAP_REPO` |
| `.github/workflows/release-gap-fill.yml` | Daily tag-vs-release reconciliation; dispatches backfill for missing releases | `vars.RELEASE_GAP_FILL_ENABLED == 'true'` (manual dispatch always works) |
| `.github/workflows/sync-upstream.yml` | Scheduled fork→upstream merge with protected-file auto-resolution | `vars.SYNC_UPSTREAM_REPO` set |
| `.github/local-workflows.txt` | Registry of fork-local files that survive a sync ("ours" on conflict) | n/a (empty template here) |
| `Formula/*.rb` | Homebrew formula templates (placeholders sed'd at publish time) | only read by the jobs above |
| `packaging/Info.plist`, `scripts/create-{app,dmg,pkg}.sh` | macOS .app/.dmg/.pkg packaging helpers | only invoked by sign jobs |

## Repository variables (Actions → Variables)

| Variable | Effect when set | Canonical repo |
|---|---|---|
| `SIGNING_ENABLED` | `'true'` enables the sign/notarize jobs | unset |
| `HOMEBREW_TAP_REPO` | `owner/homebrew-name` tap repo to publish formulas to; also enables the homebrew jobs | unset |
| `RELEASE_GAP_FILL_ENABLED` | `'true'` enables the daily gap-fill schedule | unset |
| `SYNC_UPSTREAM_REPO` | `owner/repo` to merge from on a schedule (forks only) | unset |
| `GITLEAKS_DISABLED` | `'true'` disables the gitleaks secret-scan job in `ci.yml`; for forks that cannot obtain a gitleaks org/commercial license or prefer an alternative scanner | unset |

This is the same fail-safe pattern as `vars.JR_E2E_ENABLED`
(`docs/specs/e2e-fork-safe-ci-enablement.md`): scheduling-time gates on
repository variables, with unset evaluating falsy so forks and the canonical
repo skip cleanly.

## Secrets (only needed by repos that opt in)

| Secret | Used by |
|---|---|
| `APPLE_CERTIFICATE_P12` / `APPLE_CERTIFICATE_PASSWORD` | Developer ID Application cert (codesign) |
| `APPLE_INSTALLER_CERTIFICATE_P12` / `APPLE_INSTALLER_CERTIFICATE_PASSWORD` | Developer ID Installer cert (pkg) |
| `APPLE_SIGNING_IDENTITY` / `APPLE_INSTALLER_IDENTITY` | Identity strings passed to codesign/productsign |
| `APPLE_NOTARIZATION_APPLE_ID` / `APPLE_NOTARIZATION_PASSWORD` / `APPLE_NOTARIZATION_TEAM_ID` | notarytool |
| `HOMEBREW_TAP_TOKEN` | Push access to the tap repo |
| `SYNC_UPSTREAM_SSH_KEY` | Deploy key used by sync-upstream to push merged branches |

Signing jobs additionally run in the `release` environment so a fork can put
approval rules around them.

## Formula templates

`Formula/*.rb` carry `REPO_PLACEHOLDER`, `TAP_PLACEHOLDER`,
`VERSION_PLACEHOLDER`, `TAG_PLACEHOLDER`, and `SHA256_*_PLACEHOLDER`. The
publish jobs substitute them from `github.repository`,
`vars.HOMEBREW_TAP_REPO`, and the release metadata, so the templates are
repo-neutral. The macOS bundle identifier in `packaging/Info.plist` and
`scripts/create-pkg.sh` (`com.arcavenae.jr`) reflects the first signing fork;
a different signing fork should override it to match its own Apple team.

## Shared docs (CLAUDE.md, README, ADRs)

Files that exist in both repos and that both repos edit — `CLAUDE.md`,
`README.md`, `docs/adr/*` — are deliberately NOT protected by
`local-workflows.txt`. Listing them there would freeze the fork's copy and
silently drop every upstream improvement to the same file.

The recurring conflict shape is a small hunk where both repos added an
equivalent bullet (e.g. a `JR_*` env-var doc line for a feature that landed
in both) at the same position with slightly different phrasing. The sync
workflow can't auto-resolve it, so the merge stops for a human. To avoid it:

1. **Send the doc bullet upstream alongside the feature.** When a fork-local
   feature touches a shared doc, open a small `docs(CLAUDE.md)` PR upstream
   in the same window as the feature. The bullet lands in upstream-canonical
   placement once; the fork's next sync fast-forwards through it.
2. **If the doc must land fork-first**, keep upstream-conventional placement
   and phrasing so the eventual merge resolves cleanly when upstream adds
   an equivalent.
3. **Never add shared docs to `local-workflows.txt`.** That converts shared
   content into fork divergence and swallows upstream edits to the same file.

## Security constraints (sign-and-publish.yml / backfill-release.yml)

These requirements apply to **every job** in `sign-and-publish.yml` or
`backfill-release.yml` that meets any of the following criteria, computed
per-job by inspection — NOT by a hardcoded job-name list:

- (a) the job uses any `secrets.*` value (Apple Developer ID credentials,
  tap token, etc.), OR
- (b) the job or the workflow declares `permissions: contents: write`, OR
- (c) the job references a named `environment:` that carries secrets.

An implementing guard MUST compute scope by inspecting each job for criteria
(a)–(c) above. Hardcoding a fixed job-name list (e.g. only "sign" and
"release") is prohibited: it silently misses newly-added jobs that meet the
criterion. The job names `stable-sign`, `alpha-sign`, `sign`, and `release`
are illustrative examples of current jobs that happen to meet the criterion —
they are NOT a normative enumeration. Any job added to either file that meets
(a), (b), or (c) is automatically in scope.

### No inline context data in shell run-blocks (CWE-77)

**DEFAULT-DENY rule for `run:` script bodies:** the ONLY context expressions
permitted inline in a `run:` body of any in-scope job are the format-safe
allowlist below. EVERY other context expression MUST be bound via a step-level
`env:` mapping and referenced as a double-quoted shell variable inside the
`run:` block — regardless of whether the value appears server-generated.

**Allowlist** (safe to expand inline — these are FORMAT-CONSTRAINED values:
`github.sha` is a hex string `[0-9a-f]{40}`, `github.run_id` and
`github.run_number` are integers, `github.repository` and
`github.repository_owner` are constrained to `[A-Za-z0-9._-]` with a single
`/` separator for owner/repo — GitHub's naming rules prohibit shell
metacharacters in repository and owner names; their format makes them safe
regardless of provenance):
- `github.sha`
- `github.run_id`
- `github.run_number`
- `github.repository`
- `github.repository_owner`

The allowlist is the ONLY exception set. Any context expression not explicitly
listed here MUST be bound via `env:`.

**Explicitly prohibited inline** (illustrative, non-exhaustive — the default-deny
rule covers everything not on the allowlist, including but not limited to):
- `github.event.*` (all fields — in particular
  `github.event.workflow_run.head_branch`)
- `github.head_ref`
- `github.ref_name`
- `github.ref`
- `github.base_ref`
- `github.actor`
- `github.triggering_actor`
- `inputs.*` (all workflow_dispatch inputs)
- `steps.*.outputs.*` and `needs.*.outputs.*` — REGARDLESS of whether the
  output appears server-generated; a step or job output can launder an
  attacker-controlled value through multiple hops (e.g.
  `stable-sign.outputs.tag` derived from
  `github.event.workflow_run.head_branch`), and a guard cannot reliably trace
  cross-job derivation chains. The safe, enforceable rule is to bind ALL
  non-allowlisted expressions, not to maintain a derivation-provenance list.

**`matrix.*` and `runner.*` are NOT subject to this rule.** `matrix.*` values
are workflow-author-defined static literals set in the matrix include list —
they are author-controlled, not attacker-controlled, and their format depends
entirely on what the author wrote. `runner.*` values are runner-provided
metadata (OS, architecture, temp path) and equally author/platform-controlled.
Neither source accepts attacker input. They may appear inline in `run:` bodies
without env-binding.

```yaml
# REQUIRED
- name: Extract release metadata
  env:
    HEAD_BRANCH: ${{ github.event.workflow_run.head_branch }}
  run: |
    TAG="$HEAD_BRANCH"
    ...

# PROHIBITED
- name: Extract release metadata
  run: |
    TAG="${{ github.event.workflow_run.head_branch }}"  # shell-injection vector
```

**Dangerous-sink rule:** a bound value, even when double-quoted, MUST NOT be
passed to any context that re-parses or re-executes it — including but not
limited to: `eval`, `bash -c`/`sh -c`, backticks, unquoted command
substitution, arithmetic context `$(( ))`, indirect expansion `${!var}`,
`source`/`.` of attacker-influenced content, here-strings or here-docs feeding
a parser, or re-execution via `xargs sh`/`printf -v`-then-execute.
Double-quoting is insufficient if the value reaches any such sink.

For `actions/github-script` steps, context values MUST be passed via the step
`env:` map and read as `process.env.X` in the JS body; inline `${{ }}`
interpolation into the script string is prohibited regardless of the source.

**Guard scope note:** The CI regression guard (below) MUST NOT flag context
expansions in `env:`, `with:`, or `if:` keys — ONLY those textually inside
a `run:` script body. A correctly-written step with `env: { HEAD_BRANCH: ${{
github.event.workflow_run.head_branch }} }` must not trigger the guard. The
canonical-repo ci-gate MUST pass with the env-bound HEAD_BRANCH example above
present; if the guard fires on `env:` keys it MUST be fixed before merge.

Rationale: GitHub's "Security hardening for GitHub Actions — Understanding the
risk of script injections" identifies inline `${{ expression }}` expansion in
`run:` blocks as a shell-injection risk. A branch name containing shell
metacharacters (`;`, backticks, `$()`) is interpreted as shell syntax during
template expansion, before the runner executes the script. Binding via `env:`
passes the value as a literal environment variable — metacharacters are inert
at the variable-reference site, but remain active at any downstream code sink.

During an F5 adversarial pass, scan ALL `${{ }}` inline expansions in every
job that has secrets OR `contents: write` in scope (per criteria (a)–(c) above)
across BOTH workflow files — not just the identified `head_branch` field — to
confirm no context expressions outside the allowlist are inline-expanded in
`run:` blocks. The scope is computed by inspecting each job, not from a fixed
list: current in-scope examples include `stable-sign` and `alpha-sign` in
`sign-and-publish.yml`, and the `sign` and `release` jobs in
`backfill-release.yml`. `backfill-release.yml` inlines `${{ inputs.tag }}`
across multiple signing-path `run:` blocks; these are in scope for the same
CWE-77 env-binding remediation. `${{ github.repository }}` is on the allowlist
(format-constrained) so the existing inline usage in the `release` and homebrew
jobs requires no remediation. The F5 scan scope and this normative rule cover
the same surface: every job meeting criteria (a)–(c) across both files.

### Atomic alpha-tag creation (no TOCTOU)

The `alpha-sign` job MUST use atomic, server-side tag creation via the GitHub
API. Two concurrent `develop` pushes can observe the same tag count and compute
the same tag name, producing a race to last-write; only a server-enforced
uniqueness check eliminates the TOCTOU window.

**Complete control flow for the "Generate alpha version" step** — the entire
tag-reservation sequence MUST be implemented exactly as described here.
Piecewise interpretation is prohibited; this block is the sole normative
source of truth for an F4 implementer.

```
1. Bind inputs from env: (CWE-77 rule)
     COMMIT_SHA  ← env: bound from github.sha
     GH_TOKEN    ← env: bound from github.token
     DATE        ← $(date -u +%Y%m%d)   [computed locally, not from context]

2. Compute seed hint
     EXISTING ← git ls-remote --tags origin "refs/tags/alpha-${DATE}.*" | wc -l
     SEQ      ← EXISTING + 1
     (The seed is a starting hint only. Correctness does NOT depend on its
      accuracy. The atomic reservation + retry loop is the sole guarantor of
      uniqueness. Re-counting on retry is FORBIDDEN — see step 4.)

3. Attempt atomic reservation (first try, sequence = SEQ)
     gh api --method POST /repos/{owner}/{repo}/git/refs \
       -f ref="refs/tags/alpha-${DATE}.${SEQ}" \
       -f sha="$COMMIT_SHA"
     → HTTP 201: reservation succeeded. TAG="alpha-${DATE}.${SEQ}". Go to step 5.
     → HTTP 422: ref already exists (another run reserved it). Go to step 4.
     → Any other non-zero exit: fatal — exit 1 with diagnostic.

4. Retry loop (on 422 only)
     MAX_ATTEMPTS=10   [includes the first attempt in step 3]
     ATTEMPT=1
     while ATTEMPT < MAX_ATTEMPTS:
       SEQ ← SEQ + 1               [increment from just-rejected sequence;
                                     NEVER re-count remote tags]
       ATTEMPT ← ATTEMPT + 1
       gh api --method POST /repos/{owner}/{repo}/git/refs \
         -f ref="refs/tags/alpha-${DATE}.${SEQ}" \
         -f sha="$COMMIT_SHA"
       → HTTP 201: TAG="alpha-${DATE}.${SEQ}". Go to step 5.
       → HTTP 422: continue loop.
       → Any other non-zero exit: fatal — exit 1 with diagnostic.
     Loop exhausted without success:
       exit 1  ["alpha tag reservation failed after N attempts — burst contention
                 ceiling exceeded; retry when concurrent workflow runs settle"]
     Silent success, '|| true', and swallowed skips are PROHIBITED on exhaustion.

5. Export reserved name
     echo "tag=$TAG"         >> "$GITHUB_OUTPUT"
     echo "version=$TAG"     >> "$GITHUB_OUTPUT"
```

There is NO pre-reservation purge step. The unconditional
`gh release delete --cleanup-tag` that appeared in prior versions of this step
is DROPPED. Rationale: a pre-purge targeting the seed name (e.g. `.1`) is
destructive and TOCTOU-unsafe — it can delete a ref that a concurrent run just
reserved if both runs compute the same seed. With atomic reservation and
bounded retry, a pre-existing ref (orphan from a prior failed run) simply
causes a 422 and the loop walks to the next sequence number, producing a
harmless gap rather than a destructive overwrite. Sequence-number gaps in the
alpha channel are acceptable. Correctness is guaranteed by the reservation
loop, not by pre-cleaning.

Orphaned alpha tags and releases from prior failed runs are NOT cleaned by this
step. Orphan cleanup is a SEPARATE out-of-scope concern; a future housekeeping
story should address it (e.g. a scheduled job that removes alpha tags/releases
older than N days with no associated binary assets).

**Post-reservation invariant:** nothing after the reservation (step 5) may
delete or recreate the reserved ref. `gh release create` and all subsequent
steps consume the already-reserved `$TAG` without touching the tag ref itself.

`$COMMIT_SHA` MUST be bound from an `env:` value (consistent with the CWE-77
rule above — never inline-expanded from context). `gh api --method POST` exits
non-zero on any 4xx response; the 422 body may additionally be captured for
diagnostic logging.

The `gh` CLI with `GH_TOKEN` is the established credential model for these
jobs (`persist-credentials: false` is set on the checkout); `git push` for
alpha-tag creation in the `alpha-sign` job is prohibited because the job
deliberately holds no git remote credentials and `git push` would require
re-introducing them. (The alpha-homebrew and stable-homebrew jobs legitimately
use `git push` to the tap repo via an x-access-token clone — this prohibition
is scoped to the `alpha-sign` job's tag-creation operation only. Both homebrew
jobs already access these values safely: `vars.HOMEBREW_TAP_REPO` is bound via
a step-level `env:` key and referenced as `$HOMEBREW_TAP_REPO` in the run
block; `github.repository` is read via the runner-provided env var
`${GITHUB_REPOSITORY}` — neither is inlined as `${{ }}` inside a `run:` shell
script. There is therefore no inline-context-injection sink in the homebrew
jobs for these two values. Both jobs remain within the CWE-77 audit scope and
carry lower attacker-control risk than event-sourced context, but require no
remediation for this particular pair of values.)

`git push --force` on a tag is prohibited (defeats atomicity).
`--force-with-lease` does not apply to new tag creation and provides no safety.

### Required CI regression guard

A one-shot F5 scan provides no protection against re-introduction of inline
context expansions in these secret-bearing workflows. A CI guard is REQUIRED
(not optional) that:

1. Scans all `run:` blocks reachable from every job meeting criteria (a)–(c)
   above across BOTH workflow files — including any local composite actions they
   invoke (not only two hard-coded workflow filenames) — for inline `${{ }}`
   expansions of context expressions not on the allowlist. The extraction MUST
   be YAML-structure-aware — either by parsing the document and iterating
   `jobs.*.steps[].run`, or by mandating a workflow-security linter that models
   `run:` block boundaries such as zizmor or actionlint. A naive line-oriented
   grep is INSUFFICIENT: it cannot reliably delimit `run:` scope and misses
   `${{` split across lines in folded or block scalars.
2. Fails CI if any inline non-allowlisted expansions are found inside `run:`
   blocks. Context expansions in `env:`, `with:`, or `if:` keys MUST NOT be
   flagged. `matrix.*` and `runner.*` inline expressions MUST NOT be flagged
   (see above).
3. Emits a runtime-computed positive-coverage assertion on success that reports
   the COUNT OF JOBS classified in-scope AND total `${{ }}` occurrences scanned
   vs. classified — for example: "N jobs in-scope, scanned M run-blocks across P
   workflows, K total ${{}} expressions scanned, 0 inline non-allowlisted
   expansions" — so a broken script or unexpectedly low scanned count is
   immediately visible and cannot produce a silent false-green.
4. Is wired into CI by adding its job to `ci-gate.needs` per the project's
   CI-gate convention (CLAUDE.md). Do NOT wire it directly into branch
   protection.
5. **Fails closed (non-zero exit) on any of the following error conditions:**
   - A YAML parse error in any scanned workflow file.
   - The YAML parsing library is unavailable or fails to load.
   - A workflow file that is in scope cannot be read (missing, permission
     denied, etc.).
   - Structural scope detection finds ZERO in-scope jobs in a file that is
     expected to contain signing or secrets jobs. Zero in-scope jobs is a
     sentinel for a broken scope-detection heuristic or a silently-renamed
     job — it MUST be treated as a guard failure, not a clean pass. The
     positive-coverage assertion on job count (requirement 3) is the
     mechanism: if the count is zero, the guard exits non-zero with a
     diagnostic explaining that no in-scope jobs were found.

The guard MUST be implemented as a script in `scripts/` or via an action-lint
tool (zizmor preferred; actionlint as an alternative). F6 is responsible for
implementing this guard; it is in-scope for story S-FORK-OPS-SIGN-1.

**F6 negative-fixture sub-deliverable:** F6 MUST exercise the guard against a
deliberately-injected violation fixture — a sample `run:` block containing an
inline high-risk `${{ github.event.* }}` expansion — and confirm the guard
rejects it (non-zero exit). This demonstrates the detector is not a no-op and
prevents the TD-VSDD-057 false-green class where an inert detector reports
exit-0 for every input.

### Verify-step shell conventions (CWE-377 / CWE-390 / CWE-362)

All signature-verification steps in both workflow files must satisfy two shell
hygiene requirements:

**Temp-file safety (CWE-377/362):** Use `mktemp` with a `trap '...' EXIT`
cleanup for any temporary file. Predictable paths such as `/tmp/cs.out` or
`/tmp/spctl.out` are prohibited on shared runners.

```bash
# REQUIRED
CS_OUT=$(mktemp)
SPCTL_OUT=$(mktemp)
trap 'rm -f "$CS_OUT" "$SPCTL_OUT"' EXIT
codesign -dvv "$BIN" 2>&1 | tee "$CS_OUT"
spctl --assess --type install --verbose=4 "$PKG" 2>&1 | tee "$SPCTL_OUT"

# PROHIBITED
codesign -dvv "$BIN" 2>&1 | tee /tmp/cs.out   # predictable path
```

**Pipefail (CWE-390):** Verification steps must start with `set -eo pipefail`.
Without `pipefail`, a failing `codesign` or `spctl` in a `... | tee` chain
exits the pipe with `tee`'s exit code (0), masking the failure. `set -e` alone
only catches the last command in a pipeline.

```bash
# REQUIRED
set -eo pipefail
codesign -dvv "$BIN" 2>&1 | tee "$CS_OUT"

# PROHIBITED
set -e                                         # pipefail missing
codesign -dvv "$BIN" 2>&1 | tee "$CS_OUT"    # failure silently masked
```

Note: with `set -eo pipefail` active, the `codesign ... | tee "$CS_OUT"` line
is itself the failure point when `codesign` exits non-zero. The subsequent
`grep ... || { ...; exit 1; }` guard that many verification steps include MUST
NOT be removed as "redundant" — it checks a different condition (pattern
presence in the output), which `pipefail` alone does not enforce. Both guards
are required.

**Temp-file reuse across loop iterations:** one temp file per verification
target, reused across loop iterations, is acceptable. Do not create a new temp
file per iteration — that produces unbounded files on a shared runner.
The `trap '...' EXIT` cleanup handles the single set of named temp files
regardless of how many loop passes run.

## Known limitations

- `sync-upstream.yml` hardcodes the branch matrix
  (`main`, `develop`, `factory-artifacts`) to this repo's branch layout.
- Bulk-gap backfills are throttled (`max` input, default 5/run); remaining
  tags are picked up on subsequent scheduled runs.
- The alpha channel builds from source on every `develop` push (only when
  `SIGNING_ENABLED` is set), independent of tagged releases.
