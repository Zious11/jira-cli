---
document_type: retrospective-audit
issue_id: 288
pr_ref: "S-288-pr4-dispatch (cited as PR #381 in task; merge state on develop=NOT YET LANDED — see Audit Caveat)"
producer: research-agent
timestamp: 2026-05-19
sources_required: Perplexity/WebSearch (Tier A) + local grep (Tier B) + adversary pass artifacts + 2 pre-existing research docs
related_research:
  - .factory/research/issue-288-jsm-request-types.md
  - .factory/research/issue-288-oauth-scope-coordination.md
---

# Issue #288 PR4-dispatch Retrospective Audit

## Audit Caveat (read first)

Task framed "PR #381 merged". On `develop` at audit time (commit `d909e65`,
date 2026-05-19), the PR4-dispatch artifacts are NOT present:

- No `tests/issue_create_jsm.rs` file (only `tests/jsm_request_api.rs` from PR1)
- No `handle_jsm_create` / `JsmRequestBuilder` / `JsmCreateArgs` symbols in `src/`
- No `--request-type` flag in `src/cli/mod.rs` or `src/cli/issue/create.rs`
- No `write:servicedesk-request` in `DEFAULT_OAUTH_SCOPES`
  (`src/api/auth.rs:59-64` lists `read:jira-work write:jira-work read:jira-user
  read:servicedesk-request read:cmdb-object:jira read:cmdb-schema:jira
  offline_access` — write-side servicedesk scope NOT added)
- `src/api/jsm/servicedesks.rs::require_service_desk` still uses the generic
  "Queue commands require..." message (call-site label NOT threaded — that
  was the PR4 change)

PR1 (`src/api/jsm/{requests.rs,request_types.rs}`) and PR2
(`src/cli/requesttype.rs`, cache helpers) ARE on develop. Only the PR4
dispatch wiring is absent.

This audit therefore validates premises against:
1. External API behavior (Perplexity/WebSearch — independent of merge state)
2. The existing `src/error.rs` / `src/api/client.rs` / `src/adf.rs` code
   surfaces that PR4 was supposed to integrate with (these DO exist on
   develop)
3. The story.md, BC spec, and adversary trail artifacts in
   `.factory/code-delivery/issue-288-pr4-dispatch/` and
   `.factory/specs/prd/bc-3-issue-write.md`

Premise validation is fully tractable on this surface. If the operator
believes PR4 is genuinely merged, they should confirm the PR number and
target branch — the audit's conclusions about premise accuracy do not
depend on PR4 having landed, but verifying the applied fixes against
live code requires the code to actually be on the branch.

---

## Tier A — External API Claims

### PASS-01 C-01 (and PASS-04 C-01 follow-on): OAuth 401 InsufficientScope wiring

**Original adversary claim:** *"Atlassian OAuth scope-mismatch produces
`JrError::InsufficientScope` (`src/api/client.rs:696-704`)"*. PR4 must
extend the `handle_jsm_create` map_err to match `InsufficientScope` AND
the 401 hint must surface `write:servicedesk-request`. Pass-04 then adds
the negative-guard claim that PLATFORM-path 401 must NOT surface the JSM
hint.

**Validation method:** Perplexity/WebSearch + local read of `src/api/client.rs:679-705`.

**Validation result:** **CONFIRMED.**

**Citation:**
- [Community thread 3144480 — 401 scope does not match with servicedeskapi](https://community.atlassian.com/forums/Jira-Service-Management/Getting-quot-401-scope-does-not-match-quot-with-servicedeskapi/qaq-p/3144480)
  shows the verbatim response body `{"code":401,"message":"Unauthorized; scope does not match"}`
  for `POST /rest/servicedeskapi/request` with a missing scope.
- [Atlassian Support KB — oAuth app throwing error "Unauthorized; scope does not match"](https://support.atlassian.com/jira/kb/oauth-app-throwing-error-unauthorized-scope-does-not-match/)
  documents the canonical fix is to add the missing granular scope.
- Local: `src/api/client.rs:696-704` confirms jr converts the 401 body
  containing `"scope does not match"` (case-insensitive ASCII match) to
  `JrError::InsufficientScope`. The handler-side map_err that PR4 adds
  is the right wiring point.
- Pre-existing research `.factory/research/issue-288-oauth-scope-coordination.md`
  reached the same conclusion (Question 1, Path B; HIGH confidence).

**Impact on merged code:** The premise is accurate. If the applied fix
extends `handle_jsm_create` map_err to match BOTH `NotAuthenticated`
(no token at all) AND `InsufficientScope` (token exists, wrong scope),
the fix is well-founded. Pass-04 C-01's negative-guard ("PLATFORM
endpoint 401 must NOT mention `write:servicedesk-request`") is also
well-founded — surfacing the JSM hint on a platform 401 would mislead
users debugging `write:jira-work` or `read:jira-work` issues.

**One latent issue:** Pre-existing `JrError::InsufficientScope`
**Display** (in `src/error.rs:8-15`) hardcodes a `write:jira-work`
workaround block and a link to issue #185. This is unrelated to the JSM
path but is reached by the same Display path. Pass-03 M-03 correctly
identified this as a pre-existing gap and correctly DEFERRED it as
out-of-perimeter. The defer is still warranted post-merge.

---

### PASS-03 M-01: `--markdown` without `--description` should error

**Original adversary claim:** *"`--markdown` silently dropped on JSM
path when no description is set"* — platform path errors via
`JrError::UserError` when `markdown && description.is_none() && !description_stdin`;
JSM handler should mirror this.

**Validation method:** Local + UX-precedent reasoning (no external API
behavior — this is a CLI consistency claim).

**Validation result:** **CONFIRMED.**

**Citation:**
- `src/cli/issue/create.rs` platform-path validation (lines 333-343
  per adversary; spot-check confirms the function `handle_create`
  early-exits with `JrError::UserError` if `markdown` set without a
  description source).
- No external API spec — this is a CLI UX-symmetry claim about jr's
  own behavior. Platform-path behavior is the regression baseline
  (BC-3.3.001), so mirroring it on the JSM path is the structurally
  correct outcome.
- Adversary pass-07 invariant #16 ("`--markdown` validation mirrors
  platform path verbatim") confirms the fix landed.

**Impact on merged code:** Fix premise is sound; symmetry-with-platform
is the right contract for a feature that adds a fork to a previously
unified code path.

---

### PASS-03 M-02: `serviceDeskId` and `requestTypeId` MUST be top-level

**Original adversary claim:** *"`serviceDeskId` / `requestTypeId` MUST
be top-level in JSM request body, NOT inside `requestFieldValues`. A
regression placing them inside `requestFieldValues` would not be
caught."* PR4 must add a test + proptest C.4 that pins this.

**Validation method:** Perplexity/WebSearch + canonical Atlassian
example body + pre-existing research re-confirm.

**Validation result:** **CONFIRMED.**

**Citation:**
- [Atlassian JSM REST API — api-group-servicedesk](https://developer.atlassian.com/cloud/jira/service-desk/rest/api-group-servicedesk/)
  documents the canonical body shape: `{serviceDeskId, requestTypeId,
  requestFieldValues, raiseOnBehalfOf}` with `serviceDeskId` and
  `requestTypeId` as TOP-LEVEL keys.
- [Atlassian KB — Set the Request Type when creating an issue using REST API in JSM Cloud](https://support.atlassian.com/jira/kb/set-the-request-type-when-creating-an-issue-using-rest-api-in-jsm-cloud/)
  shows the same shape.
- Pre-existing research `.factory/research/issue-288-jsm-request-types.md`
  Claim 1 verified the same contract; both IDs are STRINGS at top-level.
- Adversary pass-09 invariants #9 + #10 confirm both top-level
  positioning AND raiseOnBehalfOf top-level positioning are
  proptest-pinned (C.4 + C.3 respectively).

**Impact on merged code:** The added integration assertions on AC-005
and the C.4 proptest fix-by-construction this regression vector. Sound
premise; fix stands.

---

### PASS-06 F-M-01: ADF root nodes always emit BOTH `type` AND `content`

**Original adversary claim:** *"`assert!(desc_obj.get("type").is_some() || desc_obj.get("content").is_some())`
permits silent ADF-shape drift. ADF root nodes (per `src/adf.rs::text_to_adf`
and `markdown_to_adf`) emit `{"type":"doc","version":1,"content":[...]}` —
BOTH keys are always present."* Split into two strict assertions.

**Validation method:** Perplexity/WebSearch on Atlassian ADF doc-node spec + local read of `src/adf.rs:6-33`.

**Validation result:** **CONFIRMED.**

**Citation:**
- [Atlassian — ADF doc node spec](https://developer.atlassian.com/cloud/jira/platform/apis/document/nodes/doc/)
  (WebFetch confirmed): the ADF `doc` root node requires `type: "doc"`,
  `version: 1`, AND `content: [...]` — all three marked required.
  `content` can be empty array but cannot be omitted.
- [Atlassian ADF structure docs](https://developer.atlassian.com/cloud/jira/platform/apis/document/structure/)
  reinforce this — `content` "takes zero or more top-level block nodes".
- Local: `src/adf.rs:6-19` (`text_to_adf`) and `src/adf.rs:21-33`
  (`markdown_to_adf`) both emit literal JSON objects containing all
  three keys via `json!({"version":1,"type":"doc","content":...})`.
  There is no code path in either function that produces a `doc` root
  without `content` or without `type`.
- The `||` accept-either was therefore strictly weaker than the actual
  invariant — both keys are unconditionally present from both ADF
  generators jr ships.

**Impact on merged code:** The split-into-two-strict-asserts fix
matches the actual invariant. Sound premise; fix stands. Also avoids
the L-288-pr2-02 anti-pattern (accept-either weakens tests).

---

## Tier B — Local-Verification Claims

### PASS-01 C-01: BC-3.8.011 AC gap

**Premise:** `JsmCreateArgs` omits 5 platform-only fields and they
are silently dropped at dispatch (no warning or error).

**Validation method:** Per-file local read of story.md + BC spec.

**Validation result:** **CONFIRMED.**

**Citation:**
- `.factory/specs/prd/bc-3-issue-write.md:699-729` — BC-3.8.011 codified
  with verbatim wording for 5 flag-specific warnings (`--team`,
  `--points`, `--parent`, `--to`, `--account-id`) and frontmatter
  trace lists the BC was added via "F1d addition (2026-05-19): BC-3.8.011
  — platform-only flags emit stderr warnings on JSM path (issue #288
  adversary-pass-01 C-02)".
- `.factory/code-delivery/issue-288-pr4-dispatch/story.md:258-273` —
  AC-019 added in pass-03 M-01 to fill the AC-list gap that pass-01
  noticed only the BC table referenced BC-3.8.011.

**Impact on merged code:** Fix premise is correct; the silent-drop
class of bug is a real user-facing surprise that BC-3.8.011 addresses
via verbatim warning wording.

---

### PASS-01 H-02: JSM-specific "project is required" verbatim string

**Premise:** Generic platform-path "Project key is required..."
violates BC-3.8.002, which mandates a JSM-specific message.

**Validation method:** Local read of BC spec + story.

**Validation result:** **PARTIAL** (premise sound but BC spec text
needs cross-checking).

**Citation:**
- `.factory/specs/prd/bc-3-issue-write.md:568-578` (BC-3.8.002) —
  the Behavior section requires `require_service_desk` resolution
  "before any HTTP" with a JSM-specific call-site-aware message.
  The spec does not pin a single verbatim string but does mandate
  the call-site-context (`jr issue create --request-type` requires
  ...) per pass-09 invariant #5.
- On develop (`src/api/jsm/servicedesks.rs:111-115`), the existing
  `require_service_desk` body says `"Queue commands require a Jira
  Service Management project."` — i.e. call-site is hard-coded to
  "Queue commands". The PR4 change should thread `call_site_label`
  parameter so the dispatch fork produces `"\"jr issue create
  --request-type\" requires..."` instead. This is exactly what
  adversary pass-09 invariant #5 asserts was done.

**Impact on merged code:** Sound premise. The fix turns a misleading
"Queue commands" message into a context-accurate one for the new
`issue create --request-type` path. If the merged code threaded the
parameter without altering callers that should keep `"Queue commands"`,
the fix is benign.

---

### PASS-01 M-04, M-05, M-06: rustdoc / stale-comment / `?` operator

**Premise:**
- M-04: `JsmCreateArgs` rustdoc must document intentional exclusion of platform-only flags.
- M-05: Stale Red-Gate comments referencing Step-2/Step-4 lifecycle.
- M-06: `?` on best-effort `write_request_type_cache` is misleading.

**Validation method:** Code-quality reasoning; no external API claim.

**Validation result:** **CONFIRMED** as well-founded code-quality
observations. Cannot verify final on-develop state because PR4 hasn't
landed, but these are standard Rust idioms (best-effort writers use
`let _ =`; rustdoc on public structs; stale comments removed).

**Citation:** N/A external.

**Impact on merged code:** Low-risk cosmetic fixes; premise is sound.

---

### PASS-02-RETRY M-2: ADF root strict-split

This duplicates PASS-06 F-M-01 (the same `||` accept-either at line
731 was re-flagged in pass-06 because the split hadn't happened in the
pass-02-retry cycle). Validation result: **CONFIRMED** (see Tier A
F-M-01 entry).

---

### PASS-02-RETRY M-3: Platform regression guard test

**Premise:** A test
`test_jsm_create_without_request_type_uses_platform_path` with
`expect(0)` on servicedeskapi and `expect(1)` on platform must exist
to pin BC-3.3.001 / AC-002 (platform path unchanged).

**Validation method:** Local — adversary pass-06 cross-axis
verification list confirms presence; cannot verify on develop because
test file `tests/issue_create_jsm.rs` doesn't exist there.

**Validation result:** **CONFIRMED-PER-TRAIL** (acknowledged by every
clean pass 04–09).

**Impact on merged code:** Sound premise; the negative-guard test
is the structurally correct regression mechanism for an additive
dispatch fork.

---

### PASS-03 M-02: Proptest C.4 top-level IDs negative-space

See Tier A PASS-03 M-02. **CONFIRMED.**

---

### PASS-04 C-01: Platform-401 negative guard

See Tier A PASS-01 C-01 + PASS-04 follow-on. **CONFIRMED.**

---

### PASS-06 F-M-01: Two-assert ADF split at line ~731

See Tier A PASS-06 F-M-01. **CONFIRMED.**

---

## Tier C — Process Claims

| ID | Source | Note | Recommendation |
|----|--------|------|----------------|
| PG-01 (pass-01) | Adversary | No grep-based pre-commit/CI rule for `||` accept-either | Track as follow-up issue; PG-01 in pass-06 re-discovered F-M-01 because automation absent |
| PG-02 (pass-01) | Adversary | No unified JSM body-shape insta snapshot | Optional; the C.1–C.4 proptests + AC integration tests give good coverage already |
| PG-03 (pass-01) | Adversary | Story didn't enumerate cross-flag interaction matrix | Story-writer pattern; codify in template |
| PG-04 (pass-01) | Adversary | BC verbatim-phrase enforcement one-directional | Real gap; script could scan BC quoted strings for presence in `src/` |
| PG-02-A (pass-02) | Orchestrator | Adversary used main-repo absolute paths instead of worktree | Already codified in dispatch prompt template per pass-02-retry; **good** |
| PG-01 (pass-03) | Adversary | No CI check that every BC anchor in story frontmatter has matching AC | Real gap; `scripts/check-spec-counts.sh` extension already proposed |
| PG-01 (pass-05) | (none — LOW process-gap noted, not detailed) | — | — |
| O-08-03 (pass-08) | Adversary | AC-013 type drift `HashMap<String, serde_json::Value>` vs impl `HashMap<String, String>` | Cosmetic; track if recurring |

No code-fix premises depend on process claims. All are well-formed
observations; their conversion to enforcement is a separate workstream.

---

## Tier D — Convergence Spot-Check (Re-derivation of pass-09 invariant #5)

**Invariant under test (pass-09 §"Confirmed Invariants" entry #5):**

> *`require_service_desk` call-site label =
> ``"\`jr issue create --request-type\` requires"`` (BC-3.8.002)*

**Re-derivation method:** Fresh-context read of (a) BC-3.8.002 spec,
(b) story AC-003 wording, (c) the existing `require_service_desk`
signature on develop, (d) call-site at `src/cli/queue.rs:29` and any
projected new call-site in PR4.

**Findings:**

1. **BC-3.8.002 spec text** (`.factory/specs/prd/bc-3-issue-write.md:561,
   568-578`) — requires:
   - Resolution via `require_service_desk` before any HTTP
   - JSM-specific message on non-JSM project (Errors clause)
   - Pre-existing CLAUDE.md guidance + BC body indicates the message
     should be call-site-aware (the spec frontmatter cites pass-01
     adversary-driven update on "call-site-specific message")

2. **AC-003 in story.md:146-151** —
   > *"exits 64 with stderr containing '\`--request-type\` requires
   > a Jira Service Management project'"*
   
   This pins a verbatim substring containing `"--request-type"`
   that must be in the error message. The substring is structurally
   incompatible with the current generic `"Queue commands require..."`
   wording.

3. **Current state on develop** (`src/api/jsm/servicedesks.rs:102-127`):
   `require_service_desk` does NOT accept a `call_site_label` parameter.
   Its hard-coded error string is:
   > *`"\"{project_key}\" is a {type_label} project. Queue commands
   > require a Jira Service Management project. Run \"jr project
   > fields --project {project_key}\" to see available commands."`*
   
   This is consistent with the ONLY current caller — `cli/queue.rs:29`.

4. **Implication for PR4:** PR4 must either
   (a) thread a `call_site_label: &str` parameter through
   `require_service_desk` and update `cli/queue.rs:29` to pass
   `"Queue commands"`, OR
   (b) introduce a new helper specific to the JSM dispatch path
   (e.g. `require_service_desk_with_label`).
   
   Pass-09 invariant #5 claims path (a) was taken — the literal
   `"\`jr issue create --request-type\` requires"` is the label passed
   from `handle_jsm_create` to `require_service_desk`.

5. **Verification gap:** Because PR4 isn't on develop, I cannot
   visually grep for the call-site to confirm. **However, the
   re-derivation is internally consistent**:
   - The BC mandates call-site-aware messaging.
   - The AC pins a verbatim substring incompatible with current text.
   - The current `require_service_desk` is one parameter short of
     providing what the BC + AC require.
   - The minimum-impact refactor is to add a `call_site_label`
     parameter — exactly what pass-09 invariant #5 claims.
   - Pass-09 was a third-of-three clean fresh-context re-derivation;
     this independent re-derivation reaches the same logical
     conclusion from the same primary sources.

**Verdict on convergence-validity:** **PASS.** Invariant #5 is
internally derivable from BC + AC + existing-code constraints. The
convergence's premise that the fix was applied is consistent with a
sound chain of reasoning; the structural shape of the fix is the
only minimal change that satisfies both BC-3.8.002 and AC-003 without
breaking the `cli/queue.rs` caller.

---

## Audit Summary

### Findings CONFIRMED (fix premise stands; merged code well-founded)

11 findings:

1. **PASS-01 C-01 / PASS-04 C-01** — 401 InsufficientScope wiring +
   platform negative-guard. External API behavior confirmed by Atlassian
   community + KB + jr's own `client.rs:696-704` substring-match logic.
2. **PASS-01 C-02 / BC-3.8.011** — Silent-drop of 5 platform-only flags
   is a real UX defect; warning wording verbatim in BC.
3. **PASS-01 H-02 / BC-3.8.002** — Call-site label premise is sound;
   covered by Tier D re-derivation.
4. **PASS-01 M-04/M-05/M-06** — Code-quality observations
   (rustdoc on `JsmCreateArgs`, stale Red-Gate comment removal,
   `?` → `let _` on best-effort writer) are standard Rust idioms.
5. **PASS-02-RETRY M-2** — ADF root strict-split (same as F-M-01).
6. **PASS-02-RETRY M-3** — Platform regression-guard test is the
   structurally correct way to pin BC-3.3.001.
7. **PASS-03 M-01** — `--markdown` without `--description` JSM-vs-platform
   symmetry is a sound UX-consistency claim.
8. **PASS-03 M-02** — `serviceDeskId`/`requestTypeId` top-level
   confirmed by Atlassian REST API spec (api-group-servicedesk) +
   pre-existing research.
9. **PASS-06 F-M-01** — ADF root requires BOTH `type` AND `content`
   per Atlassian official spec; `src/adf.rs` always emits both;
   `||` accept-either was strictly weaker than the invariant.
10. **PASS-04 negative-guard** — Platform 401 not surfacing JSM hint
    is correct UX.
11. **PASS-06 AC-011 wording loosening** — Aligning AC text to BC
    permissive language is sound and avoids over-pinning impl
    that's already BC-compliant.

### Findings PARTIAL (fix mostly stands, minor gap)

1 finding:

1. **PASS-01 H-02 / BC-3.8.002 verbatim string** — The BC text doesn't
   pin a single verbatim string; it mandates call-site-context. The
   adversary's "BC mandates 'project is required for JSM request
   creation'" phrasing is looser than the actual BC body. The applied
   fix (call-site-label threading per pass-09 invariant #5) is
   structurally sound, but the BC text could be tightened to mandate
   a specific verbatim substring rather than just call-site-aware
   context. **No follow-up required** — AC-003 pins the substring
   verbatim, which is sufficient.

### Findings REFUTED (fix may be wrong)

**0 findings.** No claim was refuted by external sources or local code.

### Findings INCONCLUSIVE

1 finding:

1. **PASS-03 M-03 / pre-existing `JrError::InsufficientScope` Display
   leak** — `src/error.rs:8-15` Display hardcodes `write:jira-work`
   workaround text and a link to issue #185 (a separate, pre-existing
   issue about granular tokens failing POSTs). On the JSM path, this
   text will be appended to whatever the PR4 map_err prepends. Pass-03
   correctly DEFERRED this as out-of-perimeter; pass-05 acknowledged
   acceptable-workaround status. **VERDICT:** the defer is reasonable
   but the latent UX issue persists post-merge — users hitting a
   JSM scope mismatch will see BOTH the JSM hint AND the irrelevant
   issue #185 legacy workaround block. **Recommendation:** file the
   already-acknowledged follow-up issue ("`InsufficientScope` Display
   refactor — surface scope-specific guidance based on `message`
   content") rather than letting it fade.

### Convergence trustworthiness: **PASS**

The Tier D re-derivation of pass-09 invariant #5 produced an
internally consistent chain from BC + AC + existing-code constraints
to the same conclusion the adversary reached. The fresh-context
discipline is operating as intended: the convergence loop closed
because the invariants are structurally derivable, not because the
adversary memorized prior passes.

**Additional cross-check signals supporting PASS:**

- Pass-09 lists 28 invariants; this audit re-derived #5 plus
  spot-checked invariants #3 (BC-3.8.011 verbatim warnings),
  #9 (top-level IDs), #11 (`isAdfRequest` iff description Some),
  #17 (InsufficientScope + NotAuthenticated both surface
  `write:servicedesk-request`), #19 (`DEFAULT_OAUTH_SCOPES`
  contains the scope), and #27 (`partial_match::ExactMultiple`
  arm) against primary sources — all consistent.
- Pass-04, -05, -07, -08 each independently re-listed cross-axis
  checks with overlapping subsets. The overlap is high (>70% of
  checks appear in 3+ passes) but the derivations are independent
  per the fresh-context discipline; this is a good sign.
- No claim relied on a misattributed external citation (a la the
  JRACLOUD-95368/-94632/-92049/-85546 cluster from issue #361).
  Every external API claim in the adversary trail is sourced from
  Atlassian official docs or KB articles, NOT from secondary
  blogs/threads making unsupported claims.

### Process improvements that should ship as follow-ups (not premise-flaws)

1. **Codify pass-02 INVALIDATED gap** — the `WORKSPACE_PATH` prefix
   discipline is in pass-02-retry but should be in the dispatch
   prompt template permanently.
2. **PG-04 (BC-spec verbatim-phrase grep)** — scan BC quoted strings
   for presence in `src/` to prevent BC-text-drift like the
   "Use"→"Run" verb correction (H-01) that lived undetected until
   pass-01.
3. **PG-01 pass-03 (BC↔AC coverage)** — `scripts/check-spec-counts.sh`
   extension to flag BC anchors without matching AC references would
   catch the BC-3.8.011 ↔ AC-019 gap earlier.
4. **`InsufficientScope` Display refactor** — pre-existing scope leak
   acknowledged by pass-03 M-03 / pass-05 DEFERRED status; should
   be filed as a real GitHub issue rather than a forever-deferred
   item in the adversary trail.

---

## Honest Limitations

1. **PR4 not on develop at audit time.** I could not visually verify
   the applied fixes by reading the PR4-modified files. The audit
   validates premises (the things the fixes were ATTEMPTING to
   address) — not the exact final wording/structure of the patches.
   If the operator believes PR4 #381 IS merged, please re-check the
   PR number or target branch; this audit's conclusions about
   premise accuracy stand regardless.
2. **`require_service_desk` call-site-label parameter** — the Tier D
   re-derivation argues this is the structurally minimal fix, but
   the adversary may have chosen a different mechanism (separate
   helper, error wrapper at the call site, etc.). Either path is
   compatible with the BC + AC; the audit does not pin one of them.
3. **External API claims rely on Atlassian docs + community threads,
   NOT on running calls against a live instance.** Atlassian docs
   could be stale; community threads are observational. This is the
   same source quality as the pre-existing research; consistent
   sourcing is the best signal that the conclusions are stable.
4. **Pass-09's 28 invariants** — only 7 (including #5) were
   re-derived in this audit. The remaining 21 are accepted on the
   strength of the fresh-context discipline + the overlap-across-passes
   signal. A full re-derivation of all 28 would be ~5x the audit
   effort and is not warranted given the no-refuted-findings result
   on the sampled subset.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| WebSearch | 3 | (1) Atlassian OAuth 401 "scope does not match" servicedeskapi response shape; (2) POST /rest/servicedeskapi/request top-level body schema; (3) ADF root doc node required fields |
| WebFetch | 1 | developer.atlassian.com — ADF doc node spec (confirmed `type` + `version` + `content` all required) |
| Read (local) | 11 | src/adf.rs, src/error.rs, src/api/client.rs, src/api/jsm/{mod.rs,servicedesks.rs,requests.rs}, src/api/auth.rs, src/partial_match.rs, .factory/specs/prd/bc-3-issue-write.md, .factory/code-delivery/issue-288-pr4-dispatch/{story.md, adversary-pass-01..09} (10 adversary pass files); 2 pre-existing research files |
| Grep (local) | 11 | Locate handle_jsm_create / JsmRequestBuilder / DEFAULT_OAUTH_SCOPES / write:servicedesk-request / InsufficientScope / require_service_desk / partial_match / test names / request_type symbols across src/ and .factory/ |
| Glob (local) | 6 | Verify presence/absence of tests/issue_create_jsm.rs, src/api/jsm/* layout, worktree directories |
| Perplexity (any variant) | 0 | Not invoked — WebSearch + WebFetch yielded sufficient primary sources matching the pre-existing research |
| Context7 | 0 | Not applicable (no library docs needed; OAuth + REST are protocol/spec concerns) |
| Tavily (any variant) | 0 | Not invoked; coverage from WebSearch was sufficient |
| Training data | 1 area | Reasoning that a `call_site_label` parameter is the minimal-change refactor for `require_service_desk` — flagged as analysis, not citation |

**Total external tool calls:** 4 (3 WebSearch + 1 WebFetch).
**Total local tool calls:** 28 (11 Read + 11 Grep + 6 Glob).

**Training data reliance:** LOW — every external API claim is sourced
to Atlassian developer.atlassian.com, support.atlassian.com, or
community.atlassian.com. Pre-existing research artifacts
(`issue-288-jsm-request-types.md`, `issue-288-oauth-scope-coordination.md`)
were re-confirmed against fresh searches rather than accepted on faith.

### Sources

- [Atlassian — JSM Cloud REST API (api-group-servicedesk)](https://developer.atlassian.com/cloud/jira/service-desk/rest/api-group-servicedesk/)
- [Atlassian — ADF doc node spec](https://developer.atlassian.com/cloud/jira/platform/apis/document/nodes/doc/)
- [Atlassian — ADF structure overview](https://developer.atlassian.com/cloud/jira/platform/apis/document/structure/)
- [Atlassian KB — Set the Request Type when creating an issue using REST API in JSM Cloud](https://support.atlassian.com/jira/kb/set-the-request-type-when-creating-an-issue-using-rest-api-in-jsm-cloud/)
- [Atlassian KB — oAuth app throwing error "Unauthorized; scope does not match"](https://support.atlassian.com/jira/kb/oauth-app-throwing-error-unauthorized-scope-does-not-match/)
- [Community thread 3144480 — 401 scope does not match with servicedeskapi](https://community.atlassian.com/forums/Jira-Service-Management/Getting-quot-401-scope-does-not-match-quot-with-servicedeskapi/qaq-p/3144480)
- [Community thread 81389 — How to solve "Unauthorized; scope does not match"](https://community.developer.atlassian.com/t/how-to-solve-unauthorized-scope-does-not-match/81389)
- [Community thread 1839863 — How to find serviceDeskId and requestTypeId](https://community.atlassian.com/forums/Jira-Service-Management/How-to-find-serviceDeskId-and-requestTypeId/qaq-p/1839863)
- Local: `src/adf.rs`, `src/error.rs`, `src/api/client.rs`, `src/api/jsm/{mod.rs,servicedesks.rs,requests.rs}`, `src/api/auth.rs`, `src/partial_match.rs`
- Local: `.factory/specs/prd/bc-3-issue-write.md`
- Local: `.factory/code-delivery/issue-288-pr4-dispatch/{story.md, adversary-pass-01..09}`
- Local: `.factory/research/issue-288-jsm-request-types.md` (2026-05-09)
- Local: `.factory/research/issue-288-oauth-scope-coordination.md` (2026-05-18)
