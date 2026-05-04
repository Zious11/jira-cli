# Pass 4: NFR Catalog — jira-cli (jr)

Snapshot SHA: `dea166471e22eff55974d7675593469b37048c5f` (v0.5.0-dev.7)
Source root: `/Users/zious/Documents/GITHUB/jira-cli/.reference/jira-cli/`
Analysis date: 2026-05-04
Builds on: Pass 0 (inventory), Pass 1 (architecture), Pass 2 (domain model), Pass 3 (behavioral contracts).

> **Method.** Every NFR claim cites a `<file>:<line>` pair, a config value, or a release-profile setting. CLAUDE.md and ADRs are starting points; deviations from those are flagged. Where a dimension is intentionally thin (the project deliberately ships a small surface), the absence is documented rather than padded. Phase B (convergence) will deepen.

---

## 1. Performance NFRs

### 1.1 Build profile (`Cargo.toml:47-52`)

| Setting | Value | Intent |
|---|---|---|
| `opt-level` | `3` | Maximum runtime optimization. Default for release builds; explicit here to be unambiguous. |
| `lto = "thin"` | `"thin"` | Link-time optimization across crate boundaries. **Thin** (not `"fat"` / `true`) — chosen for faster link times at near-equivalent runtime perf. Trade-off: slightly larger binary than `lto = true`, faster CI builds. |
| `codegen-units` | `1` | Single codegen unit per crate — squeezes additional perf from inlining at the cost of slower compilation. Acceptable because the binary is shipped via release pipelines, not built from source by users. |
| `strip = true` | `true` | Removes debug symbols from the shipped binary. Reduces tarball size and limits casual binary forensics (defense-in-depth for the embedded XOR'd OAuth secret — see §2.4). |
| `panic = "abort"` | `"abort"` | No unwinding code in the release binary. Smaller binary, faster panic-path, no unwind tables. **Implication:** any `panic!`, `unwrap()`, or `expect()` in a release binary aborts the process immediately — there is no destructor cleanup. The OAuth state generator (`api/auth.rs:881-895`) explicitly avoids `unwrap` for OS CSPRNG failures because of this. |

### 1.2 HTTP layer (`src/api/client.rs`)

- **Backend:** `reqwest 0.13` with `default-features = false, features = ["json", "rustls"]` (`Cargo.toml:25`). Per ADR-0003, **no native-tls dependency** — eliminates the OpenSSL/SecureTransport build surface.
- **Per-request timeout:** **30 seconds**, set on the `Client` itself: `Client::builder().timeout(Duration::from_secs(30)).build()?` (`api/client.rs:84`). Applies to ALL request methods (`get`, `post`, `put`, `delete`, `post_no_content`, `send_raw`, `get_from_instance`, `post_to_instance`, `get_assets`, `post_assets`). No per-request override mechanism is exposed.
- **Connection pooling:** Default `reqwest`/`hyper` connection pool — not explicitly tuned. The `JiraClient` owns one `reqwest::Client`, so all requests for a single `jr` invocation share the pool.
- **Test client:** `JiraClient::new_for_test` (`api/client.rs:111-122`) uses `Client::new()` (no timeout) so wiremock latency assertions are deterministic.
- **OAuth token endpoint clients:** `oauth_login` (`api/auth.rs:607`) and `refresh_oauth_token` (`api/auth.rs:708`) construct **fresh** `reqwest::Client::new()` instances rather than reusing the `JiraClient` — these are unauthenticated calls to `auth.atlassian.com` which has its own host. **Note:** these clients do NOT inherit the 30s timeout (a real-deployment latency cliff, see §1.7 Gaps).
- **Body size:** No explicit request/response body size cap. Relies on `reqwest` defaults (effectively bounded only by available memory).

### 1.3 Pagination (`src/api/pagination.rs`, 374 LOC)

| Strategy | Used by | Endpoints | Source |
|---|---|---|---|
| **Cursor (`nextPageToken`)** | `CursorPage<T>` | `POST /rest/api/3/search/jql` only | `api/jira/issues.rs:70` (search_issues) |
| **Offset (`startAt`+`maxResults`+`total`)** | `OffsetPage<T>` | Most Jira REST v3 (boards, sprints, worklogs, comments, projects, link-types, fields) | `api/jira/*.rs` |
| **JSM offset (`size`/`start`/`limit`/`isLastPage`)** | `ServiceDeskPage<T>` | `/rest/servicedeskapi/*` (workspace discovery, queues) | `api/assets/workspace.rs`, `api/jsm/queues.rs` |
| **Assets offset (`startAt`/`maxResults`/`total`/`isLast`)** | `AssetsPage<T>` | Assets/CMDB `POST /object/aql` | `api/assets/objects.rs` |

**Page size defaults:**
- `DEFAULT_LIMIT = 30` (`cli/mod.rs:740`) — the CLI-side default for `--limit` when neither `--limit N` nor `--all` is set.
- Server-side max for `/search/jql`: 100 (Atlassian limit; not encoded as a constant).
- `MAX_SPRINT_ISSUES = 50` (`cli/sprint.rs:107`) — Atlassian Agile API per-call cap on `sprint add` / `sprint remove`.
- User-search `--all` advances `startAt` by **requested** `maxResults`, NOT by returned count (regression test `tests/user_pagination.rs`).

**Total-records short-circuit:** Yes — `cli/issue/list.rs` and other paginated handlers stop once they reach the user's effective limit, even if the server reports more available. `Mock::expect(0)` tests confirm follow-up pages are not fetched once limit is satisfied (e.g., `tests/issue_list_errors.rs:388`).

**`AssetsPage::is_last` tolerance:** Custom `deserialize_bool_or_string` (`api/pagination.rs:118-128`) accepts both `bool` and `string` — pinning Atlassian API inconsistency. Performance impact: zero (deserialization cost only, no runtime branching).

### 1.4 Caching (`src/cache.rs`, 899 LOC)

| Cache | File | Layout | TTL | Invalidation |
|---|---|---|---|---|
| Teams | `teams.json` | Whole-file `TeamCache { fetched_at, teams }` | 7 days | TTL only |
| Project meta | `project_meta.json` | Map cache `HashMap<project_key, ProjectMeta>`, **per-entry TTL** | 7 days per entry | TTL only |
| Workspace ID | `workspace.json` | Whole-file `WorkspaceCache { workspace_id, fetched_at }` | 7 days | TTL only |
| Resolutions | `resolutions.json` | Whole-file `ResolutionsCache { resolutions, fetched_at }` | 7 days | TTL or `--refresh` flag |
| CMDB fields | `cmdb_fields.json` | Whole-file `CmdbFieldsCache { fields: Vec<(id, name)>, fetched_at }` | 7 days | TTL only |
| Object-type attrs | `object_type_attrs.json` | Map cache `HashMap<type_id, Vec<CachedObjectTypeAttr>>`, **per-file TTL** | 7 days | TTL only |

- **TTL constant:** `CACHE_TTL_DAYS = 7` (`cache.rs:7`).
- **TTL mechanism:** Embedded `fetched_at: DateTime<Utc>` field in each cache struct, compared to `Utc::now()` via `(now - fetched_at).num_days() >= 7` (`cache.rs:30`). **Not file mtime** — survives `touch` and copy-with-preserve.
- **Versioned root:** `~/.cache/jr/v1/<profile>/` (`cache.rs:76-77`). Bumping to `v2/` orphans all `v1/` files — clean schema-break path.
- **Per-profile namespacing:** Every reader/writer takes `profile: &str` first arg. Cross-profile cache leakage is a **correctness** bug (sandbox vs prod custom-field IDs differ), not a UX issue.
- **Cache miss/hit cost orders of magnitude:**
  - **Teams cache miss:** GraphQL org-id fetch + paginated `GET /gateway/api/public/teams/v1/org/{orgId}/teams` (potentially many pages with cursor `entities`); cache hit is one filesystem read.
  - **CMDB fields cache miss:** `GET /rest/api/3/field` + filter to CMDB schema (single call, ~1KB-100KB JSON depending on instance). Cache hit avoids the call entirely on every list/view that uses `--asset` or `--assets`.
  - **Workspace ID cache miss:** `GET /rest/servicedeskapi/assets/workspace`. Cache hit avoids the call on every `assets` and `issue assets` invocation.
  - **Resolutions cache miss:** `GET /rest/api/3/resolution` (small, fixed-size response). Cache hit avoids the call on every `issue move --resolution`.
- **Cache miss policy** (`cache.rs:14-34`): NotFound → `Ok(None)`; deserialization failure → stderr warning + `Ok(None)`; expired → `Ok(None)`. **Corruption is treated as miss, not error** — survives format changes.
- **Cache write atomicity:** Direct `fs::write` (`cache.rs:41`) — **not atomic**. A crash mid-write can leave a partial file. The cache miss policy makes this self-healing on next read.

### 1.5 Concurrency

- **Tokio runtime:** `tokio = { version = "1", features = ["full"] }` (`Cargo.toml:29`). `#[tokio::main]` in `main.rs:9` — defaults to **multi-threaded** runtime (`features = ["full"]` includes the `rt-multi-thread` feature, and `#[tokio::main]` defaults to `flavor = "multi_thread"`).
- **Single user-driven HTTP sequence:** Every `jr` invocation does at most one user-action's worth of work; HTTP requests within a single command are predominantly **serialized**. Pagination loops are sequential awaits.
- **Asset enrichment is serialized, not concurrent:** Per-field `client.get_assets(workspace_id, "object/{key}")` calls in `cli/issue/view.rs` and per-row enrichment in `cli/issue/list.rs` are awaited one-at-a-time. **Implication:** N+1 query pattern; for issues with many CMDB fields × many results, latency scales linearly. Not fixed by concurrency primitives despite `futures` being a direct dep.
- **`futures` direct dep (`Cargo.toml:33`):** features `async-await` only. No `try_join_all` / `buffer_unordered` usages found in the source — present for trait/utility imports, not concurrent fan-out.
- **OAuth callback listener:** Single `accept().await` (`api/auth.rs:572`); intentionally single-shot.

### 1.6 Performance budgets / SLOs

- **No explicit time budgets** in source or docs. `grep` for `Duration::from`, `timeout`, `budget`, `SLO`, `SLA` returned only the 30s reqwest client timeout and the 1s default 429 retry delay (`api/client.rs:11-14, 84`).
- **Implicit budgets:** the 30s per-request timeout caps tail latency on any single Jira call. With `MAX_RETRIES = 3` on 429, worst-case latency for a single logical request is `30s × 4 attempts + 3 × Retry-After` ≈ 120s+ (Retry-After is uncapped — Jira can ask for arbitrarily large delays).
- **No test timeouts** that would encode a perf budget — `wiremock`-backed tests are <1s by construction.

### 1.7 Performance gaps (rolled into §7)

- OAuth `Client::new()` instances at `auth.rs:607, 708` lack the 30s timeout — token exchange / refresh can hang indefinitely.
- Asset enrichment is N+1 serialized; no concurrency cap or batching.
- 429 Retry-After has no upper bound — a malicious or misconfigured proxy returning `Retry-After: 86400` would make the client sleep for 24h (× 3 attempts).

---

## 2. Security NFRs

### 2.1 Threat model & secrets

- **Embedded OAuth app (ADR-0006, supersedes ADR-0002):** `client_id` + `client_secret` are XOR-obfuscated at compile time by `build.rs` (125 LOC) and emitted as module-private constants in `$OUT_DIR/embedded_oauth.rs`. The XOR key is **fresh per build** (32 random bytes from `/dev/urandom` on Unix, `BCryptGenRandom` direct FFI on Windows — no extra build deps).
- **Stated threat model** (`api/auth_embedded.rs` module docstring + ADR-0006): "Obfuscation defeats automated secret scanners. Motivated reverse engineers can still extract the plaintext from a debugger; the operational mitigation is `client_secret` rotation." **What it actually protects against:**
  - Casual scraping of the binary by GitHub/GitLab secret-scanning bots → blocked.
  - Plaintext in the shipped binary that could trigger Atlassian's automated secret-revocation flow → blocked.
  - A determined adversary with `gdb`/`lldb`/IDA → **NOT blocked** (this is acknowledged and accepted; OAuth installed-app threat model per RFC 8252 is inherently weak).
- **Per-build XOR key rotation:** even if someone reverse-engineers one binary, the next `cargo build` rotates the key, invalidating cached deobfuscation tooling.
- **Lazy decode + `OnceLock`:** `embedded_oauth_app()` decodes once per process. `embedded_oauth_app_present()` (`api/auth_embedded.rs:132-136`) checks presence WITHOUT decoding — defense in depth so `jr auth status` doesn't materialize plaintext when only a presence check is needed.
- **`Debug` impl redaction:** `EmbeddedOAuthApp` has a custom `Debug` (`api/auth_embedded.rs:34-41`) that replaces `client_secret` with a placeholder. Pinned by test `embedded_oauth_app_debug_redacts_secret`.

### 2.2 Keychain layout (`src/api/auth.rs:18-32, 88-169`)

- **Service name:** `jr-jira-cli` (constant `DEFAULT_SERVICE_NAME`, `api/auth.rs:8`); overrideable via `JR_SERVICE_NAME` env (test isolation).
- **Shared (account-level, flat keys):** `email`, `api-token`, `oauth_client_id`, `oauth_client_secret`. Used across all profiles because:
  - Email/api-token: many users have one Atlassian account spanning multiple profiles.
  - OAuth app credentials: BYO users register one Developer Console app, used across profiles.
- **Per-profile (cloudId-scoped, namespaced):** `<profile>:oauth-access-token`, `<profile>:oauth-refresh-token`. Namespaced because tokens are tied to a specific cloudId (Jira site) — sharing them would cross-pollinate sessions across distinct Jira sites.
- **Legacy migration:** `oauth-access-token`, `oauth-refresh-token` (flat keys, pre-multi-profile). Read-only after migration, lazy-migrated on first read for the `"default"` profile **only** (`api/auth.rs:111-169`). Non-default profiles never inherit legacy keys (cross-pollination guard).
- **Backends:**
  - macOS: `apple-native` feature → Keychain (`Cargo.toml:23`).
  - Linux: `linux-native` feature → Secret Service (gnome-keyring/kwallet).
  - Windows: `keyring` 3 default → Credential Manager.
- **`read_keyring_optional` discriminates `NoEntry` from real backend failures** (`api/auth.rs:181-187`). Naive `.ok()` would collapse permission-denied / locked-keyring / platform-error into "no entry" and silently flip users onto the embedded fallback. This is an explicit security guard.

### 2.3 Transport (TLS)

- **`reqwest` features `["json", "rustls"]`** (`Cargo.toml:25`); `default-features = false` ensures no `native-tls` linkage.
- **`rustls`** uses webpki-roots / Mozilla CA bundle by default. **No FIPS** — rustls supports FIPS only via the `aws-lc-rs` feature, which is not enabled here.
- **No custom CA support** — there is no env var or config for adding additional roots. Atlassian on `*.atlassian.net` is the only target; no on-prem support.
- **HTTP redirects:** `reqwest 0.13` default redirect policy is **up to 10 redirects**, automatically followed. Not explicitly configured. `grep` for `redirect(` in source returned only OAuth callback redirect plumbing — no `Policy::none()` or `Policy::limited(N)` calls.
- **Cert validation:** rustls default — full validation. No `danger_accept_invalid_certs` calls anywhere in source.

### 2.4 OAuth flow security (`src/api/auth.rs:545-895`)

- **State parameter (CSRF):** 32 bytes from OS CSPRNG via `rand::rngs::OsRng.try_fill_bytes` (`api/auth.rs:882-895`), rendered as 64 hex chars (256 bits of entropy). `try_fill_bytes` is used (not panicking variant) because release `panic = "abort"` makes `unwrap()` a process kill — sandboxed/seccomp environments without `getrandom(2)` get a clean error message instead.
- **State verification:** `if returned_state != state { anyhow::bail!("State mismatch — possible CSRF attack"); }` (`api/auth.rs:588-590`).
- **PKCE (Proof Key for Code Exchange):** **NOT implemented.** `grep` for `code_verifier`, `code_challenge`, `S256`, `PKCE` returned no results. The flow is plain authorization-code with `client_secret` (RFC 6749 §4.1), not RFC 7636 PKCE. **Risk:** acceptable for a confidential client (the OAuth app has a `client_secret`, even if XOR'd). PKCE is recommended for public clients; an installed-app CLI is borderline. Adding PKCE would be defense-in-depth.
- **Callback URL:** `http://127.0.0.1:53682/callback` for embedded (literal IPv4); `http://localhost:<dynamic>/callback` for BYO. **Why literal `127.0.0.1`:**
  1. Atlassian validates `redirect_uri` by exact string match (no RFC 8252 normalization, per JRACLOUD-92180 reference in `api/auth.rs:486-488`).
  2. macOS/Chrome resolve `localhost` to `::1` (IPv6) before `127.0.0.1` (IPv4); the listener binds to `127.0.0.1` only, so an IPv6 connection would fail (`api/auth.rs:507-516`).
  3. Limits attack surface to the local user's loopback interface.
- **TOCTOU closure (`api/auth.rs:386-477`):** `RedirectUriStrategyRequest::bind()` returns a `ResolvedRedirect` that **owns the bound `TcpListener`** (private field). `oauth_login` consumes the listener directly via `into_parts()` instead of re-binding — eliminates the window where another local process could grab the fixed port between probe and accept.
- **Authorization code is single-use and short-lived** — error message at `api/auth.rs:577-580` correctly tells the user re-running `jr auth login --oauth` is safe.
- **Uniform percent-encoding** (`api/auth.rs:846-861`): all four dynamic params (`client_id`, `scopes`, `redirect_uri`, `state`) go through `urlencoding::encode` (RFC 3986: spaces → `%20`, not `+`). Defense-in-depth against a `client_id` containing `&`, `=`, `#`, or `?` reshaping the query string. Pinned because Atlassian's authorize endpoint requires `%20`, not `+`, for scope separators.
- **Single-shot listener:** OAuth callback reads exactly one HTTP request, writes one HTML response, drops (`api/auth.rs:572-604`). No persistent server.

### 2.5 Token lifecycle

- **Access token TTL:** Atlassian default is 1 hour (not enforced/checked client-side; relied upon).
- **Refresh token TTL:** Long-lived (Atlassian default ~90 days of inactivity). `offline_access` scope is in `DEFAULT_OAUTH_SCOPES` (`api/auth.rs:58-63`) — without it, refresh tokens are not issued.
- **Refresh token rotation:** Atlassian rotates refresh tokens on use (RFC 6749 §6 implementation). The `refresh_oauth_token` impl (`api/auth.rs:760-768`) writes both new `access_token` AND new `refresh_token` back to keychain. Partial-write surface explicitly: failure to save new tokens → user gets explicit error message naming the recovery path.
- **`refresh_oauth_token` has NO production callers** — the `pub` function exists for a future 401 auto-refresh integration (`api/auth.rs:700-703`). The user-facing `jr auth refresh` is a **clear-and-relogin** flow (deletes keychain entries + cache + re-runs login), NOT a refresh-token grant. Security implication: BYO users with rotated client secrets get a clean re-prompt rather than silent failure.
- **Refresh-side resolver** (`api/auth.rs:781-812`): `keychain → embedded` only. Flag and env are deliberately omitted because the refresh grant must use the same app that issued the refresh token. Keeping keychain ahead of embedded prevents a returning BYO user from silently flipping to the embedded app mid-session (which would invalidate their refresh token because it was issued by a different app).
- **"scope does not match" 401 → InsufficientScope** (`api/client.rs:337-348`): special-cased non-retryable error; user must re-authorize with new scopes via `jr auth login`. Pinned by BCs 015-018.

### 2.6 Credential redaction in logs/errors

- **`tracing` / `log` crates:** NOT in dependency tree (verified `Cargo.toml`). All diagnostic output is `eprintln!` / `println!`.
- **`--verbose` HTTP logging** (`api/client.rs:197-204, 274-279`) prints `[verbose] <METHOD> <URL>` and request body. **Bearer tokens are NOT in URLs** (Atlassian uses Authorization header, not query-param tokens). **Authorization header is NOT logged** — `try_clone()` happens after `.header("Authorization", ...)` is appended, but `r.method()` and `r.url()` are extracted without dumping headers. Verified by reading the full block.
- **Verbose body logging** (`api/client.rs:200-202, 276-278`): prints the request body via `String::from_utf8_lossy(bytes)`. **Risk:** if a future caller sends credentials in a JSON body (e.g., a `password` field on a hypothetical endpoint), they would be logged. Currently no such endpoint is called — OAuth token exchange happens in a separate `reqwest::Client::new()` instance that does NOT have verbose plumbing (`api/auth.rs:607-618` and `708-718`). The `client_id`/`client_secret` going to `auth.atlassian.com/oauth/token` ARE in the request body of those calls but are NOT logged because that client doesn't share `JiraClient`'s verbose path.
- **OAuth code/state in the local callback URL:** The local listener reads the raw HTTP request line (`api/auth.rs:574-581`). The URL containing `code=...&state=...` is in `request` but is NEVER printed — only `extract_query_param` parses it.
- **Error messages:** `extract_error_message` (`api/client.rs:448-490`) returns the Jira error body, which does not include credentials. Refresh token error (`api/auth.rs:725-744`) prints up to 500 chars of the response body — Atlassian's RFC 6749 error response is `{error, error_description}`, no token material.
- **`EmbeddedOAuthApp::Debug`** redacts `client_secret` (`api/auth_embedded.rs:34-41`).
- **Conclusion:** No credential leakage in logs/errors observed in the read paths. The verbose body-log path is a latent risk if future endpoints accept secret-bearing payloads.

### 2.7 Input handling

- **JQL escaping (`src/jql.rs:1-100`):** `escape_value` escapes backslash first, then double-quote (order is load-bearing — reversing allows escape neutralization). Property-tested by `escaped_value_never_has_unescaped_quote`. Prevents JQL injection via project names, summaries, status filters, asset keys, etc.
- **JQL relative-date validation:** `validate_duration` (`jql.rs:16-33`) — `<digits><single-unit>`, case-sensitive. Rejects combined units, garbage, empty.
- **Asset key validation:** `validate_asset_key` (`jql.rs:39-54`) — `<alnum>-<digits>` pattern. Prevents AQL injection via `--asset CUST-5; DROP TABLE` style payloads.
- **Date validation:** `validate_date` (`jql.rs:88-92`) — strict `YYYY-MM-DD` via `chrono::NaiveDate::parse_from_str`. Rejects feb-30, feb-29 in non-leap years.
- **AQL escaping:** `build_asset_clause` (`jql.rs:61-82`) runs `escape_value` on **both** the field name and the asset key. The wrapped expression is `Key = "<escaped_key>"`.
- **Profile name validation** (`config.rs:113-140`): `[A-Za-z0-9_-]{1,64}`, rejects Windows-reserved names (CON, NUL, AUX, PRN, COM1-9, LPT1-9). Validated at three boundaries:
  1. The `--profile` CLI flag (`main.rs:62-64`).
  2. Every key in `[profiles.*]` in TOML (`config.rs:274-282`).
  3. The resolved active-profile name (`config.rs:304`).
  Rationale: the resolved name flows into cache paths AND keyring keys; a bad value (`foo:bar`, `../../etc/passwd`) would corrupt those namespaces. Path-traversal prevention is the security driver.
- **Issue keys:** Bare `String` — no client-side validator beyond what Jira itself enforces. JQL composition wraps issue keys in escaped JQL string literals.
- **Filename / path inputs:**
  - `--description-file`: opened via standard `std::fs::read_to_string`. **No explicit path-traversal guard** — relies on OS file permissions. The user is the only one running `jr`, so traversal is "user can read their own files" — no privilege boundary.
  - `--description-stdin`: stdin via `BufRead`. No path involved.
- **No SQL anywhere** — no SQL injection class.

### 2.8 Build supply chain

- **`cargo-deny` configuration (`deny.toml`, 26 LOC):**
  - `[licenses]` allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, MPL-2.0, Unicode-3.0, Unicode-DFS-2016, CDLA-Permissive-2.0, OpenSSL, Zlib. `confidence-threshold = 0.8`.
  - `[bans] multiple-versions = "warn"` — duplicate transitive crate versions are flagged but **not blocking**. Surface area concern, not vulnerability concern.
  - `[bans] wildcards = "allow"` — wildcard version requirements permitted.
  - `[sources] unknown-registry = "warn"`, `unknown-git = "warn"` — non-crates.io sources are flagged but not blocked.
  - `[advisories] ignore = []` — no advisories are silenced.
- **CI integration** (`.github/workflows/ci.yml:47-52`): `EmbarkStudios/cargo-deny-action@v2` runs on every push to `main`/`develop` and every PR.
- **Dependabot** (`.github/dependabot.yml`): weekly updates for Cargo + GitHub Actions, max 5 open PRs each.
- **`Cargo.lock` committed:** Yes — required because this is a binary CLI; reproducible builds depend on the lockfile being canonical.
- **Direct deps:** 24 runtime + 6 dev (Pass 0 §1).
- **Transitive deps:** 332 packages in `Cargo.lock` (Pass 0 §1).
- **No SBOM generation in CI** — verified `.github/workflows/{ci,release}.yml` for `cargo-cyclonedx`, `cargo-sbom`, `syft`, `spdx`. None present.
- **MSRV verification in CI** (`.github/workflows/ci.yml:38-45`): `cargo check --all-features` against Rust 1.85.0 — pins MSRV claim from `Cargo.toml:7`.
- **Coverage in CI** (`ci.yml:54-69`): `cargo llvm-cov` → `lcov.info` → Codecov. Not an NFR per se, but a quality gate.

### 2.9 Build-time CI secrets

- `JR_BUILD_OAUTH_CLIENT_ID` / `JR_BUILD_OAUTH_CLIENT_SECRET` are GitHub Actions secrets (`release.yml:39-41`), passed as env at build time.
- For cross-compiled targets, `CROSS_CONTAINER_OPTS="-e JR_BUILD_OAUTH_CLIENT_ID -e JR_BUILD_OAUTH_CLIENT_SECRET"` propagates them into the container (`release.yml:43-46`).
- **Defense-in-depth presence check** (`release.yml:62-132`): post-build smoke verifies `embedded_oauth.rs` constants are populated and the binary's `auth status` reports `OAuth app: embedded`. Skipped on forks (legitimately produces unbranded BYO binaries) and on cross-compiled non-native targets (can't exec foreign binaries on the runner).
- **Fork release path** explicitly handled: a fork without secrets gets a working unbranded BYO binary, not a hard-failed release.

### 2.10 Release artifacts

- **SHA256 sums** generated for every tarball (`release.yml:55-60`) using `sha256sum` (Linux) or `shasum -a 256` (macOS).
- **Per-target tarballs:** `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`.
- **Auto-generated release notes:** `softprops/action-gh-release@v2` with `generate_release_notes: true` (`release.yml:152-158`).
- **Pre-release detection:** `prerelease: ${{ contains(github.ref_name, '-') }}` — tags like `v0.5.0-dev.7` are pre-releases.
- **No GPG/sigstore signing** of binaries.

---

## 3. Observability NFRs

### 3.1 `src/observability.rs` (39 LOC, full read)

The entire module:

```rust
// (paraphrased; see file)
pub(crate) fn log_parse_failure_once(
    flag: &AtomicBool, site: &str, iso: &str, verbose: bool,
) {
    if verbose && !flag.swap(true, Ordering::Relaxed) {
        eprintln!("[verbose] {site} timestamp failed to parse: {iso}");
    }
}
```

- **Visibility:** `pub(crate)` — NOT in the integration-test public surface. The single call site convention is a function-local `static AtomicBool`, so each parser fires at most one `[verbose]` line per process per site label.
- **Module docstring:** "Intentionally tiny: the project has no tracing/log crate, and a single `--verbose`-gated `eprintln!` is the established pattern (see `src/api/client.rs` for HTTP-request logging). Expand to a real tracing layer when there is cross-subsystem need."
- **Call sites:** `cli/issue/changelog.rs`, `cli/issue/format.rs` (per Pass 1 §3c), and `types/jira/issue.rs::team_id` (verbose warning on unexpected team object shape, per Pass 2 §2a.4).
- **Test:** `verbose_false_leaves_flag_untouched` — pins the short-circuit order (`verbose` check BEFORE `flag.swap`), preventing a non-verbose run from burning the gate flag and suppressing later verbose logs.

### 3.2 Logging convention

- **No `tracing` / `log` / `slog` / `env_logger`** anywhere in `Cargo.toml`. Verified by reading the full file.
- **Canonical:** `eprintln!` for stderr diagnostics, `println!` for stdout data.
- **`--verbose` flag:** Defined in `cli/mod.rs` (referenced from `JiraClient::from_config(_, verbose)` in `main.rs:80, 154, 173`). Default off.
- **No log levels** — verbose is binary on/off.
- **No log filtering by module / site** — the `log_parse_failure_once` flag is per-call-site only; no env-variable filtering like `RUST_LOG`.

### 3.3 Output channel separation (stdout vs stderr discipline)

- **Data → stdout** (`println!`, `output::print_output`, `output::render_table`, `output::render_json`).
- **Diagnostics, warnings, errors → stderr** (`eprintln!`, `output::print_success`, `output::print_warning`, `output::print_error`).
- **Pinned by `print_success/_warning/_error` going to stderr** (`output.rs:1-76` per Pass 1 §3b) so `--output json` keeps stdout clean for piping into `jq`.
- **Verbose diagnostics:** `[verbose]`-prefixed lines on stderr.
- **Rate-limit warning:** `warning: rate limited by Jira — gave up after 3 retries.` to stderr (`api/client.rs:233-237, 308-313`).
- **Error reporting in `main.rs:34-48`:**
  - `--output json`: `eprintln!("{}", json!({"error": ..., "code": ...}))` — structured error to **stderr**.
  - default: `eprintln!("Error: {e}")`.

### 3.4 `--output json` contract

- Global flag (`cli/mod.rs:47-51`, type `OutputFormat::{Table, Json}`).
- Write operations return structured JSON: `{"key": "FOO-123"}` for create (`cli/issue/json_output.rs`), `{"key", "status", "transitioned": bool}` for move, `{"key", "assignee", "changed": bool}` for assign, `{"id": <u64>, "self": <url>}` for remote-link (Pass 3 BC catalog).
- Error format: `{"error": <message>, "code": <exit_code>}` (`main.rs:36-44`).
- **No JSON streaming** — every JSON output is a fully-buffered single value rendered via `serde_json::to_string_pretty`.

### 3.5 `--no-color` and `NO_COLOR`

- `--no-color` flag in `cli/mod.rs`. Auto-honored from `NO_COLOR` env (`main.rs:13-15`):
  ```rust
  if cli.no_color || std::env::var("NO_COLOR").is_ok() {
      colored::control::set_override(false);
  }
  ```
- `colored 3` crate handles ANSI suppression. NO_COLOR env follows the no-color.org convention.

### 3.6 `--no-input` and TTY detection (`main.rs:17-23`)

```rust
if !cli.no_input {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        cli.no_input = true;
    }
}
```

- Auto-enables on non-TTY stdin (pipes, AI agents, CI scripts).
- Once `no_input = true`, all interactive prompts must have flag equivalents (CLAUDE.md convention).
- **AI-agent ergonomics:** documented in CLAUDE.md AI Agent Notes — every command can be driven non-interactively.

### 3.7 Error messages "always suggest what to do next"

CLAUDE.md convention (`Conventions` section). Verified samples:
- `JrError::ConfigError("Profile {:?} has no URL configured. Run \"jr auth login --profile {}\".")` (`api/client.rs:54-58`).
- `JrError::ConfigError("Cloud ID not configured. Run \"jr init\" to set up your instance.")` (`api/client.rs:391-396`).
- OAuth partial-state error (`api/auth.rs:675-683`): names the recovery action including the URL to manage Atlassian connected apps.
- `auth refresh` against unconfigured profile: stderr names "no URL configured" + `jr auth login --url` (BC-011).
- "OAuth keychain entries for profile X are partial ... Run `jr auth logout` then `jr auth login`" (`api/auth.rs:161-166`).

### 3.8 Exit codes (`src/error.rs:1-62`)

| Variant | Code | Convention |
|---|---:|---|
| Success | 0 | Default |
| Generic / Internal / Network / API / Http / Io / Json | 1 | Catch-all |
| `NotAuthenticated`, `InsufficientScope` | 2 | Auth |
| `UserError` | 64 | EX_USAGE (sysexits.h) — bad CLI input |
| `ConfigError` | 78 | EX_CONFIG (sysexits.h) — config issue |
| `Interrupted` | 130 | SIGINT (128 + 2) |

The 64 / 78 / 130 mappings follow `<sysexits.h>` and POSIX conventions — scriptable detection of error class without parsing stderr.

---

## 4. Reliability NFRs

### 4.1 Rate limit handling (`src/api/client.rs:184-253, 265-320`, `src/api/rate_limit.rs:1-30`)

- **Constants:** `MAX_RETRIES = 3` (`client.rs:11`), `DEFAULT_RETRY_SECS = 1` (`client.rs:14`).
- **Detection:** `response.status() == StatusCode::TOO_MANY_REQUESTS`.
- **Retry-After parsing:** `RateLimitInfo::from_headers` (`rate_limit.rs:14-29`):
  - `Retry-After`: integer seconds via `trim().parse::<u64>()`. **Does NOT support HTTP-date format** — Atlassian uses integer seconds, but if they ever switched the format silently, jr would fall back to `DEFAULT_RETRY_SECS = 1`.
  - `X-RateLimit-Remaining`: parsed but **only stored, never enforced** — diagnostic only.
- **Backoff:** **Constant** (no exponential, no jitter) — `tokio::time::sleep(Duration::from_secs(delay))` where `delay = retry_after.unwrap_or(1)`. The "no jitter" choice is acceptable for a single-user CLI (no thundering-herd risk; one client at a time).
- **Two retry paths:**
  - `send()` parses 4xx/5xx → `JrError`. After exhausting retries on 429, returns `JrError::ApiError { status: 429, ... }`.
  - `send_raw()` returns the raw `Response` regardless of status (used by `jr api` raw passthrough). Same retry loop on 429, but final response is returned to caller without error parsing.
- **User-visible warning** after exhausted retries: `warning: rate limited by Jira — gave up after 3 retries. Wait a moment and try again.` to stderr (`client.rs:233-237, 308-313`).
- **`try_clone()` panic:** `request.try_clone().expect("request should be cloneable (JSON body)")` (`client.rs:191-193`). Unreachable in practice because the client only sends JSON or no-body — both are cloneable.
- **`send_raw` body-drop discipline:** explicitly `drop(response)` before sleep on 429 (`client.rs:303`) so the body socket isn't held open across the wait.

### 4.2 401 / token refresh

- **Detection** (`client.rs:330-348`): `parse_error` reads body first, then dispatches:
  - 401 + body matches `scope does not match` (case-insensitive ASCII) → `JrError::InsufficientScope` (exit 2, no retry, user must re-authorize with new scopes).
  - 401 otherwise → `JrError::NotAuthenticated` (exit 2).
  - Non-401 4xx/5xx → `JrError::ApiError`.
- **Auto-refresh on 401:** **NOT implemented in production paths.** `refresh_oauth_token` (`api/auth.rs:704-770`) exists but has no production callers (commented at `auth.rs:700-703`). Users hitting 401 must run `jr auth refresh` manually.
- **Session expiry behavior:** when an OAuth access token expires, Jira returns 401 → user gets `Not authenticated. Run "jr auth login"`. The user has to retry. This is a UX rough edge but a deliberate one — Pass 1 §8 deviation #4 documented this.

### 4.3 Idempotency (state-changing commands exit 0 if already in target state)

- **`issue move <key> [status]`** (`cli/issue/workflow.rs:192-224` per Pass 2 §2b.1, BCs in Pass 3): fetches transitions + current status; if `current == target`, prints success and exits 0 without HTTP write. Mechanism: read-then-compare-then-write-or-skip.
- **`issue assign <key> --to/--account-id`**: checks current assignee before write (CLAUDE.md convention, BC in Pass 3).
- **`auth logout`** (per Pass 2 §2b.1): second call is no-op (entries already deleted).
- **`auth switch <name>`**: idempotent (writes the same `default_profile` value).

### 4.4 Cancellation / Ctrl+C (`src/main.rs:73, 261-267`)

```rust
let main_task = async { /* ... */ };
tokio::select! {
    result = main_task => result,
    _ = tokio::signal::ctrl_c() => {
        eprintln!("\nInterrupted");
        std::process::exit(130);
    }
}
```

- **Abrupt cancellation by `process::exit(130)`** — no `Drop` cleanup, no graceful shutdown.
- **In-flight HTTP request:** `reqwest`/`hyper` future is dropped, TCP connection drops when process exits.
- **OAuth callback listener:** `TcpListener` future is dropped; user gets "tab can't connect".
- **429 retry sleep:** `tokio::time::sleep` future cancels cleanly.
- **Cache writes:** non-atomic; mid-write Ctrl+C can leave a partial file. Self-healing via cache miss policy (corruption → `Ok(None)`).
- **Risk window: Ctrl+C during OAuth refresh** (Pass 1 §7 risk #7). The actual partial-state surface is between `auth.atlassian.com/oauth/token` (`auth.rs:710-718`) returning rotated tokens AND `store_oauth_tokens` (`auth.rs:760-768`) committing them to keychain. Ctrl+C in this window means: Atlassian rotated → old refresh token now invalid → jr's keychain still has the old one → next request gets 401. **Recovery:** `jr auth refresh` (clear-and-relogin) — same path the explicit error message at `auth.rs:760-768` already directs the user to.

### 4.5 Cache failure modes

- **NotFound** → `Ok(None)` (cache miss, refetch).
- **Deserialization failure** (corrupt or schema-changed) → stderr warning + `Ok(None)`. The user sees `warning: cache file <name> unreadable; will refetch` and the command proceeds.
- **Expired** (`(now - fetched_at).num_days() >= 7`) → `Ok(None)`.
- **`project_meta.json` corruption** (`cache.rs:158-163`): map-cache gets a more pointed warning that "starting fresh — other cached projects will be lost." The map is rebuilt fresh; only the affected project's metadata is lost.
- **TeamCache corruption pinned by Pass 3 H-005** (BC in Pass 3 — read-only handler tolerates a corrupt teams.json by issuing a UUID + refresh hint inline, never panics).

### 4.6 Filesystem failure

- **Cache directory creation:** `fs::create_dir_all(&dir)` (`cache.rs:38, 151`) — handles both first-write and concurrent creation.
- **Config file write:** `Config::save_global` (Pass 1 §3k) uses a file-only baseline (no env overlay) so transient `JR_*` vars can't leak into migrated `config.toml`. Save is direct `fs::write`, not atomic temp-file-rename.
- **Malformed config TOML** (Pass 3 BC-012): exits 78 (`ConfigError`); on-disk file unchanged (no silent overwrite). Pre-fix bug: `unwrap_or_default()` swallowed parse errors and `save_global()` overwrote with defaults. Issue #258 regression — pinned.

### 4.7 Network failure

- **DNS / connect failure:** `reqwest::Error` → mapped to `JrError::NetworkError(host)` (`client.rs:208-214, 283-289`). Host extracted from `e.url().and_then(...)`. Exit code 1.
- **Timeout:** `reqwest`'s timeout error also maps through the same `Err(e)` arm → `NetworkError`. Exit 1.
- **TLS handshake failure:** `reqwest::Error` → `JrError::Http(reqwest::Error)` (the `#[from]` transparent variant) → exit 1.

### 4.8 Test reliability

- **`wiremock 0.6`** — HTTP fixture server; `JR_BASE_URL` env injects mock URL into `JiraClient::from_config` at `client.rs:37-65`.
- **`insta 1`** — snapshot tests with `.snap` files (17 snapshots per Pass 0); deterministic.
- **`proptest 1`** — property tests on JQL escaping, partial_match, duration parsing. Regression corpus in `proptest-regressions/jql.txt`.
- **`#[ignore]`-gated tests:**
  - `JR_RUN_KEYRING_TESTS=1` → 8 keyring round-trip tests (Linux CI may lack secret-service).
  - `JR_RUN_OAUTH_INTEGRATION=1` → 1 deferred embedded-OAuth integration test.
  - 13 total `#[ignore]` attrs (Pass 0 §9).
- **`tempfile 3`** + per-test `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` overrides — full filesystem isolation per test.
- **`JR_SERVICE_NAME`** override scopes keychain access in tests (`api/auth.rs:14-16`).

---

## 5. Scalability NFRs

### 5.1 Topology

- **Single static binary**, no daemon, no IPC, no helper processes (Pass 1 §5).
- **No background workers**, no queue consumers, no long-running tasks.

### 5.2 Memory footprint

- **Issue lists:** Fully buffered before output. `client.search_issues` returns `Vec<Issue>` (`api/jira/issues.rs:42-95` per Pass 1 §1c). Pagination loops accumulate into a single Vec and only render once paginating completes. **Implication:** for very large `--all` results, memory grows linearly.
- **JSON output:** Fully buffered via `serde_json::to_string_pretty` (`output.rs`). No streaming.
- **Asset enrichment N+1 risk:** Per-row enrichment in `cli/issue/list.rs` issues one `client.get_assets(workspace_id, "object/{key}")` per linked asset per issue. For a list of 50 issues with 3 CMDB fields each, this can be 150+ extra GETs — all serial, all fully buffered. **Mitigation:** the CMDB fields cache (7-day TTL) avoids re-discovery; the per-asset GETs are not cached.

### 5.3 Concurrent profile use

- **Different profiles, simultaneous `jr` invocations:** SAFE for cache (per-profile dirs, no shared file). Keychain writes are per-key and atomic at the OS keychain layer (Apple Keychain / Secret Service / Windows Credential Manager all serialize internally).
- **Same profile, simultaneous `jr` invocations:**
  - **Cache write race:** Two writers can clobber each other on `teams.json`, `project_meta.json`, etc. (non-atomic `fs::write`). Last-write-wins; the cache miss policy makes a torn write self-healing on next read.
  - **Keychain race during refresh:** `jr auth refresh` deletes then writes keys. Two simultaneous refreshes can interleave; the `partial state` recovery branch in `load_oauth_tokens` (`auth.rs:150-167`) handles this gracefully.
  - **OAuth callback listener race:** Two simultaneous `jr auth login --oauth` invocations both try to `bind 127.0.0.1:53682`; the second gets `EADDRINUSE`, surfaces friendly "port in use" error (`auth.rs:437-443`) suggesting BYO override.
- **`tempfile`-based test isolation** confirms the per-profile dir convention is sufficient for parallel `cargo test`.

### 5.4 Issue volume

- **Cursor pagination on `/search/jql`** scales to arbitrary issue counts; the practical limit is memory (Vec buffering all results).
- **`--limit N` vs `--all`:** `--all` removes the limit; `resolve_effective_limit` returns `None` (`cli/mod.rs:740` per Pass 2 §2a.3). Server-side max-per-page is 100; client paginates as many times as needed.
- **`MAX_SPRINT_ISSUES = 50`** (`cli/sprint.rs:107`): hard cap on a single `sprint add` / `sprint remove` call. The Atlassian Agile API rejects payloads larger than 50 issues — encoded as a client-side guard with a friendly error before the network call.
- **No batching for `jr issue create`** when called in a loop — each invocation creates one issue. Bulk creation is a documented future-feature gap (not in spec).

### 5.5 Future expansion

- **`api/confluence/` directory placeholder** — does not yet exist, but the `api/{jira,jsm,assets}/` namespacing pattern (Pass 1 §6.1) is sibling-ready.
- **OAuth scope set** is configurable per-profile (`oauth_scopes` in `ProfileConfig`) — adding Confluence scopes (e.g., `read:content:confluence`) is a config change.
- **`Cli::Command` enum** (`cli/mod.rs:54-133`) is already structured to accept new top-level commands without churning existing handlers.

---

## 6. Configuration values encoding NFR decisions

| Constant / config key | Value | Where defined | NFR dimension | Rationale |
|---|---|---|---|---|
| `MAX_RETRIES` | `3` | `api/client.rs:11` | Reliability | Bounds latency vs success rate on 429. Worst-case: 3 retries × Retry-After seconds. |
| `DEFAULT_RETRY_SECS` | `1` | `api/client.rs:14` | Reliability | Fallback when 429 response lacks `Retry-After` header. |
| HTTP client timeout | `30s` | `api/client.rs:84` | Performance/Reliability | Cap per-request latency. Applied to JiraClient only — NOT inherited by OAuth `Client::new()` instances. |
| `MAX_SPRINT_ISSUES` | `50` | `cli/sprint.rs:107` | Scalability/Reliability | Atlassian Agile API per-call limit on sprint add/remove. |
| `EMBEDDED_CALLBACK_PORT` | `53682` | `api/auth.rs:384` | Security | Fixed to match Developer Console registration. Changing is a breaking release. |
| `DEFAULT_LIMIT` | `30` | `cli/mod.rs:740` | UX/Performance | Default page size for list commands. Capped to keep responses snappy. |
| `CACHE_TTL_DAYS` | `7` | `cache.rs:7` | Performance | Balance freshness vs API cost. Custom-field discovery is slow; 7-day staleness is acceptable. |
| Cache root version | `v1` | `cache.rs:76-77` | Reliability/Forward-compat | Future schema-bump path: bump to `v2/` to orphan stale files cleanly. |
| `DEFAULT_SERVICE_NAME` | `"jr-jira-cli"` | `api/auth.rs:8` | Security | Keychain service name. Overrideable via `JR_SERVICE_NAME` for test isolation. |
| `DEFAULT_OAUTH_SCOPES` | 7-scope string | `api/auth.rs:58-63` | Security | Pinned scope set including `offline_access`. Embedded app must match exactly or `invalid_scope`. |
| State entropy | `32 bytes (256 bits)` | `api/auth.rs:884` | Security | OS CSPRNG. Replaces a previous wall-clock-nanosecond impl (~30 bits). |
| `lto = "thin"` | release | `Cargo.toml:49` | Performance/Size | Cross-crate LTO; faster link than `fat`. |
| `codegen-units = 1` | release | `Cargo.toml:50` | Performance | Single CGU per crate; max inlining. |
| `strip = true` | release | `Cargo.toml:51` | Security/Size | No debug symbols in shipped binary; defense-in-depth for XOR'd OAuth secret. |
| `panic = "abort"` | release | `Cargo.toml:52` | Reliability/Size | No unwinding code; smaller binary. Forces non-panicking error paths (e.g., `try_fill_bytes`). |
| `opt-level = 3` | release | `Cargo.toml:48` | Performance | Max runtime optimization. |
| TLS implementation | `rustls` | `Cargo.toml:25` | Security/Reliability | No native-tls dep; webpki-roots Mozilla CA bundle. ADR-0003. |
| `multiple-versions` | `"warn"` | `deny.toml:21` | Supply-chain | Surface area noted, not blocking. |
| `unknown-registry`, `unknown-git` | `"warn"` | `deny.toml:25-26` | Supply-chain | Non-crates.io sources flagged. |
| MSRV | `1.85` | `Cargo.toml:7` | Reliability/Compat | Pinned by CI `cargo check` against 1.85.0. |
| Tokio runtime | `multi_thread` (full features) | `Cargo.toml:29` + `main.rs:9` | Performance | Default for `#[tokio::main]`. |
| Reqwest features | `["json", "rustls"]`, `default-features = false` | `Cargo.toml:25` | Security | No native-tls leak. |
| `JR_BUILD_OAUTH_CLIENT_ID/_SECRET` | CI-only env | `release.yml:39-41`, `build.rs:21-29` | Security | Build-time secrets; build.rs emits XOR'd constants if present, else `None`. |
| `confidence-threshold` | `0.8` | `deny.toml:18` | Supply-chain | License detection confidence floor. |
| Dependabot interval | `weekly` | `dependabot.yml:5,11` | Supply-chain | Cadence of dependency PRs (max 5 open each ecosystem). |
| Cross-compile target list | 4 targets | `release.yml:15-25` | Distribution | x86_64+aarch64 for both macOS and Linux. |
| OAuth token endpoint | `auth.atlassian.com/oauth/token` | `api/auth.rs:609, 710` | Security | Atlassian IdP. Hard-coded; no override. |
| OAuth resource discovery | `api.atlassian.com/oauth/token/accessible-resources` | `api/auth.rs:643` | Security | cloudId enumeration after token grant. |

---

## 7. NFR Gaps & Risks

Honest list — substantive items only.

### 7.1 Performance gaps

1. **OAuth token-endpoint clients lack the 30s timeout** (`api/auth.rs:607, 708`). The `JiraClient` is configured with `Client::builder().timeout(Duration::from_secs(30))` but the OAuth flows construct fresh `reqwest::Client::new()` with no timeout. A hung `auth.atlassian.com` connection would block `jr auth login` indefinitely. **Severity:** medium (rare in practice, but non-zero).
2. **Asset enrichment is N+1 serial** — no concurrent fan-out, no batching. For lists with many CMDB fields × many issues, latency scales linearly. **Severity:** medium for power users.
3. **No upper bound on `Retry-After`** (`api/client.rs:219, 294`). A misbehaving proxy returning `Retry-After: 86400` causes the client to sleep up to 24h × 3 attempts. **Severity:** low (Atlassian is unlikely to send this; defense-in-depth missing).
4. **No HTTP-date format support in Retry-After** (`api/rate_limit.rs:17-18`). RFC 7231 allows both integer and date formats. Atlassian uses integer; if they switched, jr would silently fall back to 1s. **Severity:** low.
5. **Memory: full-buffer rendering** of large lists. `--all` on a project with 100k issues OOMs the client. **Severity:** low (no real users hit this; documented as gap for spec phase).

### 7.2 Security gaps

6. **No PKCE in OAuth flow.** RFC 7636 not implemented. Acceptable for confidential-client-with-secret threat model, but defense-in-depth missing. **Severity:** low-medium (depends on threat model).
7. **No SBOM in CI** — verified `.github/workflows/{ci,release}.yml`; no `cargo-cyclonedx`, `syft`, or SPDX generation. For a tool that handles OAuth tokens, SBOM publication helps downstream security teams. **Severity:** low-medium.
8. **No release binary signing** (no GPG, no sigstore/cosign). SHA256 sums alone don't authenticate provenance. **Severity:** medium (mitigated by GitHub Releases trust + release workflow being the only producer).
9. **Verbose body logging** (`api/client.rs:200-202, 276-278`) prints request bodies. Currently safe (no secret-bearing request bodies on the JiraClient path), but a future endpoint accepting credentials in JSON would leak under `--verbose`. **Severity:** low (latent).
10. **No FIPS-validated TLS** (rustls without `aws-lc-rs` feature). Affects users in FIPS-required environments. **Severity:** very low (CLI for Atlassian Cloud, not for FedRAMP workloads).
11. **`reqwest` default redirect policy is "up to 10 redirects"** — not explicitly limited or disabled. A malicious proxy could redirect Authorization-bearing requests; reqwest does strip Authorization on cross-origin redirects, but the policy is implicit, not asserted. **Severity:** low.
12. **`cargo-deny multiple-versions = "warn"`** — duplicate transitive crate versions don't fail the build. With 332 transitive deps, the surface area is non-trivial. **Severity:** low-medium (standard for Rust ecosystem).

### 7.3 Reliability gaps

13. **Ctrl+C during OAuth refresh has no rollback** (Pass 1 risk #7 confirmed). The risk window is between Atlassian rotating tokens and `store_oauth_tokens` writing them. Recovery is manual `jr auth refresh`. **Severity:** low (recoverable).
14. **Cache writes are not atomic** (`fs::write` not via temp-rename). A torn write produces a corrupt JSON file; the cache miss policy auto-recovers on next read, but a concurrent reader during the partial write sees corruption. **Severity:** very low (single-user CLI, brief window).
15. **No 401 auto-refresh in production paths.** `refresh_oauth_token` exists but has no callers. Users hitting expired access tokens see `Not authenticated. Run "jr auth login"` and must run `jr auth refresh` manually. **Severity:** medium (UX, not correctness).
16. **`config.toml` save not atomic** — direct `fs::write`. Crash mid-write can leave a partial config. Pass 3 BC-012 covers parse-tolerance, but doesn't address mid-write corruption. **Severity:** very low.

### 7.4 Observability gaps

17. **`observability.rs` is 39 LOC** — no `tracing`, no log levels, no structured spans. For a tool that talks to a complex remote API, this constrains in-the-wild debugging. The module docstring explicitly defers this to "when there is cross-subsystem need." **Severity:** low for users, medium for AI-agent integrators wanting parseable execution traces.
18. **No request-id / correlation-id propagation.** `--verbose` shows method+URL+body but no Atlassian `x-arequestid` or similar. A failed request can't be cross-referenced with Atlassian support tickets without re-running with packet capture. **Severity:** low-medium.
19. **No metrics emission** (Prometheus, OTel, statsd). Acceptable for a single-user CLI; flagged for completeness. **Severity:** very low.
20. **`--verbose` is binary; no per-module filter.** Once on, it's on for everything. **Severity:** very low.

### 7.5 Scalability gaps

21. **Same-profile concurrent invocations have no file locking.** Two `jr issue list --all` runs on the same profile can race on cache writes. **Severity:** very low (last-write-wins; cache miss self-heals).
22. **No bulk-create command** for issues. `jr issue create` is single-issue per call. **Severity:** low (out of v1 scope).
23. **No streaming output for very large lists.** `--all` on 100k+ issues OOMs. **Severity:** very low (synthetic; no real workflow).

---

## 8. State Checkpoint

```yaml
pass: 4
status: complete
nfr_dimensions: 5
config_values_cataloged: 27
nfr_gaps_identified: 23
files_examined: 18
timestamp: 2026-05-04T00:00:00Z
next_pass: 5
inputs_consumed:
  - .factory/semport/jira-cli/jira-cli-pass-0-inventory.md
  - .factory/semport/jira-cli/jira-cli-pass-1-architecture.md
  - .factory/semport/jira-cli/jira-cli-pass-2-domain-model.md (head)
  - .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md (head)
  - .reference/jira-cli/CLAUDE.md
  - .reference/jira-cli/Cargo.toml
  - .reference/jira-cli/deny.toml
  - .reference/jira-cli/.github/workflows/ci.yml
  - .reference/jira-cli/.github/workflows/release.yml
  - .reference/jira-cli/.github/dependabot.yml
  - .reference/jira-cli/src/main.rs
  - .reference/jira-cli/src/api/client.rs
  - .reference/jira-cli/src/api/rate_limit.rs
  - .reference/jira-cli/src/api/auth.rs (heads + critical sections; full file is 1397 LOC)
  - .reference/jira-cli/src/cache.rs (head)
  - .reference/jira-cli/src/observability.rs
  - .reference/jira-cli/src/jql.rs (head)
  - .reference/jira-cli/src/cli/sprint.rs (MAX_SPRINT_ISSUES section)
verification_methods:
  - file_line_citations: 80+
  - constants_verified: 27
  - workflow_files_read: 3
  - cargo_lock_inspected_indirectly: yes (Pass 0 reported 332 transitive)
  - grep_for_pkce: returned no results
  - grep_for_redirect_policy: only OAuth callback plumbing
  - grep_for_unsafe: found in expected places (cache.rs FFI, auth.rs OAuth, build.rs)
```
