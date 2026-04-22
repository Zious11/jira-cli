# OAuth Scopes Configurable via `config.toml` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users override `jr`'s hardcoded OAuth 2.0 scopes via `config.toml`, falling back to the existing classic scopes when the key is unset.

**Architecture:** Add one field to `InstanceConfig`. Rename the existing `SCOPES` constant to `pub const DEFAULT_OAUTH_SCOPES` so callers can use it as a fallback. A small private helper in `src/cli/auth.rs` picks config-over-default, trims whitespace, and errors on empty input. The helper's result is passed as a new `scopes: &str` parameter to `oauth_login`, replacing the use of the constant inside the authorize-URL builder.

**Tech Stack:** Rust, figment (already used for config), serde, anyhow, `urlencoding` (already used in authorize URL).

**Spec:** `docs/specs/oauth-scopes-configurable.md`

---

## File map

| Path | Change |
|---|---|
| `src/config.rs` | Add `oauth_scopes: Option<String>` to `InstanceConfig`; add unit tests for TOML parse and missing-key default-to-`None` behavior |
| `src/api/auth.rs` | Rename `const SCOPES` → `pub const DEFAULT_OAUTH_SCOPES`; add `scopes: &str` parameter to `oauth_login`; use the parameter in the authorize URL |
| `src/cli/auth.rs` | Add module-private `resolve_oauth_scopes(&Config) -> Result<String>`; call it in `login_oauth` and pass the result into `oauth_login`; add unit tests covering four cases |
| `README.md` | Add `oauth_scopes` to the global config example and a short note on classic-vs-granular + `offline_access` |

No additional implementation or test files are created by the plan. The spec (`docs/specs/oauth-scopes-configurable.md`) and this plan itself are new in this PR but are documentation, not implementation. No integration test file is needed — `oauth_login` hits `auth.atlassian.com` directly and is out of scope for unit-testable mocking. Env-var override is likewise out of scope (see spec Data flow section).

---

## Task 1: Add `oauth_scopes` to `InstanceConfig`

**Files:**
- Modify: `src/config.rs`
- Test: `src/config.rs` (inline `#[cfg(test)]` module)

### Step 1: Write the failing test

- [ ] Append to the existing `#[cfg(test)] mod tests` in `src/config.rs`:

```rust
#[test]
fn instance_config_parses_oauth_scopes_from_toml() {
    use figment::{Figment, providers::{Format, Toml}};

    let toml = r#"
        [instance]
        url = "https://example.atlassian.net"
        auth_method = "oauth"
        oauth_scopes = "read:issue:jira write:issue:jira offline_access"
    "#;

    let config: GlobalConfig = Figment::new()
        .merge(Toml::string(toml))
        .extract()
        .unwrap();

    assert_eq!(
        config.instance.oauth_scopes.as_deref(),
        Some("read:issue:jira write:issue:jira offline_access")
    );
}

#[test]
fn instance_config_oauth_scopes_missing_is_none() {
    use figment::{Figment, providers::{Format, Toml}};

    let toml = r#"
        [instance]
        url = "https://example.atlassian.net"
        auth_method = "oauth"
    "#;

    let config: GlobalConfig = Figment::new()
        .merge(Toml::string(toml))
        .extract()
        .unwrap();

    assert!(config.instance.oauth_scopes.is_none());
}
```

### Step 2: Run test to verify it fails

- [ ] `cargo test --lib instance_config_parses_oauth_scopes_from_toml` — expect FAIL with "no field `oauth_scopes`" or similar serde/figment error.

### Step 3: Write minimal implementation

- [ ] Modify `InstanceConfig` in `src/config.rs`:

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct InstanceConfig {
    pub url: Option<String>,
    pub cloud_id: Option<String>,
    pub org_id: Option<String>,
    pub auth_method: Option<String>,
    pub oauth_scopes: Option<String>,
}
```

No other changes required — the `Deserialize` and `Default` derives pick this up automatically from the TOML layer. **Do not** assume `JR_INSTANCE_OAUTH_SCOPES` works via the current `Env::prefixed("JR_")` merge; figment's default env provider can't reach nested struct fields without a `.split(...)` configuration, and the spec at `docs/specs/oauth-scopes-configurable.md:50` scopes nested env-var support out of this PR. The `tests/auth_refresh.rs:71` reference in earlier drafts was defensive env-clearing, not proof that `JR_INSTANCE_AUTH_METHOD` actually propagates through figment.

### Step 4: Run tests to verify they pass

- [ ] `cargo test --lib instance_config_parses_oauth_scopes_from_toml instance_config_oauth_scopes_missing_is_none` — expect PASS.

### Step 5: Checks + commit

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`

```bash
git add src/config.rs
git commit -m "feat(config): add oauth_scopes to InstanceConfig (#184)

Adds Option<String> field so users can override the hardcoded OAuth 2.0
scope list via config.toml. Env-var override via JR_INSTANCE_OAUTH_SCOPES
is out of scope for this PR — see spec data-flow note. No behavior change
yet — the field is unused until Task 3 wires it into oauth_login.
Spec: docs/specs/oauth-scopes-configurable.md."
```

---

## Task 2: Add `resolve_oauth_scopes` helper + expose `DEFAULT_OAUTH_SCOPES`

**Files:**
- Modify: `src/api/auth.rs` (rename `SCOPES` → `pub const DEFAULT_OAUTH_SCOPES`)
- Modify: `src/cli/auth.rs` (add helper + unit tests)

### Step 1: Write the failing tests

- [ ] Add a `#[cfg(test)] mod tests` block at the bottom of `src/cli/auth.rs` (creating one if it doesn't exist). Use explicit crate paths for `Config`, `GlobalConfig`, `InstanceConfig`, and the constant:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::auth::DEFAULT_OAUTH_SCOPES;
    use crate::config::{Config, GlobalConfig, InstanceConfig};

    fn config_with_scopes(scopes: Option<&str>) -> Config {
        Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    oauth_scopes: scopes.map(String::from),
                    ..InstanceConfig::default()
                },
                ..GlobalConfig::default()
            },
            project: Default::default(),
        }
    }

    #[test]
    fn resolve_oauth_scopes_none_returns_default() {
        let config = config_with_scopes(None);
        assert_eq!(resolve_oauth_scopes(&config).unwrap(), DEFAULT_OAUTH_SCOPES);
    }

    #[test]
    fn resolve_oauth_scopes_trims_and_collapses_whitespace() {
        let config = config_with_scopes(Some("  read:issue:jira   write:comment:jira\n\toffline_access  "));
        assert_eq!(
            resolve_oauth_scopes(&config).unwrap(),
            "read:issue:jira write:comment:jira offline_access"
        );
    }

    #[test]
    fn resolve_oauth_scopes_empty_string_is_config_error() {
        let config = config_with_scopes(Some(""));
        let err = resolve_oauth_scopes(&config).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("oauth_scopes is empty"), "unexpected error: {msg}");
    }

    #[test]
    fn resolve_oauth_scopes_whitespace_only_is_config_error() {
        let config = config_with_scopes(Some("   \n\t  "));
        let err = resolve_oauth_scopes(&config).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("oauth_scopes is empty"), "unexpected error: {msg}");
    }
}
```

### Step 2: Run tests to verify they fail

- [ ] `cargo test --lib resolve_oauth_scopes` — expect FAIL with "cannot find value `resolve_oauth_scopes`" and "cannot find value `DEFAULT_OAUTH_SCOPES`".

### Step 3: Rename the constant

- [ ] In `src/api/auth.rs`, change:

```rust
const SCOPES: &str = "read:jira-work write:jira-work read:jira-user offline_access";
```

to:

```rust
/// Default OAuth 2.0 scopes used when `oauth_scopes` is not set in
/// config.toml. Matches Atlassian's "classic" scope recommendation for
/// Jira Platform apps. Users who configured their Developer Console app
/// with granular scopes (e.g., for least-privilege agent use) should
/// override via `[instance].oauth_scopes` in config.toml.
pub const DEFAULT_OAUTH_SCOPES: &str =
    "read:jira-work write:jira-work read:jira-user offline_access";
```

Update the reference at `src/api/auth.rs:168` (the only in-module use):

```rust
urlencoding::encode(DEFAULT_OAUTH_SCOPES),
```

(This reference will be replaced again in Task 3 — leaving the rename standalone keeps this task self-contained.)

### Step 4: Add the helper

- [ ] At the top of `src/cli/auth.rs`, ensure the following imports are present (reuse existing ones where possible):

```rust
use anyhow::Result;
use crate::config::Config;
use crate::error::JrError;
```

- [ ] Add near the other module-private helpers in `src/cli/auth.rs`:

```rust
/// Pick the OAuth scope string: user override from `[instance].oauth_scopes`
/// if set, else the compiled-in default. Trims and collapses interior
/// whitespace so multi-line TOML strings encode cleanly. Empty or
/// whitespace-only overrides are a configuration error.
fn resolve_oauth_scopes(config: &Config) -> Result<String> {
    match config.global.instance.oauth_scopes.as_deref() {
        None => Ok(crate::api::auth::DEFAULT_OAUTH_SCOPES.to_string()),
        Some(raw) => {
            let collapsed: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
            if collapsed.is_empty() {
                Err(JrError::ConfigError(
                    "oauth_scopes is empty; remove the setting to use defaults \
                     or list at least one scope"
                        .into(),
                )
                .into())
            } else {
                Ok(collapsed)
            }
        }
    }
}
```

### Step 5: Run tests to verify they pass

- [ ] `cargo test --lib resolve_oauth_scopes` — expect 4/4 PASS.

### Step 6: Checks + commit

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings` — note: `resolve_oauth_scopes` is unused outside tests until Task 3; clippy should not warn thanks to `#[cfg(test)]` referencing it, but if it does, add `#[allow(dead_code)]` to the helper with a comment pointing at Task 3.
- [ ] `cargo test`

```bash
git add src/api/auth.rs src/cli/auth.rs
git commit -m "feat(auth): add resolve_oauth_scopes helper + expose DEFAULT_OAUTH_SCOPES (#184)

Renames the private SCOPES constant to pub const DEFAULT_OAUTH_SCOPES so
the CLI handler can fall back to it when no override is configured.
Introduces resolve_oauth_scopes(&Config) -> Result<String> with four-case
coverage: None → default, valid → trimmed + whitespace-collapsed, empty
and whitespace-only → JrError::ConfigError. Not yet wired into the
login flow — Task 3 threads it through oauth_login."
```

---

## Task 3: Thread scopes into `oauth_login` and wire the call site

**Files:**
- Modify: `src/api/auth.rs` (add `scopes: &str` parameter, remove direct use of `DEFAULT_OAUTH_SCOPES` inside `oauth_login`)
- Modify: `src/cli/auth.rs` (`login_oauth` calls `resolve_oauth_scopes` and passes the result)

No new tests — the helper (Task 2) covers the selection logic, and the OAuth flow itself isn't unit-tested in the current codebase.

### Step 1: Update the API signature

- [ ] Change `oauth_login` at `src/api/auth.rs:150` from:

```rust
pub async fn oauth_login(client_id: &str, client_secret: &str) -> Result<OAuthResult> {
```

to:

```rust
pub async fn oauth_login(
    client_id: &str,
    client_secret: &str,
    scopes: &str,
) -> Result<OAuthResult> {
```

- [ ] Replace the single internal reference (the line currently `urlencoding::encode(DEFAULT_OAUTH_SCOPES),` after Task 2) with:

```rust
urlencoding::encode(scopes),
```

Update the function's doc comment to note the new parameter:

```rust
/// Run the full OAuth 2.0 (3LO) authorization code flow:
/// 1. Open browser to Atlassian authorization page requesting `scopes`
/// 2. Listen on a local port for the callback
/// 3. Exchange the authorization code for tokens
/// 4. Fetch accessible resources to get the cloud ID
/// 5. Store tokens in the system keychain
///
/// `scopes` is a space-separated scope string (URL-encoded internally).
/// Callers should use `DEFAULT_OAUTH_SCOPES` when no user override is set.
/// Note: `refresh_oauth_token` does NOT take a scope parameter — the
/// refresh_token grant inherits scopes from the original authorization.
```

### Step 2: Update the call site

- [ ] In `src/cli/auth.rs`, replace the call at line 182:

```rust
let result = crate::api::auth::oauth_login(&client_id, &client_secret).await?;
```

with:

```rust
let scopes = resolve_oauth_scopes(&Config::load().unwrap_or_default())?;
let result =
    crate::api::auth::oauth_login(&client_id, &client_secret, &scopes).await?;
```

Note: `Config::load()` is called twice in `login_oauth` now (once here, once at line 184 for saving). That's fine — config load is cheap and the second load picks up the state already present on disk. If clippy complains about `needless_late_init` or similar, hoist the `Config::load().unwrap_or_default()` into a single `let mut config = ...` at the top of the function and reuse it.

### Step 3: Build + checks

- [ ] `cargo build` — expect success; the three-arg `oauth_login` signature is consistent across the one call site.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test` — all tests (including the four from Task 2) pass.

### Step 4: Commit

- [ ] 

```bash
git add src/api/auth.rs src/cli/auth.rs
git commit -m "feat(auth): thread configurable OAuth scopes through authorize URL (#184)

oauth_login now accepts scopes as a parameter and the CLI login handler
passes resolve_oauth_scopes(&config)?. Users with an Atlassian Developer
Console app configured with granular scopes (e.g., write:issue:jira for
least-privilege LLM/agent use) can now opt in by setting

    [instance]
    oauth_scopes = \"read:issue:jira write:issue:jira offline_access\"

in config.toml. Unset keeps the existing classic default — zero behavior
change for everyone else. refresh_oauth_token is unchanged; the
refresh_token grant doesn't take a scope parameter. Closes #184."
```

---

## Task 4: Documentation

**Files:**
- Modify: `README.md`

### Step 1: Update the global config example

- [ ] Edit README.md line 216 context (the `[instance]` block in the Configuration section) to include `oauth_scopes`:

```toml
[instance]
url = "https://yourorg.atlassian.net"
auth_method = "api_token"  # or "oauth"
# Optional: override the OAuth 2.0 scope list when auth_method = "oauth".
# Must match what the OAuth app configured in the Atlassian Developer
# Console actually has. Classic and granular scopes CANNOT mix in one
# request, and "offline_access" is required for refresh tokens to be
# issued. If unset, jr uses Atlassian's recommended classic scopes.
# oauth_scopes = "read:issue:jira write:issue:jira write:comment:jira read:jira-user offline_access"
```

### Step 2: Add one-sentence reference in the OAuth login section

- [ ] In the OAuth section of the README (near line 90, where `jr auth login --oauth` is shown), add a note after the command:

```markdown
To override the default scope list — for example, to request granular
scopes for least-privilege agent access — set `[instance].oauth_scopes`
in `~/.config/jr/config.toml`. See the Configuration section below.
```

### Step 3: Checks + commit

- [ ] `cargo fmt --all -- --check` (no-op for docs but keeps the habit)
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`

```bash
git add README.md
git commit -m "docs: document oauth_scopes config key (#184)

Adds an example of oauth_scopes to the global config block and a short
note near jr auth login --oauth pointing users at the override. Includes
the two constraints Perplexity-validated against Atlassian's docs:
classic and granular scopes cannot mix, and offline_access is required
for refresh tokens. Spec: docs/specs/oauth-scopes-configurable.md."
```

---

## Self-review

**Spec coverage:** every requirement from `docs/specs/oauth-scopes-configurable.md` maps to a task:
- Config shape → Task 1
- `DEFAULT_OAUTH_SCOPES` + `resolve_oauth_scopes` helper + four-case error handling → Task 2
- Data flow + `oauth_login` signature change → Task 3
- README docs → Task 4
- "No integration test for the OAuth flow itself" — respected (none planned)

**Placeholder scan:** none. Every code block is complete; every command is runnable as written.

**Type consistency:** `resolve_oauth_scopes` signature is the same everywhere it appears (`fn resolve_oauth_scopes(config: &Config) -> Result<String>`). `DEFAULT_OAUTH_SCOPES` is the same constant name everywhere. `oauth_login` parameter order `(client_id, client_secret, scopes)` matches the signature change in Task 3 and the call site update.
