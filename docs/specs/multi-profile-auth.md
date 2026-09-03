# Multi-Profile Authentication

## Goal

Let `jr` target multiple Atlassian Cloud sites (production, sandbox, additional teams' Jira instances) from one local install, with one `jr auth switch <profile>` command to flip between them. Reuse a single classic Atlassian API token across profiles where possible (account-level credentials authenticate the same user against any Atlassian site), but keep per-site OAuth tokens isolated (cloudId-scoped, not transferable).

## Motivation

Today, `GlobalConfig.instance` holds a single URL + cloud_id + auth_method, and the keyring stores one flat set of credentials. To work against a sandbox, a user has to re-run `jr init` and overwrite their prod config — there's no two-environments-per-team workflow.

The classic API token is account-level by Atlassian's design — the same `email + token` pair authenticates against `acme.atlassian.net` and `acme-sandbox.atlassian.net` for the same user. OAuth tokens, in contrast, are issued against a specific cloudId and don't transfer. The design must reflect both realities: shared API token, per-profile OAuth.

The blast radius is small: only 5 source files (`api/auth.rs`, `api/client.rs`, `cli/auth.rs`, `cli/init.rs`, `cli/team.rs`) and `config.rs` read `config.global.instance.*` today.

## Scope

- **In scope:** Multi-profile config schema, named-profile keyring layout, per-profile cache directory, `jr auth login/switch/list/status/logout/remove/refresh` CLI surface, auto-migration of legacy single-instance configs, gated keyring round-trip tests.
- **Out of scope:** A `jr profile` subcommand tree separate from `jr auth`; profile renaming (use `login + remove`); per-repo `.jr.toml` profile pinning (use `direnv` with `JR_PROFILE`); a `KeyringProvider` trait abstraction (file as follow-up issue); making `Config::save_global` atomic via tempfile + rename (existing limitation, file as follow-up issue).

## Validation Summary

Design decisions validated against industry conventions via Perplexity:

| Decision | Convention validated |
|---|---|
| Classic API token reusable across Atlassian sites | Confirmed — Atlassian docs explicitly support this |
| Shared API token + per-profile OAuth tokens | Mirrors kubectl users (shared) + gh hosts (per-host) |
| Auto-migrate legacy `[instance]` block on first load | Matches kubectl, npm, gh, cargo conventions for non-breaking schema changes |
| Inline `default_profile` in `config.toml` | Matches kubectl's `current-context:` (gcloud's separate `active_config` file is the alternative; kubectl pattern fits jr's single-file shape better) |
| Keyring: single service + namespaced keys (`<profile>:oauth-*`) | `:` is safe across macOS Keychain, libsecret, Windows Credential Manager; matches the keyring-rs `Entry::new(service, user)` API shape |
| Per-profile cache subdirectory | kubectl `~/.kube/cache/discovery/<host_port>/` pattern, empirically confirmed |
| Versioned cache root (`~/.cache/jr/v1/`) | Matches pip, Cargo, npm — orphan old files via path versioning, no marker files |
| Lazy/opportunistic OAuth-token migration | Tools typically don't auto-migrate keyring entries; read-fallback is the convention |
| TOML migration upfront, keyring migration lazy | Cost-benefit differs: TOML lazy = perpetual two-schema read; keyring lazy = single-fallback in load_oauth_tokens |
| `[profiles.<name>]` table-of-tables in TOML | Idiomatic (matches Flyway), maps to `BTreeMap<String, ProfileConfig>` cleanly in serde |
| Per-profile field IDs (`team_field_id`, `story_points_field_id`) | AWS-style full duplication — fields are site-scoped, drift would be a correctness bug |
| `jr auth` consolidated lifecycle (no `jr profile` subtree) | gh-style; lower surface area; profile data is auth-adjacent in jr |
| Keyring testing via real backend + `JR_SERVICE_NAME` | Pragmatic: existing pattern; trait abstraction is a separate refactor |

## Config Schema

New `config.toml` shape:

```toml
default_profile = "default"

[profiles.default]
url = "https://acme.atlassian.net"
auth_method = "api_token"          # "api_token" | "oauth"
cloud_id = "abc-123"               # optional for api_token, required for oauth
org_id = "def-456"                 # optional, used for team queries
oauth_scopes = "..."               # optional, used for oauth
team_field_id = "customfield_10001"
story_points_field_id = "customfield_10002"

[profiles.sandbox]
url = "https://acme-sandbox.atlassian.net"
auth_method = "oauth"
cloud_id = "xyz-789"
oauth_scopes = "read:jira-work write:jira-work offline_access"

[defaults]
output = "table"
```

Rust types:

```rust
pub struct GlobalConfig {
    pub default_profile: Option<String>,
    pub profiles: BTreeMap<String, ProfileConfig>,   // BTreeMap for deterministic `jr auth list`
    pub defaults: DefaultsConfig,
}

pub struct ProfileConfig {
    pub url: Option<String>,
    pub auth_method: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub oauth_scopes: Option<String>,
    pub team_field_id: Option<String>,
    pub story_points_field_id: Option<String>,
    pub project: Option<String>,
    pub env: Option<String>,   // free-form label, e.g. "prod"/"sandbox" (S-cycle3-env-tag)
}
```

`team_field_id` and `story_points_field_id` are per-profile because they're Jira-site-scoped (custom field IDs can differ between sites). Sandbox/prod are usually clones with identical IDs, but silent drift would be a correctness bug. AWS-style full per-profile duplication; `defaults.output` stays global (genuine user preference, site-agnostic).

## Active-Profile Resolution

Precedence (highest wins):

1. `--profile <NAME>` CLI flag (global, sibling of `--output`, `--project`, `--no-input`, `--no-color`)
2. `JR_PROFILE` env var
3. `default_profile` field in `config.toml`
4. Literal name `"default"` if none of the above set

`Config::load()` resolves this once at startup. Result lives as `Config::active_profile_name: String`. The active profile is reached through two accessors plus a load-time validation:

- `Config::active_profile() -> ProfileConfig` — returns an owned `ProfileConfig`,
  cloning the active profile entry from the map. Falls back to
  `ProfileConfig::default()` if the active profile name isn't in the map (this
  branch is reachable only for in-memory `Config` values built directly in tests
  — `Config::load()` already errors with `UserError` for the load-time case).
- `Config::active_profile_or_err() -> anyhow::Result<&ProfileConfig>` — strict
  variant that errors with `JrError::ConfigError` when the active profile
  isn't in the map. Used by callers that want to fail loudly rather than
  silently returning a default.
- `Config::load()` validates that the resolved active profile name (when
  `[profiles]` is non-empty) exists in the map, and returns
  `JrError::UserError` (exit 64) if not. The error message lists the known
  profile names, matching the `switch`/`remove`/`logout`/`status` handlers'
  wording.

## Profile Name Validation

Allowed: `[A-Za-z0-9_-]{1,64}`. Validation lives in `config::validate_profile_name(name) -> Result<(), JrError>` so the rule is single-sourced — every entry point (CLI, config-load migration) calls it.

Two validation layers:

1. **Character set + length**: regex above. Rejects empty strings, whitespace, `:`, `/`, `.`, and other shell/path metacharacters. The `:` rejection guarantees keyring key parsing remains unambiguous; the `/` and `.` rejections keep cache subdirectory paths clean.

2. **Windows reserved names** (case-insensitive): `CON`, `NUL`, `AUX`, `PRN`, `COM1`–`COM9`, `LPT1`–`LPT9`. Profile names matching these (with or without an extension) are rejected on every platform — even on macOS and Linux where they'd technically work — so configs stay portable across machines. Without this, a `CON` profile created on macOS would fail on Windows when the cache subdir was created.

Error message: `invalid profile name "<name>"; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved Windows names (CON, NUL, AUX, PRN, COM1-9, LPT1-9) excluded`.

## Keyring Layout

**Shipped model (ADR-0020; DEC-315/325/326; BC-1.4.031/BC-1.4.027) — supersedes the flat/shared layout originally proposed below in this section's initial draft.** Single service name (`jr-jira-cli`, honoring `JR_SERVICE_NAME` for tests). Credentials are namespaced **per profile** except the OAuth app registration itself:

| Key | Scope | Notes |
|---|---|---|
| `<profile>:email` | Per-profile | User's Atlassian account email for that profile (S-cycle3-percred-storage) |
| `<profile>:api-token` | Per-profile | Classic API token for that profile — NOT shared across profiles, even though the underlying Atlassian token is account-level; each profile stores its own copy |
| `oauth_client_id` | Shared | OAuth app registered once per Atlassian org (account-level, flat key) |
| `oauth_client_secret` | Shared | Same |
| `<profile>:oauth-access-token` | Per-profile | OAuth tokens are cloudId-scoped |
| `<profile>:oauth-refresh-token` | Per-profile | Same |

Only `oauth_client_id`/`oauth_client_secret` remain flat, account-level keychain keys. `email`/`api-token` are namespaced per profile just like the OAuth token pair — there is no shared/global credential entry for either.

### Public API (`src/api/auth.rs`)

```rust
// Shared (account-level, flat keys)
pub fn store_oauth_app_credentials(client_id: &str, client_secret: &str) -> Result<()>
pub fn load_oauth_app_credentials() -> Result<(String, String)>

// Per-profile (all credential read/write paths take `profile: &str`)
pub fn store_api_token(profile: &str, email: &str, token: &str) -> Result<()>
pub fn load_api_token(profile: &str) -> Result<(String, String)>
pub fn store_oauth_tokens(profile: &str, access: &str, refresh: &str) -> Result<()>
pub fn load_oauth_tokens(profile: &str) -> Result<(String, String)>

// Clear helpers
pub fn clear_profile_oauth_pair(profile: &str) -> Result<()>     // OAuth keys for one profile only (used by `auth logout` on an oauth-method profile)
pub fn clear_profile_creds(profile: &str) -> Result<()>          // BOTH the OAuth pair AND the <profile>:email/<profile>:api-token pair for one profile (used by `auth remove`)
```

`load_api_token` deliberately has **no legacy-flat-key fallback** for any profile, including `"default"` — see the Migration section's "(4) Keyring API-token credentials" for the no-copy detect-and-instruct contract this implies. This is a difference from the OAuth token pair, whose `"default"` profile still lazy-migrates legacy flat keys on first read (Migration section, "(2)").

### `:` Separator Safety

`:` is documented-safe across all three backends (macOS Keychain `kSecAttrAccount` accepts arbitrary CFStrings; libsecret attributes are arbitrary string-string; Windows Credential Manager target names already use `:` internally as a legacy delimiter). Profile-name validation rejects `:` so collisions are impossible.

## Cache Layout

Versioned root, per-profile subdirectory:

```
~/.cache/jr/
├── v1/
│   ├── default/
│   │   ├── teams.json
│   │   ├── project_meta.json
│   │   ├── workspace.json
│   │   ├── cmdb_fields.json
│   │   ├── object_type_attrs.json
│   │   └── resolutions.json
│   └── sandbox/
│       └── ...
└── (legacy flat *.json files, never read by new code)
```

```rust
pub fn cache_root() -> PathBuf { ... }                     // ~/.cache/jr
pub fn cache_dir(profile: &str) -> PathBuf {               // ~/.cache/jr/v1/<profile>
    cache_root().join("v1").join(profile)
}
```

All six cache reader/writer pairs gain `profile: &str` as the first arg:

```rust
pub fn read_team_cache(profile: &str) -> Result<Option<TeamCache>>
pub fn write_team_cache(profile: &str, teams: &[CachedTeam]) -> Result<()>
pub fn read_project_meta(profile: &str, project_key: &str) -> Result<Option<ProjectMeta>>
pub fn write_project_meta(profile: &str, project_key: &str, meta: &ProjectMeta) -> Result<()>
// ... same shape for workspace, cmdb_fields, object_type_attrs, resolutions
```

Callers pass `config.active_profile_name()`. Every call site already has `&Config` in scope.

`clear_profile_cache(profile: &str)` is `std::fs::remove_dir_all(cache_dir(profile))`. `clear_all_caches()` removes `~/.cache/jr/v1/` (preserves any future v2 sibling).

### Legacy Cache Handling

Old `~/.cache/jr/*.json` files are never read by the new code (they live above `v1/`, outside the new path). They expire by their existing 7-day TTL or the user can `rm` them manually. **No migration code, no warning, no marker file** — versioned root is self-sufficient.

## CLI Surface

### New global flag (on `Cli`)

```
--profile <NAME>     Override the active profile for this invocation.
                     Precedence: this flag > JR_PROFILE env >
                                 default_profile in config > "default".
```

### `jr auth` subcommands

**Shipped behavior (DEC-313/321/322/323/324/325/326/327; S-663-1) — this subsection reflects the current CLI, not the original proposal below it in this doc's earlier draft.**

```
jr auth login [--profile NAME] [--url URL] [--oauth] [--api-token] [--no-input]
    Log in (creates profile if absent). --profile defaults to active.
    --url required when creating a new profile under --no-input;
        in interactive mode, jr prompts for the URL.
    --url on an EXISTING profile is allowed and transparently updates that
        profile's URL (e.g., user moved sites, or wants to change cloud_id
        via re-discovery). Passing --url is itself the explicit confirmation
        of intent — no separate prompt — so agents and scripts that pass
        --url get a deterministic write without an interactive gate.
    Defaults to OAuth at profile creation in INTERACTIVE mode only
        (DEC-313/327) — the auth-method picker (a TTY-only prompt) defaults
        its selection to OAuth, but the user can still choose API Token.
        Under --no-input or a non-TTY stdin, a brand-new profile with
        neither --oauth nor --api-token passed is created as an
        api_token-method profile instead — the interactive OAuth default
        does not extend to non-interactive creation. --api-token (DEC-323)
        explicitly selects the classic API-token flow in either mode.
        --oauth is DEPRECATED (accepted indefinitely, no hard removal) —
        passing it still works and selects the OAuth flow, but it is
        redundant with the interactive default; prefer omitting it, or use
        --api-token when you want the non-default flow.
    --oauth (or --api-token) on an existing profile whose auth_method
        differs switches that profile's mechanism transparently (relogin-
        then-replace — DEC-321/BC-1.2.051 — the new credential is obtained
        and confirmed usable FIRST, and only then does it replace the
        stored one; a failed switch leaves the existing credential intact).
    Each profile stores its own `<profile>:email`/`<profile>:api-token`
        pair — does not reuse another profile's stored token.

jr auth switch <NAME>
    Positional-only (S-663-1) — `--profile` is rejected on this subcommand
        (exit 64); only the positional <NAME> selects the switch target.
    Set default_profile in config.toml to NAME. Errors on unknown profile.
    No credential prompts.

jr auth list
    Show all configured profiles. Mark active with `*`.
    Table columns: NAME | URL | ENV | AUTH | STATUS    (DEC-324 — ENV
        inserted between URL and AUTH; ENV is a free-form label such as
        prod/sandbox/uat, or `-` when unset)
    STATUS ∈ {configured, unset}
    JSON: [{"name", "url", "env", "auth_method", "status", "active"}]

jr auth status [--profile NAME]
    Show one profile's auth state (default: active). Human text only —
        no --output json support for this subcommand.

jr auth logout [--profile NAME]
    Session-clear only, non-destructive (DEC-322, BC-1.2.013/BC-1.2.014):
      • oauth-method profile: clears only that profile's OAuth session
        tokens (<profile>:oauth-access-token / <profile>:oauth-refresh-
        token). Profile entry and identity in config.toml stay in place.
      • api-token-method profile: no OAuth session exists to clear — logout
        emits an informational, non-error stderr notice ("This profile uses
        API-token auth — nothing to log out; use `jr auth remove <profile>`
        to delete stored credentials.") and exits 0. The api-token pair is
        left untouched.
    `jr auth remove` is the full delete for either mechanism — see below.

jr auth remove <NAME>
    Delete the profile entirely:
      • BOTH the OAuth pair AND the <NAME>:email/<NAME>:api-token pair for
        that profile in keyring (whichever the profile actually used)
      • profile entry in config.toml
      • cache subdirectory ~/.cache/jr/v1/<NAME>/
    Only the shared, account-level oauth_client_id/oauth_client_secret are
        NEVER touched — other profiles may use them. To clear those, manage
        them via the OS keychain UI directly (out of scope for this
        feature; tracked as a follow-up).
    Errors if NAME == default_profile (must `jr auth switch` first).
    Errors if NAME doesn't exist.
    Confirmation prompt unless --no-input.

jr auth refresh [--profile NAME] [--oauth] [--api-token] [--email/--token/--client-id/--client-secret]
    Refresh (re-obtain) credentials for the named profile (defaults to
        active). DEC-321: the flow is ALWAYS selected from the target
        profile's own stored auth_method — `--oauth`/`--api-token` on
        `refresh` are INERT with respect to flow selection; they do NOT
        force or override the path (unlike `jr auth login`, where they do
        select/switch the mechanism). Passing either flag on `refresh`
        only emits a stderr, human-mode-only notice (deprecation notice for
        --oauth; inert-on-refresh notice for --api-token) — changing a
        profile's auth mechanism is done via an explicit `jr auth login
        <profile> [--oauth|--api-token]` re-declaration, not via refresh.
    Ordering is relogin-then-replace (DEC-321, BC-1.2.051 Invariant 2):
        the new credential value is obtained and confirmed usable FIRST;
        only then is the existing stored value overwritten. A refresh that
        fails to obtain a usable replacement (network error, cancelled
        interactive re-prompt, EOF on stdin) leaves the profile's existing
        credentials completely untouched — no prior clear/delete step.
      • api_token flow (per the profile's own auth_method): re-prompts via
        flag → env → TTY, then overwrites that profile's namespaced
        <profile>:email/<profile>:api-token pair.
      • oauth flow (per the profile's own auth_method): re-runs the FULL
        3LO browser flow (oauth_login), not the silent refresh_token grant,
        then overwrites that profile's <profile>:oauth-access-token/
        <profile>:oauth-refresh-token pair.
    Per-profile token isolation: refreshing profile X never touches
        another profile's stored credentials, of either kind.
```

### `jr init` interaction

If run on a config that already has any profile configured, prompt: `"Profiles configured: <list>. Add another? [y/N]"`. If no, exit early. If yes, prompt for new profile name and run the existing `jr init` flow against that new profile. Replaces the current "init silently overwrites whatever is there" behavior.

## Migration

Three migration domains; each handled differently per its constraints.

### (1) `config.toml` — auto, one-time, in `Config::load()`

Trigger: `[instance]` block exists AND `default_profile` field absent AND `[profiles]` map empty.

```rust
// Pseudocode in Config::load(), after toml deserialization:
if legacy_shape_detected {
    let old_instance = config.legacy_instance.take().expect("checked above");
    let mut profile = ProfileConfig::from(old_instance);
    profile.team_field_id = config.legacy_fields.team_field_id.take();
    profile.story_points_field_id = config.legacy_fields.story_points_field_id.take();
    config.profiles.insert("default".to_string(), profile);
    config.default_profile = "default".to_string();
    config.save_global()?;
    eprintln!(
        "Migrated config to multi-profile layout (single profile \"default\"). \
         Run 'jr auth list' to view profiles."
    );
}
```

Idempotent (trigger condition is false after first run). Failure handling matches existing `Config::save_global` semantics — out of scope to make atomic in this feature.

### (2) Keyring OAuth tokens — lazy, on first read

Old flat `oauth-access-token` / `oauth-refresh-token` keys are read on first miss-then-fall-back inside `load_oauth_tokens`:

```rust
pub fn load_oauth_tokens(profile: &str) -> Result<(String, String)> {
    let access_key = format!("{profile}:oauth-access-token");
    let refresh_key = format!("{profile}:oauth-refresh-token");
    if let (Ok(a), Ok(r)) = (entry(&access_key)?.get_password(), entry(&refresh_key)?.get_password()) {
        return Ok((a, r));
    }
    if profile == "default" {
        if let (Ok(a), Ok(r)) = (entry("oauth-access-token")?.get_password(), entry("oauth-refresh-token")?.get_password()) {
            // Opportunistic migration: copy to new keys, best-effort delete legacy
            store_oauth_tokens("default", &a, &r)?;
            let _ = entry("oauth-access-token")?.delete_credential();
            let _ = entry("oauth-refresh-token")?.delete_credential();
            return Ok((a, r));
        }
    }
    Err(JrError::NotAuthenticated.into())
}
```

Properties: invisible to user, idempotent (second call sees new keys), failure-safe (partial migration leaves legacy keys readable on next attempt).

### (3) Cache — none, by versioned root

Already covered: legacy flat files live at `~/.cache/jr/*.json`, never touched by the new code paths in `~/.cache/jr/v1/<profile>/`.

### (4) Keyring API-token credentials — no auto-migration, detect-and-instruct

Unlike (2) above, `email`/`api-token` deliberately does **not** get an
opportunistic copy-then-delete migration for any profile, including
`"default"` (S-cycle3-percred-storage, BC-1.4.031 Invariant 2). This was
revisited once — an initial design considered mirroring the OAuth
copy-then-delete shape for the API-token pair too — and rejected by a human
decision (**DEC-326, no-copy detect-and-instruct**): the legacy flat
`email`/`api-token` pair is a shared, environment-unbound Basic-auth
credential, and silently handing it to a freshly-tagged profile (which may
point at a different Jira site) was judged unsafe. Instead,
`load_api_token(profile)` (`src/api/auth.rs`) fails loudly and instructs:

```rust
pub fn load_api_token(profile: &str) -> Result<(String, String)> {
    let email = read_keyring_optional(&api_token_email_key(profile))?;
    let token = read_keyring_optional(&api_token_key(profile))?;

    match (email, token) {
        (Some(e), Some(t)) => Ok((e, t)),
        (None, None) => {
            // Existence-only check (never read as a credential, never
            // copied, never deleted) — kept purely to mirror (2)'s
            // migration-detection step; does not change the error below.
            let _legacy_pair_present = legacy_flat_pair_exists()?;
            Err(JrError::UserError(format!(
                "No credentials stored for profile '{profile}'. This version of jr \
                 requires per-profile credentials — run `jr auth login {profile}` to set them up."
            )).into())
        }
        _ => Err(JrError::UserError(format!(
            "Incomplete credentials stored for profile '{profile}' — run \
             `jr auth login {profile}` to fix this."
        )).into()),
    }
}
```

Properties (BC-1.4.032/BC-1.4.033/BC-1.4.034):

- **No-copy invariant:** the legacy flat pair, if present, is read only for
  existence (a bool), never for its values — it is never written into a
  `<profile>:email`/`<profile>:api-token` entry, and never deleted.
- **Byte-identical error regardless of legacy-pair state:** whether the old
  flat pair exists or not, the both-absent error text is exactly the same —
  its presence changes nothing observable.
- **No profile special-casing:** `"default"` goes through the identical
  branch as any other profile name — contrast (2)'s OAuth migration, which
  is intentionally `"default"`-only.
- **Distinct partial-write error, checked first:** exactly one of the two
  namespaced keys present (e.g. `jr auth login` interrupted mid-write) is a
  separate, more specific "incomplete credentials" error, and this check
  runs *before* any legacy-pair consideration — a partial namespaced pair
  never falls through to the both-absent message even when a legacy pair
  also happens to exist.
- **Never suggests `jr auth logout`:** that command is a no-op for
  API-token profiles; the only valid remediations surfaced are
  `jr auth login <profile>` (repair) or `jr auth remove <profile>`
  (abandon).
- **One-time cost:** running the remediation (`jr auth login <profile>`)
  once permanently resolves the failure for that profile — there is no
  first-call-migrates/subsequent-call-differs shape, since this whole path
  is read-only with no mutating side effect of its own.

Out of scope for this story (tracked as a follow-up): surfacing this
absence state proactively in `jr auth list`'s `STATUS` column ahead of the
next command that actually needs the credential.

### Rollback story (manual only)

A user who wants to revert can `cp config.toml config.toml.backup` first (release notes will suggest this) and manually re-author `[instance]` from `[profiles.<name>]`. No `jr config rollback` ships — forward-only matches every surveyed CLI's migration behavior.

## Error Handling

| Failure | `JrError` variant | Exit code | Message shape |
|---|---|---|---|
| `--profile X` unknown | `UserError` | 64 | `unknown profile: foo; known: default, sandbox` |
| `JR_PROFILE=X` unknown | `UserError` | 64 | (same as above) |
| `default_profile = "X"` in config but X missing from `[profiles]` | `UserError` | 64 | `default_profile "foo" not in [profiles]; fix config.toml or run "jr auth list"` |
| `jr auth switch <unknown>` | `UserError` | 64 | `unknown profile: foo; known: …` |
| `jr auth remove <name>` where `name == default_profile` | `UserError` | 64 | `cannot remove active profile "default"; switch first with "jr auth switch …"` |
| `jr auth remove <unknown>` | `UserError` | 64 | `unknown profile: foo; known: …` |
| `jr auth login --profile X --no-input` and target has no URL | `UserError` | 64 | `--url required when the target profile has no URL configured` |
| `jr auth refresh --profile X` where X uses api_token auth and has no URL | `UserError` | 64 | `profile "X" has no URL configured. Use "jr auth login --profile X --url <https://…>" instead of refresh — refresh assumes the profile is already set up and only rotates credentials.` |
| Profile name fails character/length validation | `UserError` | 64 | `invalid profile name "foo:bar"; allowed: A-Z a-z 0-9 _ - up to 64 chars; reserved Windows names (CON, NUL, AUX, PRN, COM1-9, LPT1-9) excluded` |
| Profile name matches a Windows reserved name | `UserError` | 64 | (same message — reserved name list embedded) |
| TOML migration write fails | `Internal` | 1 | `Internal error: config migration failed: <io>` |
| Keyring read fails on per-profile key | `ConfigError` (existing) | 78 | (existing message) |

## Testing

TDD; existing test stack (`proptest`, `insta`, `tempfile`, `assert_cmd`, `wiremock`) covers everything. No new test crates.

### Unit tests (in-module)

`config::tests`:
- Active-profile resolution precedence (4 cases: flag, env, config field, default fallback)
- Profile-name validation: regex character/length cases (proptest with random strings; assert accept ⇔ regex match), plus an explicit table-driven test for Windows reserved names (CON, con, Con, NUL, AUX, PRN, COM1, COM9, LPT1, LPT9 — case-insensitive — all rejected on every platform)
- Migration: synthetic legacy `[instance]` TOML → assert post-migration `GlobalConfig` shape
- Migration is idempotent (second run is a no-op)
- `[fields]` carried into `[profiles.default]` during migration
- `Config::active_profile()` returns the right `ProfileConfig` (owned clone) and `Config::active_profile_or_err()` returns `&ProfileConfig` or errors with `JrError::ConfigError` for callers that want to fail loudly
- Unknown `default_profile` returns `UserError` (matches the unified active-profile existence check; the value comes from user-edited config, env, or flag — UserError is the honest classification)

`api::auth::tests`:
- `store_oauth_tokens(profile, ...) + load_oauth_tokens(profile, ...)` round-trip per profile (uses `JR_SERVICE_NAME=jr-jira-cli-test-<test_name>`)
- Profile A's OAuth tokens not visible to profile B
- Shared `api-token` accessible from any profile load path
- Lazy OAuth migration: pre-seed flat keys, call `load_oauth_tokens("default", ...)`, assert new keys exist + flat keys gone + return value matches
- Lazy migration only fires for `"default"` profile
- `clear_profile_creds("sandbox")` removes only that profile's OAuth, leaves `default:oauth-*` and shared keys intact

`cache::tests`:
- Existing 24 tests updated to thread `profile` through helpers
- New: cross-profile isolation — write team cache as profile A, attempt read as profile B, assert miss
- New: `clear_profile_cache("sandbox")` removes only that profile's subdir

### Integration tests (`tests/`)

`tests/auth_profiles.rs` (new):
- `jr auth login --profile sandbox --url https://… --no-input` (with `JR_API_TOKEN` env preset to skip prompt) — assert config.toml gains the profile, keyring gains shared API token, exit 0
- `jr auth list --output json` — assert JSON shape, active marker
- `jr auth switch sandbox` — assert default_profile mutated, exit 0
- `jr auth switch nonexistent` — assert exit 64, error message names known profiles
- `jr auth remove sandbox` — assert config entry gone, OAuth keys gone, cache subdir gone
- `jr auth remove default` (when active) — assert exit 64, no state mutated
- Precedence: `--profile` flag overrides `JR_PROFILE` env; `JR_PROFILE` overrides `default_profile` in config

### Snapshot tests (`insta`)

- `jr auth list --output table` for a 3-profile fixture
- `jr auth list --output json` for the same fixture
- Migration on-disk snapshot: write legacy TOML to tempdir, run migration, snapshot the resulting file

### Migration integration (`tests/migration_legacy.rs`, new)

- Set up tempdir with legacy `[instance]` TOML + flat keyring keys
- Run `jr auth list`
- Assert TOML migrated, OAuth keys migrated, exit 0
- Re-run; assert no second migration notice (idempotency)
- Both an in-memory roundtrip (write → load → assert) and an on-disk snapshot (insta)

### Test isolation

- `JR_SERVICE_NAME=jr-jira-cli-test-<test_name>` per test (unique service names prevent collisions)
- `XDG_CACHE_HOME` and `XDG_CONFIG_HOME` set to per-test tempdirs
- Existing `ENV_MUTEX` pattern in `config::tests` handles env-var race conditions

### Keyring CI compatibility

Linux CI runners often lack an active D-Bus session for the `secret-service` backend. New keyring round-trip tests are gated with `#[ignore]` and an opt-in env var (`JR_RUN_KEYRING_TESTS=1`):

```rust
#[test]
#[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
fn store_and_load_per_profile_oauth_tokens() {
    if std::env::var("JR_RUN_KEYRING_TESTS").is_err() { return; }
    // ...
}
```

CI runs them on macOS/Windows by default; Linux CI either provides D-Bus or skips. Local devs run them automatically.

The gating rule applies to ANY test that calls into a live keychain path — not only tests that write
credentials. A test that runs `jr auth status` against a profile whose `auth_method` is `api_token`
reaches `src/cli/auth/status.rs::status()` → `auth::load_api_token()` → `keyring::Entry::get_password()`,
which can block or prompt on a CI host without a keychain daemon. Such tests MUST also carry
`#[ignore]` + `JR_RUN_KEYRING_TESTS` guard (MAINT-MUTANTS-GLOBS-01 / #526-F6-KEYRING-GATE).

Current gated tests (non-exhaustive; search `#[ignore]` + `JR_RUN_KEYRING_TESTS` for the full list):
- `tests/auth_profiles.rs::auth_login_creates_new_profile_with_url` — writes API token to keychain
- `tests/auth_profiles.rs::auth_login_with_jr_profile_pointing_to_unrelated_profile_still_creates_target` — writes API token
- `tests/auth_profiles.rs::global_profile_flag_targets_auth_status` — reads keychain via `load_api_token()` on an `api_token` profile (added #526-F6-KEYRING-GATE)
- `tests/multi_cloudid_disambiguation.rs` — multiple `auth login --oauth` tests that write OAuth tokens
- `tests/oauth_refresh_integration.rs` — AC-002 / AC-009..AC-011 OAuth refresh coordinator tests
- `tests/auth_output_json.rs::test_auth_login_emits_json_when_output_json_set` — writes API token

## Concurrency & Cross-Platform Notes

**Concurrent `jr` invocations writing `config.toml`**: two simultaneous mutating commands (e.g., `jr auth switch` and `jr auth login` in different terminals) can race; the last writer wins, the other's changes are lost. This is a *pre-existing* limitation of `Config::save_global` (which uses non-atomic `std::fs::write`), not a regression introduced by multi-profile. Mitigated by the same atomic-save follow-up listed below.

**Concurrent OAuth refresh against the same profile**: two simultaneous `jr auth refresh --profile X` (or any commands that trigger refresh) can both POST to `/oauth/token`, with the second response invalidating the first. Last writer wins on the keyring side. Pre-existing single-instance limitation, not a regression. The retry path on a 401 already handles the case where a stale refresh token rejects — users see one extra retry, not a hard failure.

**Cross-machine portability**: `config.toml` is plain TOML and copies cleanly between machines. **Credentials in the OS keyring do NOT migrate** (by design — never write secrets to disk). Users moving to a new machine re-run `jr auth login --profile <each>` to re-establish credentials. Matches every CLI surveyed.

## Out of Scope / Follow-ups

- **`jr profile` subcommand tree** — separate from `jr auth`. May be revisited if non-auth per-profile config grows beyond the current set.
- **Profile renaming** — multistep workaround works for now (`jr auth login --profile new --url ...; jr auth logout --profile old; jr auth remove old`).
- **Per-repo `.jr.toml` profile pinning** — direnv with `JR_PROFILE` covers it. Adding it natively conflicts with the universal `flag > env > global > default` convention surveyed across kubectl/aws/gh/gcloud.
- **`KeyringProvider` trait abstraction** — file as a follow-up issue for testability and CI portability. Outside the scope of multi-profile semantics.
- **Atomic `Config::save_global` (tempfile + rename)** — file as a follow-up issue. Existing limitation, not a regression of this feature.
- **Source-profile fallback for API tokens (AWS-style)** — only useful for the niche service-account-per-environment case. Can be layered on later if asked.
- **Bulk-clear command for shared credentials** — `jr auth logout --all` or similar. Today users would manage shared keychain entries via the OS keychain UI. Low frequency (the shared API token is the user's account credential; rarely wiped except on uninstall). File as follow-up.
