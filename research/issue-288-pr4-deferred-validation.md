---
document_type: research
research_type: validation
research_id: issue-288-pr4-deferred-validation
title: "S-288-pr4-dispatch deferred-finding validation (M-03, O-01, O-08-01..07)"
date: 2026-05-19
producer: research-agent
status: complete
related_pr: 381
related_issue: 288
---

# Validation: S-288-pr4-dispatch deferred adversarial findings

## Pre-validation note on the working tree

**IMPORTANT — the on-disk `develop` checkout at `/Users/zious/Documents/GITHUB/jira-cli`
does NOT contain the PR #381 code.** The most recent commit visible in the git status
snapshot is `d909e65` (2026-05-16); STATE.md records PR #381 merging at
`95232555` on 2026-05-19. The working tree appears not to have been fast-forwarded
since the merge. Symptoms confirming this:

- `src/cli/issue/create.rs` contains no `handle_jsm_create`, no `JsmCreateArgs`,
  no `parse_field_kv`, no destructure of `request_type` / `field` / `on_behalf_of`.
- `tests/issue_create_jsm.rs` does not exist.
- `src/api/jsm/requests.rs` contains only `create_jsm_request` (pr1 surface);
  no `JsmRequestBuilder::build`.
- `grep` for `--request-type`, `--on-behalf-of`, `handle_jsm_create`,
  `JsmRequestBuilder` returns 0 hits across the entire `src/` tree.
- `src/cli/mod.rs` `IssueCommand::Create` variant lacks the three new fields.

**Validation strategy adjustment.** Where a finding concerns shared code that is
present on `develop` (e.g., `src/error.rs::InsufficientScope`, `parse_error` in
`src/api/client.rs`, `partial_match`, `require_service_desk`), I read it directly
and cite line numbers. Where a finding concerns code that only exists in the
unpulled PR #381 changeset (e.g., `handle_jsm_create` map_err logic,
`JsmRequestBuilder::build` body shape, `--field` `clap` `help` text), I treat the
adversary's documented findings as the most authoritative source available
locally and explicitly flag each such finding as **INCONCLUSIVE-LOCAL** (the
shared-code premise is verified; the PR-specific code I cannot read first-hand).
The user should run `git pull --ff-only origin develop` before filing, then
re-verify the INCONCLUSIVE-LOCAL items directly against the merged code.

---

## FINDING 1 — M-03: stale `write:jira-work` text in `InsufficientScope` Display

**Status:** CONFIRMED (shared-code claim verifiable on `develop`).

**Evidence (local):**

`src/error.rs:8-15` — the `Display` impl for `JrError::InsufficientScope`:

```
#[error(
    "Insufficient token scope: {message}\n\n\
     The Atlassian API gateway rejects granular-scoped personal tokens on POST \
     requests (while PUT/GET succeed). Workarounds:\n  \
     • Use a classic token with \"write:jira-work\" scope instead of granular scopes, or\n  \
     • Try OAuth 2.0 (run \"jr auth login --oauth\") — may avoid this bug, not verified\n\n\
     See https://github.com/Zious11/jira-cli/issues/185 for details."
)]
InsufficientScope { message: String },
```

`src/error.rs:170-186` — test `insufficient_scope_display_includes_workarounds`
asserts the Display output literally contains `"write:jira-work"`, `"OAuth 2.0"`,
and `"github.com/Zious11/jira-cli/issues/185"`. So the string is BOTH present
AND pinned by an existing test. Any refactor that removes the legacy strings
must also update this test.

`src/api/client.rs:949-975` (`parse_error`) and `:680-704` (the inline 401
scope-mismatch branch in `send_inner`) BOTH construct `InsufficientScope { message }`
when a 401 body contains `"scope does not match"` (case-insensitive). Neither
site prepends any caller context — they pass the raw gateway message and let the
Display impl format the workaround block.

`src/cli/issue/create.rs` on `develop` does NOT yet contain the `handle_jsm_create`
map_err that the adversary describes. So on `develop` today, every
`InsufficientScope` reaching stderr emits the legacy `write:jira-work` workaround
block verbatim — irrespective of which endpoint produced the 401.

**Evidence (external):** None needed — claim is purely about shared in-repo code.

**Verdict:** The Display impl unconditionally emits `write:jira-work` and the
GitHub issue #185 citation, both of which are irrelevant on the JSM path. Even
if `handle_jsm_create` prepends a correct JSM hint via `map_err` (which the
adversary documents but I cannot read on disk), the trailing Display text still
mentions a Jira-platform-only scope and links to an unrelated bug. The
adversary's pass-03 framing — "JSM users see both the correct JSM hint AND
irrelevant legacy advice" — is accurate.

**Issue text impact:** No edits needed to the planned issue body. The framing
"stale `write:jira-work` legacy text surfaces on JSM path" is accurate.

Recommended additions to the issue body that the original adversary write-up
omits:

1. The text is **pinned by an existing assertion** (`insufficient_scope_display_includes_workarounds`).
   Any refactor must update that test. State this explicitly so the reviewer
   doesn't accidentally break the pin.
2. There are **two construction sites** in `src/api/client.rs` (`send_inner` 401
   branch, line ~700; `parse_error` 401 branch, line ~969). Both pass the raw
   message verbatim. Any redesign that takes per-call-site context (e.g., "JSM
   path / platform path / generic") needs to thread the context through both
   sites or convert one into the other.
3. Consider whether the right fix is (a) make `InsufficientScope` parameterized
   by a hint kind enum (`Generic`, `Jsm`, `Platform`), (b) drop the inline
   workaround block and have callers add hints contextually, or (c) leave Display
   minimal and prepend hints at every map_err site. The choice has knock-on
   effects for all 401-producing endpoints, not just JSM.

---

## FINDING 2 — O-01: platform path silently drops `--field` and `--on-behalf-of`

**Status:** INCONCLUSIVE-LOCAL (PR #381 code not on disk to first-hand verify;
adversary documentation supports the claim).

**Evidence (local):** Cannot verify directly. `src/cli/issue/create.rs` on the
current working tree does not contain `request_type`, `field`, or `on_behalf_of`
fields in the destructure — they are introduced by PR #381. I cannot grep the
PR-#381-merged `handle_create` to confirm "no warning fires when `field` or
`on_behalf_of` is set without `request_type`".

**Evidence (external):** Adversary pass-05 finding O-01 explicitly states the
inverse silent-drop is present:

> "Platform path silently drops `--field` and `--on-behalf-of` (inverse of
> BC-3.8.011 which mandates platform-only-flag warnings on JSM path). Clap help
> text says 'JSM requests only' but no runtime feedback."

Adversary pass-09 invariant #3 ("All 5 platform-only flag warnings fire
pre-dispatch with verbatim BC-3.8.011 strings") confirms warnings are emitted
on the JSM path for `--team`, `--points`, `--parent`, `--to`, `--account-id` —
i.e., the symmetric warning policy applies in one direction (platform-only
flags on JSM path) but the inverse (JSM-only flags on platform path) is
intentionally absent per BC-3.8.011's directional wording. BC-3.8.011 in
`bc-3-issue-write.md:699-725` does not mandate the inverse direction.

**Verdict:** Likely CONFIRMED, but I cannot read the actual `handle_create`
guard logic. The adversary explicitly flagged this as "pending intent — author
may have intentionally relied on clap help text" and routed it to S-7.01
human-intent adjudication. The premise (asymmetric warning) is well-grounded
in the BC table.

**Issue text impact:** Add an "intent question" framing to the issue body — the
author may have intentionally chosen clap-help-text-only feedback for the
inverse path because `--field` and `--on-behalf-of` were named in the help text
as "JSM requests only". If the user files this as a behavior change, suggest:

- (a) match BC-3.8.011 symmetry with a stderr warning when `--field` /
  `--on-behalf-of` are set without `--request-type` and the project is non-JSM, OR
- (b) tighten clap with `requires("request_type")` on both flags so the user
  gets a clap-level error rather than silent drop.

Option (b) is the cleaner UX but is a breaking change if any user scripts pass
the flags speculatively. Option (a) is non-breaking and mirrors the JSM-direction
BC. Either way, the issue body should ask the original PR author which intent
applies, rather than asserting "this is a bug" outright.

**Pre-file action recommended:** `git pull --ff-only origin develop` then
grep `src/cli/issue/create.rs` for `field.is_empty()` and `on_behalf_of.is_some()`
guards in the platform-path branch (before the dispatch fork). If no such guards
exist, finding is CONFIRMED. If guards exist (e.g., emit a stderr warning), the
finding is REFUTED.

---

## FINDING 3 — O-08-01..07 BUNDLE

### O-08-01: Basic-auth API token expiry triggers misleading JSM-scope hint

**Status:** PARTIAL (the upstream mechanism is verifiable on `develop`; the
JSM-path-specific phrasing requires PR #381 code).

**Evidence (local):**

`src/api/client.rs:711-722` — the `send_inner` 401 path explicitly branches on
auth scheme:

```rust
// Guard: only fire auto-refresh for OAuth (Bearer) auth. Basic auth
// clients use API tokens, not OAuth refresh tokens — there is nothing
// to refresh. Returning NotAuthenticated directly is correct for Basic
// auth 401s.
if !self.auth_header.starts_with("Bearer ") {
    return Err(JrError::NotAuthenticated {
        hint: "Run \"jr auth login\" to connect.".to_string(),
    }
    .into());
}
```

So Basic-auth 401s produce `NotAuthenticated { hint: "Run jr auth login to
connect." }`, NOT `InsufficientScope`. The `parse_error` path
(`src/api/client.rs:964-975`) also returns `NotAuthenticated` on any 401 whose
body does NOT contain "scope does not match".

Per adversary pass-09 invariant #17 ("OAuth `InsufficientScope` 401 AND Basic
`NotAuthenticated` 401 both surface `write:servicedesk-request` hint"), the
PR #381 `handle_jsm_create` map_err appears to map BOTH error variants to a
hint containing `write:servicedesk-request`. This is the source of O-08-01: a
Basic-auth user whose API token has simply expired (NOT a scope problem,
because API tokens have implicit full-permission scope) will see the hint
"missing `write:servicedesk-request` OAuth scope" — which is misleading because
they aren't using OAuth.

**Evidence (external):** Atlassian's official docs do not document a distinct
401 body shape for expired Basic-auth API tokens vs OAuth tokens. Community
threads confirm both surfaces return generic 401 with `{"errorMessages": [...]}`
shape; only the OAuth-scope-mismatch path uses the gateway-specific "scope does
not match" string per `src/api/client.rs:696-704`. Per CLAUDE.md note about
Atlassian's expired-access-token 401 response shape: "there is NO machine-readable
`code` field and NO RFC-6750-compliant `WWW-Authenticate: Bearer
error=\"invalid_token\"` header." So the client cannot tell scope-mismatch from
generic-expiry from generic-permission-denied on the wire for Basic auth.

**Verdict:** The shared-code premise is correct: Basic-auth 401s land in
`NotAuthenticated` (verifiable). Whether `handle_jsm_create` then map_err's that
to a JSM-scope hint is what the adversary describes but I cannot verify on disk.
Per adversary pass-09 invariant #17, it does — so the bug is real, but
preventing it requires distinguishing Basic vs OAuth at the map_err call-site.
`JiraClient` already knows the auth scheme (`self.auth_header.starts_with("Bearer ")`);
the fix is to expose `is_oauth()` (or similar) on `JiraClient` and gate the
JSM-scope hint on it.

**Issue text impact:** Refine the issue body to specify the fix surface:
"Expose `JiraClient::is_oauth_auth() -> bool` (or similar predicate) and gate
the `handle_jsm_create` map_err's `write:servicedesk-request` hint on
`is_oauth_auth() == true`. For Basic-auth `NotAuthenticated` 401s, surface a
hint specific to API-token expiry ('Generate a new API token at
https://id.atlassian.com/manage-profile/security/api-tokens') instead." This is
actionable; the original O-08-01 wording is symptom-only.

### O-08-02: JSM-path "project is required" terser than platform-path equivalent

**Status:** INCONCLUSIVE-LOCAL.

**Evidence (local):** Cannot verify the JSM-path error string verbatim — it is
in PR #381 `handle_jsm_create`. The platform-path equivalent in
`src/cli/issue/create.rs:62-68` is:

```
"Project key is required. Use --project or configure .jr.toml. \
 Run \"jr project list\" to see available projects."
```

**Evidence (external):** Adversary pass-08 finding O-08-02 states the JSM-path
hint is "terser" than the platform-path one above. The pass does not quote the
JSM-path text verbatim, so I cannot compute the exact delta.

**Verdict:** Likely a real UX delta but I cannot quantify the gap. The platform
hint above is verbose (24 words, mentions both flag and config file, suggests
discovery command). If the JSM-path hint is e.g. just "Project key required for
JSM dispatch.", that is materially terser. But without the verbatim JSM string
I cannot confirm.

**Issue text impact:** Pre-file action — once `git pull` completes, grep
`src/cli/issue/create.rs` for the JSM-specific "project" error string (likely
inside `handle_jsm_create` early validation). Quote both verbatim in the issue
body so the maintainer can see the delta at a glance. Suggest harmonizing on
the platform-path verbose form (or factoring out a shared helper that takes a
context label).

### O-08-03 [process-gap]: AC-013 declares `HashMap<String, serde_json::Value>`; impl uses `HashMap<String, String>`

**Status:** PARTIAL — AC-013 wording confirmed, impl type inferred from
adversary report; finding correctly framed but is documentation-only.

**Evidence (local):**

Story `.factory/code-delivery/issue-288-pr4-dispatch/story.md` line 220 (AC-013):

> `parse_field_kv(args: &[String]) -> Result<HashMap<String, serde_json::Value>, JrError>`
> is a standalone, unit-testable pure function...

BC-3.8.008 in `.factory/specs/prd/bc-3-issue-write.md:657` documents the field
behavior:

> "Each pair inserted into `requestFieldValues`; merged with other field sources."

BC-3.8.008 does NOT specify the in-memory type — it specifies the wire
behavior. So strictly speaking, AC-013 is at odds with BC-3.8.008 only if
the impl signature differs from the declared AC. Adversary pass-08 O-08-03
states the impl actually returns `HashMap<String, String>` (not `Value`).
I cannot read the PR-#381 impl on disk to confirm; the adversary's reading
during pass-08 is the source.

**Verdict:** PARTIAL. The story-doc declaration is verifiable; the impl type
is taken on adversary's word. If the impl is indeed `HashMap<String, String>`,
the AC text is stale and should be updated. **This is documentation drift,
not a bug** — the simpler `String` type is BC-compliant because BC-3.8.008 only
constrains the wire shape (all `--field` values are strings on the CLI input
side; serialization to JSON happens later in `JsmRequestBuilder::build`).

**Issue text impact:** Reframe the finding from "spec mismatch" to "story-doc
AC text drift, cosmetic". Suggest a one-line update to AC-013 to change
`HashMap<String, serde_json::Value>` → `HashMap<String, String>` if the impl
is indeed the simpler type. No code change needed; no behavior change. Tag the
issue as `docs` not `bug`. Lower priority than the original adversary triage.

### O-08-04: `--request-type ""` empty-string degrades to "Ambiguous matches all"

**Status:** CONFIRMED (verified against `partial_match` semantics on `develop`).

**Evidence (local):**

`src/partial_match.rs:16-43` — the algorithm:

1. Look for exact (case-insensitive) matches.
2. If none, fall back to substring match: `candidates.iter().filter(|c|
   c.to_lowercase().contains(&lower_input))`.
3. With `lower_input == ""`, `"<anything>".contains("")` is `true` for EVERY
   candidate. So every candidate is added to `matches`.
4. With `matches.len() >= 1`, returns `MatchResult::Ambiguous(matches)`.

So passing `""` to `partial_match` against any non-empty candidate list returns
`Ambiguous(all candidates)`. The caller in `handle_jsm_create` (per adversary
pass-08) maps `Ambiguous` to exit 64 with the candidate list, no POST issued.

**Evidence (external):** None needed.

**Verdict:** CONFIRMED. The user passes `--request-type ""`, the resolver
returns an "Ambiguous" arm with EVERY request type listed as a candidate, the
error message reads as if the user provided an over-broad query that hit
several types. The actual cause (empty string) is not surfaced.

**Issue text impact:** No major edits needed; the original adversary framing
("mild UX paper-cut") is accurate. Suggest the fix: add an explicit
`if name.trim().is_empty()` guard in `handle_jsm_create` BEFORE calling
`partial_match`, with a dedicated message like `--request-type cannot be empty
(got ""). Pass a name or numeric ID.` Exit code stays 64.

Alternative: extend `partial_match` itself with an empty-input guard at the top
that returns `MatchResult::None(candidates)` or a new `MatchResult::Empty`
variant. This is the more invasive fix but benefits every caller of
`partial_match` (e.g., the same bug almost certainly affects `--type ""`,
`--priority ""`, etc. on the platform path). Lean toward the localized fix in
`handle_jsm_create` unless the user wants to do an audit pass.

### O-08-05: `require_service_desk` 401 cannot surface JSM scope hint

**Status:** CONFIRMED (shared-code claim verifiable on `develop`).

**Evidence (local):**

`src/api/jsm/servicedesks.rs:102-127` (`require_service_desk`) — the function
calls `get_or_fetch_project_meta`, which in turn calls
`client.get("/rest/api/3/project/{key}")` (platform-API endpoint, not
servicedeskapi). On a 401 here, the response is mapped by
`src/api/client.rs::parse_error` to `NotAuthenticated { hint: "Run jr auth login
to connect." }` — NO JSM-specific hint. The function has no `map_err` wrapping.

The JSM-scope-specific hint (`write:servicedesk-request`) is only added by
`handle_jsm_create`'s map_err around the `create_jsm_request` call (per
adversary pass-09 invariant #17). Calls earlier in the dispatch chain
(`require_service_desk`, `list_request_types`) flow through their own paths
without the hint.

**Evidence (external):** None needed.

**Verdict:** CONFIRMED. If `require_service_desk`'s underlying
`/rest/api/3/project/{key}` GET returns 401 (e.g., the token has zero Jira-platform
read scope, perhaps because the user logged in with only JSM scopes), the user
sees "Run jr auth login to connect" — not "missing `write:servicedesk-request`
or `read:servicedesk` scope".

**Issue text impact:** Refine the issue body — the OAuth scope this surface
actually needs is `read:jira-work` (to call `/rest/api/3/project/{key}`) plus
the implicit `read:servicedesk-request` (to list service desks), not
`write:servicedesk-request`. The fix is to either (a) wrap `require_service_desk`
calls inside `handle_jsm_create`'s map_err with a scope-aware hint, or (b)
extend `JrError::InsufficientScope` to carry a scope-list field and have the
caller specify which scopes the failed endpoint needs. The localized (a) fix
is simpler.

NOTE — the adversary's pass-08 framing "Partial-credit hint already exists for
write-scope-only users" is misleading. The hint only fires for the final POST
to /servicedeskapi/request, which is reached only after `require_service_desk`
succeeds. A user with NO platform-read scope would never reach the
`create_jsm_request` call. So "partial credit" is overstating it — the hint
covers the write side only.

### O-08-06: `--field description=plain` + `--markdown` desyncs `isAdfRequest: true`

**Status:** PARTIAL (the desync claim is structurally verifiable; the "Atlassian
would 400" claim is unverified per Atlassian docs).

**Evidence (local):** Cannot read `JsmRequestBuilder::build` on disk
(in PR #381). Per the BC-3.8.006/008 documented shapes:

- `--description "X" --markdown` → body has
  `isAdfRequest: true` + `requestFieldValues.description = <ADF root object>`
- `--field description=plain --markdown` → if `--field` is processed AFTER
  `--description` ADF construction, the string `"plain"` overwrites the ADF
  object at `requestFieldValues.description`. `isAdfRequest: true` remains.

This is the desync the adversary describes.

**Evidence (external):**

WebFetch of Atlassian's official servicedeskapi docs
(`https://developer.atlassian.com/cloud/jira/service-desk/rest/api-group-request/#api-rest-servicedeskapi-request-post`):

> The documentation shows `requestFieldValues` as a map but does not explicitly
> specify different formats based on `isAdfRequest`... **The documentation does
> not specify** what status code returns if `isAdfRequest` is true but
> description is plain text instead of ADF.

A community thread title from the same search ("Unable to Create a Customer
Request via Service Desk REST API") suggests 400-class errors are common but
the specific shape mismatch isn't documented. Without sandbox verification,
the "Atlassian would 400" claim is plausible but not confirmed.

**Verdict:** PARTIAL. The desync is real and contradicts BC-3.8.006's intent
("description → ADF; isAdfRequest: true"). The user-impact severity depends on
Atlassian's tolerance — if Atlassian accepts a plain string description with
`isAdfRequest: true` (silently coerces, or ignores `isAdfRequest`), the only
visible symptom is the user's description not being rendered as ADF. If
Atlassian rejects with 400, the user sees a confusing error.

**Issue text impact:** Reframe as "deliberate-misuse latent bug" with the
fix-suggestion:

1. **Defensive**: when `--field description=...` is set AND `--markdown` is set,
   emit a stderr warning "`--field description=...` overrides `--description`
   parsing; `--markdown` is ignored for the field-form description". Set
   `isAdfRequest: false` if no other ADF-bearing field is present.
2. **Strict**: reject the combination at the CLI level with exit 64,
   "Cannot combine `--field description=...` with `--description`/`--markdown`.
   Use one form or the other."

Lean toward (1) — non-breaking, surfaces the issue, lets the user proceed if
they know what they're doing. Caveat: drop the "Atlassian would 400" claim
from the issue body unless someone runs a sandbox test to verify. Frame as
"may result in a JSM API error or silently dropped formatting" instead.

### O-08-07: `--type` warning fires pre-dispatch even when project is non-JSM

**Status:** INCONCLUSIVE-LOCAL (PR #381 code required).

**Evidence (local):** Cannot read `handle_create` / `handle_jsm_create` on disk
to confirm the order of:

1. `--type` + `--request-type` warning emission (per BC-3.8.010)
2. `require_service_desk` non-JSM-project rejection (per BC-3.8.002 — exits 64)

**Evidence (external):** Adversary pass-09 invariant #3 says "All 5 platform-only
flag warnings fire pre-dispatch with verbatim BC-3.8.011 strings". Pre-dispatch
is presumably "before the POST is issued" but it's ambiguous whether that means
"before `require_service_desk` is called" or "after `require_service_desk` but
before POST".

If warnings fire BEFORE `require_service_desk`, then on a non-JSM project, the
user sees both a warning (`--type ignored when --request-type set`) AND an error
(`project is not a JSM service desk`). The warning is misleading because the
command was never going to dispatch JSM anyway.

If warnings fire AFTER `require_service_desk` succeeds, this finding is
REFUTED.

**Verdict:** Inconclusive without reading the merged code. BC-3.8.010 explicitly
says the warning "need not fire" on early-exit paths, so the implementation is
BC-compliant either way; this is a UX-polish observation.

**Issue text impact:** Pre-file action — once `git pull` completes, read
`handle_create` and locate the relative position of (a) `--type` warning
emission, (b) `require_service_desk` call. If (a) is before (b), CONFIRM and
suggest moving (a) into `handle_jsm_create` AFTER `require_service_desk` so the
warning only fires on confirmed JSM projects. If (a) is after (b), REFUTE and
do not file.

---

## Summary

### SAFE to file as issues (CONFIRMED or PARTIAL with revised framing)

| Finding | Status | Filing recommendation |
|---------|--------|----------------------|
| **M-03** — stale `write:jira-work` text | **CONFIRMED** | File as-is. Add note about the existing `insufficient_scope_display_includes_workarounds` test pin and the two construction sites in `client.rs`. Suggest the parameterized-by-context-enum refactor. |
| **O-08-01** — Basic-auth → JSM scope hint mismatch | **PARTIAL** | File with refined fix: expose `JiraClient::is_oauth_auth()` predicate and gate the JSM scope hint on it. Surface a Basic-auth-specific hint ("regenerate API token at id.atlassian.com") for the Basic case. |
| **O-08-04** — `--request-type ""` empty-string ambiguity | **CONFIRMED** | File as-is. Suggest the localized empty-input guard in `handle_jsm_create` before `partial_match` is called. Note that the same bug almost certainly exists on every `partial_match` call site (`--type ""`, `--priority ""`, etc.) — flag as a follow-up audit candidate but do not bundle. |
| **O-08-05** — `require_service_desk` 401 has no JSM hint | **CONFIRMED** | File with refined framing — the right scope is `read:jira-work` + `read:servicedesk-request`, not `write:servicedesk-request`. Suggest wrapping `require_service_desk` calls inside `handle_jsm_create`'s map_err. |
| **O-08-06** — description ADF desync | **PARTIAL** | File with corrected framing. **Drop the "Atlassian would 400" claim** — official docs do not confirm. Replace with "may produce a JSM 400 error or silently drop ADF formatting". Suggest the defensive stderr warning + `isAdfRequest` reset as the fix. |

### Need PRE-FILE verification (INCONCLUSIVE-LOCAL)

| Finding | What's needed |
|---------|--------------|
| **O-01** — platform path drops `--field`/`--on-behalf-of` | Run `git pull --ff-only origin develop` then grep `handle_create` for `field.is_empty()` and `on_behalf_of.is_some()` guards in the platform-path branch. If absent, finding is CONFIRMED and safe to file. If present, REFUTE. |
| **O-08-02** — JSM "project required" terser | Pull and grep `handle_jsm_create` for the JSM-specific "project" error string. Quote both verbatim in the issue body. |
| **O-08-07** — `--type` warning fires pre-non-JSM-rejection | Pull and read `handle_create` / `handle_jsm_create`. Determine whether `--type` warning emission is before or after `require_service_desk`. If before, file. If after, do not file. |

### DOCUMENTATION-DRIFT only (low value as issues, but file if doing housekeeping)

| Finding | Note |
|---------|------|
| **O-08-03** — AC-013 type drift | Cosmetic story-doc fix. Lower priority. If the user is filing a "spec hygiene" issue anyway, bundle. Otherwise can be batch-fixed in the next docs sweep. Reframe from "spec mismatch" to "AC text drift, cosmetic". |

### DROPPED (none — no findings were fully REFUTED)

None of the 9 findings (1 + 1 + 7) are clearly REFUTED. The two PARTIAL findings
(O-08-01 and O-08-06) survive but with material reframing — the original
adversary write-ups overstated the fix surface (O-08-01) or the failure mode
(O-08-06).

---

## Cross-cutting recommendations for the user

1. **Run `git pull --ff-only origin develop` before filing.** Three of the nine
   findings (O-01, O-08-02, O-08-07) are INCONCLUSIVE-LOCAL specifically
   because the merged PR #381 code is not on the on-disk working tree. A pull
   lets these be flipped to CONFIRMED/REFUTED in seconds.
2. **File M-03 first.** It's the only finding that's verifiable on the current
   working tree, has the clearest fix, and unblocks any future audit of error
   message UX. The other findings build on the same surface.
3. **Bundle O-08-01 + O-08-05 into a single "JSM 401 hint surface refinement"
   issue.** Both share the same fix substrate (per-call-site map_err with
   scope-aware hints, parameterized by JiraClient auth-scheme predicate).
   Filing them separately would force two parallel fixes that touch the same
   code in incompatible ways.
4. **Defer O-08-03 to the next docs hygiene sweep** unless the user wants a
   "story-doc drift" issue thread open for tracking. Single-line AC update with
   zero code impact.
5. **Drop the misattributed "Atlassian would 400" claim** from O-08-06 per
   CLAUDE.md's `feedback_perplexity_copilot_reviews` and the citation-discipline
   note about external-tracker IDs — Atlassian docs and community threads do
   NOT confirm the 400 specifically for this shape mismatch, and the recent
   JRACLOUD-95368 episode (per CLAUDE.md gotcha) shows that misattributing
   Atlassian behavior in user-facing issue text leads to expensive rework.

---

## Research Methods

| Tool | Queries | Purpose |
|------|---------|---------|
| Read | 9 | Local source (`src/error.rs`, `src/cli/issue/create.rs`, `src/api/jsm/requests.rs`, `src/api/jsm/servicedesks.rs`, `src/partial_match.rs`, `src/api/client.rs`, `src/cli/issue/mod.rs`); story `.factory/code-delivery/issue-288-pr4-dispatch/story.md`; adversary passes 03/05/08/09; PR description; STATE.md grep |
| Glob | 6 | Locate PR #381 code (`.reference/`, factory worktree, `tests/issue_create_jsm*`); enumerate `tests/*.rs`, `src/api/jsm/`, adversary files |
| Grep | 12 | Verify presence/absence of `handle_jsm_create`, `JsmRequestBuilder`, `write:jira-work`, `--request-type`, `require_service_desk`, `NotAuthenticated` / `InsufficientScope` / 401 handling, `BC-3.8.*` |
| WebSearch | 2 | Atlassian behavior on Basic-auth vs OAuth 401 response shape; servicedeskapi `isAdfRequest` mismatch |
| WebFetch | 1 | Atlassian official docs page for `POST /rest/servicedeskapi/request` body shape |
| Perplexity (any tool) | 0 | MCP unused — local code + adversary documentation answered all in-scope claims; Atlassian web search adequate for the two external questions |
| Context7 | 0 | Not needed — no library docs in scope |
| Tavily | 0 | Not needed — WebSearch + WebFetch cross-validated against Atlassian's own docs |
| Training data | 2 areas | (1) Rust standard `String::contains(&str)` empty-string semantics — confirmed by reading `partial_match.rs` test coverage. (2) `clap` `requires(...)` semantics for flag-dependency wiring — used only in the issue-body fix-suggestions, not in validation. Both are low-risk training-data uses. |

**Total non-local tool calls:** 3 (2 WebSearch + 1 WebFetch)
**Training-data reliance:** low — every CONFIRMED finding is backed by a
read of source code on the current `develop` working tree. Every
INCONCLUSIVE-LOCAL finding is explicitly flagged as such and routed to
post-pull re-verification.

**Caveat to revisit after `git pull`:** The five INCONCLUSIVE-LOCAL items
(O-01, O-08-02, O-08-07) and the three findings whose impl read is not
on-disk (O-08-01, O-08-03, O-08-06) all need a quick read-pass against
the PR-#381-merged tree before the user files. The fastest path is:

```
git pull --ff-only origin develop
# then re-grep:
rg 'handle_jsm_create' src/cli/issue/create.rs
rg 'JsmRequestBuilder' src/api/jsm/requests.rs
rg 'fn parse_field_kv' src/cli/issue/create.rs
# Inspect for:
#   - O-01: any guard on `field.is_empty()` / `on_behalf_of.is_some()` in
#     the platform-path branch (after the `if request_type.is_some() { ... }`
#     early-return)
#   - O-08-02: the verbatim "project" error string inside handle_jsm_create
#   - O-08-03: the parse_field_kv signature (HashMap<String, String> vs
#     HashMap<String, serde_json::Value>)
#   - O-08-06: how JsmRequestBuilder::build orders --field overlays vs
#     --description ADF construction
#   - O-08-07: relative position of --type warning emission vs
#     require_service_desk call
```

---

## Post-Pull Re-Validation (O-01 / O-08-02 / O-08-07)

**Working tree state:** Local `develop` has been pulled to commit `9523255`
(PR #381 merge). PR4 code now present:
- `src/cli/issue/create.rs` (86,619 bytes) — contains `handle_create`,
  `handle_jsm_create`, dispatch fork.
- `src/api/jsm/requests.rs` (13,469 bytes) — `JsmRequestBuilder`.
- `tests/issue_create_jsm.rs` (81,390 bytes) — 29 integration tests.

This section converts the three INCONCLUSIVE-LOCAL findings to first-hand
verdicts. No other sections of this document are modified.

---

### O-01 — Platform path silently drops `--field` and `--on-behalf-of`

**Status:** **CONFIRMED**.

**Evidence (first-hand, post-pull):**

1. `src/cli/issue/create.rs:39-59` — `handle_create` destructures the
   `IssueCommand::Create` variant, including `field: field_pairs` (line 54)
   and `on_behalf_of` (line 55).
2. `src/cli/issue/create.rs:63-118` — dispatch fork `if request_type.is_some()`.
   Inside this block, the 6 platform-only flag warnings fire (lines 67-96),
   and `field_pairs` + `on_behalf_of` are forwarded into `handle_jsm_create`
   via the `JsmCreateArgs` struct (lines 113-114).
3. `src/cli/issue/create.rs:120-end-of-handle_create` — the platform-path
   branch (executed when `request_type.is_none()`) NEVER references
   `field_pairs` or `on_behalf_of` again. There is NO guard like
   `if !field_pairs.is_empty()` or `if on_behalf_of.is_some()` to emit a
   stderr warning. Grep confirms: in `create.rs`, `field_pairs` appears only
   at lines 54, 114, 1822, 1860, 1948 (all JSM-path uses or struct
   destructure); `on_behalf_of` appears only at lines 55, 113, 1821, 1859,
   1959 (same pattern). No platform-path read.
4. `src/cli/mod.rs:393-401` — clap definitions for `--field` and
   `--on-behalf-of` have NO `requires("request_type")` attribute. They are
   accepted in any combination at the parse layer.
5. `tests/issue_create_jsm.rs` — grep for `platform.*field`, `field.*platform`,
   `on_behalf.*platform`, `without.*request_type` returns one test
   (`test_jsm_create_without_request_type_uses_platform_path` at line 254)
   that exercises the platform path WITHOUT `--field` or `--on-behalf-of`.
   NO test passes `--field` / `--on-behalf-of` without `--request-type` to
   pin (or refute) the silent-drop behavior. The 6 JSM-side platform-only
   flag warnings are pinned (tests at lines 1242, 1587, 1643, 1699, 1755,
   1811), but the inverse direction is unpinned.

**Comparison to BC-3.8.011:** BC-3.8.011 (`bc-3-issue-write.md:699-725`) is
unidirectional — it specifies that 6 platform flags emit warnings on the
JSM path; it does NOT mandate the inverse warnings on the platform path.
So the asymmetry is BC-compliant but is a real UX gap: users who set
`--field foo=bar` on a platform-path create get NO feedback that the flag
was dropped on the floor.

**Verdict:** The original adversary finding is fully confirmed by first-hand
code read. The pre-pull caveat — "the silent-drop is INCONCLUSIVE-LOCAL
pending pull" — is resolved. Filing is safe with the same intent-question
framing recommended in the pre-pull pass (intentional clap-help-only
feedback vs. accidental asymmetry).

**Issue text impact:** No additional refinement beyond the pre-pull
recommendation. The pre-pull suggestions (option (a) symmetric warning,
option (b) clap `requires`) still stand. Add a one-line code reference for
the maintainer: "`src/cli/issue/create.rs:120-end` — no guard on
`field_pairs.is_empty()` or `on_behalf_of.is_some()` in the
`request_type.is_none()` branch."

---

### O-08-02 — JSM-path "project is required" terser than platform-path

**Status:** **CONFIRMED**.

**Evidence (first-hand, post-pull):**

**Platform-path string** (`src/cli/issue/create.rs:130-136`, inside
`handle_create` `ok_or_else`):

```
Project key is required. Use --project or configure .jr.toml. \
 Run "jr project list" to see available projects.
```

(After the line-continuation collapse, the rendered string is:
`Project key is required. Use --project or configure .jr.toml. Run "jr project list" to see available projects.`)

**JSM-path string** (`src/cli/issue/create.rs:1877`, inside
`handle_jsm_create` `ok_or_else`):

```
project is required for JSM request creation
```

**Delta analysis:**

| Dimension | Platform | JSM |
|-----------|----------|-----|
| Word count | ~17 words, 3 sentences | 6 words, 1 sentence |
| Mentions `--project` flag | yes | no |
| Mentions `.jr.toml` config | yes | no |
| Mentions discovery command (`jr project list`) | yes | no |
| Capitalization | "Project key…" (sentence case) | "project is required…" (all lowercase) |
| Path-context label | implicit (no JSM mention) | explicit ("for JSM request creation") |

The JSM string is materially terser AND drops three actionable affordances
(flag name, config path, discovery command). It also has inconsistent
capitalization vs. the platform sibling (lowercase `project` vs.
`Project key`). The path-context label ("for JSM request creation") is the
only thing the JSM string adds.

**Verdict:** The original adversary finding is confirmed. The platform-path
hint is meaningfully more helpful and the JSM-path hint should be harmonized
to include the `--project`/`.jr.toml`/`jr project list` affordances. The
"for JSM request creation" suffix can be preserved as a leading or trailing
context label.

**Issue text impact:** Quote both strings verbatim (as above). Suggest the
harmonized form for the JSM path, e.g.:

```
Project key is required for JSM request creation. Use --project or \
 configure .jr.toml. Run "jr project list" to see available JSM projects.
```

Note for the maintainer: factoring out a shared `project_key_required_error(context: &str)` helper would prevent future drift between the two paths.
This is a low-priority polish issue (no functional impact).

---

### O-08-07 — `--type` warning fires pre-dispatch on non-JSM project (dual output)

**Status:** **CONFIRMED**.

**Evidence (first-hand, post-pull):**

Code-order tracing in `src/cli/issue/create.rs`:

1. **(a) `--type` warning emission** — `src/cli/issue/create.rs:67-71`,
   inside `handle_create`, INSIDE the `if request_type.is_some()` block
   but BEFORE `handle_jsm_create` is invoked at line 98. The verbatim string
   is:
   ```
   warning: --type is ignored when --request-type is set; \
   request type encodes the issue type
   ```
2. **(b) `require_service_desk` call** — `src/cli/issue/create.rs:1892-1897`,
   inside `handle_jsm_create` (called at line 98 of `handle_create`). On a
   non-JSM project, `require_service_desk`
   (`src/api/jsm/servicedesks.rs:112-131`) returns
   `JrError::UserError(format!("Project \"{}\" is a {} project. {} a Jira Service Management project. Run \"jr project list\" to find a JSM project.", ...))`.

**Code order:** (a) is at `handle_create` line 67; (b) is at `handle_jsm_create`
line 1892 which is called from `handle_create` line 98. So (a) executes
BEFORE (b) along every code path where both fire.

**User-visible dual output on stderr** when running
`jr issue create --project SW --request-type "Bug Report" --type Task --summary x --no-input`
against a software-typed project:

```
warning: --type is ignored when --request-type is set; request type encodes the issue type
Error: Project "SW" is a Jira Software project. `jr issue create --request-type` requires a Jira Service Management project. Run "jr project list" to find a JSM project.
```

Both lines appear on stderr; exit code is 64 (from `JrError::UserError`).

**Test coverage check:**

- `tests/issue_create_jsm.rs:1242` (`test_jsm_create_type_flag_ignored_with_warning`)
  pins the warning string but uses a JSM-mounted project (HELP), so the
  warning + success path is exercised. NOT the dual-output non-JSM scenario.
- `tests/issue_create_jsm.rs:328` (`test_jsm_create_non_jsm_project_exits_64_zero_http`)
  pins the non-JSM error string but does NOT pass `--type`, so it doesn't
  exercise the dual-output path.
- Grep for `non_jsm.*type`, `software.*--type`, `warning.*non_jsm` in
  `tests/issue_create_jsm.rs` returns ZERO matches. **No test pins or
  refutes the dual-output behavior.**

**BC compliance:** BC-3.8.010 says the warning "need not fire" on early-exit
paths — i.e., the BC explicitly permits firing OR suppressing on the
non-JSM rejection path. The current implementation fires; this is BC-compliant
but is a UX polish gap.

**Verdict:** Original adversary finding fully confirmed. On a non-JSM
project with `--request-type X --type Y`, the user sees BOTH the warning
AND the non-JSM error on stderr. The warning is misleading because the
command was never going to dispatch JSM anyway, and is BC-permitted but
sub-optimal.

**Issue text impact:** Move the 6 `--type`/`--team`/`--points`/`--parent`/
`--to`/`--account-id` warnings from `handle_create:67-96` into
`handle_jsm_create` AFTER `require_service_desk` succeeds (i.e., after
line 1897). That way the warnings only fire on confirmed-JSM dispatch.
Alternatively, leave platform-only warnings where they are and accept the
dual-output as "informative" — but the warning text would be more accurate
if reframed as a generic "platform flags ignored when targeting JSM" rather
than asserting the JSM path was taken.

**Maintainer note:** A regression test for the dual-output behavior should
be added — either pinning the current dual-output OR pinning that the
warning is suppressed on non-JSM projects, depending on which intent the
maintainer chooses. Today's behavior is implicitly relying on whichever
path runs first; without a test, a future refactor could silently flip
the order.

---

## Post-Pull TL;DR

| Finding | Status (post-pull) | One-line verdict | Recommendation |
|---------|-------------------|------------------|----------------|
| **O-01** | **CONFIRMED** | Platform path has no warning for `--field` / `--on-behalf-of`; clap has no `requires` attr; no test pins inverse direction. | **FILE** with intent-question framing (symmetric warning vs clap `requires`); add code-line citations. |
| **O-08-02** | **CONFIRMED** | JSM hint (6 words) drops `--project`/`.jr.toml`/discovery-command affordances vs. platform sibling (17 words). | **FILE** as low-priority polish; suggest harmonized string + shared helper. |
| **O-08-07** | **CONFIRMED** | `--type` warning at `handle_create:67` fires BEFORE `require_service_desk` at `handle_jsm_create:1892`; non-JSM users see dual stderr output; no test pins behavior. | **FILE** with fix: move 6 warnings into `handle_jsm_create` after `require_service_desk` succeeds; add regression test for chosen intent. |

**Net result:** All three INCONCLUSIVE-LOCAL findings flip to CONFIRMED on
first-hand read. None refute. All three are safe to file; none require
external (Perplexity/web) validation because all claims are about local code
state, not Atlassian API behavior. The pre-pull "Need PRE-FILE verification"
table in the Summary section above is now fully resolved — promote all
three to the "SAFE to file" table when the user merges this addendum into
filing actions.
