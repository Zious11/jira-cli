# OAuth Scopes Configurable via `config.toml`

**Issue:** [#184](https://github.com/Zious11/jira-cli/issues/184)

## Problem

`jr`'s OAuth 2.0 (3LO) flow at `src/api/auth.rs:27` hardcodes a classic scope string:

```rust
const SCOPES: &str = "read:jira-work write:jira-work read:jira-user offline_access";
```

Users who configure an OAuth app in the Atlassian Developer Console with a different scope set — typically granular scopes for least-privilege agent use, e.g. `read:issue:jira write:comment:jira` — cannot make `jr auth login --oauth` request those scopes. The authorize URL always asks for the hardcoded classic string regardless of what the Developer Console app actually has.

## Atlassian constraints (validated)

Confirmed against Atlassian's own developer docs via Perplexity:

1. **Classic scopes remain recommended.** Atlassian's docs state: *"use classic scopes to the maximum extent possible"*; granular scopes are for cases *"when you can't use classic scopes."* No end-of-life has been announced for classic scopes in Jira Platform, Service Management, or Confluence. Exception: Jira **Software**-specific scopes are granular-only.
2. **Classic and granular scopes cannot be mixed in one `/authorize` request.** The scope parameter must only contain scopes already added to the app in the Developer Console, and apps configure one model or the other — not both.
3. **`offline_access` is required** for a refresh token to be issued, in both classic and granular scope modes.

Implication: the current hardcoded default is a safe, recommended default for classic apps. The fix is simply to let users override it when their Developer Console app uses a different scope set. `jr` performs no scope-mix validation — Atlassian returns an actionable error on invalid combinations, and maintaining a client-side allowlist of scope names would be brittle as Atlassian revises the list.

## Design

### Config shape

Add one optional field to `InstanceConfig` (`src/config.rs`):

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct InstanceConfig {
    pub url: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub auth_method: Option<String>,
    pub oauth_scopes: Option<String>, // new
}
```

TOML usage:

```toml
[instance]
auth_method = "oauth"
oauth_scopes = "read:issue:jira write:issue:jira write:comment:jira read:jira-user offline_access"
```

Env override comes free from the existing `Env::prefixed("JR_")` merge in `Config::load()`: `JR_INSTANCE_OAUTH_SCOPES` sets or overrides the value. Empirically confirmed by the existing `JR_INSTANCE_AUTH_METHOD` usage in `tests/auth_refresh.rs:71`.

### Default

The existing classic string stays as the fallback when `oauth_scopes` is unset. Renamed for clarity:

```rust
// src/api/auth.rs
pub const DEFAULT_OAUTH_SCOPES: &str =
    "read:jira-work write:jira-work read:jira-user offline_access";
```

Zero behavior change for any user who doesn't set the new config field.

### Data flow

```
Config::load() → cli::auth::handle_login()
  → resolve_oauth_scopes(&config)?
  → api::auth::oauth_login(client_id, client_secret, scopes)
    → urlencoding::encode(scopes) in the authorize URL
```

- `oauth_login` gains a third parameter `scopes: &str`, replacing the `SCOPES` constant at the existing use site (`src/api/auth.rs:168`).
- `refresh_oauth_token` is **unchanged**. The OAuth `refresh_token` grant does not accept a `scope` parameter at Atlassian's token endpoint; scopes for refreshed access tokens inherit from the original authorization.

### Scope-resolution helper

A small private helper in `src/cli/auth.rs` owns the selection and validation:

```rust
// src/cli/auth.rs (module-private)
fn resolve_oauth_scopes(config: &Config) -> Result<String> {
    match config.global.instance.oauth_scopes.as_deref() {
        None => Ok(crate::api::auth::DEFAULT_OAUTH_SCOPES.to_string()),
        Some(raw) => {
            let trimmed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if trimmed.is_empty() {
                Err(JrError::ConfigError(
                    "oauth_scopes is empty; remove the setting to use defaults \
                     or list at least one scope".into(),
                ).into())
            } else {
                Ok(trimmed)
            }
        }
    }
}
```

Trimming collapses arbitrary whitespace (TOML multi-line literals, trailing newlines, stray tabs) into single spaces, matching how `urlencoding::encode` will render the string to Atlassian. `split_whitespace().collect::<Vec<_>>().join(" ")` is the idiomatic way to do this in Rust.

### Errors

| Condition | Behavior |
|---|---|
| `oauth_scopes` unset | Use `DEFAULT_OAUTH_SCOPES`, no warning |
| `oauth_scopes = "..."` with at least one token after trim | Pass through trimmed value |
| `oauth_scopes = ""` or whitespace-only | `JrError::ConfigError`, actionable message, exit 78 |
| Invalid scope combination (e.g. mix, unknown scope) | Atlassian returns `invalid_scope` at token exchange; existing `anyhow::bail!("Token exchange failed: {body}")` path surfaces it |

No client-side mix detection. No warning for missing `offline_access`. Both are documented caveats, not enforcement.

### Documentation

- `docs/specs/oauth-scopes-configurable.md` (this file)
- README section on OAuth configuration: call out that the scope string must match what the Developer Console app has configured, that classic and granular cannot mix, and that `offline_access` is required for unattended re-authentication
- Existing `jr auth login --oauth` help text: reference the config key so users discover it without reading the README

## Testing strategy

All unit tests in the affected modules. No integration test for the OAuth flow itself — `oauth_login` hits `auth.atlassian.com` and the value of mocking the authorization-code dance is low compared to the risk of the mock diverging from Atlassian's actual behavior.

- `src/config.rs` (`#[cfg(test)]` module): `oauth_scopes` parses from TOML; missing field yields `None`; env var `JR_INSTANCE_OAUTH_SCOPES` overrides
- `src/cli/auth.rs` (`#[cfg(test)]` module): `resolve_oauth_scopes` — None path returns default, Some(valid) returns trimmed, Some(empty/whitespace) returns `ConfigError`, Some with internal whitespace is collapsed

## Out of scope

- Validating scope mix (classic vs granular) or token shape client-side — let Atlassian return `invalid_scope`; our error path already surfaces it
- Warning when `offline_access` is absent — docs-only caveat
- Migrating the default to granular — Atlassian still recommends classic for Jira Platform; Jira Software users who need granular will simply set the config
- Per-project `oauth_scopes` in `.jr.toml` — global-only, matches how `auth_method`, `cloud_id`, and `url` are scoped today

## Non-breaking

Every existing user continues to see the same authorize URL until they opt in by setting `oauth_scopes`. No migration, no deprecation notice.
