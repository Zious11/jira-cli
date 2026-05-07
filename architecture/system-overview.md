# System Overview — jr (jira-cli)

**traces_to:** README.md
**Snapshot SHA:** `dea166471e22eff55974d7675593469b37048c5f`

---

## Product Summary

`jr` (package `jr`, binary `jr`) is a Rust 2024 / MSRV-1.85 single-crate CLI for automating Atlassian Jira Cloud workflows. It is a **thin client** (ADR-0001) wrapping Jira Core REST v3, Agile REST, JSM REST, the Atlassian Teams API, GraphQL `tenantContexts`, and the Assets/CMDB API directly via `reqwest 0.13` (rustls-tls per ADR-0003). No generated client, no intermediate abstraction layer.

**23,334 src/ LOC + 16,958 tests/ LOC + 125 build.rs LOC = 40,417 total Rust LOC.**

---

## 5-Layer Architecture

```
L0  main.rs (268 LOC)
      ├── tokio runtime construction (multi-thread)
      ├── Ctrl+C handler → process::exit(130) [abrupt, no graceful shutdown]
      └── error chain walker → JrError::exit_code() → process::exit(N)

L1  cli/ (clap derive)
      ├── cli/mod.rs (772 LOC) — top-level Command enum, 14 commands
      │   Global flags: --output, --project, --profile, --no-input, --no-color, --verbose
      └── per-subcommand sub-enums: IssueCommand (17 variants), AuthCommand (7 variants), etc.

L2  cli handlers (I/O: HTTP, stdin, stdout/stderr, cache, config)
      ├── cli/issue/{list,view,comments,changelog,create,workflow,links,assets,format,helpers,json_output}.rs
      ├── cli/{auth,assets,board,sprint,worklog,team,user,queue,project,init,api}.rs

L3  api/ (HTTP plumbing — bifurcated into two paths; see below)
      ├── api/client.rs (490 LOC) — JiraClient; 11 public HTTP methods
      ├── api/auth.rs (1,397 LOC) — OAuth 2.0 flow, keychain CRUD, per-profile namespacing
      ├── api/auth_embedded.rs (250 LOC) — XOR-obfuscated embedded OAuth credentials
      ├── api/pagination.rs (374 LOC) — 4 pagination shapes
      └── api/rate_limit.rs (56 LOC) — Retry-After integer parser

L4  api resource impls (impl JiraClient blocks, 18 files)
      ├── api/jira/{issues,boards,sprints,fields,statuses,links,teams,worklogs,projects,users,resolutions}.rs
      ├── api/jsm/{queues,servicedesks}.rs
      └── api/assets/{linked,objects,workspace,schemas,tickets}.rs

L5  types/ (pure Serde structs, no I/O, no imports from L0-L4)
      ├── types/jira/{issue,board,sprint,user,worklog,team,changelog,project}.rs
      ├── types/jsm/{queue,servicedesk}.rs
      └── types/assets/{linked,object,schema,ticket}.rs

L6  Cross-cutting utilities (mix of pure and I/O-bound)
      Pure: adf.rs, duration.rs, error.rs, jql.rs, output.rs, partial_match.rs,
            observability.rs (pub(crate) — 39 LOC, single function)
      I/O:  cache.rs (899 LOC), config.rs (1,223 LOC)
```

The dependency graph is a strict acyclic DAG (verified by cycle cross-check in Pass 1 R2). No upward edges: L6 utilities depend only on `std` or other L6 utilities; types (L5) have no imports from L0-L4; L4 resource impls depend on L3 client but not on L2 handlers.

---

## L3 HTTP-Path Bifurcation (Pass 1 R1 §6d)

`JiraClient` exposes two distinct behavioral contracts:

| Path | Methods | Error handling | Consumer |
|------|---------|---------------|----------|
| **Validated** | `get`, `post`, `put`, `post_no_content`, `delete`, `get_from_instance`, `post_to_instance`, `get_assets`, `post_assets` (9 total) | `send → parse_error → JrError` | All api/jira, api/jsm, api/assets impls (~50 call sites) |
| **Raw passthrough** | `request` + `send_raw` (used as a pair) | Returns `reqwest::Response`; caller owns status | `cli/api.rs::handle_api` ONLY |

Both paths share the 429-retry loop (MAX_RETRIES=3, DEFAULT_RETRY_SECS=1) and auth-header injection. They diverge on what callers receive for non-2xx responses.

The raw passthrough exists solely for `jr api` — the `curl`-style HTTP escape hatch. Any spec of `JiraClient` must preserve this distinction.

---

## Deployment Topology (Single Binary)

```
~/.config/jr/config.toml        Global config: profiles, default_profile, defaults
<cwd>/.jr.toml                  Per-project config: project key, board_id (optional)
~/.cache/jr/v1/<profile>/       Versioned per-profile cache (7-day TTL)
  teams.json
  project_meta.json
  workspace.json
  cmdb_fields.json
  object_type_attrs.json
  resolutions.json

OS Keychain (jr-jira-cli)
  Shared (account-level, flat keys):
    email, api-token, oauth_client_id, oauth_client_secret
  Per-profile (cloudId-scoped, namespaced):
    <profile>:oauth-access-token
    <profile>:oauth-refresh-token
  Legacy (default profile only, lazy-migrated):
    oauth-access-token, oauth-refresh-token

Build-time artifacts (CI release only):
  JR_BUILD_OAUTH_CLIENT_ID / _SECRET → build.rs → $OUT_DIR/embedded_oauth.rs
  (XOR-obfuscated; per-build random 32-byte key from OS CSPRNG)
```

---

## Network Egress Map

| Endpoint | API | Auth method |
|----------|-----|-------------|
| `https://<site>.atlassian.net/rest/api/3/*` | Jira Core REST v3 | API token or OAuth bearer |
| `https://<site>.atlassian.net/rest/agile/1.0/*` | Agile REST (boards, sprints) | same |
| `https://<site>.atlassian.net/rest/servicedeskapi/*` | JSM REST (queues, service desks, asset workspace) | same |
| `https://<site>.atlassian.net/gateway/api/graphql` | GraphQL (ADR-0005: `tenantContexts` + `hostNames`) | same |
| `https://<site>.atlassian.net/gateway/api/public/teams/v1/*` | Teams API | same |
| `https://api.atlassian.com/ex/jira/<cloud_id>/*` | OAuth-proxied Jira API | OAuth bearer only |
| `https://api.atlassian.com/ex/jira/<cloud_id>/jsm/assets/workspace/<wid>/v1/*` | Assets/CMDB API | OAuth bearer only |
| `https://auth.atlassian.com/authorize` | OAuth authorize | n/a (browser redirect) |
| `https://auth.atlassian.com/oauth/token` | OAuth token exchange | client_id + client_secret + code |
| `https://api.atlassian.com/oauth/token/accessible-resources` | Cloud site discovery | OAuth bearer |

**Inbound TCP** (only during `jr auth login --oauth`):
- Embedded app: `127.0.0.1:53682` (fixed, literal IPv4 — ADR-0006)
- BYO app: `127.0.0.1:0` (ephemeral port)
Both bindings are TOCTOU-closed via `ResolvedRedirect` private-field pattern.

---

## Purity Boundary

**Pure (no I/O, verifiable):** `adf`, `duration`, `error`, `jql`, `output`, `partial_match`, `observability`, `api/pagination`, `api/rate_limit`, all `types/*`, `api/auth_embedded::{decode, build_embedded_app}`, `config::validate_profile_name`, `api/client::extract_error_message`, `cli::resolve_effective_limit`.

**I/O-bound (effectful shell):** `main.rs`, all `cli::*::handle*`, `api::client::JiraClient` methods, `api::auth` (keychain + OAuth network + listener), `cache::{read_*,write_*,clear_*}`, `config::Config::{load,save_global,find_project_config}`, all L4 resource impls (HTTP), `api::assets::workspace::get_or_fetch_workspace_id` (HTTP + cache), `api::assets::linked::get_or_fetch_cmdb_fields` (HTTP + cache).

---

## Multi-Profile Model

Profile resolution precedence (left wins): `--profile` flag > `JR_PROFILE` env > `default_profile` config field > literal `"default"`. Threaded as a parameter through `Config::load_with(cli_profile)` — NOT an env-var seam.

Every cache reader/writer takes `profile: &str` as its first argument. `JiraClient` carries `profile_name: String` (set at construction) and exposes `profile_name()` for L4 modules that have a client but not a config.

Per-profile isolation is a convention-enforced (soft-fence) correctness invariant. A `Profile(String)` newtype would provide compile-time enforcement — see ADR-0011 (DEFERRED).

---

## Test Infrastructure

| Mechanism | Purpose |
|-----------|---------|
| `JR_BASE_URL` env | Route all HTTP to wiremock instance |
| `JR_AUTH_HEADER` env | Bypass keychain auth (ALSO works in production — NFR-S-B) |
| `JR_SERVICE_NAME` env | Isolate keychain service name per test |
| `JiraClient::new_for_test(base_url, auth_header)` | Direct constructor for integration tests |
| `JR_RUN_KEYRING_TESTS=1` | Enable `#[ignore]`-gated keyring round-trip tests |
| proptest | Property-based tests on `jql.rs`, `partial_match.rs`, `duration.rs` |
| insta | 17 snapshot files for output format regression |
| wiremock | HTTP mock server for integration tests |

**Security note:** `JR_AUTH_HEADER` has no `#[cfg(test)]` gate — it functions in production binaries (NFR-S-B, HIGH). See ADR-0007 discussion and risk-register.md R1-NEW-2.
