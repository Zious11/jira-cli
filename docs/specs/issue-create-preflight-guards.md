# `jr issue create` pre-flight exit-64 guards for `--field` / `--on-behalf-of`

**Issue:** [#639](https://github.com/Zious11/jira-cli/issues/639)
**Story:** S-639-1 (`.factory/stories/S-639-1.md`)
**Decision:** DEC-188
**Supersedes:** S-383's warn-and-proceed contract (BC-3.8.012/013 pre-amendment)
**Related ADR:** [ADR-0014](../adr/0014-jsm-request-type-dispatch.md) (JSM dispatch fork — architecture
unchanged; only its "byte-for-byte unchanged" claims are amended by this story)

> **⚠️ PARTIALLY SUPERSEDED — DEC-310 (S-578-4, #578, registered 2026-08-26) REVERSED the
> `--field`-alone half of this guard.** `jr issue create --field NAME=VALUE` on the platform
> (non-JSM) path **no longer exits 64 pre-flight**. It now resolves each `--field` pair against
> the target project/issue-type's Create screen (`createmeta`, BC-3.3.010/BC-3.3.011) using the
> same resolution machinery as `issue edit --field`, instead of being rejected outright. The
> `--on-behalf-of` guard (BC-3.8.013) below is **UNCHANGED** — it still exits 64 pre-flight
> without `--request-type`, exactly as this document describes. The combined-guard row in the
> behavior table below ("Both present, `--request-type` absent") is also superseded: only
> `--on-behalf-of` now triggers the exit-64 path in that case; `--field` no longer contributes to
> it. Everything else in this document (guard placement/ordering, the `--on-behalf-of` single-flag
> error string, the "why not `#[arg(requires = ...)]`" rationale, zero-HTTP-guarantee mechanics)
> remains historically accurate for `--on-behalf-of` and for understanding the DEC-188 baseline
> DEC-310 reversed. See CLAUDE.md's `jr issue create --field NAME[:kind]=VALUE` gotcha entry for
> the current DEC-310 behavior, and `docs/adr/0014-jsm-request-type-dispatch.md` for the amended
> ADR-0014 notes. Treat the `--field` content in this file as **historical**, describing the
> now-superseded DEC-188 guard shape — not current behavior.

## Problem

`--field NAME=VALUE` and `--on-behalf-of <accountId>` are self-declared JSM-only flags on
`jr issue create` — their names, and their sole effect (merging into `requestFieldValues` /
`raiseOnBehalfOf` on the JSM request body), only make sense with `--request-type`. Before this
story (S-383), supplying either flag on the platform path (i.e. without `--request-type`) emitted
a stderr warning and then proceeded to create the platform issue anyway (exit 0), silently
dropping the flag's intent.

This is misleading: a caller who supplies `--field`/`--on-behalf-of` without `--request-type`
has made a categorical error, not an ambiguous choice — there's no such thing as a "field" or an
"on-behalf-of" issue creation on the platform API. Waiting for a warning that scrolls past on a
successful command is a bad failure mode for something that is never intentional.

**Asymmetry with the warn-only group:** `--team`/`--points` (BC-3.8.011) remain warn-only on the
JSM path — those are general platform flags that the JSM API happens not to support, and the
flag choice there is genuinely ambiguous (a user might reasonably always pass `--team X`
regardless of project type via a shell alias). Do NOT apply this story's exit-64 pattern to that
group; the two categories are deliberately different.

## Design

### Behavior

`jr issue create` now performs a pre-flight guard on the platform path (i.e. after the JSM
dispatch fork has determined `--request-type` was NOT supplied):

> **DEC-310 SUPERSEDES the `--field` row below.** As of S-578-4 (#578, DEC-310), `--field`
> present + `--request-type` absent no longer exits 64 — it resolves via `createmeta`
> (BC-3.3.010/BC-3.3.011). The `--on-behalf-of` row and the "Neither present" row are still
> current. The "Both present" row is superseded too: only `--on-behalf-of` triggers exit 64 in
> that case now, and it is BC-3.8.013's single-flag error, not the combined error described
> below. See the top-of-file note for the full picture.

| Invocation | Behavior |
|---|---|
| `--field` present, `--request-type` absent | **[SUPERSEDED by DEC-310]** Historical DEC-188 behavior: exit 64, BC-3.8.012 single-flag error. Current: resolves via `createmeta`, no guard fires. |
| `--on-behalf-of` present, `--request-type` absent | Exit 64, BC-3.8.013 single-flag error (unchanged) |
| Both present, `--request-type` absent | **[SUPERSEDED by DEC-310]** Historical DEC-188 behavior: exit 64, ONE combined error. Current: only `--on-behalf-of` fires, as BC-3.8.013's single-flag error. |
| Either/both present, `--request-type` present | No guard fires — routes to the JSM path (ADR-0014) |
| Neither present | No guard fires — platform path proceeds unaffected |

The guard is presence-only: `!field_pairs.is_empty()` (regardless of how many `--field` values
are supplied, or whether any individual pair is malformed/empty) and `on_behalf_of.is_some()`
(an empty string `--on-behalf-of ""` is still `Some("")`, so it still trips the guard).

### Verbatim error strings (byte-for-byte, `error-taxonomy.md` §6)

**`--field` alone:**
```
--field is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to submit a JSM request with custom fields, or drop --field to create a standard platform issue.
```

**`--on-behalf-of` alone:**
```
--on-behalf-of is only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to raise a request on behalf of another user, or drop --on-behalf-of to create a standard platform issue.
```

**Both flags (combined):**
```
--field and --on-behalf-of are only valid with --request-type (JSM service-desk requests). Add --request-type <NAME> to use these flags, or drop them to create a standard platform issue.
```

All three are `JrError::UserError` (exit code 64). In `--output json` mode, the error surfaces
through `src/main.rs::main`'s standard `{"error": "…", "code": 64}` envelope on stderr, with an
empty stdout — the same channel-separation contract every other pre-flight `UserError` follows
(JSON render invariant, #526).

### Guard placement (step ordering, BC-3.8.012 "Platform-Path Guard Ordering SSOT")

The guard lives in `src/cli/issue/create.rs::handle_create`, immediately after the JSM dispatch
fork and before everything else. Step numbers below match the BC-3.8.012 "Platform-Path Guard
Ordering — `handle_create`" SSOT block verbatim (`.factory/specs/prd/bc-3-issue-write.md`;
argument destructuring precedes step 1 and is not itself guard-relevant, so it is not numbered):

```
1. JSM dispatch fork — if request_type.is_some() { return handle_jsm_create(...) }   [ADR-0014, unchanged]
2. Pre-flight guard (THIS STORY):
     a. if !field_pairs.is_empty() && on_behalf_of.is_some() → combined UserError, return
     b. if !field_pairs.is_empty()                            → BC-3.8.012 UserError, return
     c. if on_behalf_of.is_some()                              → BC-3.8.013 UserError, return
3. Project-key resolution (interactive prompt possible here)
4. Interactive prompts — issue-type resolution, summary resolution (interactive prompt possible here)
4a. --description-stdin blocking read (spawn_blocking)
5. Helper HTTP — field-by-field body assembly (--team / --points / --to trigger helper HTTP here)
6. Platform POST — POST /rest/api/3/issue
```

Steps 3–6 are all suppressed by an early guard return at step 2 — this is the zero-HTTP
guarantee: no HTTP call of any kind (not `GET /rest/api/3/myself`, not the team-org GraphQL
lookup, not `GET /rest/api/3/field` for CMDB discovery, nothing) happens when the guard fires,
even when other flags (`--team`, `--to`) would normally trigger pre-POST helper HTTP.

**Why step 2 sits where it does:**
- **After the JSM fork (step 1):** so `--field a=b --request-type ""` still routes to
  `handle_jsm_create` (which then fires its own BC-3.8.016 "request type cannot be empty" guard)
  rather than mis-firing BC-3.8.012. `request_type.is_some()` is true for `Some("")`, so the
  dispatch fork's routing decision is unaffected by string emptiness.
- **Before project-key resolution (step 3):** so a projectless invocation
  (`jr issue create --field a=b`) reports the BC-3.8.012 guard error, not "Project key is
  required" — the caller's actual mistake (a stray JSM-only flag) is surfaced first.
- **Before interactive prompts (steps 3–4) and the blocking stdin read (step 4a):** so the guard
  fires deterministically in TTY mode and never blocks on stdin waiting for description content
  that will never be used.
- **Before all HTTP (steps 3, 5, 6):** the zero-HTTP guarantee above.

### Why not `#[arg(requires = "request_type")]`

Clap's `requires` attribute is tempting but wrong here: a clap-level `requires` violation exits
**2** (clap parse error), not the **64** (`JrError::UserError`) that BC-3.8.012/013 require. The
two exit codes carry different meaning in this CLI's error taxonomy — 2 signals a malformed
invocation the shell/parser itself rejected, 64 signals a semantically invalid but
parser-valid combination this program's own domain logic rejects. `#[arg(requires = …)]` was
deliberately not used.

### Guard implementation pattern

Modeled on the mutual-exclusion block in `src/cli/issue/edit.rs::handle_edit` (the `--field` +
`--label` guard, issue #396) — same "check combined first, then singles, return early" shape.
The guard logic itself (`!field_pairs.is_empty()` / `on_behalf_of.is_some()`) is pure — no I/O,
no side effects — but is embedded directly in the effectful `handle_create` shell rather than
extracted to a standalone function; a 3-branch if/else chain does not benefit from extraction.

## Interaction with the JSM dispatch fork (ADR-0014)

This story does **not** change ADR-0014's architecture: the dispatch fork
(`if request_type.is_some() { return handle_jsm_create(...) }`) is still the sole, unconditional
routing decision, and it still fires before anything else in `handle_create`. What changes is
what happens in the `else` branch (i.e. the code that runs when the fork does NOT return early):
that branch used to warn-and-proceed for `--field`/`--on-behalf-of`, and now exits 64 pre-flight
instead. ADR-0014's "byte-for-byte unchanged" claims about the platform path are amended (not
retracted) to note this conditionality — see the amendment notes added directly in ADR-0014 at
each site making that claim.

## Testing

All coverage lives in `tests/issue_create_jsm.rs` (co-located with the JSM-path tests by
explicit precedent set in the S-383 story and carried forward here — these are platform-path
tests, no `--request-type` flag, but they exercise the inverse symmetry of the BC-3.8.011
forward-direction JSM-path warnings already in that file).

21 acceptance criteria (5 inverted from S-383's exit-0 assertions, 2 vacuity→non-vacuity
transitions, 14 new):

- **AC-1/AC-10** — `--field` alone, human mode / `--output json` mode (symmetric twins)
- **AC-2** — `--on-behalf-of` alone, `--output json` mode
- **AC-3/AC-13** — combined guard fires once, not two singles (including the `--on-behalf-of ""`
  edge case where the empty string is still `is_some()`)
- **AC-4** — clean invocation (neither flag) stays exit 0 — the breaking-change regression pin
- **AC-5** — idempotency: `--field a=b` vs `--field a=b --field c=d` produce byte-identical stderr
- **AC-6/AC-20/AC-21** — JSM-path non-mis-fire (guard does not fire when `--request-type` is present,
  for `--field` alone, `--on-behalf-of` alone, and both together)
- **AC-7/AC-19** — malformed (`--field bareflagnoequals`) and empty-value (`--field a=`) pairs
  still trip the guard (presence-only, not value-inspecting)
- **AC-8** — zero-HTTP guarantee even with `--team`/`--to` present (isolated `MockServer`,
  `received_requests().is_empty()` normative proof)
- **AC-9/AC-17** — guard fires before project-key resolution, including under `--markdown`
- **AC-11** — guard fires before any interactive prompt (`JR_STDIN_IS_TTY=1` seam)
- **AC-12** — `--help` documents the `--request-type` requirement on both flags
- **AC-14** — `--request-type ""` routes to the JSM fork (and its own empty-RT guard), not this one
- **AC-15** — a clap-level `conflicts_with` violation exits 2, not 64 (guard never reached)
- **AC-16** — `--on-behalf-of ""` alone still fires BC-3.8.013
- **AC-18** — guard fires before the `--description-stdin` blocking read

## Holdout coverage

`holdout-scenarios.md` Group 20, H-NEW-PREFLIGHT-001 through -006 (all MUST-PASS):
`--field` alone, `--on-behalf-of` alone, both together, neither (regression pin), JSM
non-mis-fire, and `--output json` envelope shape.

## Out of scope

- **`--team`/`--points` and other JSM-warn-only flags (BC-3.8.011):** unaffected by this story;
  remain warn-and-proceed. See "Asymmetry with the warn-only group" above.
- **Bulk `jr issue create`:** `jr issue create` has no bulk form; not applicable.
- **E2E test changes:** discharged at F2 (F64-001) — `tests/e2e_live.rs` was scanned and contains
  zero `issue create --field`/`--on-behalf-of` invocations on the platform path.
