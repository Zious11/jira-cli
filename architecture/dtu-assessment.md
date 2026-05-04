---
name: jira-cli
version: "0.5.0-dev.7"
date: 2026-05-04
status: COMPLETE
DTU_REQUIRED: false
---

# DTU Assessment — jira-cli (jr)

## §1: Third-Party Services Inventory

The following external services are integrated by `jr` across its API and auth layers. Every entry is grounded in the semport analysis (Pass 1 architecture, Pass 8 synthesis) and verified against `Cargo.toml` and `src/api/` module structure.

| # | Service | Integration Point | Auth Model | Protocol |
|---|---------|------------------|------------|----------|
| 1 | **Atlassian Jira REST API v3** (`<instance>.atlassian.net/rest/api/3/`) | `api/jira/{issues,boards,fields,statuses,links,projects,resolutions,sprints,teams,users,worklogs}.rs` (11 files) | Basic (email+token) or OAuth 2.0 bearer | HTTPS / JSON |
| 2 | **Atlassian Agile REST API** (`<instance>.atlassian.net/rest/agile/1.0/`) | `api/jira/{boards,sprints}.rs` | Same as #1 | HTTPS / JSON |
| 3 | **Atlassian JSM REST API** (`<instance>.atlassian.net/rest/servicedeskapi/`) | `api/jsm/{servicedesks,queues}.rs` | Same as #1 | HTTPS / JSON |
| 4 | **Atlassian Assets / CMDB API** (`<instance>.atlassian.net/rest/assets/1.0/` and `api.atlassian.com/jsm/assets/workspace/<id>/`) | `api/assets/{workspace,linked,objects,schemas,tickets}.rs` | Same as #1 + workspace-scoped URLs | HTTPS / JSON |
| 5 | **Atlassian GraphQL `tenantContexts`** (`api.atlassian.com/graphql`) | `api/jira/teams.rs` — org discovery (ADR-0005) | OAuth 2.0 bearer | HTTPS / GraphQL |
| 6 | **Atlassian OAuth 2.0 token endpoint** (`auth.atlassian.com/oauth/token`) | `api/auth.rs` — token exchange + refresh | Client credentials (embedded XOR-obfuscated; ADR-0006) | HTTPS / form-encoded |
| 7 | **Atlassian OAuth 2.0 authorize endpoint** (`auth.atlassian.com/authorize`) | `api/auth.rs` — browser-based authorization-code flow initiation (no PKCE — see ADR-0006) | None (redirect) | HTTPS / browser redirect |
| 8 | **OS browser launcher** (`open` crate → `xdg-open` / `open` / `start`) | `api/auth.rs:embedded_oauth_app` — opens authorize URL | N/A (local OS call) | Process exec |
| 9 | **OS keychain** (`keyring` crate → macOS Keychain / Linux secret-service / Windows Credential Manager) | `api/auth.rs` — store/retrieve email, api-token, oauth_client_{id,secret}, per-profile oauth-{access,refresh}-token | OS credential manager | Platform API |

**Total external services inventoried: 9**

Notes:
- Services 1–5 are all Atlassian Cloud APIs at domains `*.atlassian.net` and `api.atlassian.com`. They are operated by Atlassian PLC; `jr` is a consumer, not an owner.
- Service 6–7 are the Atlassian identity platform (auth.atlassian.com); also operated by Atlassian.
- Services 8–9 are local-system integrations (OS process exec and OS credential store), not network services.
- There are no self-hosted databases, message queues, caches, or observability sinks. `jr` is a stateless CLI whose only persistent state lives in `~/.config/jr/` (TOML), `~/.cache/jr/v1/<profile>/` (JSON), and the OS keychain.

---

## §2: DTU Candidacy per Service

### Service 1: Atlassian Jira REST API v3

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Very large — 11 resource modules, ~40+ distinct endpoints (search, get, create, edit, transitions, link, worklog, fields, statuses, users, projects, resolutions, comments, assignable users) |
| Stateful from CLI perspective | Stateful: create/edit/move/assign mutate server state. Read endpoints are stateless from CLI's view. |
| Existing test coverage | `wiremock` mocks HTTP at the transport layer for all 11 resource modules. `tests/common/fixtures.rs` provides 35+ JSON factory functions. 324 integration tests exercise these paths. |
| DTU clone value | Low. `wiremock` already plays the role of a per-test HTTP double. A DTU clone would replicate mock infrastructure already present. The CLI cannot own Atlassian's API schema or behavior. |
| Legal/feasibility | NOT FEASIBLE. Atlassian REST API is proprietary SaaS. Cloning its behavior would require reverse-engineering the production service, which is prohibited under Atlassian's ToS and Developer Program Agreement. |

**Verdict: NOT-CANDIDATE**

### Service 2: Atlassian Agile REST API

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Small — board list/config (2 endpoints), sprint list/issues (3 endpoints) |
| Existing test coverage | `wiremock` fixtures: `board_config_response`, `board_list_response`, `sprint_list_response`, `sprint_issues_response`, `sprint`. |
| DTU clone value | Low — same reasoning as Service 1. Mock is already in place. |
| Legal/feasibility | NOT FEASIBLE — same ToS constraints as Service 1. |

**Verdict: NOT-CANDIDATE**

### Service 3: Atlassian JSM REST API

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Small — service desk list (2 endpoints), queue list/issues (3 endpoints) |
| Existing test coverage | JSM-scoped integration tests via `wiremock`. `ServiceDeskPage` pagination struct handles JSM's non-standard envelope. |
| DTU clone value | Low. |
| Legal/feasibility | NOT FEASIBLE — same ToS constraints as Service 1. |

**Verdict: NOT-CANDIDATE**

### Service 4: Atlassian Assets / CMDB API

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Medium — workspace discovery, AQL search, object get/resolve, field discovery, connected tickets |
| Stateful from CLI perspective | Read-only in the CLI surface (no CMDB write operations currently) |
| Existing test coverage | `wiremock` used in `api/assets/` tests; workspace ID and CMDB fields are cached (7-day TTL). |
| DTU clone value | Low. The most complex behavior (AQL → object resolution) is tested via wiremock stubs returning predetermined JSON. |
| Legal/feasibility | NOT FEASIBLE — same ToS constraints as Service 1. |

**Verdict: NOT-CANDIDATE**

### Service 5: Atlassian GraphQL `tenantContexts`

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Single GraphQL query; returns `orgId` + `cloudId` pairs |
| Existing test coverage | `graphql_org_metadata_json()` fixture in `tests/common/fixtures.rs`; stubbed via `wiremock`. |
| DTU clone value | Negligible — one query shape, simple response. |
| Legal/feasibility | NOT FEASIBLE — same ToS constraints as Service 1. |

**Verdict: NOT-CANDIDATE**

### Service 6–7: Atlassian OAuth 2.0 Endpoints (token + authorize)

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Token exchange (one POST), refresh (one POST), browser redirect (one GET). Stateful: tokens are issued by Atlassian's identity provider. |
| Existing test coverage | OAuth token exchange and refresh are tested via `api::auth` unit tests with mocked keyring (`JR_RUN_KEYRING_TESTS=1` gate for live keychain). Token endpoint is not wiremocked (test isolation relies on not calling it). Authorization-code flow exercised; PKCE not implemented (see NFR-S-A in PRD nfr-catalog.md). |
| DTU clone value | A behavioral clone of `auth.atlassian.com/oauth/token` could enable end-to-end OAuth flow tests without live credentials. However, the OAuth flow is not part of the regression-critical surface being evolved in Phase 3/4 stories — it is infrastructure tested once at setup. |
| Legal/feasibility | NOT FEASIBLE as a full behavioral clone. The token endpoint is a proprietary Atlassian identity service. A lightweight stub (returning a static `access_token`) could be built with wiremock for specific unit tests, but this is already the practice for auth unit tests that call `new_for_test`. |

**Verdict: NOT-CANDIDATE** (stub-level coverage sufficient; full DTU clone not feasible or necessary)

### Service 8: OS Browser Launcher (`open` crate)

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Minimal — `open::that(url)` is a one-line system call with no response |
| Stateful | No |
| Existing test coverage | `open::that` is mocked implicitly in unit tests by not calling the OAuth browser flow (tests use `--no-input` / non-interactive paths). |
| DTU clone value | None — no behavioral state to clone. |
| Legal/feasibility | OUT-OF-SCOPE — local OS API, not a network service. |

**Verdict: OUT-OF-SCOPE**

### Service 9: OS Keychain (`keyring` crate)

| Factor | Assessment |
|--------|-----------|
| Behavioral surface | Small — get/set/delete credential per (service, username) pair. `jr` manages ~8 distinct key names across shared and per-profile namespaces. |
| Stateful | Yes — credentials persist across CLI invocations. |
| Existing test coverage | Keyring tests gated behind `JR_RUN_KEYRING_TESTS=1` + `#[ignore]` to avoid CI dependency on secret-service. `api/auth.rs` unit tests use mock keyring entries where possible. |
| DTU clone value | Low. The keyring interface is simple (CRUD on (service, key) pairs). Existing mock keyring approach in unit tests is adequate. A DTU clone would not catch behavioral drift because `keyring` crate behavior is a local OS API, not a third-party service. |
| Legal/feasibility | OUT-OF-SCOPE — local OS API, not a network service. |

**Verdict: OUT-OF-SCOPE**

---

## §3: Verdict

**DTU_REQUIRED: false**

No DTU clones are required for `jr`. All 9 services inventoried are either:

1. **Atlassian proprietary SaaS APIs** (Services 1–7) — legally and technically not clonable; existing `wiremock`-based HTTP stubbing already provides per-test behavioral doubles at the HTTP transport layer.
2. **Local OS APIs** (Services 8–9) — not network services; mock patterns already in place.

---

## §4: Rationale

### 4.1 Atlassian APIs are external, not owned

DTU (Digital Twin Universe) clones are most valuable when the team owns the service being cloned — i.e., when behavioral drift in a service under our control could cause regressions in the consumer. `jr` integrates exclusively with external Atlassian SaaS APIs that `jr` does not own, operate, or have API over. The bug classes DTU clones catch (drift in a service we deploy) do not apply.

### 4.2 Wiremock is already a per-test HTTP DTU

`wiremock 0.6` (a dev-dependency) is used throughout the integration test suite to stub Atlassian API responses at the HTTP transport layer. `JiraClient::new_for_test(base_url, auth_header)` injects a wiremock server URL at the client level. `tests/common/fixtures.rs` (446 LOC, 35+ factory functions) provides predetermined JSON response shapes for every major endpoint. This is functionally equivalent to a DTU clone for the purposes of regression detection: each integration test controls the exact API response and asserts the exact CLI output. If `jr`'s parsing logic regresses against a known Atlassian API response shape, the wiremock fixture catches it.

### 4.3 Insta snapshots cover output regression

`insta 1.x` (dev-dependency) provides 17 snapshot files that lock CLI output shapes. Any regression in table formatting, JSON field naming, or error message text is caught by snapshot diff without a DTU.

### 4.4 Legal constraint eliminates the option

Atlassian's Developer Program Agreement and Terms of Service prohibit reverse-engineering or replicating the production service behavior. Even if DTU clones added value, they are not a legal option for Atlassian Cloud APIs.

---

## §5: Implications for Phase 4 Holdout Evaluation

Without DTU clones, the Phase 4 holdout evaluation strategy is:

1. **Holdout candidate set:** 48 holdout candidates (H-001..H-047 plus H-NEW-MP-001) identified in Pass 3 R1–R4 behavioral contract analysis.
2. **Test execution:** All holdout tests run against `wiremock` servers configured with fixtures from `tests/common/fixtures.rs`. No live Atlassian API calls are made during evaluation.
3. **Response fidelity:** Wiremock fixtures are authored to match the actual Atlassian API response shapes observed during semport analysis (Pass 2 domain model, Pass 3 behavioral contracts). Predetermined response shapes cover pagination envelopes (`OffsetPage`, `CursorPage`, `ServiceDeskPage`, `AssetsPage`), error envelopes (`errorMessages` array), and all 14 bounded context response types.
4. **Output regression:** `insta` snapshots lock table and JSON output. Snapshot diff is the primary regression signal.
5. **Known gap:** Live Atlassian API behavioral drift (e.g., Atlassian deprecating a field or changing pagination semantics) is not caught by wiremock fixtures. This is accepted risk — the fixtures must be manually updated when Atlassian changes API shapes. This is the correct trade-off for a CLI consumer of external SaaS: monitor Atlassian changelog, not a DTU.

---

## §6: Future Revisit Triggers

The DTU assessment should be revisited if any of the following conditions arise:

### 6.1 `jr` adds an embedded local server with stateful behavior

If `jr` ships an embedded local HTTP server (beyond the current ephemeral OAuth callback listener at `127.0.0.1:53682` which is already unit-tested), that server's behavior becomes a candidate for DTU cloning if it persists state across CLI invocations.

### 6.2 `jr` targets Jira Server / Data Center (self-hostable)

Currently `jr` targets Jira Cloud only (Cloud-hosted SaaS at `*.atlassian.net`). If support is added for Jira Server or Jira Data Center — where the customer operates the instance — that self-hosted instance becomes a candidate for a containerized DTU clone (e.g., a Docker image of Jira Server used in CI for end-to-end integration tests). This is a materially different situation from the current Cloud-only model.

### 6.3 A new inbound data source is added that `jr` owns

If `jr` adds an owned inbound feed (e.g., a companion sync daemon or webhook receiver) whose behavior must be faithfully cloned for regression detection, that component becomes a DTU candidate.

### 6.4 Wiremock fixture maintenance becomes a bottleneck

If Atlassian API changes occur frequently enough that wiremock fixture drift becomes a recurring source of false-negative test failures, a lightweight record-and-replay DTU (e.g., using `wiremock`'s recording mode against a sandbox Atlassian tenant) should be evaluated. This would not be a "behavioral clone" but a captured session replayer — a weaker but legally compliant alternative.

---

_Assessment produced by the Architect agent (vsdd-factory) for jira-cli semport Phase 1, 2026-05-04._
