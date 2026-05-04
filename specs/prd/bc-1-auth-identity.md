---
context: bc-1
title: "Auth & Identity"
total_bcs: 57   # cumulative claim (incl. range-collapsed); definitional_count below is individually-bodied headings
definitional_count: 46   # count of `#### BC-` headings in this file
last_updated: 2026-05-04
source_pass: 3
trace: |
  - L2: .factory/specs/domain-spec/bc-01-auth-identity.md
  - Source broad: .factory/semport/jira-cli/jira-cli-pass-3-behavioral-contracts.md §2.1
  - Source R1: .factory/semport/jira-cli/jira-cli-pass-3-deep-r1.md §3.1
  - Source R4: .factory/semport/jira-cli/jira-cli-pass-3-deep-r4.md §3.8-3.9
---

# BC-1 — Auth & Identity

57 behavioral contracts across 5 subdomains: OAuth flow (1.1), Profile management (1.2),
Embedded OAuth app (1.3), Token keychain (1.4), OAuth state machine (1.5).

---

## Subdomains

### 1.1 OAuth Flow & Profile Resolution

#### BC-1.1.001: `auth list` against fresh-install returns empty JSON array

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:53-60`
**Subject**: Auth & Identity
**Behavior**: When no `~/.config/jr/config.toml` exists (or no `[profiles.*]` keys), `jr auth list --output json` exits 0 and stdout is `[]`.
**Effects**: stdout = `[]`, exit 0, no HTTP, no keychain access.
**Edge cases**: fresh install with no config file at all.
**Error taxonomy**: none.
**Trace**: Pass 3 BC-001; L2 E-01-01

---

#### BC-1.1.002: `auth status` against fresh install exits 0 with helpful stderr

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:62-75`
**Subject**: Auth & Identity
**Behavior**: `jr auth status` against an uninitialized config exits 0 and prints `No profiles configured` to stderr. Supports first-run probes by setup scripts/CI.
**Edge cases**: no config.toml; no `[profiles]` section.
**Error taxonomy**: none — intentionally success.
**Trace**: Pass 3 BC-002

---

#### BC-1.1.003: `auth switch <unknown>` exits 64

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:42-50`
**Subject**: Auth & Identity
**Behavior**: Switching to an unknown profile exits 64 (`UserError`) with no config mutation.
**Error taxonomy**: `JrError::UserError` (exit 64).
**Trace**: Pass 3 BC-003

---

#### BC-1.1.004: `auth status --profile <unknown>` exits 64 with "unknown profile"

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:78-96`
**Subject**: Auth & Identity
**Behavior**: Explicit `--profile` flag naming absent profile → exit 64; stderr contains `unknown profile`.
**Error taxonomy**: `JrError::UserError`.
**Trace**: Pass 3 BC-004

---

#### BC-1.1.005: `auth logout --profile <unknown>` exits 64

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:98-118`
**Subject**: Auth & Identity
**Behavior**: Logout against unknown profile exits 64 with `unknown profile` in stderr.
**Error taxonomy**: `JrError::UserError`.
**Trace**: Pass 3 BC-005

---

#### BC-1.1.006: `auth remove <active>` is rejected with exit 64

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:120-140`
**Subject**: Auth & Identity
**Behavior**: Removing the currently-active profile exits 64 with stderr `cannot remove active`. No file changes, no keychain deletion.
**Error taxonomy**: `JrError::UserError`.
**Trace**: Pass 3 BC-006

---

#### BC-1.1.007: Profile resolution precedence: flag > JR_PROFILE env > config.default_profile > "default"

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:142-186`; `src/config.rs:95-110`
**Subject**: Auth & Identity
**Behavior**: `Config::load_with(cli_profile)` resolves active profile via precedence chain. Test populates three profiles (from-config / from-env / from-flag) — flag wins. `Config.active_profile_name` set accordingly.
**Effects**: `auth list --output json` returns exactly one element with `"active": true`.
**Trace**: Pass 3 BC-007

---

#### BC-1.1.008: Global `--profile` flag propagates to `auth status` via main.rs composition

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:193-231`
**Subject**: Auth & Identity
**Behavior**: `jr --profile sandbox auth status` (no subcommand-level `--profile`) targets sandbox. main.rs composes effective profile via `subcmd.profile.or(cli.profile)`.
**Effects**: stderr/stdout reflect sandbox URL/name.
**Trace**: Pass 3 BC-008 → superseded by BC-030 (R1)

---

#### BC-1.1.009: `auth login --profile <new>` creates profile even when profile doesn't yet exist

**Confidence**: HIGH (`#[ignore]`-gated by JR_RUN_KEYRING_TESTS)
**Source**: `tests/auth_profiles.rs:241-280`
**Subject**: Auth & Identity
**Behavior**: Login uses lenient config load (skips strict active-profile-existence check), then writes `[profiles.NEW]` with URL + auth_method.
**Effects**: writes config, writes shared `email`/`api-token` keychain keys.
**Trace**: Pass 3 BC-009

---

#### BC-1.1.010: `auth login --profile X` succeeds even when JR_PROFILE points to absent profile

**Confidence**: HIGH (`#[ignore]`-gated)
**Source**: `tests/auth_profiles.rs:290-332`
**Subject**: Auth & Identity
**Behavior**: Login uses lenient load throughout — top-level + internal reloads in login_token/login_oauth. `JR_PROFILE=ghost` doesn't abort creation of a different profile.
**Trace**: Pass 3 BC-010 → refined by BC-029 (R1)

---

#### BC-1.1.011: `auth refresh --no-input` against unconfigured profile exits 64 naming "no URL configured"

**Confidence**: HIGH
**Source**: `tests/auth_refresh.rs:43-106`
**Subject**: Auth & Identity
**Behavior**: With `--no-input` AND no profile URL configured, refresh exits 64 with stderr matching `no URL configured` + `jr auth login` + `--url`. Critically: stderr does NOT contain `panic`. Credentials NOT cleared on failure.
**Error taxonomy**: `JrError::UserError`.
**Trace**: Pass 3 BC-011 → refined by BC-025 (R1)

---

#### BC-1.1.012: Malformed config TOML errors exit 78 and does NOT overwrite the file

**Confidence**: HIGH
**Source**: `tests/auth_login_config_errors.rs:18-97`
**Subject**: Auth & Identity
**Behavior**: When `~/.config/jr/config.toml` is malformed, `auth login --oauth ...` exits 78. Stderr contains `toml` or `parse`. The on-disk file is byte-identical to before (no silent overwrite). This is BC-1139 from Pass 3.
**Error taxonomy**: `JrError::ConfigError` (exit 78).
**Trace**: Pass 3 BC-012; BC-1139 (R4 tightened)

---

### 1.2 Profile Lifecycle Management

#### BC-1.2.013: `auth logout` deletes only `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token`

**Confidence**: HIGH (PROMOTED from MEDIUM in R1)
**Source**: `src/api/auth.rs:24-32, 88-97`; `src/cli/auth.rs::handle_logout`
**Subject**: Auth & Identity
**Behavior**: Deletes `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token` via `delete_credential`. Profile config entry preserved. Shared keys (`email`, `api-token`, `oauth_client_id`, `oauth_client_secret`) untouched. Re-login uses preserved API-token/OAuth credentials.
**Trace**: Pass 3 BC-013-R

---

#### BC-1.2.014: `auth remove <name>` performs three-step delete: config entry, OAuth tokens, cache directory

**Confidence**: HIGH (PROMOTED from MEDIUM in R1)
**Source**: `src/cli/auth.rs::handle_remove`; `src/cache.rs:82-88`; `tests/auth_profiles.rs:120-140`
**Subject**: Auth & Identity
**Behavior**: Three-step: (1) remove `[profiles.<name>]` from config, (2) delete `<name>:oauth-*` keychain keys, (3) `cache::clear_profile_cache(name)` removes `~/.cache/jr/v1/<name>/`. Step (3) is no-op if dir absent. All three are best-effort; partial state does not cascade. Errors if name == active (exit 64 first).
**Trace**: Pass 3 BC-014-R

---

#### BC-1.2.015: `auth refresh --help` includes the `--oauth` flag

**Confidence**: HIGH
**Source**: `tests/auth_refresh.rs:7-24`
**Subject**: Auth & Identity
**Behavior**: `jr auth refresh --help` exits 0; stdout contains both `refresh` and `--oauth`.
**Trace**: Pass 3 BC-026 (R1)

---

#### BC-1.2.016: `auth refresh --oauth --help` is accepted in either flag order

**Confidence**: HIGH
**Source**: `tests/auth_refresh.rs:26-40`
**Subject**: Auth & Identity
**Behavior**: clap accepts both `--oauth --help` and `--help --oauth`, exit 0.
**Trace**: Pass 3 BC-027 (R1)

---

#### BC-1.2.017: `auth login --profile X` against `JR_PROFILE=ghost` succeeds creating profile X

**Confidence**: HIGH (`#[ignore]`-gated)
**Source**: `tests/auth_profiles.rs:282-333`
**Subject**: Auth & Identity
**Behavior**: Round-5 regression fix. Both internal reloads in login flow use `load_lenient_with`. Test sets `JR_PROFILE=ghost`, runs `jr auth login --profile fresh --url https://fresh.example`, asserts `[profiles.fresh]` written.
**Trace**: Pass 3 BC-029 (R1)

---

#### BC-1.2.018: Global `--profile` propagates to all auth subcommands via subcmd.profile.or(cli.profile)

**Confidence**: HIGH
**Source**: `tests/auth_profiles.rs:188-231`
**Subject**: Auth & Identity
**Behavior**: Round-10 regression fix. main.rs now composes `subcmd.profile.or(cli.profile)`.
**Trace**: Pass 3 BC-030 (R1)

---

### 1.3 Embedded OAuth App

#### BC-1.3.019: Embedded OAuth app `Debug` redacts client_secret

**Confidence**: HIGH
**Source**: `src/api/auth_embedded.rs:34, 220-239`
**Subject**: Auth & Identity
**Behavior**: `format!("{:?}", EmbeddedOAuthApp{...})` never emits plaintext secret. Custom Debug impl substitutes `<redacted>`. This is BC-1168 from Pass 3 R4.
**Trace**: Pass 3 BC-019; BC-1168 (R4)

---

#### BC-1.3.020: Build with empty XOR inputs → `embedded_oauth_app()` returns None

**Confidence**: HIGH
**Source**: `src/api/auth_embedded.rs:100-106`
**Subject**: Auth & Identity
**Behavior**: Setting `JR_BUILD_OAUTH_CLIENT_ID=""` at build time → binary returns `None` from embedded accessor. BYO/prompt fallback proceeds.
**Trace**: Pass 3 BC-020

---

#### BC-1.3.021: `embedded_oauth_app_present()` checks presence without decoding

**Confidence**: HIGH
**Source**: `src/api/auth_embedded.rs:132-136`
**Subject**: Auth & Identity
**Behavior**: Presence check inspects only `EMBEDDED_ID.is_some_and(|s| !s.is_empty())`. Does NOT invoke `decode()`. Used by `auth status` to report `OAuthAppSource::Embedded` without materializing plaintext.
**Trace**: Pass 3 BC-021; BC-022-R (R1)

---

#### BC-1.3.022: `OAuthAppSource` resolution chain: Flag > Env > Keychain > Embedded > Prompt > None

**Confidence**: HIGH (PROMOTED from MEDIUM in R1)
**Source**: `src/api/auth_embedded.rs:46-57`; `src/cli/auth.rs::peek_oauth_app_source`
**Subject**: Auth & Identity
**Behavior**: First non-None-equivalent source wins; lower-priority sources never short-circuit higher. `auth status` reports source via this chain.
**Trace**: Pass 3 BC-022-R

---

#### BC-1.3.023: DEFAULT_OAUTH_SCOPES includes `offline_access`, CMDB scopes, and `write:jira-work`

**Confidence**: HIGH
**Source**: `src/api/auth.rs:34-63`
**Subject**: Auth & Identity
**Behavior**: Scope string: `read:jira-work write:jira-work read:jira-user read:servicedesk-request read:cmdb-object:jira read:cmdb-schema:jira offline_access`. Regression test asserts no double spaces.
**Effects**: Embedded `jr` app must be registered with exactly this scope set in Developer Console.
**Trace**: Pass 3 BC-035 (R1)

---

#### BC-1.3.024: Embedded OAuth integration test is `#[ignore]`-gated and stubs `unimplemented!()`

**Confidence**: HIGH
**Source**: `tests/oauth_embedded_login.rs:13-32`
**Subject**: Auth & Identity
**Behavior**: Test intentionally `unimplemented!()` when `JR_RUN_OAUTH_INTEGRATION=1`. Without that env var, test early-returns. Guards against false coverage signals.
**Trace**: Pass 3 BC-028 (R1)

---

### 1.4 Token Keychain Layout

#### BC-1.4.025: `default` profile lazy-migrates legacy flat OAuth keys; non-default profiles never inherit

**Confidence**: HIGH (PROMOTED from MEDIUM in R1)
**Source**: `src/api/auth.rs:111-169`
**Subject**: Auth & Identity
**Behavior**: `load_oauth_tokens(profile)`: if both namespaced keys present → return. If both missing → ONLY `"default"` reads legacy flat keys, copies to namespaced, deletes legacy. Non-default profiles error on partial state with actionable message. Two `if profile == "default"` guards at lines 124 and 151.
**Trace**: Pass 3 BC-023-R

---

#### BC-1.4.026: `refresh_oauth_token` signature is `(profile: &str)` only — resolves credentials internally

**Confidence**: HIGH (PROMOTED from LOW in R1)
**Source**: `src/api/auth.rs:700-770`; CLAUDE.md
**Subject**: Auth & Identity
**Behavior**: Function takes only `profile: &str`. Internally resolves keychain → embedded. No production callers as of v0.5.0-dev.7 — exists for future 401 auto-refresh. Re-introducing `client_id/_secret` would break embedded-OAuth path.
**Trace**: Pass 3 BC-024-R

---

#### BC-1.4.027: Per-profile keychain keys: `<profile>:oauth-access-token` / `<profile>:oauth-refresh-token`

**Confidence**: HIGH
**Source**: `src/api/auth.rs:24-32`
**Subject**: Auth & Identity
**Behavior**: All OAuth token storage/retrieval uses namespaced keys. Shared keys (`email`, `api-token`, `oauth_client_id`, `oauth_client_secret`) are NOT namespaced.
**Trace**: Pass 3 BC-1153 (R4)

---

#### BC-1.4.028: `load_oauth_tokens` errors on PARTIAL state (one token present, other missing)

**Confidence**: HIGH
**Source**: `src/api/auth.rs:1249-1269`
**Subject**: Auth & Identity
**Behavior**: Access-token without refresh-token (or vice versa) → `Err`. Prevents silent half-credential use.
**Trace**: Pass 3 BC-1156 (R4)

---

#### BC-1.4.029: `load_oauth_tokens("sandbox")` does NOT inherit legacy flat keys

**Confidence**: HIGH
**Source**: `src/api/auth.rs:1323-1341`
**Subject**: Auth & Identity
**Behavior**: Lazy migration is `default`-profile-only by design. "sandbox" only reads `sandbox:oauth-*` namespaced keys.
**Trace**: Pass 3 BC-1158 (R4)

---

#### BC-1.4.030: `resolve_refresh_app_credentials` prefers KEYCHAIN over EMBEDDED

**Confidence**: HIGH
**Source**: `src/api/auth.rs:1347-1357`
**Subject**: Auth & Identity
**Behavior**: BYO user does NOT silently flip onto embedded mid-session. Keychain wins.
**Trace**: Pass 3 BC-1159 (R4)

---

### 1.5 OAuth State Machine

#### BC-1.5.031: Embedded OAuth callback URL is exactly `http://127.0.0.1:53682/callback`

**Confidence**: HIGH
**Source**: `src/api/auth.rs:374-477`; CLAUDE.md; ADR-0006
**Subject**: Auth & Identity
**Behavior**: `EMBEDDED_CALLBACK_PORT: u16 = 53682`. IPv4 literal `127.0.0.1` (NOT `localhost` — avoids macOS/Chrome `localhost`→`::1` resolver pitfall). Atlassian validates `redirect_uri` by EXACT string match. Changing this is a breaking release.
**Trace**: Pass 3 BC-031 (R1); BC-1140/1141 (R4)

---

#### BC-1.5.032: `RedirectUriStrategyRequest::Fixed(p)` produces EADDRINUSE friendly error

**Confidence**: HIGH
**Source**: `src/api/auth.rs:427-447`
**Subject**: Auth & Identity
**Behavior**: On port-in-use: `"port {p} is in use; the jr OAuth callback needs this port. Set --client-id/--client-secret (or set JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET) to fall back to a dynamic port."` Contains 5 substrings: `port 53682 is in use`, `the jr OAuth callback needs this port`, `--client-id/--client-secret`, `JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET`, `dynamic port`.
**Trace**: Pass 3 BC-032 (R1); BC-1161 (R4)

---

#### BC-1.5.033: `ResolvedRedirect` private fields prevent listener detachment from strategy

**Confidence**: HIGH
**Source**: `src/api/auth.rs:455-477`
**Subject**: Auth & Identity
**Behavior**: Type-system-enforced TOCTOU-closure. Caller cannot move listener out and derive a redirect_uri from strategy that no longer matches.
**Trace**: Pass 3 BC-033 (R1)

---

#### BC-1.5.034: BYO OAuth uses `DynamicPort` (dynamic `:0`); embedded uses `FixedPort(53682)`

**Confidence**: HIGH
**Source**: `src/api/auth.rs:927-937`
**Subject**: Auth & Identity
**Behavior**: `RedirectUriStrategy::FixedPort(53682).redirect_uri() == "http://127.0.0.1:53682/callback"` (IPv4). `DynamicPort(54321).redirect_uri() == "http://localhost:54321/callback"` (localhost). The two literals differ; Atlassian validates by exact match.
**Trace**: Pass 3 BC-1140 (R4)

---

#### BC-1.5.035: `generate_state()` produces 32 bytes from OsRng encoded as 64 hex chars

**Confidence**: HIGH
**Source**: `src/api/auth.rs:882`; Pass 3 R4 §3.10
**Subject**: Auth & Identity
**Behavior**: CSRF state token generation. State is validated at callback step.
**Trace**: Pass 3 BC-1146 (R4)

---

#### BC-1.5.036: OAuth flow has NO PKCE (`code_challenge`/`code_verifier` absent)

**Confidence**: HIGH
**Source**: `src/api/auth.rs:608-616`
**Subject**: Auth & Identity
**Behavior**: `build_authorize_url` does not include PKCE parameters. NFR-S-A (MEDIUM): defense-in-depth gap per RFC 8252. Documented as POLICY-DECISION.
**Trace**: Pass 3 BC-1148, BC-1149 (R4)

---

#### BC-1.5.037: `build_authorize_url` percent-encodes hostile `client_id` containing injection chars

**Confidence**: HIGH
**Source**: `src/api/auth.rs:1043-1060`
**Subject**: Auth & Identity
**Behavior**: `client_id` containing `&redirect_uri=evil.example#frag` → output has `client_id=real_id%26redirect_uri%3Devil.example%23frag` and MUST NOT contain `&redirect_uri=evil.example`.
**Trace**: Pass 3 BC-1149 (R4); Top-30 BC rank #2

---

#### BC-1.5.038: `accessible_resources` first-wins for cloud_id discovery (silent first-only)

**Confidence**: HIGH
**Source**: Pass 3 R4 §3.10; `src/api/auth.rs`
**Subject**: Auth & Identity
**Behavior**: After token exchange, `accessible_resources.first()` is used for cloud_id. No prompt if multiple sites — first is silently used (NEW-INV-179).
**Trace**: Pass 3 BC-1176 (R4)

---

#### BC-1.5.039: OAuth token stored as `<profile>:oauth-access-token` and `<profile>:oauth-refresh-token` post-login

**Confidence**: HIGH
**Source**: `src/api/auth.rs` (post-exchange persistence)
**Subject**: Auth & Identity
**Behavior**: Tokens are namespaced to profile. Profile config written post-storage.
**Trace**: Pass 3 BC-1151 (R4)

---

#### BC-1.5.040: OAuth callback validates state (CSRF check) before token exchange

**Confidence**: HIGH
**Source**: `src/api/auth.rs:898`; Pass 3 R4 §3.10
**Subject**: Auth & Identity
**Behavior**: State mismatch → abort with error; keychain NOT touched.
**Trace**: Pass 3 H-047 (holdout)

---

#### BC-1.5.041: `extract_query_param` parses `code` and `state` from HTTP GET request line

**Confidence**: HIGH
**Source**: `src/api/auth.rs:948-965`
**Subject**: Auth & Identity
**Behavior**: `extract_query_param("GET /callback?code=abc123&state=xyz HTTP/1.1\r\n", "code")` → `Some("abc123")`. Missing param → `None`. No query string → `None`.
**Trace**: Pass 3 BC-1142, BC-1143, BC-1144 (R4)

---

### 1.6 Auth Error Handling & 401 Dispatch

#### BC-1.6.042: 401 + `scope does not match` body → InsufficientScope with 5 required substrings

**Confidence**: HIGH
**Source**: `tests/api_client.rs:99-144`
**Subject**: Auth & Identity
**Behavior**: 401 body containing `scope does not match` (case-insensitive) → `JrError::InsufficientScope`. Display MUST contain: `Insufficient token scope`, raw gateway message, `write:jira-work`, `OAuth 2.0`, `github.com/Zious11/jira-cli/issues/185`. Exit code 2.
**Trace**: Pass 3 BC-015; BC-1085 (R4); Top-30 BC rank #1

---

#### BC-1.6.043: 401 without scope-mismatch substring → NotAuthenticated, NOT InsufficientScope

**Confidence**: HIGH
**Source**: `tests/api_client.rs:146-181`
**Subject**: Auth & Identity
**Behavior**: 401 with `Session expired` body → `Not authenticated`. MUST NOT contain `Insufficient token scope`.
**Trace**: Pass 3 BC-016; BC-1086 (R4)

---

#### BC-1.6.044: 401 scope-mismatch match is case-insensitive (`to_ascii_lowercase`)

**Confidence**: HIGH
**Source**: `tests/api_client.rs:183-216`
**Subject**: Auth & Identity
**Behavior**: `"Unauthorized; Scope Does Not Match"` (mixed case) → InsufficientScope.
**Trace**: Pass 3 BC-017; BC-1087 (R4)

---

#### BC-1.6.045: Non-401 status with scope-mismatch substring does NOT dispatch to InsufficientScope

**Confidence**: HIGH
**Source**: `tests/api_client.rs:219-255`
**Subject**: Auth & Identity
**Behavior**: 403 with `scope does not match policy` → `API error (403)`, NOT InsufficientScope. Status gate prevents broadening.
**Trace**: Pass 3 BC-018; BC-1088 (R4)

---

#### BC-1.6.046: `auth list` table snapshot: 4 columns, active profile with `* ` prefix

**Confidence**: HIGH
**Source**: `src/cli/snapshots/jr__cli__auth__tests__list_table_snapshot.snap`
**Subject**: Auth & Identity
**Behavior**: Columns: `NAME, URL, AUTH, STATUS`. Active profile prefixed `* ` (asterisk-space). Inactive: `  ` (2 spaces). 3-profile fixture: default* (api_token), sandbox (oauth), staging (api_token). All STATUS cells `configured`.
**Trace**: Pass 3 BC-1115 (R4)

---

## Summary Stats

| Subdomain | BCs | Confidence |
|-----------|-----|-----------|
| 1.1 OAuth Flow & Profile Resolution | 12 | All HIGH |
| 1.2 Profile Lifecycle Management | 6 | All HIGH |
| 1.3 Embedded OAuth App | 6 | All HIGH |
| 1.4 Token Keychain Layout | 6 | All HIGH |
| 1.5 OAuth State Machine | 11 | All HIGH |
| 1.6 Auth Error Handling & 401 Dispatch | 5 | All HIGH |
| **Total** | **46** | **46 HIGH** |

Note: 57 total BCs including 11 additional from R4 (BC-1140..1178 subset) incorporated inline above. The complete pass-3 BC mapping is in BC-INDEX.md.
