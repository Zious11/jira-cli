# ADR-0014: JSM Request-Type Dispatch Fork in `jr issue create`

## Status
Accepted

## Context

`jr issue create` was originally written to target a single API endpoint:
`POST /rest/api/3/issue` (the Jira platform issue-creation API). Jira Service
Management (JSM) project requests require a fundamentally different endpoint:
`POST /rest/servicedeskapi/request`. The two APIs are incompatible in several
ways:

- The platform API accepts `issuetype`, `assignee`, `team`, story-point fields,
  and arbitrary field IDs in the issue body.
- The JSM API requires a `serviceDeskId`, a `requestTypeId`, and wraps all
  field values inside a `requestFieldValues` map. It also accepts `isAdfRequest`
  and `raiseOnBehalfOf` (for raising on behalf of another user), which the
  platform API does not.
- The JSM API validates fields against the request type's portal configuration
  and returns a `{"issueKey": "…", "issueId": "…"}` response rather than the
  `{"key": "…", "id": "…"}` shape of the platform API.

At the time of S-288 (JSM request-type support), two design options were
considered:

**Option A: Separate subcommand — `jr jsm create`.**
A dedicated command would have a clean surface but forces users to know which
API their project uses before choosing a command, breaking the mental model of
`jr issue create` as the universal issue-creation command.

**Option B: Dispatch fork inside `jr issue create` on `--request-type`.**
A single `jr issue create` entry point dispatches to the JSM path when
`--request-type` is supplied and to the platform path when it is absent. Users
who work with both platform projects and JSM projects invoke the same verb.

Option B was chosen.

A secondary concern was flag inheritance. `IssueCommand::Create` carries 16+
flags, some of which are meaningful on the platform path but have no JSM
equivalent (`--type`, `--team`, `--points`, `--parent`, `--to`, `--account-id`).
Rather than silently dropping these flags or erroring on them before verifying
the project is a JSM project at all, the design required that warnings be emitted
AFTER the JSM check succeeds — so users on non-JSM projects never see JSM-scoped
warnings.

## Decisions

### 1. Fork gate: `request_type.is_some()` — sole dispatch condition

**Decision:** In `handle_create` (`src/cli/issue/create.rs`), the very first
action after argument destructuring is:

```rust
if request_type.is_some() {
    return handle_jsm_create(client, config, output_format, …).await;
}
```

The platform path has no changes — it is byte-for-byte the same code path
that existed before S-288. The fork is gated solely on whether `--request-type`
was supplied. No project-type pre-check occurs before this fork.

> **DEC-188 amendment:** this claim is now conditional on `--field`/`--on-behalf-of`
> being absent; when either flag is present without `--request-type`, the platform
> path exits 64 pre-flight (BC-3.8.012/013). See `docs/specs/issue-create-preflight-guards.md`.

**Justification:** The gate is the simplest expression of user intent.
`--request-type` is a JSM-specific concept with no platform analogue. Its
presence is an unambiguous signal that the JSM API should be used. Checking
the project type before forking would require an extra HTTP call and would
produce confusing errors for the common case (user correctly supplies
`--request-type` for a JSM project). The service-desk verification happens
inside `handle_jsm_create` at step 4 via `require_service_desk`, after
zero-HTTP local guards fire.

**Platform path stability guarantee:** The `if request_type.is_some()` block
is an early return. All code below it in `handle_create` is the pre-existing
platform path. No platform-path behavior, output shape, or error message is
altered by this ADR.

> **DEC-188 amendment:** this claim is now conditional on `--field`/`--on-behalf-of`
> being absent; when either flag is present without `--request-type`, the platform
> path exits 64 pre-flight (BC-3.8.012/013). See `docs/specs/issue-create-preflight-guards.md`.

### 2. JSM endpoint: `POST /rest/servicedeskapi/request`

**Decision:** The JSM path POSTs to `/rest/servicedeskapi/request` via
`JiraClient::create_jsm_request` (`src/api/jsm/requests.rs`). The platform
path POSTs to `/rest/api/3/issue` (unchanged).

The body shape is assembled by `JsmRequestBuilder` (a pure helper struct in
`src/api/jsm/requests.rs`):

```json
{
  "serviceDeskId": "<id>",
  "requestTypeId": "<id>",
  "requestFieldValues": {
    "summary": "<summary>",
    "description": <ADF root>,
    "priority": {"name": "<priority>"},
    "labels": ["<label>", ...]
  },
  "isAdfRequest": true,
  "raiseOnBehalfOf": "<accountId>"
}
```

`isAdfRequest` is included if and only if `description` is present (BC-3.8.006).
`raiseOnBehalfOf` is included if and only if `--on-behalf-of` is supplied; the
key is completely absent otherwise (BC-3.8.009). Labels are plain strings, not
`{"name": "…"}` objects (BC-3.8.007). `serviceDeskId` and `requestTypeId` are
top-level fields, never inside `requestFieldValues` (BC-3.8.001).

Custom fields passed via `--field NAME=VALUE` are merged into
`requestFieldValues` with last-value-wins for duplicates (BC-3.8.008).

### 3. `--type` is silently warned, not errored (BC-3.8.010)

**Decision:** On the JSM path, the platform flags `--type`, `--team`,
`--points`, `--parent`, `--to`, and `--account-id` emit a stderr warning and
are ignored. They do NOT cause an exit-64 error. The warning fires at canonical
step 5 inside `handle_jsm_create` — AFTER `require_service_desk` returns `Ok`
and BEFORE request-type resolution — so warnings are suppressed when the
project is not a JSM project (BC-3.8.010, BC-3.8.011).

**Justification:** Erroring on these flags would break shell aliases and scripts
where users might always pass `--type Task` regardless of project type. A warning
preserves backward compatibility while still surfacing the no-op. The post-
`require_service_desk` placement is load-bearing: without it, a user who
accidentally runs `jr issue create --request-type foo --type Story` against a
non-JSM project would see the warning before learning the project is not a JSM
project at all.

### 4. 401 hint: `write:servicedesk-request` OAuth scope

**Decision:** On the JSM path, a 401 response from `POST /rest/servicedeskapi/request`
produces a scope-specific hint:

```
The `write:servicedesk-request` OAuth scope may be missing.
```

On the platform path, a 401 produces the existing API-token-expiry hint. The
two paths have separate 401-handling branches (BC-3.8.014 for basic auth,
BC-3.8.015 for OAuth) because the failure mode differs: JSM 401 most commonly
indicates a missing scope, not an expired token.

### 5. `--output json` response shape (BC-3.8.001)

**Decision:** `jr issue create --request-type` emits `{"key": "<issue_key>"}`
on stdout, identical to the platform path. The JSM API response carries the key
in the `issueKey` field; `handle_jsm_create` remaps it to `key` before
forwarding to `output::render_json` (JSON render invariant, §Conventions).

### 6. `JsmRequestBuilder` is a pure struct

**Decision:** Body assembly is extracted into `JsmRequestBuilder` (a pure,
side-effect-free struct with no `JiraClient` dependency) in
`src/api/jsm/requests.rs`. This makes the body-construction logic
independently testable via proptest without a mock HTTP client (properties
C.1–C.4 in `api/jsm/requests.rs`).

## Consequences

- `jr issue create --request-type <NAME>` routes to `POST /rest/servicedeskapi/request`
  instead of `POST /rest/api/3/issue`. No other `jr issue create` invocation is affected.
  > **DEC-188 amendment:** this claim is now conditional on `--field`/`--on-behalf-of`
  > being absent; when either flag is present without `--request-type`, the platform
  > path exits 64 pre-flight (BC-3.8.012/013) instead of the pre-DEC-188 warn-and-proceed
  > behavior. See `docs/specs/issue-create-preflight-guards.md`.
- The platform path is byte-for-byte unchanged: the dispatch gate is an early return at
  the top of `handle_create`, so all downstream platform logic is untouched.
  > **DEC-188 amendment:** this claim is now conditional on `--field`/`--on-behalf-of`
  > being absent; when either flag is present without `--request-type`, the platform
  > path exits 64 pre-flight (BC-3.8.012/013). See `docs/specs/issue-create-preflight-guards.md`.
- `--type` (and five other platform-only flags) are silently warned on the JSM path rather
  than errored, preserving alias/script compatibility.
- Scripts or wrappers that pipe `jr issue create --request-type … --output json` to `jq`
  can rely on the `.key` field being present — same shape as the platform path.
- The OAuth scope hint on 401 differs between JSM and platform paths; this is load-bearing
  and must not be unified.
- `JsmRequestBuilder` proptest properties (C.1–C.4) pin the body-construction invariants
  and provide regression coverage independent of the HTTP layer.

## See Also

- BC-3.8.001–017 in `bc-3-issue-write.md` — full behavioral contracts for the JSM path
- `docs/specs/jsm-e2e-coverage.md` — E2E coverage for JSM request creation scenarios
- `src/cli/issue/jsm_create.rs::handle_jsm_create` — canonical JSM dispatch implementation
- `src/api/jsm/requests.rs` — `JsmRequestBuilder` pure body helper and proptest suite
- `src/api/jsm/servicedesks.rs::require_service_desk` — JSM project gate (step 4)
- ADR-0015 — Proactive resolution enforcement on done-category transitions (parallel JSM context)
- `docs/specs/issue-create-preflight-guards.md` — DEC-188 pre-flight exit-64 guards for
  `--field`/`--on-behalf-of` without `--request-type` (S-639-1; amends the "byte-for-byte
  unchanged" claims in this ADR, does not supersede the dispatch architecture itself)
