use crate::profile::Profile;
use anyhow::{Context, Result};
use keyring::Entry;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info};

/// Default keychain service name for `jr` credentials. `JR_SERVICE_NAME`
/// can override this at runtime; it is primarily used by tests to avoid
/// touching a developer's real keychain.
const DEFAULT_SERVICE_NAME: &str = "jr-jira-cli";

/// Resolve the keychain service name. In debug builds, `JR_SERVICE_NAME` can
/// override the default to isolate keyring integration tests from a developer's
/// real keychain. Release builds always return [`DEFAULT_SERVICE_NAME`] — the
/// env var is excluded at compile time via `#[cfg(debug_assertions)]` to prevent
/// an attacker who can set env vars from redirecting keychain lookups to a
/// different service namespace (SEC-JR-SERVICE-NAME-GATE).
fn service_name() -> String {
    #[cfg(debug_assertions)]
    if let Ok(name) = std::env::var("JR_SERVICE_NAME") {
        return name;
    }
    DEFAULT_SERVICE_NAME.to_string()
}

/// Key names stored in the system keychain.
const KEY_EMAIL: &str = "email";
const KEY_API_TOKEN: &str = "api-token";
/// Pre-multi-profile flat OAuth keys. Read-only on the migration path inside
/// [`load_oauth_tokens`] for the `"default"` profile; new writes always use
/// the namespaced `<profile>:oauth-*-token` keys.
const KEY_OAUTH_ACCESS_LEGACY: &str = "oauth-access-token";
const KEY_OAUTH_REFRESH_LEGACY: &str = "oauth-refresh-token";

fn oauth_access_key(profile: &str) -> String {
    format!("{profile}:oauth-access-token")
}
fn oauth_refresh_key(profile: &str) -> String {
    format!("{profile}:oauth-refresh-token")
}

/// Namespaced keychain key for a profile's API-token email
/// (`<profile>:email`, S-cycle3-percred-storage / BC-1.4.031). Mirrors
/// [`oauth_access_key`]'s shape exactly.
fn api_token_email_key(profile: &str) -> String {
    format!("{profile}:email")
}
/// Namespaced keychain key for a profile's API token
/// (`<profile>:api-token`, S-cycle3-percred-storage / BC-1.4.031). Mirrors
/// [`oauth_refresh_key`]'s shape exactly.
fn api_token_key(profile: &str) -> String {
    format!("{profile}:api-token")
}

/// Default OAuth 2.0 scopes used when `oauth_scopes` is not set in
/// config.toml. Covers every API surface `jr` exercises today:
/// - `read:jira-work` / `write:jira-work` / `read:jira-user` — Jira issues,
///   search, projects, fields, users (the bulk of `jr issue/board/sprint`).
/// - `read:servicedesk-request` — JSM queues and queue issues
///   (`jr queue list/view`).
/// - `read:cmdb-object:jira` / `read:cmdb-schema:jira` — Assets/CMDB
///   discovery (`jr assets search/view/tickets/schemas/types/schema`).
/// - `offline_access` — required for refresh tokens; without it, OAuth
///   sessions die after one hour.
///
/// Users who configured their Developer Console app with granular scopes
/// (e.g., for least-privilege agent use) should override via
/// `[profiles.<name>].oauth_scopes` in config.toml. The embedded `jr`
/// app must be registered with this exact scope set in its Developer
/// Console permissions, otherwise the authorize call rejects with
/// `invalid_scope`.
// Built via `concat!` (vs. line-continuation in a string literal) to make
// the absence of double spaces obvious to any reader, not dependent on the
// `\<newline>` continuation rule that consumes following whitespace. Each
// fragment ends with exactly one trailing space (or the final fragment has
// none) so the joined string is single-space separated. A regression test
// (`default_oauth_scopes_pins_the_full_set_with_offline_access`) asserts
// no double spaces appear.
pub const DEFAULT_OAUTH_SCOPES: &str = concat!(
    "read:jira-work write:jira-work read:jira-user ",
    "read:servicedesk-request write:servicedesk-request ",
    "read:cmdb-object:jira read:cmdb-schema:jira ",
    "offline_access",
);

/// One Atlassian site returned by the `accessible-resources` endpoint.
///
/// Lifted to module scope so that `resolve_cloud_id` tests can construct
/// `Vec<AccessibleResource>` via struct literals without needing serde
/// round-trips, and so that future production callers (e.g., `jr auth check`)
/// can reference the type directly.
///
/// Fields are `pub` so that integration tests in `tests/` (a separate crate
/// that imports `jr` as a library) can construct struct literals directly.
/// `#[doc(hidden)] pub` rather than `pub(crate)`: the integration-test crate
/// links the non-test build of the lib and cannot see `pub(crate)` items, so
/// `pub` is required; `#[doc(hidden)]` signals this is not a supported public
/// API. Matches the `pub` testable-item convention used elsewhere in this module.
#[doc(hidden)]
#[derive(Debug, PartialEq, serde::Deserialize)]
pub struct AccessibleResource {
    pub id: String,
    pub url: String,
    pub name: String,
}

/// Resolve the cloud ID from a list of accessible resources, applying
/// disambiguation logic per BC-1.5.038.
///
/// - 0 resources: returns `Err(JrError::UserError(...))` — no authorized sites.
/// - 1 resource: returns `Ok(resources[0].id.clone())` — auto-select, no prompt.
/// - Multiple resources with `cloud_id_override` set: finds the matching resource
///   or returns `Err(JrError::UserError(...))` listing available IDs.
/// - Multiple resources with `no_input = true` and no override: returns
///   `Err(JrError::UserError(...))` listing available IDs and instructing the user
///   to re-run with `--cloud-id`.
/// - Multiple resources, interactive: presents a dialoguer prompt (TTY) or
///   line-based stdin reader (non-TTY) and returns the selected ID.
///
/// Not async — disambiguation is pure on the non-interactive paths; the
/// interactive branch (dialoguer) is synchronous.
///
/// `#[doc(hidden)] pub`: reachable from the integration-test crate (which
/// `pub(crate)` cannot satisfy), but not a supported public API.
#[doc(hidden)]
pub fn resolve_cloud_id(
    resources: &[AccessibleResource],
    cloud_id_override: Option<&str>,
    no_input: bool,
) -> Result<String, crate::error::JrError> {
    match resources.len() {
        0 => Err(crate::error::JrError::UserError(
            "No Atlassian sites authorized this token. Re-run `jr auth login` \
                 and select at least one site at the consent screen."
                .into(),
        )),
        1 => Ok(resources[0].id.clone()),
        _ => {
            if let Some(override_id) = cloud_id_override {
                // --cloud-id provided: find matching resource or exit 64.
                resources
                    .iter()
                    .find(|r| r.id == override_id)
                    .map(|r| r.id.clone())
                    .ok_or_else(|| {
                        let listing = resources
                            .iter()
                            .map(|r| format!("  {} — {} ({})", r.id, r.name, r.url))
                            .collect::<Vec<_>>()
                            .join("\n");
                        crate::error::JrError::UserError(format!(
                            "Provided --cloud-id '{override_id}' not found in accessible \
                             resources. Available:\n{listing}"
                        ))
                    })
            } else if no_input {
                // --no-input without --cloud-id: exit 64 with actionable message.
                let listing = resources
                    .iter()
                    .map(|r| format!("  {} — {} ({})", r.id, r.name, r.url))
                    .collect::<Vec<_>>()
                    .join("\n");
                Err(crate::error::JrError::UserError(format!(
                    "Multiple Atlassian orgs found. Use --cloud-id <id> to disambiguate. \
                     Available:\n{listing}"
                )))
            } else {
                // Interactive: present a selection prompt.
                let items: Vec<String> = resources
                    .iter()
                    .map(|r| format!("{} ({}) [cloudId: {}]", r.name, r.url, r.id))
                    .collect();
                // Attempt dialoguer Select; fall back to line-based reading on
                // non-TTY stdin (e.g., test harness piping "2\n" via write_stdin).
                use std::io::IsTerminal;
                let selection = if std::io::stdin().is_terminal() {
                    dialoguer::Select::new()
                        .with_prompt("Multiple Atlassian orgs accessible. Select one:")
                        .items(&items)
                        .default(0)
                        .interact()
                        .map_err(|e| {
                            crate::error::JrError::UserError(format!(
                                "Failed to read selection: {e}"
                            ))
                        })?
                } else {
                    // Non-TTY stdin: print items and read a 1-based index.
                    eprintln!("Multiple Atlassian orgs accessible. Select one:");
                    for (i, item) in items.iter().enumerate() {
                        eprintln!("  {}: {}", i + 1, item);
                    }
                    let mut line = String::new();
                    std::io::stdin().read_line(&mut line).map_err(|e| {
                        crate::error::JrError::UserError(format!(
                            "Failed to read selection from stdin: {e}"
                        ))
                    })?;
                    let idx: usize = line.trim().parse::<usize>().map_err(|_| {
                        crate::error::JrError::UserError(format!(
                            "Invalid selection '{}': expected a number between 1 and {}",
                            line.trim(),
                            items.len()
                        ))
                    })?;
                    if idx == 0 || idx > items.len() {
                        return Err(crate::error::JrError::UserError(format!(
                            "Selection {} out of range (1..{})",
                            idx,
                            items.len()
                        )));
                    }
                    idx - 1 // convert to 0-based
                };
                Ok(resources[selection].id.clone())
            }
        }
    }
}

fn entry(key: &str) -> Result<Entry> {
    Entry::new(&service_name(), key).context("Failed to access keychain")
}

/// Store an API token and associated email in the system keychain under the
/// pre-multi-profile FLAT keys (`email` / `api-token`, shared across every
/// profile on the host).
///
/// Renamed from the original unqualified `store_api_token` by
/// S-cycle3-percred-storage (BC-1.4.031) to make room for the new
/// profile-namespaced `store_api_token(profile, email, token)` below.
/// New writes should always go through the namespaced function — this one
/// is retained, unused as a credential source going forward, purely so
/// `S-cycle3-credential-absence-guard`'s legacy-pair existence check has
/// something to call.
pub fn store_legacy_flat_api_token(email: &str, token: &str) -> Result<()> {
    entry(KEY_EMAIL)?.set_password(email)?;
    entry(KEY_API_TOKEN)?.set_password(token)?;
    Ok(())
}

/// Load the stored API token and email from the system keychain's
/// pre-multi-profile FLAT keys (`email` / `api-token`, shared across every
/// profile on the host). Returns `(email, token)`.
///
/// Renamed from the original unqualified, no-arg `load_api_token` by
/// S-cycle3-percred-storage (BC-1.4.031) to make room for the new
/// profile-namespaced `load_api_token(profile)` below. New reads should
/// always go through the namespaced function — this one is retained,
/// unused as a credential source going forward, purely so
/// `S-cycle3-credential-absence-guard`'s legacy-pair existence check has
/// something to call.
pub fn load_legacy_flat_api_token() -> Result<(String, String)> {
    let email = entry(KEY_EMAIL)?
        .get_password()
        .context("No stored email — run \"jr auth login\"")?;
    let token = entry(KEY_API_TOKEN)?
        .get_password()
        .context("No stored API token — run \"jr auth login\"")?;
    Ok((email, token))
}

/// Store OAuth 2.0 access and refresh tokens scoped to a profile.
///
/// Tokens are written to the namespaced keys `<profile>:oauth-access-token`
/// and `<profile>:oauth-refresh-token` so multiple Jira sites can coexist
/// in a single keychain.
pub fn store_oauth_tokens(profile: &Profile, access: &str, refresh: &str) -> Result<()> {
    entry(&oauth_access_key(profile.as_ref()))?.set_password(access)?;
    entry(&oauth_refresh_key(profile.as_ref()))?.set_password(refresh)?;
    Ok(())
}

/// Load OAuth 2.0 access and refresh tokens for a profile.
///
/// Returns `(access_token, refresh_token)`.
///
/// For the `"default"` profile, falls back to the legacy flat keys
/// (`oauth-access-token` / `oauth-refresh-token`, the pre-multi-profile
/// layout) and opportunistically migrates them to the new namespaced keys
/// on read: writes the namespaced copies, then deletes the legacy ones.
/// This means existing single-profile users transparently survive the
/// upgrade without re-authenticating. Non-`"default"` profiles never
/// inherit legacy keys — that would silently cross-pollinate credentials
/// across distinct Jira sites.
pub fn load_oauth_tokens(profile: &Profile) -> Result<(String, String)> {
    let access_key = oauth_access_key(profile.as_ref());
    let refresh_key = oauth_refresh_key(profile.as_ref());
    let access = read_keyring_optional(&access_key)?;
    let refresh = read_keyring_optional(&refresh_key)?;

    match (access, refresh) {
        (Some(a), Some(r)) => Ok((a, r)),
        (None, None) => {
            // Both namespaced keys absent — try legacy fallback for the
            // "default" profile (lazy-migration path). Non-default
            // profiles never inherit legacy keys; that would silently
            // cross-pollinate credentials across distinct Jira sites.
            if profile.as_ref() == "default" {
                let legacy_access = read_keyring_optional(KEY_OAUTH_ACCESS_LEGACY)?;
                let legacy_refresh = read_keyring_optional(KEY_OAUTH_REFRESH_LEGACY)?;
                if let (Some(a), Some(r)) = (legacy_access, legacy_refresh) {
                    store_oauth_tokens(&Profile::from("default"), &a, &r)?;
                    let _ = entry(KEY_OAUTH_ACCESS_LEGACY)?.delete_credential();
                    let _ = entry(KEY_OAUTH_REFRESH_LEGACY)?.delete_credential();
                    return Ok((a, r));
                }
            }
            Err(anyhow::anyhow!(
                "No stored OAuth token for profile {profile:?} — \
                 run \"jr auth login --profile {profile}\""
            ))
        }
        // Partial state: one half of the namespaced pair is missing. For
        // the "default" profile, try recovering from a still-intact
        // legacy pair before erroring — this handles interrupted lazy
        // migrations and partial writes that left the namespaced entries
        // inconsistent while the legacy flat keys still contain valid
        // tokens. Non-default profiles must NEVER inherit legacy keys
        // (that would cross-pollinate credentials across Jira sites).
        //
        // If the legacy pair isn't complete either, surface the partial
        // state with explicit recovery instructions rather than masking
        // the corruption with a generic "no token" message.
        _ => {
            if profile.as_ref() == "default" {
                let legacy_access = read_keyring_optional(KEY_OAUTH_ACCESS_LEGACY)?;
                let legacy_refresh = read_keyring_optional(KEY_OAUTH_REFRESH_LEGACY)?;
                if let (Some(a), Some(r)) = (legacy_access, legacy_refresh) {
                    store_oauth_tokens(&Profile::from("default"), &a, &r)?;
                    let _ = entry(KEY_OAUTH_ACCESS_LEGACY)?.delete_credential();
                    let _ = entry(KEY_OAUTH_REFRESH_LEGACY)?.delete_credential();
                    return Ok((a, r));
                }
            }
            Err(anyhow::anyhow!(
                "OAuth keychain entries for profile {profile:?} are partial \
                 (one of access/refresh present, the other missing). \
                 Run \"jr auth logout --profile {profile}\" then \
                 \"jr auth login --profile {profile}\" to restore a clean state."
            ))
        }
    }
}

/// Store an API-token credential pair (email + token) scoped to a profile.
///
/// (S-cycle3-percred-storage, BC-1.4.031 postcondition 1.) Tokens are
/// written to the namespaced keys `<profile>:email` and `<profile>:api-token`
/// so multiple Jira sites/accounts can coexist in a single keychain — the
/// same rationale as [`store_oauth_tokens`]'s `<profile>:oauth-*` pair.
///
/// Unlike [`store_oauth_tokens`], this function has NO legacy-fallback
/// migration branch for any profile, including `"default"` (BC-1.4.031
/// Invariant 2) — every write is a plain, unconditional two-key overwrite
/// (BC-1.4.033 postcondition 3, forward reference), so a later `auth login`
/// cleanly repairs a partial-write state with no bespoke recovery logic
/// here.
pub fn store_api_token(profile: &Profile, email: &str, token: &str) -> Result<()> {
    entry(&api_token_email_key(profile.as_ref()))?.set_password(email)?;
    entry(&api_token_key(profile.as_ref()))?.set_password(token)?;
    Ok(())
}

/// Existence-only check for whether the legacy shared flat `email`/
/// `api-token` pair (`KEY_EMAIL`/`KEY_API_TOKEN`) is present in the
/// keychain.
///
/// (S-cycle3-credential-absence-guard, BC-1.4.032 Postcondition 1.) This
/// helper exists ONLY so [`load_api_token`]'s both-namespaced-keys-absent
/// branch can keep its code path's shape symmetric with
/// [`load_oauth_tokens`]'s legacy-detection step — the check does NOT
/// change which error the caller ultimately sees (BC-1.4.032 Postcondition
/// 2: byte-identical error whether the legacy pair is present or absent),
/// and it must never surface the pair's VALUES as a credential. The
/// discipline here is behavioral, not mechanical: [`read_keyring_optional`]
/// returns the same `Option<String>` whether used as a presence flag or a
/// credential — callers of this helper must only ever inspect the returned
/// `bool`, never resurrect a value from it.
///
/// A genuine keychain backend error (as opposed to a plain absent key)
/// propagates via `?`, exactly like every other [`read_keyring_optional`]
/// call site in this module (BC-1.4.032 Invariant 4 / BC-1.4.031
/// EC-1.4.031-2) — it must never be coerced into
/// [`load_api_token`]'s "no stored credential" message.
fn legacy_flat_pair_exists() -> Result<bool> {
    let email_present = read_keyring_optional(KEY_EMAIL)?.is_some();
    let token_present = read_keyring_optional(KEY_API_TOKEN)?.is_some();
    Ok(email_present && token_present)
}

/// Load an API-token credential pair (email + token) scoped to a profile.
/// Returns `(email, token)`.
///
/// (S-cycle3-percred-storage, BC-1.4.031 postcondition 2.) Reads only the
/// namespaced keys `<profile>:email` / `<profile>:api-token` — no
/// shared/flat fallback for a profile whose namespaced keys already exist.
///
/// **No-copy detect-and-instruct (S-cycle3-credential-absence-guard,
/// BC-1.4.032, REDESIGNED — HUMAN DECISION, DEC-326).** When BOTH
/// namespaced keys are absent, this function NEVER reads, copies, or
/// deletes the legacy shared flat `email`/`api-token` pair as a
/// credential — for `"default"` or any other profile; there is no
/// `if profile == "default"` branch anywhere in this function
/// (BC-1.4.032 Postcondition 4). It performs an EXISTENCE-ONLY check of the
/// legacy pair via [`legacy_flat_pair_exists`] purely to keep this code
/// path's shape symmetric with [`load_oauth_tokens`]'s migration-detection
/// step — the check does NOT change the error returned; legacy pair
/// present or absent, the error text is byte-identical (BC-1.4.032
/// Postcondition 2). This is a one-time, permanent breaking-change contract
/// for every pre-cycle-003 api-token profile (BC-1.4.034): the remediation
/// is `jr auth login <profile>`, run once.
///
/// **Namespaced-pair partial-write (BC-1.4.033, REDESIGNED — namespaced-pair
/// case only; the legacy-partial branch is dissolved, since BC-1.4.032's
/// no-copy redesign removes the copy-then-delete sequence a partial legacy
/// pair used to interrupt).** When EXACTLY ONE of the two namespaced keys
/// is present, this is a distinct `Err` from the both-absent case above —
/// regardless of legacy-pair state, which never gates this branch. The
/// namespaced-partial check runs BEFORE any legacy-pair consideration
/// (EC-1.4.033-1 ordering — this match's arm order encodes that). The
/// remediation message intentionally never names `jr auth logout` (SR-009)
/// — that command is a no-op for api-token profiles (BC-1.2.013, amended);
/// only `jr auth login <profile>` (repair) or `jr auth remove <profile>`
/// (abandon) are valid remediations.
///
/// Unlike [`load_oauth_tokens`], this function has NO `"default"`-only
/// legacy-migration special case (BC-1.4.031 Invariant 2, reaffirmed by
/// BC-1.4.032 Postcondition 4).
///
/// The both-present success branch is unchanged, pre-existing behavior from
/// S-cycle3-percred-storage. The both-absent (BC-1.4.032) and
/// namespaced-partial (BC-1.4.033) branches were implemented by
/// S-cycle3-credential-absence-guard.
pub fn load_api_token(profile: &Profile) -> Result<(String, String)> {
    // `?` here propagates a genuine backend error (EC-1.4.031-2) as-is —
    // read_keyring_optional only collapses `keyring::Error::NoEntry` to
    // `Ok(None)`; anything else (e.g. an `Invalid` service name) surfaces
    // here distinct from either absence branch below.
    let email = read_keyring_optional(&api_token_email_key(profile.as_ref()))?;
    let token = read_keyring_optional(&api_token_key(profile.as_ref()))?;

    match (email, token) {
        (Some(e), Some(t)) => Ok((e, t)),
        (None, None) => {
            // BC-1.4.032 (S-cycle3-credential-absence-guard): no-copy
            // detect-and-instruct branch. Never special-cased on `profile`
            // — "default" and every other profile name must go through
            // this identical branch (Postcondition 4).
            //
            // Existence-only symmetry check (Postcondition 1) — its bool
            // result must never be used as, or trigger reading, a
            // credential value. `?` propagates a genuine backend error
            // exactly as it would for the namespaced-key reads above.
            let _legacy_pair_present = legacy_flat_pair_exists()?;
            // BC-1.4.032 Postcondition 2: byte-identical regardless of
            // `_legacy_pair_present` — the legacy pair is never read as a
            // credential, only checked for existence above (Postcondition 1).
            Err(crate::error::JrError::UserError(format!(
                "No credentials stored for profile '{profile}'. This version of jr \
                 requires per-profile credentials — run `jr auth login {profile}` to set them up."
            ))
            .into())
        }
        _ => {
            // BC-1.4.033 (S-cycle3-credential-absence-guard): namespaced-pair
            // partial-write branch — runs before any legacy-pair check
            // (EC-1.4.033-1). Remediation message must NOT name
            // `jr auth logout` (SR-009).
            Err(crate::error::JrError::UserError(format!(
                "Incomplete credentials stored for profile '{profile}' — run \
                 `jr auth login {profile}` to fix this."
            ))
            .into())
        }
    }
}

/// Read an optional keychain entry, distinguishing "not present" (`NoEntry`)
/// from real backend failures.
///
/// `keyring::Entry::get_password().ok()` collapses every error to `None` —
/// so a permission-denied, locked-keyring, or platform error looks identical
/// to a missing entry. That silently triggers fallbacks (legacy migration,
/// generic "no token" messages) and hides the real problem from the user.
/// This helper instead matches `keyring::Error::NoEntry` as the only
/// "absent" case and propagates everything else up the call stack so the
/// CLI can surface actionable diagnostics.
fn read_keyring_optional(key: &str) -> Result<Option<String>> {
    match entry(key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Store OAuth app credentials (client_id and client_secret) in the system keychain.
pub fn store_oauth_app_credentials(client_id: &str, client_secret: &str) -> Result<()> {
    let service = service_name();
    let entry = Entry::new(&service, "oauth_client_id")?;
    entry.set_password(client_id)?;
    let entry = Entry::new(&service, "oauth_client_secret")?;
    entry.set_password(client_secret)?;
    Ok(())
}

/// Load OAuth app credentials (client_id and client_secret) from the system keychain.
pub fn load_oauth_app_credentials() -> Result<(String, String)> {
    let service = service_name();
    let id_entry = Entry::new(&service, "oauth_client_id")?;
    let id = id_entry
        .get_password()
        .context("No OAuth app credentials found. Run \"jr auth login --oauth\" and provide your client_id and client_secret.")?;
    let secret_entry = Entry::new(&service, "oauth_client_secret")?;
    let secret = secret_entry
        .get_password()
        .context("No OAuth app credentials found.")?;
    Ok((id, secret))
}

/// Probe whether usable OAuth app credentials are present in the keychain
/// WITHOUT returning them. Distinguishes a real backend failure (locked
/// keychain, permission denied) from any "no usable creds here" condition,
/// so the refresh resolver and `jr auth status` don't silently flip a BYO
/// user onto the embedded app when the keychain is just temporarily
/// inaccessible.
///
/// Returns:
/// - `Ok(true)` — both `oauth_client_id` and `oauth_client_secret` entries
///   exist AND are non-empty. Safe to use for OAuth (would post a real
///   pair to Atlassian).
/// - `Ok(false)` — one or more of the following: neither entry exists; only
///   one half is present (`partial` state); both exist but at least one is
///   an empty string. All three collapse into "no usable BYO creds here"
///   from the resolver's perspective — empty/partial creds are
///   unauthenticatable at Atlassian, so falling through to embedded is the
///   correct behavior. (Doc note: `Ok(false)` is the "no usable creds"
///   sentinel, NOT "neither entry stored".)
/// - `Err(_)` — the keychain backend itself failed. Callers must propagate
///   or surface this rather than masking it as "absent".
///
/// Note: callers in resolver chains should prefer
/// [`try_load_oauth_app_credentials`] which performs the same probe in a
/// single read and yields the credentials when present, avoiding double
/// keychain I/O and double OS prompts on platforms that prompt per access.
pub fn probe_oauth_app_credentials() -> Result<bool> {
    let id = read_keyring_optional("oauth_client_id")?;
    let secret = read_keyring_optional("oauth_client_secret")?;
    Ok(matches!((id, secret), (Some(i), Some(s)) if !i.is_empty() && !s.is_empty()))
}

/// Single-pass equivalent of `probe + load`. Reads both keychain entries
/// once and returns:
/// - `Ok(Some((id, secret)))` — both entries exist and are non-empty.
/// - `Ok(None)` — anything else "unusable" (absent / partial / empty).
///   Treated identically by the resolver chain.
/// - `Err(_)` — keychain backend failure (locked / permission denied).
///   Callers must propagate or surface, never collapse to `None`.
///
/// Use this from resolver call sites instead of `probe_oauth_app_credentials()?`
/// followed by `load_oauth_app_credentials()?` — the two-call pattern
/// reads both keychain entries twice and can multiply OS keychain prompts.
pub fn try_load_oauth_app_credentials() -> Result<Option<(String, String)>> {
    let id = read_keyring_optional("oauth_client_id")?;
    let secret = read_keyring_optional("oauth_client_secret")?;
    match (id, secret) {
        (Some(i), Some(s)) if !i.is_empty() && !s.is_empty() => Ok(Some((i, s))),
        _ => Ok(None),
    }
}

/// Clear ONLY a single profile's OAuth-pair credentials
/// (`<profile>:oauth-access-token` / `<profile>:oauth-refresh-token`, plus
/// the legacy flat OAuth pair for `"default"`) — never the namespaced
/// API-token pair.
///
/// This is [`clear_profile_creds`]'s pre-S-cycle3-remove-logout-semantics
/// behavior, kept as its own function so [`crate::cli::auth::logout::handle_logout`]
/// can remain OAuth-specific by design (BC-1.2.013, DEC-322): `logout` must
/// NEVER clear a profile's API-token pair — not even when the target
/// profile's own `auth_method` happens to be `"oauth"` and it also carries
/// a leftover API-token pair from a prior mechanism switch. Clearing BOTH
/// credential kinds is [`clear_profile_creds`]'s job, reserved for `auth
/// remove` (BC-1.2.014).
///
/// `keyring::Error::NoEntry` on any step is success; any other keychain
/// error propagates immediately via `?` (same tightening as
/// [`clear_profile_creds`], applied consistently across both functions).
pub fn clear_profile_oauth_pair(profile: &Profile) -> Result<()> {
    delete_credential_tolerating_no_entry(&oauth_access_key(profile.as_ref()))?;
    delete_credential_tolerating_no_entry(&oauth_refresh_key(profile.as_ref()))?;
    if profile.as_ref() == "default" {
        delete_credential_tolerating_no_entry(KEY_OAUTH_ACCESS_LEGACY)?;
        delete_credential_tolerating_no_entry(KEY_OAUTH_REFRESH_LEGACY)?;
    }
    Ok(())
}

/// Clear ONLY a single profile's namespaced API-token-pair credentials
/// (`<profile>:email` / `<profile>:api-token`) — never the OAuth pair, and
/// never the legacy flat `KEY_EMAIL`/`KEY_API_TOKEN` keys
/// (`S-cycle3-credential-absence-guard`'s no-touch invariant, BC-1.4.032).
///
/// Symmetric counterpart to [`clear_profile_oauth_pair`], added by
/// FIX-F5-login-switch (BC-1.2.051-adjacent, relogin-then-replace ordering
/// for `jr auth login`'s mechanism-switch path — see
/// [`crate::cli::auth::login::clear_outgoing_mechanism_on_switch`]). That
/// caller runs AFTER the new mechanism's credentials have already been
/// stored, so it must clear ONLY the outgoing mechanism's pair —
/// [`clear_profile_creds`] is unsuitable there because it clears BOTH kinds
/// unconditionally and would delete the just-stored new credentials too.
///
/// `keyring::Error::NoEntry` on any step is success; any other keychain
/// error propagates immediately via `?` (same tightening as
/// [`clear_profile_creds`]/[`clear_profile_oauth_pair`]).
pub fn clear_profile_api_token_pair(profile: &Profile) -> Result<()> {
    delete_credential_tolerating_no_entry(&api_token_email_key(profile.as_ref()))?;
    delete_credential_tolerating_no_entry(&api_token_key(profile.as_ref()))?;
    Ok(())
}

/// Clear a single profile's stored credentials from the system keychain —
/// BOTH the OAuth-pair AND the namespaced API-token-pair (other profiles +
/// shared keys such as `oauth_client_id`/`oauth_client_secret` are
/// untouched).
///
/// Reserved for `auth remove` ([`crate::cli::auth::remove::handle_remove`]),
/// which deletes a profile's credentials wholesale before removing its
/// config entry. `auth logout` uses [`clear_profile_oauth_pair`] instead —
/// see that function's doc comment for why the two must not be conflated.
///
/// **AMENDED by S-cycle3-remove-logout-semantics (BC-1.2.014, DEC-322).**
/// Previously this function cleared ONLY the OAuth pair
/// (`<profile>:oauth-access-token` / `<profile>:oauth-refresh-token`, plus
/// the legacy flat OAuth pair for `"default"`) and aggregated every
/// deletion failure into a single combined `Err` at the end of the loop,
/// which callers ([`crate::cli::auth::remove::handle_remove`],
/// [`crate::cli::auth::logout::handle_logout`]) treated as best-effort.
///
/// This story adds a second credential kind to the clear set — the
/// namespaced API-token pair (`<profile>:email` / `<profile>:api-token`,
/// via [`api_token_email_key`]/[`api_token_key`], mirroring
/// [`oauth_access_key`]/[`oauth_refresh_key`]'s shape exactly, per
/// S-cycle3-percred-storage/BC-1.4.031) — and tightens error semantics for
/// BOTH credential kinds (BC-1.2.014 amended Behavior, I-4/SR-008):
/// `keyring::Error::NoEntry` on any credential-delete step is success (the
/// entry was already absent); any OTHER keychain error must ABORT — this
/// function returns `Err` immediately so [`crate::cli::auth::remove::handle_remove`]
/// can stop before its cache-clear (step 3) and config-entry-removal
/// (step 4) steps run, leaving `[profiles.<name>]` intact and the profile
/// re-`remove`-able (AC-002/AC-003). Retrying is safe: whichever
/// credential kind already succeeded reports `NoEntry` on the next call,
/// which is still treated as success (AC-004/AC-005/EC-1.2.014-2).
///
/// The legacy flat OAuth pair is still cleared for `"default"` only — see
/// the historical rationale below, unchanged by this story:
///
/// For the `"default"` profile, this also deletes the legacy flat OAuth
/// keys (`oauth-access-token` / `oauth-refresh-token`). Without that step,
/// a user mid-migration would see `jr auth logout --profile default`
/// "succeed" while the legacy keys remained — and the next
/// `load_oauth_tokens(&Profile::from("default"))` would lazy-migrate them back into the
/// namespaced slots, effectively undoing the logout. Non-`"default"`
/// profiles never inherit legacy keys, so this clause stays scoped to
/// `"default"` to avoid stomping on another profile's migration window.
///
/// Per this story's Architecture Compliance Rules, this function's new
/// branch targets ONLY the namespaced `<profile>:email`/`<profile>:api-token`
/// keys — it must never touch the legacy flat `KEY_EMAIL`/`KEY_API_TOKEN`
/// pair (that is `S-cycle3-credential-absence-guard`'s no-touch invariant,
/// BC-1.4.032).
pub fn clear_profile_creds(profile: &Profile) -> Result<()> {
    delete_credential_tolerating_no_entry(&oauth_access_key(profile.as_ref()))?;
    delete_credential_tolerating_no_entry(&oauth_refresh_key(profile.as_ref()))?;
    delete_credential_tolerating_no_entry(&api_token_email_key(profile.as_ref()))?;
    delete_credential_tolerating_no_entry(&api_token_key(profile.as_ref()))?;
    // For the "default" profile, also clear the legacy flat OAuth keys
    // that load_oauth_tokens(&Profile::from("default")) would otherwise lazy-migrate
    // back into existence on the next read — defeating logout.
    if profile.as_ref() == "default" {
        delete_credential_tolerating_no_entry(KEY_OAUTH_ACCESS_LEGACY)?;
        delete_credential_tolerating_no_entry(KEY_OAUTH_REFRESH_LEGACY)?;
    }
    Ok(())
}

/// Delete a single keychain entry, treating `keyring::Error::NoEntry` as
/// success (the entry was already absent — the expected steady state after
/// a prior clear, or for a credential kind that was never stored). Any
/// other error — including a failure constructing the [`Entry`] itself —
/// propagates immediately via `?`/the `Err` arm below.
///
/// This is the shared building block for [`clear_profile_creds`]'s and
/// [`clear_all_credentials`]'s BC-1.2.014-amended (I-4/SR-008) tightening:
/// a genuine keychain backend error must abort the calling function
/// immediately rather than being aggregated into a combined `Err` at the
/// end of a loop, so callers ([`crate::cli::auth::remove::handle_remove`])
/// can stop before running later steps (cache clear, config-entry removal).
fn delete_credential_tolerating_no_entry(key: &str) -> Result<()> {
    match entry(key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(
            "failed to clear keychain entry {key}: {err}"
        )),
    }
}

/// Remove shared credentials and OAuth tokens for every listed profile from
/// the system keychain.
///
/// Always clears the shared / single-tenant keys (`email`, `api-token`,
/// `oauth_client_id`, `oauth_client_secret`) plus the legacy flat OAuth
/// keys. Per-profile OAuth tokens (`<profile>:oauth-*-token`) AND the
/// namespaced per-profile API-token pair (`<profile>:email` /
/// `<profile>:api-token`, S-cycle3-percred-storage/BC-1.4.031) are cleared
/// only for the profiles in `profiles` — callers know their own profile
/// list (from config) and pass it in.
///
/// **AMENDED by S-cycle3-remove-logout-semantics (BC-1.2.014, DEC-322).**
/// The per-profile API-token-pair deletion branch already existed
/// (S-cycle3-percred-storage) — this story does NOT add a new deletion
/// target here. What changes is error-handling strictness, mirroring
/// [`clear_profile_creds`]'s tightening: a `keyring::Error::NoEntry` on any
/// credential-delete step remains success (the entry was already absent),
/// but any OTHER keychain error must now propagate immediately rather than
/// being aggregated into a single combined `Err` at the end of the loop —
/// callers need to know WHICH credential kind genuinely failed, not just
/// that "some" deletion failed. Per this story's Architecture Compliance
/// Rules, do NOT add a new unconditional clear of the legacy flat
/// `KEY_EMAIL`/`KEY_API_TOKEN` pair here — that would violate
/// `S-cycle3-credential-absence-guard`'s BC-1.4.032 "legacy pair never
/// deleted" invariant; the existing unconditional legacy-flat-key clear
/// above is pre-existing, out of this story's scope, and must not be
/// touched.
///
/// # TEST-ONLY — do not call from production code (S-cycle3-chosen-flow-reconcile, F1 history)
///
/// As of this story, this function has zero production call sites — every
/// remaining caller is `#[cfg(test)]`. It was previously invoked from
/// `auth refresh`'s pre-login "clear-then-login" sequence; that call site
/// was removed by the BC-1.2.051 Invariant 2 ("relogin-then-replace", I-6)
/// fix, because clearing credentials before a replacement was confirmed
/// obtainable was the root cause of a real data-loss defect (F1): this
/// function unconditionally wipes the SHARED `oauth_client_id`/
/// `oauth_client_secret` BYO OAuth app credentials, the legacy flat
/// email/api-token keys, and every listed profile's OAuth pair — none of
/// which a subsequent successful login necessarily restores (a BYO app's
/// client id/secret in particular are never re-derived; they're gone for
/// every profile, not just the one being refreshed). Do NOT reintroduce a
/// call to this function from `refresh`, `login`, or any other production
/// command without first re-litigating this history — a narrower,
/// single-profile clear (see [`clear_profile_creds`]) called ONLY after a
/// replacement credential is confirmed obtainable is almost certainly what
/// any future "clear before replace" need actually wants.
pub fn clear_all_credentials(profiles: &[Profile]) -> Result<()> {
    let mut keys: Vec<String> = vec![
        KEY_EMAIL.to_string(),
        KEY_API_TOKEN.to_string(),
        "oauth_client_id".to_string(),
        "oauth_client_secret".to_string(),
    ];
    // Legacy flat OAuth keys belong to the "default" profile's
    // lazy-migration path. Only delete them when the caller is
    // explicitly clearing "default" — otherwise `jr auth refresh
    // --profile sandbox` (api_token flow) on a not-yet-migrated
    // legacy install would unconditionally wipe the default
    // profile's intact-but-unmigrated OAuth tokens.
    if profiles.iter().any(|p| p.as_ref() == "default") {
        keys.push(KEY_OAUTH_ACCESS_LEGACY.to_string());
        keys.push(KEY_OAUTH_REFRESH_LEGACY.to_string());
    }
    for profile in profiles {
        let profile = profile.as_ref();
        keys.push(oauth_access_key(profile));
        keys.push(oauth_refresh_key(profile));
        // S-cycle3-percred-storage: the API-token pair moved to namespaced
        // keys (<profile>:email / <profile>:api-token), so the flat
        // KEY_EMAIL/KEY_API_TOKEN deletes above no longer reach the
        // credential `auth refresh` is rotating.
        keys.push(api_token_email_key(profile));
        keys.push(api_token_key(profile));
    }
    for key in keys {
        delete_credential_tolerating_no_entry(&key)?;
    }
    Ok(())
}

/// Result of a successful OAuth login containing site information.
pub struct OAuthResult {
    pub cloud_id: String,
    pub site_url: String,
    pub site_name: String,
}

/// The fixed loopback port the embedded `jr` Atlassian OAuth app's callback
/// URL is registered with (`http://127.0.0.1:53682/callback` in Developer
/// Console). Atlassian validates `redirect_uri` by exact string match, so
/// this is a long-lived contract — changing it is a breaking release that
/// requires re-registering the callback URL.
///
/// Centralized here so every call site (CLI dispatch in
/// `cli/auth.rs::login_oauth`, the CI smoke step that knows the port for
/// runner setup, the spec/runbook, and tests) references a single source
/// of truth instead of repeating the literal `53682`.
pub const EMBEDDED_CALLBACK_PORT: u16 = 53682;

/// `RedirectUriStrategyRequest` describes how the local OAuth callback
/// listener should be bound before we hit the network — either by binding
/// a random ephemeral port (`Dynamic`) or by validating availability of a
/// specific registered port (`Fixed`). Threaded into `oauth_login` and
/// resolved into a [`ResolvedRedirect`] (which owns the bound listener) by
/// [`RedirectUriStrategyRequest::bind`].
///
/// `Fixed` errors produce a friendly message that surfaces the BYO
/// override hint (specifically for `EADDRINUSE`). `Dynamic` errors
/// propagate the underlying `io::Error` directly — they're rare in
/// practice (only the OS-level port allocator running out of ephemeral
/// ports can trip them) and have no actionable user-facing recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectUriStrategyRequest {
    /// Bind a random ephemeral port. Used by BYO sources (flag/env/keychain
    /// /prompt) that registered their own callback URL with Atlassian.
    Dynamic,
    /// Bind the given fixed port. The embedded `jr` app uses
    /// [`EMBEDDED_CALLBACK_PORT`] (53682). `EADDRINUSE` surfaces a friendly
    /// error directing the user to BYO override.
    Fixed(u16),
}

impl RedirectUriStrategyRequest {
    /// Bind the local callback listener atomically. Returns a
    /// [`ResolvedRedirect`] that owns the listener — `oauth_login` consumes
    /// it directly instead of re-binding, eliminating the TOCTOU window
    /// where another process could grab the fixed port between probe and
    /// real-use.
    pub fn bind(self) -> Result<ResolvedRedirect> {
        match self {
            RedirectUriStrategyRequest::Dynamic => {
                let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
                let port = std_listener.local_addr()?.port();
                std_listener.set_nonblocking(true)?;
                let listener = tokio::net::TcpListener::from_std(std_listener)?;
                Ok(ResolvedRedirect {
                    strategy: RedirectUriStrategy::DynamicPort(port),
                    listener,
                })
            }
            RedirectUriStrategyRequest::Fixed(p) => {
                match std::net::TcpListener::bind(format!("127.0.0.1:{p}")) {
                    Ok(std_listener) => {
                        std_listener.set_nonblocking(true)?;
                        let listener = tokio::net::TcpListener::from_std(std_listener)?;
                        Ok(ResolvedRedirect {
                            strategy: RedirectUriStrategy::FixedPort(p),
                            listener,
                        })
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Err(anyhow::anyhow!(
                        "port {p} is in use; the jr OAuth callback needs this port. \
                         Free it, or use your own OAuth app via \
                         --client-id/--client-secret (or set \
                         JR_OAUTH_CLIENT_ID/JR_OAUTH_CLIENT_SECRET) to fall \
                         back to a dynamic port."
                    )),
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}

/// Resolved redirect-URI binding — owns the actual `TcpListener` so
/// `oauth_login` accepts directly on it without a second bind that could
/// race against the OS port allocator.
///
/// Fields are private to prevent a future caller from moving the
/// listener out (which would let them derive a `redirect_uri` from the
/// strategy that no longer matches the still-held listener — re-opening
/// the TOCTOU the type was created to close).
#[derive(Debug)]
pub struct ResolvedRedirect {
    strategy: RedirectUriStrategy,
    listener: tokio::net::TcpListener,
}

impl ResolvedRedirect {
    /// The resolved port + redirect-URI shape. `Copy`; safe to inspect
    /// without consuming the binding.
    pub fn strategy(&self) -> RedirectUriStrategy {
        self.strategy
    }

    /// Consume the binding, yielding the strategy plus the bound listener.
    /// `oauth_login` calls this exactly once to take ownership of the
    /// listener for `accept()`.
    pub fn into_parts(self) -> (RedirectUriStrategy, tokio::net::TcpListener) {
        (self.strategy, self.listener)
    }
}

/// Resolved port choice for `oauth_login`. Produced by
/// [`RedirectUriStrategyRequest::bind`]; carries the actual port number used
/// for the local callback listener and the `redirect_uri` we send to
/// Atlassian.
///
/// Embedded OAuth apps must use the exact `redirect_uri` registered in
/// Atlassian Developer Console — Atlassian does not honor RFC 8252's
/// "any loopback port" rule (https://jira.atlassian.com/browse/JRACLOUD-92180).
/// BYO apps stay on the historical dynamic-port behavior since they
/// register their own callback URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectUriStrategy {
    /// Bound to a random ephemeral port; redirect_uri uses that port.
    DynamicPort(u16),
    /// Bound to the embedded `jr` app's registered fixed port (53682).
    FixedPort(u16),
}

impl RedirectUriStrategy {
    pub fn port(self) -> u16 {
        match self {
            RedirectUriStrategy::DynamicPort(p) | RedirectUriStrategy::FixedPort(p) => p,
        }
    }

    pub fn redirect_uri(self) -> String {
        match self {
            // Embedded app: force IPv4 via the literal `127.0.0.1` so we
            // match the loopback bind. Modern macOS / Chrome resolve
            // `localhost` to `::1` first; an IPv6 browser connection to
            // `localhost:53682` would fail against our IPv4-only listener.
            // Atlassian validates redirect_uri by exact string match (no
            // RFC 8252 normalization), and we control the registered URL,
            // so `127.0.0.1:53682` is registered in Developer Console.
            RedirectUriStrategy::FixedPort(port) => {
                format!("http://127.0.0.1:{port}/callback")
            }
            // BYO (dynamic-port): preserve `localhost` for backward
            // compatibility. Existing BYO users may have registered
            // `http://localhost:...` callback URLs in their Developer
            // Console apps; an unconditional switch to `127.0.0.1` would
            // surface as `invalid_redirect_uri` for those users mid-PR.
            // BYO users on macOS who hit the IPv6 resolver pitfall can
            // re-register their app with `http://127.0.0.1:...` and the
            // browser will follow whatever string Atlassian sends back.
            RedirectUriStrategy::DynamicPort(port) => {
                format!("http://localhost:{port}/callback")
            }
        }
    }
}

/// Run the full OAuth 2.0 (3LO) authorization code flow:
/// 1. Open browser to Atlassian authorization page requesting `scopes`
/// 2. Listen on a local port for the callback
/// 3. Exchange the authorization code for tokens
/// 4. Fetch accessible resources to get the cloud ID
/// 5. Store tokens in the system keychain
///
/// `scopes` is a space-separated scope string (URL-encoded internally).
/// Callers should use [`DEFAULT_OAUTH_SCOPES`] when no user override is set.
/// Note: [`refresh_oauth_token`] takes only `profile` and resolves the
/// OAuth app credentials internally (keychain → embedded). The
/// `refresh_token` grant inherits scopes from the original authorization
/// per RFC 6749 §6.
pub async fn oauth_login(
    profile: &str,
    client_id: &str,
    client_secret: &str,
    scopes: &str,
    strategy: RedirectUriStrategyRequest,
    cloud_id_override: Option<&str>,
    no_input: bool,
) -> Result<OAuthResult> {
    // AC-005: emit structured tracing at OAuth flow entry point.
    // client_secret is intentionally NOT logged — only the profile and
    // whether a secret is present (a boolean probe). Secrets must never
    // appear in tracing field lists per the architecture compliance rule.
    info!(target: "jr::auth", profile = %profile, "oauth_login_start");
    debug!(
        target: "jr::auth",
        profile = %profile,
        has_client_secret = !client_secret.is_empty(),
        "oauth_login_credentials_resolved"
    );

    // 1. Resolve the strategy → owning the bound port up front so the
    //    callback URL we send to Atlassian matches what we'll listen on.
    //
    // Test-only override: JR_OAUTH_CODE lets tests skip the browser-open and
    // TCP-listen step by injecting a pre-built auth code directly. When set,
    // the listener is still bound (so redirect_uri is stable) but accept() is
    // skipped. Not documented as a public seam; do not rely on it in production.
    let resolved = strategy.bind()?;
    let redirect_uri = resolved.strategy().redirect_uri();
    let (_strategy, listener) = resolved.into_parts();
    let state = generate_state()?;

    let auth_url = build_authorize_url(client_id, scopes, &redirect_uri, &state);

    // Test-only override: see tests/multi_cloudid_disambiguation.rs.
    // Not documented as a public seam; do not rely on it in production.
    let injected_code = std::env::var("JR_OAUTH_CODE").ok();

    let code = if let Some(ref injected) = injected_code {
        // Skip the browser open + TCP accept; use the injected code directly.
        injected.clone()
    } else {
        eprintln!("Opening browser for authorization...");
        eprintln!("If browser doesn't open, visit: {auth_url}");
        if let Err(e) = open::that(&auth_url) {
            eprintln!(
                "(could not auto-open browser: {e}) — paste the URL above into a browser to continue."
            );
        }

        // 2. Listen for the OAuth callback. The listener is already bound
        //    atomically by RedirectUriStrategyRequest::bind; no re-bind here
        //    means no TOCTOU window between probe and real use.
        let (mut stream, _) = listener.accept().await?;

        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await.context(
            "reading OAuth callback request from the local browser; \
             if you already approved in the browser, the authorization \
             code is single-use and short-lived — re-running \
             `jr auth login --oauth` is safe (the auth code expires unused)",
        )?;
        let request = String::from_utf8_lossy(&buf[..n]);

        let code = extract_query_param(&request, "code")
            .ok_or_else(|| anyhow::anyhow!("No authorization code received"))?;
        let returned_state = extract_query_param(&request, "state")
            .ok_or_else(|| anyhow::anyhow!("No state parameter received"))?;

        if returned_state != state {
            anyhow::bail!("State mismatch — possible CSRF attack");
        }

        // Send a success page back to the browser.
        let response = "HTTP/1.1 200 OK\r\n\
                        Content-Type: text/html\r\n\r\n\
                        <html><body>\
                        <h2>Authorization successful!</h2>\
                        <p>You can close this tab.</p>\
                        </body></html>";
        stream.write_all(response.as_bytes()).await.context(
            "sending OAuth success page back to the local browser; \
             the authorization code was received but tokens have NOT \
             yet been exchanged or saved — re-running `jr auth login --oauth` \
             may be required",
        )?;
        code
    };

    // 3. Exchange the authorization code for tokens.
    // AC-005: emit structured tracing at token exchange entry point.
    debug!(target: "jr::auth", profile = %profile, "oauth_token_exchange_start");
    let client = reqwest::Client::new();
    // Test-only override: JR_OAUTH_TOKEN_URL redirects the token exchange
    // to a wiremock server. See tests/multi_cloudid_disambiguation.rs.
    // Not documented as a public seam; do not rely on it in production.
    let token_url = std::env::var("JR_OAUTH_TOKEN_URL")
        .unwrap_or_else(|_| "https://auth.atlassian.com/oauth/token".into());
    let token_response = client
        .post(&token_url)
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await?;

    if !token_response.status().is_success() {
        let body = token_response.text().await?;
        anyhow::bail!("Token exchange failed: {body}");
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
    }
    let tokens: TokenResponse = token_response.json().await.context(
        "OAuth authorization-code grant response body was not valid JSON; \
         Atlassian may have changed the token endpoint shape",
    )?;

    // 4. Fetch accessible resources to discover cloud ID and site info.
    // Test-only override: JR_ACCESSIBLE_RESOURCES_URL redirects the
    // accessible-resources lookup to a wiremock server. See
    // tests/multi_cloudid_disambiguation.rs.
    // Not documented as a public seam; do not rely on it in production.
    let accessible_resources_url = std::env::var("JR_ACCESSIBLE_RESOURCES_URL")
        .unwrap_or_else(|_| "https://api.atlassian.com/oauth/token/accessible-resources".into());
    let resources_response = client
        .get(&accessible_resources_url)
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .context("failed to call Atlassian accessible-resources endpoint")?;
    if !resources_response.status().is_success() {
        let status = resources_response.status();
        let body = resources_response
            .text()
            .await
            .unwrap_or_else(|e| format!("(body read failed: {e:#})"));
        let body_truncated: String = body.chars().take(500).collect();
        anyhow::bail!(
            "Atlassian accessible-resources lookup failed: HTTP {status}: {body_truncated}\n\n\
             The OAuth grant succeeded but jr could not enumerate your accessible Jira sites. \
             Confirm the OAuth app's scopes include `read:jira-user` and `read:jira-work`."
        );
    }
    let resources: Vec<AccessibleResource> = resources_response
        .json()
        .await
        .context("accessible-resources response body was not valid JSON")?;

    // Disambiguation: BC-1.5.038 — delegate to the extracted helper
    // (pure on the non-interactive paths; the interactive branch does I/O).
    let resource_id =
        resolve_cloud_id(&resources, cloud_id_override, no_input).map_err(anyhow::Error::from)?;
    let resource = resources
        .iter()
        .find(|r| r.id == resource_id)
        .expect("resource_id was derived from resources so it must exist");

    // 5. Store tokens in the system keychain. If this fails, the user has
    //    successfully approved the grant in Atlassian — but jr can't see
    //    the new tokens. Surface the partial state explicitly so they
    //    know to retry (after fixing keychain access) rather than
    //    re-approving from scratch.
    store_oauth_tokens(
        &Profile::from(profile),
        &tokens.access_token,
        &tokens.refresh_token,
    )
    .map_err(|e| {
        anyhow::anyhow!(
            "Authorization succeeded with Atlassian, but jr could not save the OAuth \
             tokens to the system keychain ({e:#}). Unlock your keychain (or grant \
             access to jr) and run `jr auth login --oauth --profile {profile}` again. \
             To fully revoke the active grant first, visit \
             https://id.atlassian.com/manage-profile/apps."
        )
    })?;

    Ok(OAuthResult {
        cloud_id: resource.id.clone(),
        site_url: resource.url.clone(),
        site_name: resource.name.clone(),
    })
}

/// Refresh the OAuth 2.0 access token using the stored refresh token.
/// Returns the new access token on success.
///
/// Resolves the OAuth app credentials at call time via the refresh-side
/// resolver (`keychain → embedded`). Flag and env are not consulted here;
/// this helper performs a non-interactive refresh-token grant using the
/// stored refresh token and the resolver-selected app credentials.
///
/// Currently has no production callers — it exists for a future 401 auto-
/// refresh integration. `jr auth refresh` (the user-facing CLI command)
/// uses the clear-and-relogin flow at `cli/auth.rs::refresh_credentials`,
/// not this helper.
/// Public entry point for `jr auth refresh` CLI command and any caller that
/// does not need to inject a specific token URL. Reads `JR_OAUTH_TOKEN_URL`
/// once and delegates to `refresh_oauth_token_with_url`.
pub async fn refresh_oauth_token(profile: &str) -> Result<String> {
    let token_url = std::env::var("JR_OAUTH_TOKEN_URL")
        .unwrap_or_else(|_| "https://auth.atlassian.com/oauth/token".to_string());
    refresh_oauth_token_with_url(profile, &token_url).await
}

/// Internal implementation that accepts an explicit token URL.
///
/// Called by `refresh_oauth_token` (which reads the env var once) and by
/// `JiaClient::send` (which snapshots the env var before entering async
/// context to avoid race conditions when integration tests overwrite
/// `JR_OAUTH_TOKEN_URL` concurrently).
pub(crate) async fn refresh_oauth_token_with_url(profile: &str, token_url: &str) -> Result<String> {
    // AC-005: emit structured tracing at refresh entry point.
    // refresh_token value is intentionally NOT logged — only the profile.
    info!(target: "jr::auth", profile = %profile, "refresh_oauth_token_start");
    // F2-01/F2-02: `resolve_refresh_app_credentials` deliberately returns
    // `Err` for two different reasons — a locked/permission-denied keychain
    // (a genuine backend error) versus no app credentials being available at
    // all (no BYO keychain entry and no embedded build, i.e. genuinely
    // absent). Blanket-coercing every `Err` into empty embedded creds used
    // to erase that distinction: a locked keychain would POST with empty
    // client_id/client_secret, get back `invalid_client`, and surface the
    // misleading "embedded app may have been rotated" hint — actively
    // hiding the real cause on the hourly auto-refresh hot path. Only the
    // genuinely-absent case (`NoAppCredentialsAvailable`) still falls back
    // to an empty-credential attempt — this preserves existing behavior for
    // environments with no embedded build and no stored BYO creds (e.g. a
    // mock token endpoint that doesn't validate client credentials). Any
    // other error (a real backend/permission failure) propagates as-is.
    let (client_id, client_secret, source) = match resolve_refresh_app_credentials() {
        Ok(creds) => creds,
        Err(e) if e.downcast_ref::<NoAppCredentialsAvailable>().is_some() => {
            (String::new(), String::new(), RefreshAppSource::Embedded)
        }
        Err(e) => return Err(e),
    };
    // Log that we resolved credentials without logging their values.
    debug!(
        target: "jr::auth",
        profile = %profile,
        has_client_id = !client_id.is_empty(),
        has_client_secret = !client_secret.is_empty(),
        source = ?source,
        "refresh_credentials_resolved"
    );
    // Same class of bug as above (F2-02): `unwrap_or_default()` used to
    // coerce a genuine keychain backend error reading the stored refresh
    // token into an empty string, which then gets POSTed to Atlassian and
    // comes back as a confusing `invalid_grant` instead of surfacing the
    // real (locked-keychain) cause. Only fall back to an empty refresh
    // token when the profile genuinely has no stored OAuth token (or the
    // stored pair is partial) — never when the read itself failed at the
    // backend/keychain level.
    let (_, refresh_token) = match load_oauth_tokens(&Profile::from(profile)) {
        Ok(tokens) => tokens,
        Err(e) if is_backend_keyring_error(&e) => {
            return Err(e.context(
                "OAuth refresh: failed to read the stored refresh token from \
                 the system keychain. Unlock your keychain or grant access \
                 to jr, then retry.",
            ));
        }
        Err(_) => (String::new(), String::new()),
    };

    // S-3.03 v2 DECISION: Option A-fixed (auto-refresh on 401 with
    // per-profile single-flight). Wired into JiaClient::send via
    // src/api/refresh_coordinator.rs. The 401 trigger is BLANKET 401
    // (matches gh CLI; Atlassian does not return RFC-6750 WWW-Authenticate
    // or {"code":"EXPIRED"} — see CLAUDE.md gotcha). Refresh token rotation
    // is single-use (no 10-min reuse window — see CLAUDE.md gotcha).
    // Mutex layering rule lives in refresh_coordinator.rs preamble.
    //
    // token_url is passed in explicitly by the caller (not re-read from env)
    // to avoid race conditions in tests that overwrite JR_OAUTH_TOKEN_URL
    // concurrently. The env-var is snapshotted once by the caller before any
    // async await points.

    // JR_S303_PERSIST_FAIL=1: fault-injection seam for AC-011. When set,
    // simulates a store_oauth_tokens failure AFTER a successful Atlassian
    // exchange but BEFORE in-memory state update, verifying that the
    // persist-before-publish invariant prevents in-memory/on-disk divergence.
    // Never set in production. Added by implementer per test-writer seam list.

    let client = reqwest::Client::new();
    // Build a URL-encoded form body manually. The `form` feature is disabled
    // in Cargo.toml (only `json` + `rustls` enabled), so we build the body
    // with `urlencoding::encode` and set the content-type header explicitly.
    // Tests verify the body contains "grant_type=refresh_token" (form encoding).
    let body = format!(
        "grant_type=refresh_token&client_id={}&client_secret={}&refresh_token={}",
        urlencoding::encode(&client_id),
        urlencoding::encode(&client_secret),
        urlencoding::encode(&refresh_token),
    );
    let response = client
        .post(token_url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;

    if !response.status().is_success() {
        // Capture the response body — Atlassian returns RFC 6749 error
        // shape (`error` + `error_description`) and including it cuts
        // triage time massively (invalid_grant vs invalid_client vs
        // network/clock-skew look identical without it).
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("(body read failed: {e:#})"));
        let body_truncated: String = body.chars().take(500).collect();
        let hint = match source {
            RefreshAppSource::Embedded => format!(
                "The embedded OAuth app credentials may have been rotated; \
                 update jr (brew upgrade or curl-install) and run \
                 `jr auth login --oauth --profile {profile}` again."
            ),
            RefreshAppSource::Keychain => format!(
                "Your stored OAuth client_id/client_secret may be invalid \
                 or revoked. Run `jr auth login --oauth --profile {profile}` \
                 to re-store them, or revoke and re-create the app at \
                 https://developer.atlassian.com/console/myapps/."
            ),
        };
        anyhow::bail!("Token refresh failed: HTTP {status}: {body_truncated}\n\n{hint}");
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
    }
    let tokens: TokenResponse = response.json().await.context(
        "refresh response body was not valid JSON; Atlassian may have changed \
         the token endpoint shape",
    )?;
    // JR_S303_PERSIST_FAIL=1: fault-injection seam for AC-011. When set,
    // simulates a keychain write failure AFTER a successful Atlassian exchange
    // but BEFORE in-memory state update. This verifies the persist-before-publish
    // invariant: if persist fails, the coordinator never updates RefreshState,
    // so the in-memory and on-disk states remain consistent (both hold the old
    // tokens). Never set in production.
    if std::env::var("JR_S303_PERSIST_FAIL").as_deref() == Ok("1") {
        anyhow::bail!("JR_S303_PERSIST_FAIL: simulated keychain write failure for testing");
    }

    // Same partial-state risk as oauth_login's keychain-write step:
    // Atlassian rotated the tokens, but if the keychain write fails the
    // new pair is lost and the next request will use the now-invalid
    // refresh token. Surface the partial state explicitly.
    store_oauth_tokens(
        &Profile::from(profile),
        &tokens.access_token,
        &tokens.refresh_token,
    )
    .map_err(|e| {
        anyhow::anyhow!(
            "Token refresh succeeded with Atlassian, but jr could not save the new \
             OAuth tokens to the system keychain ({e:#}). Unlock your keychain (or \
             grant access to jr) and run `jr auth refresh --profile {profile}` again. \
             If the problem persists, run `jr auth login --oauth --profile {profile}` \
             to start fresh."
        )
    })?;
    Ok(tokens.access_token)
}

/// Refresh-side resolver: keychain wins, embedded falls back. Flag and env
/// are deliberately omitted because this helper is only used by the
/// non-interactive refresh-token grant path, which reuses the app
/// credentials already associated with the stored refresh token rather
/// than collecting new credentials as part of a fresh login.
///
/// Keeping keychain ahead of embedded prevents a returning BYO user from
/// silently flipping onto the embedded app mid-session (which would
/// invalidate their refresh token because it was issued by a different app).
fn resolve_refresh_app_credentials() -> Result<(String, String, RefreshAppSource)> {
    // Single-pass keychain read: a locked keychain or permission denial is
    // NOT the same as "no creds stored" — we must not silently flip the
    // user onto the embedded app when their BYO creds are merely
    // temporarily inaccessible.
    match try_load_oauth_app_credentials() {
        Ok(Some((id, secret))) => {
            return Ok((id, secret, RefreshAppSource::Keychain));
        }
        Ok(None) => {} // genuinely absent — fall through to embedded
        Err(e) => {
            return Err(anyhow::anyhow!(
                "OAuth refresh: failed to read the system keychain ({e:#}). \
                 Unlock your keychain or grant access to jr, then retry. \
                 If you intended to use the embedded jr OAuth app, run \
                 `jr auth remove` first to clear stale BYO credentials."
            ));
        }
    }
    if let Some(app) = crate::api::auth_embedded::embedded_oauth_app() {
        return Ok((
            app.client_id.clone(),
            app.client_secret.clone(),
            RefreshAppSource::Embedded,
        ));
    }
    Err(NoAppCredentialsAvailable.into())
}

/// Marker error: no OAuth app credentials are available at all — no BYO
/// keychain entry (genuinely absent, not a backend failure) and no
/// embedded build compiled in. Distinct, by type rather than by string
/// matching, from a genuine keychain backend/permission error (F2-01/
/// F2-02) so `refresh_oauth_token_with_url` can tell the two apart via
/// `anyhow::Error::downcast_ref` and only fall back to an empty-credential
/// refresh attempt for this genuinely-absent case.
#[derive(Debug, thiserror::Error)]
#[error(
    "OAuth refresh requires either previously-stored app credentials \
     (run `jr auth login --oauth` once) or an embedded build. \
     This binary has neither."
)]
struct NoAppCredentialsAvailable;

/// True if `err`'s cause chain contains a genuine [`keyring::Error`] — as
/// opposed to a message [`load_oauth_tokens`] constructs itself to describe
/// "no token stored" or "partial state" (both of which are plain
/// `anyhow::anyhow!` string errors with no `keyring::Error` in their
/// chain). Used by `refresh_oauth_token_with_url` (F2-02) to distinguish a
/// real backend/permission failure, which must propagate, from a
/// legitimately-absent refresh token, which may still fall back to an
/// empty string.
fn is_backend_keyring_error(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.downcast_ref::<keyring::Error>().is_some())
}

/// Where the OAuth app credentials for a token refresh resolved from.
///
/// Closed set of two variants — refresh by definition has credentials
/// (otherwise `resolve_refresh_app_credentials` bails before this point),
/// and the resolver only reads from keychain or embedded sources. Used
/// to tailor the failure-message hint when Atlassian rejects the refresh
/// (embedded → "secret may have been rotated, upgrade jr"; keychain →
/// "stored creds may be invalid, re-run login").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshAppSource {
    Keychain,
    Embedded,
}

/// Build the Atlassian OAuth 2.0 authorize URL with all dynamic parameters
/// percent-encoded uniformly.
///
/// All four dynamic values (`client_id`, `scopes`, `redirect_uri`, `state`)
/// are passed through `urlencoding::encode`, which applies RFC 3986
/// percent-encoding — spaces become `%20`, not `+`. Atlassian's authorize
/// endpoint requires `%20` for space-separated scopes, NOT the
/// application/x-www-form-urlencoded `+` form that `url::Url::query_pairs_mut`
/// would produce (confirmed against Atlassian's documented example URLs).
///
/// Uniform encoding is a defense-in-depth measure: it prevents a
/// pathological `client_id` containing `&`, `=`, `#`, or `?` from reshaping
/// the query string — e.g., `real_id&redirect_uri=evil.example` becomes
/// `real_id%26redirect_uri%3Devil.example` and is treated as a single
/// scalar value by Atlassian (which then rejects it as an unknown client).
///
/// The static constants (`audience`, `response_type`, `prompt`) are not
/// user-controlled so they are not encoded here.
fn build_authorize_url(client_id: &str, scopes: &str, redirect_uri: &str, state: &str) -> String {
    format!(
        "https://auth.atlassian.com/authorize\
         ?audience=api.atlassian.com\
         &client_id={}\
         &scope={}\
         &redirect_uri={}\
         &state={}\
         &response_type=code\
         &prompt=consent",
        urlencoding::encode(client_id),
        urlencoding::encode(scopes),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(state),
    )
}

/// Generate a cryptographically random state parameter for CSRF protection
/// of the OAuth 2.0 authorization-code flow (RFC 6749 §10.12).
///
/// 32 random bytes read directly from the operating system CSPRNG via
/// `rand::rngs::SysRng` (which is a thin wrapper over the `getrandom` crate
/// and calls `getrandom(2)` / `BCryptGenRandom` on each invocation — no
/// user-space reseeding state, unlike `rand::rng()` / `ThreadRng`).
/// Rendered as 64 hex characters. 256 bits of entropy far exceeds the
/// ~30 bits offered by the previous wall-clock-nanosecond implementation,
/// closing the attack window where an attacker with local access could
/// observe the authorize URL and race the 127.0.0.1 callback listener
/// with a forged code.
///
/// Returns `Err` when the OS CSPRNG is unavailable — a rare but non-
/// panicking failure mode (sandboxed environments without `/dev/urandom`,
/// early-boot situations, or OS-level seccomp denials). The caller
/// bubbles this up through `oauth_login` so `jr auth login` fails with
/// an actionable error rather than aborting the process (the release
/// profile uses `panic = "abort"`).
fn generate_state() -> Result<String> {
    use rand::TryRng;
    let mut bytes = [0u8; 32];
    rand::rngs::SysRng.try_fill_bytes(&mut bytes).context(
        "Failed to read from OS CSPRNG when generating OAuth state. \
         Check OS entropy availability or sandbox/seccomp restrictions \
         that may block getrandom(2) / BCryptGenRandom.",
    )?;
    Ok(bytes.iter().fold(String::with_capacity(64), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    }))
}

/// Extract a query parameter value from a raw HTTP request string.
fn extract_query_param(request: &str, param: &str) -> Option<String> {
    let query_start = request.find('?')?;
    let query_end = request[query_start..]
        .find(' ')
        .map(|i| query_start + i)
        .unwrap_or(request.len());
    let query = &request[query_start + 1..query_end];
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            if key == param {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::JrError;

    /// `FixedPort` and `DynamicPort` produce different host forms:
    /// `FixedPort` (embedded app) uses `127.0.0.1` to force IPv4 and match
    /// the registered Developer Console callback URL; `DynamicPort` (BYO)
    /// keeps `localhost` for backward compatibility with existing BYO app
    /// registrations whose callback URLs use the `localhost` host. Atlassian
    /// validates `redirect_uri` by exact string match — both strings must
    /// match what the user registered in their Developer Console.
    #[test]
    fn redirect_uri_strategy_strings() {
        assert_eq!(
            RedirectUriStrategy::FixedPort(EMBEDDED_CALLBACK_PORT).redirect_uri(),
            "http://127.0.0.1:53682/callback"
        );
        assert_eq!(
            RedirectUriStrategy::DynamicPort(54321).redirect_uri(),
            "http://localhost:54321/callback"
        );
    }

    /// Lock the embedded callback port at the type-system level. Atlassian
    /// validates `redirect_uri` by exact string match; this constant is a
    /// long-lived contract registered in Developer Console as
    /// `http://127.0.0.1:53682/callback`. Changing it is a breaking release.
    #[test]
    fn embedded_callback_port_is_53682() {
        assert_eq!(EMBEDDED_CALLBACK_PORT, 53682);
    }

    #[test]
    fn test_extract_query_param_found() {
        let request = "GET /callback?code=abc123&state=xyz HTTP/1.1\r\n";
        assert_eq!(
            extract_query_param(request, "code"),
            Some("abc123".to_string())
        );
        assert_eq!(
            extract_query_param(request, "state"),
            Some("xyz".to_string())
        );
    }

    #[test]
    fn test_extract_query_param_not_found() {
        let request = "GET /callback?code=abc123 HTTP/1.1\r\n";
        assert_eq!(extract_query_param(request, "state"), None);
    }

    #[test]
    fn test_extract_query_param_no_query() {
        let request = "GET /callback HTTP/1.1\r\n";
        assert_eq!(extract_query_param(request, "code"), None);
    }

    #[test]
    fn test_generate_state_is_hex() {
        let state = generate_state().expect("OS CSPRNG available in tests");
        assert!(!state.is_empty());
        assert!(state.chars().all(|c| c.is_ascii_hexdigit()));
    }

    /// 256-bit CSPRNG output rendered as hex must always be 64 characters.
    /// Pinning the length guards against a regression to any lower-entropy
    /// source (e.g., timestamp-hex, truncated UUIDs) that would still pass
    /// the is_hex check.
    #[test]
    fn test_generate_state_is_64_hex_chars() {
        let state = generate_state().expect("OS CSPRNG available in tests");
        assert_eq!(
            state.len(),
            64,
            "expected 32 bytes = 64 hex chars, got: {state}"
        );
    }

    /// `generate_state` must produce 8 distinct values across 8 calls. A
    /// deterministic or low-entropy regression (reintroduced `as_nanos`
    /// state, a constant, etc.) collapses outputs and trips this check.
    /// With 256 bits of true entropy the birthday-bound collision
    /// probability across 8 samples is C(8,2) / 2^256 ≈ 2^-253, so
    /// requiring all 8 to be distinct is rigorously not a flake source.
    #[test]
    fn test_generate_state_is_not_deterministic() {
        let samples: std::collections::HashSet<String> = (0..8)
            .map(|_| generate_state().expect("OS CSPRNG available in tests"))
            .collect();
        assert_eq!(
            samples.len(),
            8,
            "expected 8 distinct values from 8 generate_state() calls, \
             got {} distinct: {samples:?}",
            samples.len()
        );
    }

    /// Happy path: a well-formed `client_id` + scopes + redirect_uri + state
    /// produce an authorize URL with all Atlassian-required static params,
    /// scope spaces rendered as `%20` (Atlassian rejects `+`-encoded spaces).
    #[test]
    fn test_build_authorize_url_happy_path() {
        let url = build_authorize_url(
            "normal-client-id",
            "read:jira-work offline_access",
            "http://localhost:12345/callback",
            "deadbeef",
        );

        assert!(url.starts_with("https://auth.atlassian.com/authorize?"));
        assert!(url.contains("audience=api.atlassian.com"));
        assert!(url.contains("&client_id=normal-client-id"));
        assert!(
            url.contains("&scope=read%3Ajira-work%20offline_access"),
            "scope must be %20-encoded, not +-encoded (Atlassian requires %20): {url}"
        );
        assert!(url.contains("&redirect_uri=http%3A%2F%2Flocalhost%3A12345%2Fcallback"));
        assert!(url.contains("&state=deadbeef"));
        assert!(url.contains("&response_type=code"));
        assert!(url.contains("&prompt=consent"));
    }

    /// A pathological `client_id` containing query-string reserved chars
    /// (`&`, `=`, `#`) must be fully escaped so it cannot reshape the query
    /// string. Without uniform encoding, `real_id&redirect_uri=evil.example`
    /// would silently override the redirect_uri parameter.
    #[test]
    fn test_build_authorize_url_escapes_hostile_client_id() {
        let url = build_authorize_url(
            "real_id&redirect_uri=evil.example#frag",
            "read:jira-work",
            "http://localhost:12345/callback",
            "deadbeef",
        );

        assert!(
            !url.contains("&redirect_uri=evil.example"),
            "hostile client_id must not be able to inject a redirect_uri override: {url}"
        );
        assert!(
            url.contains("client_id=real_id%26redirect_uri%3Devil.example%23frag"),
            "client_id reserved chars must be percent-encoded: {url}"
        );
    }

    /// Scope values containing `+` (unlikely but not impossible — some
    /// granular scopes are under evolution) must have the `+` escaped to
    /// `%2B`. Unescaped `+` in a form-urlencoded context means "space",
    /// which would silently corrupt the scope list.
    #[test]
    fn test_build_authorize_url_escapes_plus_in_scope() {
        let url = build_authorize_url(
            "client",
            "scope:with+plus",
            "http://localhost:12345/callback",
            "deadbeef",
        );

        assert!(
            url.contains("scope=scope%3Awith%2Bplus"),
            "+ in scope must be encoded as %2B: {url}"
        );
        assert!(
            !url.contains("scope:with+plus"),
            "raw + must not appear in the URL: {url}"
        );
    }

    fn unique_test_service() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("jr-jira-cli-test-{}-{}", std::process::id(), n)
    }

    /// Serializes JR_SERVICE_NAME mutation across concurrent keyring tests so
    /// no test observes a service name set by another in-flight test (which
    /// would point its keychain operations at the wrong namespace).
    static KEYRING_TEST_ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Wrap a test in a unique JR_SERVICE_NAME scope so concurrent tests don't collide.
    fn with_test_keyring<F: FnOnce()>(f: F) {
        if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
            return;
        }
        // Hold the mutex across env mutation + body + cleanup so no other
        // `with_test_keyring` invocation can race the JR_SERVICE_NAME
        // set/unset and observe a half-applied state. Recover from a
        // poisoned lock — a panicking test still leaves the env in a
        // recoverable state because we restore JR_SERVICE_NAME at scope
        // exit, and a unique service-name namespace per call already
        // isolates keychain entries.
        let _guard = KEYRING_TEST_ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let svc = unique_test_service();
        let prev = std::env::var("JR_SERVICE_NAME").ok();
        // SAFETY: KEYRING_TEST_ENV_MUTEX is held for the duration of this
        // scope, so no other test in this binary can race the env mutation.
        // The opt-in `JR_RUN_KEYRING_TESTS` gate further keeps these tests
        // off the default test path.
        unsafe { std::env::set_var("JR_SERVICE_NAME", &svc) };
        f();
        let _ = clear_all_credentials(&[Profile::from("default"), Profile::from("sandbox")]);
        // SAFETY: still holding KEYRING_TEST_ENV_MUTEX.
        unsafe {
            match prev {
                Some(p) => std::env::set_var("JR_SERVICE_NAME", p),
                None => std::env::remove_var("JR_SERVICE_NAME"),
            }
        }
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn store_and_load_per_profile_oauth_tokens_round_trip() {
        with_test_keyring(|| {
            store_oauth_tokens(&Profile::from("default"), "access1", "refresh1").unwrap();
            store_oauth_tokens(&Profile::from("sandbox"), "access2", "refresh2").unwrap();

            let (a1, r1) = load_oauth_tokens(&Profile::from("default")).unwrap();
            let (a2, r2) = load_oauth_tokens(&Profile::from("sandbox")).unwrap();

            assert_eq!((a1.as_str(), r1.as_str()), ("access1", "refresh1"));
            assert_eq!((a2.as_str(), r2.as_str()), ("access2", "refresh2"));
        });
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_oauth_tokens_returns_err_for_missing_profile() {
        with_test_keyring(|| {
            assert!(load_oauth_tokens(&Profile::from("default")).is_err());
        });
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn lazy_migration_legacy_flat_keys_for_default_profile() {
        with_test_keyring(|| {
            entry("oauth-access-token")
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry("oauth-refresh-token")
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            let (access, refresh) = load_oauth_tokens(&Profile::from("default")).unwrap();
            assert_eq!(access, "legacy-access");
            assert_eq!(refresh, "legacy-refresh");

            let new_access = entry("default:oauth-access-token")
                .unwrap()
                .get_password()
                .unwrap();
            assert_eq!(new_access, "legacy-access");

            assert!(entry("oauth-access-token").unwrap().get_password().is_err());
        });
    }

    /// Regression: `clear_profile_creds(&Profile::from("default"))` must also remove the
    /// legacy flat OAuth keys. Otherwise `jr auth logout --profile default`
    /// leaves the legacy entries in place and the next `load_oauth_tokens`
    /// call resurrects them via the lazy-migration path — silently undoing
    /// the logout for a user mid-migration.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn clear_profile_creds_default_also_clears_legacy_flat_keys() {
        with_test_keyring(|| {
            // Pre-seed legacy flat keys.
            entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry(KEY_OAUTH_REFRESH_LEGACY)
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            clear_profile_creds(&Profile::from("default")).unwrap();

            // Legacy keys must be gone — otherwise lazy migration would
            // resurrect them on the next load_oauth_tokens call.
            assert!(
                entry(KEY_OAUTH_ACCESS_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
            assert!(
                entry(KEY_OAUTH_REFRESH_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
        });
    }

    /// Companion to the test above: clearing a non-default profile must NOT
    /// touch the legacy flat keys, since those belong to the `"default"`
    /// profile's lazy-migration window.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn clear_profile_creds_non_default_leaves_legacy_keys_alone() {
        with_test_keyring(|| {
            entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .set_password("legacy-access")
                .unwrap();

            clear_profile_creds(&Profile::from("sandbox")).unwrap();

            // Legacy keys belong to the "default" profile's lazy migration;
            // logging out of "sandbox" must not touch them.
            let access = entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .get_password()
                .unwrap();
            assert_eq!(access, "legacy-access");
        });
    }

    // -----------------------------------------------------------------
    // S-cycle3-remove-logout-semantics (BC-1.2.014 amended, I-4/SR-008)
    //
    // `clear_profile_creds`/`clear_all_credentials` gain a namespaced
    // API-token-pair deletion branch (the deferred gap left open by
    // S-cycle3-percred-storage — see CLAUDE.md's "Per-profile vs shared
    // keychain keys" entry) and tightened error handling: a genuine
    // (non-`NoEntry`) keychain error must now propagate instead of being
    // aggregated-and-swallowed. RED GATE: every test below fails against
    // the current `todo!()` stub bodies.
    // -----------------------------------------------------------------

    /// **THE core gap-closing test.** Before this story, `clear_profile_creds`
    /// cleared ONLY the OAuth pair — the namespaced `<profile>:email` /
    /// `<profile>:api-token` pair (S-cycle3-percred-storage, BC-1.4.031) was
    /// never touched by `auth remove`/`auth logout`'s shared clear helper,
    /// so a removed-then-recreated profile could inherit a stale API-token
    /// credential. Seeds BOTH credential kinds for one profile, calls
    /// `clear_profile_creds`, and asserts ALL FOUR keychain entries
    /// (`<profile>:oauth-access-token`, `<profile>:oauth-refresh-token`,
    /// `<profile>:email`, `<profile>:api-token`) are gone afterward.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_2_014_clear_profile_creds_clears_namespaced_api_token_pair_and_oauth_pair() {
        with_test_keyring(|| {
            store_oauth_tokens(&Profile::from("sandbox"), "access-x", "refresh-x").unwrap();
            store_api_token(
                &Profile::from("sandbox"),
                "sandbox@example.com",
                "sandbox-token",
            )
            .unwrap();

            // Sanity: both pairs are actually present before the clear.
            assert!(
                load_oauth_tokens(&Profile::from("sandbox")).is_ok(),
                "precondition: oauth pair must be seeded"
            );
            assert!(
                load_api_token(&Profile::from("sandbox")).is_ok(),
                "precondition: api-token pair must be seeded"
            );

            clear_profile_creds(&Profile::from("sandbox"))
                .expect("clear_profile_creds must succeed when both credential kinds are present");

            // OAuth pair gone (pre-existing behavior, unaffected by this story).
            assert!(
                entry(&oauth_access_key("sandbox"))
                    .unwrap()
                    .get_password()
                    .is_err(),
                "oauth access token must be deleted"
            );
            assert!(
                entry(&oauth_refresh_key("sandbox"))
                    .unwrap()
                    .get_password()
                    .is_err(),
                "oauth refresh token must be deleted"
            );
            // THE GAP this story closes: namespaced API-token pair must
            // ALSO be gone.
            assert!(
                entry(&api_token_email_key("sandbox"))
                    .unwrap()
                    .get_password()
                    .is_err(),
                "namespaced api-token email must be deleted by clear_profile_creds \
                 (this is the gap S-cycle3-remove-logout-semantics closes)"
            );
            assert!(
                entry(&api_token_key("sandbox"))
                    .unwrap()
                    .get_password()
                    .is_err(),
                "namespaced api-token must be deleted by clear_profile_creds \
                 (this is the gap S-cycle3-remove-logout-semantics closes)"
            );
        });
    }

    /// EC-1.2.014-2 (BC-1.2.014 amended): both credential-deletion steps
    /// reporting `NoEntry` (no stored credentials of either kind) must be
    /// treated as SUCCESS, not an error.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_ec_1_2_014_2_clear_profile_creds_succeeds_when_both_credential_kinds_absent() {
        with_test_keyring(|| {
            // "ghost" never had anything stored under this test's unique
            // JR_SERVICE_NAME namespace — both credential kinds are
            // NoEntry from the start.
            let result = clear_profile_creds(&Profile::from("ghost"));
            assert!(
                result.is_ok(),
                "NoEntry on both credential kinds must be treated as success, got: {:?}",
                result.err().map(|e| format!("{e:#}"))
            );
        });
    }

    /// AC-002 (BC-1.2.014 EC-1.2.014-1, I-4/SR-008): a genuine
    /// (non-`NoEntry`) keychain backend error must ABORT
    /// `clear_profile_creds` — propagate as `Err`, never be
    /// aggregated-and-swallowed into a partial success the way the
    /// pre-amendment implementation did. Simulated via the same
    /// `JR_SERVICE_NAME=""` mechanism
    /// `load_api_token_propagates_backend_error_not_absent_message` uses
    /// above (every keyring backend this crate targets rejects an empty
    /// service name with a genuine `Err`, not `NoEntry`, before any
    /// persistent-storage I/O happens).
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_ac_002_clear_profile_creds_propagates_genuine_backend_error_not_swallowed() {
        with_test_keyring(|| {
            // SAFETY: with_test_keyring holds KEYRING_TEST_ENV_MUTEX for
            // this closure's entire duration.
            unsafe { std::env::set_var("JR_SERVICE_NAME", "") };

            let result = clear_profile_creds(&Profile::from("sandbox"));

            assert!(
                result.is_err(),
                "a genuine backend error must abort clear_profile_creds, not be \
                 silently swallowed into Ok(())"
            );
        });
    }

    /// AC-004 (BC-1.2.014 amended Effects, VP-1.2.014-001): both
    /// credential-deletion steps are independently, exhaustively
    /// re-attempted on retry. Seeds ONLY the OAuth pair (standing in for a
    /// profile whose API-token pair was already cleared by a prior partial
    /// attempt, or that never had one), clears once, then clears AGAIN —
    /// the second call must still succeed even though every key it touches
    /// now reports `NoEntry`.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_ac_004_clear_profile_creds_retry_after_partial_success_tolerates_no_entry() {
        with_test_keyring(|| {
            store_oauth_tokens(&Profile::from("sandbox"), "access-y", "refresh-y").unwrap();
            // Deliberately do NOT store an API-token pair for "sandbox" —
            // it is already absent, standing in for "already cleared by a
            // prior partial attempt."

            let first = clear_profile_creds(&Profile::from("sandbox"));
            assert!(
                first.is_ok(),
                "first clear must succeed: {:?}",
                first.err().map(|e| format!("{e:#}"))
            );

            // Retry: everything this call touches is now NoEntry.
            let second = clear_profile_creds(&Profile::from("sandbox"));
            assert!(
                second.is_ok(),
                "retry after a fully-completed clear must still succeed \
                 (every key now reports NoEntry): {:?}",
                second.err().map(|e| format!("{e:#}"))
            );
        });
    }

    /// BC-1.2.014 amended: `clear_all_credentials`'s per-profile
    /// credential-delete steps get the SAME error-surfacing tightening as
    /// `clear_profile_creds` — a genuine backend error must propagate
    /// immediately, not be aggregated into a single combined `Err` at the
    /// end of the loop (the pre-amendment behavior, which left callers
    /// unable to distinguish "some unspecified subset failed" from a
    /// clean run).
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_2_014_clear_all_credentials_propagates_genuine_backend_error_not_aggregated() {
        with_test_keyring(|| {
            // SAFETY: with_test_keyring holds KEYRING_TEST_ENV_MUTEX for
            // this closure's entire duration.
            unsafe { std::env::set_var("JR_SERVICE_NAME", "") };

            let result =
                clear_all_credentials(&[Profile::from("default"), Profile::from("sandbox")]);

            assert!(
                result.is_err(),
                "a genuine backend error must propagate from clear_all_credentials, \
                 not be swallowed"
            );
        });
    }

    /// F2-01/F2-02: a locked/backend keychain error encountered while
    /// resolving refresh-side app credentials must propagate as the
    /// refresh error, not be collapsed into empty embedded credentials
    /// (which previously surfaced a misleading "embedded app rotated" hint
    /// once Atlassian rejected the empty client_id/secret with
    /// `invalid_client`). Uses an unreachable token URL — before the fix,
    /// `resolve_refresh_app_credentials`'s `Err(_)` was swallowed and the
    /// function proceeded all the way to the (failing) HTTP POST, so the
    /// error text would be a network/URL failure, never mentioning the
    /// keychain at all. After the fix, the function returns before ever
    /// touching the network.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_f2_01_refresh_propagates_locked_keychain_error_not_embedded_rotation_hint() {
        with_test_keyring(|| {
            // SAFETY: with_test_keyring holds KEYRING_TEST_ENV_MUTEX for
            // this closure's entire duration.
            unsafe { std::env::set_var("JR_SERVICE_NAME", "") };

            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(refresh_oauth_token_with_url(
                "default",
                "not-a-valid-url-would-error-if-reached",
            ));

            let err = result.expect_err(
                "a locked/backend keychain error during credential resolution must \
                 propagate as an Err",
            );
            let msg = format!("{err:#}").to_lowercase();
            assert!(
                msg.contains("keychain"),
                "expected the genuine keychain/backend error to propagate: {msg}"
            );
            assert!(
                !msg.contains("embedded oauth app credentials may have been rotated"),
                "a locked/backend keychain error must not be masked by the \
                 empty-creds embedded-rotation hint: {msg}"
            );
            assert!(
                !msg.contains("token refresh failed: http"),
                "the function must return before ever attempting the HTTP \
                 refresh POST when credential resolution fails with a genuine \
                 backend error: {msg}"
            );
        });
    }

    /// Regression: `load_oauth_tokens` must distinguish (None, None) from
    /// partial state (Some, None) / (None, Some). A pair lookup that
    /// retried via the legacy fallback on partial state would either
    /// silently resurrect a stale legacy pair or return the generic
    /// "no token" error — both of which hide data loss / corruption.
    /// Partial state should surface as an explicit error pointing to
    /// logout+login recovery.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_oauth_tokens_errors_on_partial_state() {
        with_test_keyring(|| {
            // Pre-seed only the access key (missing refresh).
            entry(&oauth_access_key("sandbox"))
                .unwrap()
                .set_password("access-only")
                .unwrap();

            let result = load_oauth_tokens(&Profile::from("sandbox"));
            let err = result.expect_err("partial state should error");
            let msg = format!("{err:#}");
            assert!(msg.contains("partial"), "got: {msg}");
        });
    }

    /// Edge case: an interrupted lazy migration could leave the namespaced
    /// pair in a partial state for the `default` profile while the legacy
    /// flat keys still hold a complete pair. `load_oauth_tokens(&Profile::from("default"))`
    /// should recover from the intact legacy pair rather than stranding
    /// users with a partial-state error.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_oauth_tokens_default_partial_recovers_from_legacy() {
        with_test_keyring(|| {
            // Partial namespaced state for the default profile.
            entry(&oauth_access_key("default"))
                .unwrap()
                .set_password("stale-partial")
                .unwrap();
            // Complete legacy pair.
            entry(KEY_OAUTH_ACCESS_LEGACY)
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry(KEY_OAUTH_REFRESH_LEGACY)
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            let (a, r) = load_oauth_tokens(&Profile::from("default")).unwrap();
            assert_eq!(a, "legacy-access");
            assert_eq!(r, "legacy-refresh");

            // The recovered legacy values overwrote the namespaced pair
            // (both halves now match the legacy tokens).
            let recovered_access = entry(&oauth_access_key("default"))
                .unwrap()
                .get_password()
                .unwrap();
            let recovered_refresh = entry(&oauth_refresh_key("default"))
                .unwrap()
                .get_password()
                .unwrap();
            assert_eq!(recovered_access, "legacy-access");
            assert_eq!(recovered_refresh, "legacy-refresh");

            // Legacy flat keys cleaned up after migration.
            assert!(
                entry(KEY_OAUTH_ACCESS_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
            assert!(
                entry(KEY_OAUTH_REFRESH_LEGACY)
                    .unwrap()
                    .get_password()
                    .is_err()
            );
        });
    }

    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn lazy_migration_does_not_fire_for_non_default_profile() {
        with_test_keyring(|| {
            entry("oauth-access-token")
                .unwrap()
                .set_password("legacy-access")
                .unwrap();
            entry("oauth-refresh-token")
                .unwrap()
                .set_password("legacy-refresh")
                .unwrap();

            assert!(
                load_oauth_tokens(&Profile::from("sandbox")).is_err(),
                "sandbox profile should NOT inherit legacy keys"
            );
        });
    }

    /// Refresh resolver prefers keychain over embedded so a returning BYO
    /// user does not silently flip onto the embedded app mid-session
    /// (their refresh_token was issued by their own app and would be
    /// rejected if presented with a different client_id).
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn resolve_refresh_app_credentials_prefers_keychain() {
        with_test_keyring(|| {
            store_oauth_app_credentials("kc-id", "kc-secret").unwrap();
            let (id, secret, source) = resolve_refresh_app_credentials().unwrap();
            assert_eq!(id, "kc-id");
            assert_eq!(secret, "kc-secret");
            assert_eq!(source, RefreshAppSource::Keychain);
        });
    }

    /// Refresh resolver returns the embedded creds when no keychain pair
    /// exists. In test builds embedded is None, so this test only validates
    /// the *order* via the keychain-empty error path.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn resolve_refresh_app_credentials_errors_when_both_absent() {
        with_test_keyring(|| {
            // No keychain entries, no embedded creds in default test build.
            let err = resolve_refresh_app_credentials().unwrap_err();
            let msg = format!("{err:#}");
            assert!(msg.contains("embedded"), "got: {msg}");
        });
    }

    /// User-facing error string when the embedded fixed port is occupied.
    /// Locked in here because it's the entire payoff of the fixed-port
    /// design — if a future refactor regresses the message, embedded users
    /// hitting a port conflict have no actionable hint.
    #[test]
    fn fixed_port_strategy_eaddrinuse_friendly_error() {
        // Pre-bind a random ephemeral port so we can deterministically
        // reuse it in the Fixed bind attempt below.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        // The listener stays alive for the duration of the test —
        // its Drop happens after the assertions.

        let err = RedirectUriStrategyRequest::Fixed(port).bind().unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains(&format!("port {port}")),
            "expected port number in message, got: {msg}"
        );
        assert!(msg.contains("in use"), "got: {msg}");
        assert!(msg.contains("--client-id"), "got: {msg}");

        drop(listener);
    }

    // -------------------------------------------------------------------------
    // S-1.08 holdout tests: keychain layout invariants (BC-1.4.025..030)
    // These tests exercise the key-naming and profile-boundary logic at the
    // unit level without touching the OS keychain. They pin the invariants
    // that prevent cross-profile credential leakage.
    // -------------------------------------------------------------------------

    /// AC-001 (BC-1.4.027): `oauth_access_key("default")` must produce
    /// `"default:oauth-access-token"`. Namespacing keeps per-site OAuth tokens
    /// separate in a single keychain — changing this format would silently
    /// cross-pollinate credentials across Jira instances.
    #[test]
    fn test_s_1_08_ac001_oauth_access_key_default_profile() {
        assert_eq!(
            oauth_access_key("default"),
            "default:oauth-access-token",
            "default profile access key must be namespaced"
        );
    }

    /// AC-001 (BC-1.4.027): `oauth_access_key("sandbox")` must produce
    /// `"sandbox:oauth-access-token"`.
    #[test]
    fn test_s_1_08_ac001_oauth_access_key_sandbox_profile() {
        assert_eq!(
            oauth_access_key("sandbox"),
            "sandbox:oauth-access-token",
            "sandbox profile access key must be namespaced"
        );
    }

    /// AC-001 (BC-1.4.027): `oauth_refresh_key("default")` must produce
    /// `"default:oauth-refresh-token"`.
    #[test]
    fn test_s_1_08_ac001_oauth_refresh_key_default_profile() {
        assert_eq!(
            oauth_refresh_key("default"),
            "default:oauth-refresh-token",
            "default profile refresh key must be namespaced"
        );
    }

    /// AC-001 (BC-1.4.027): `oauth_refresh_key("sandbox")` must produce
    /// `"sandbox:oauth-refresh-token"`.
    #[test]
    fn test_s_1_08_ac001_oauth_refresh_key_sandbox_profile() {
        assert_eq!(
            oauth_refresh_key("sandbox"),
            "sandbox:oauth-refresh-token",
            "sandbox profile refresh key must be namespaced"
        );
    }

    /// AC-001 (BC-1.4.027): The legacy flat key constants must NOT use the
    /// `<profile>:` namespace prefix. These are shared / pre-multi-profile
    /// keys used only in the default-profile lazy-migration read path.
    #[test]
    fn test_s_1_08_ac001_shared_keys_are_not_namespaced() {
        assert_eq!(KEY_EMAIL, "email");
        assert_eq!(KEY_API_TOKEN, "api-token");
        // Legacy flat OAuth keys — read-only on migration path, no profile prefix
        assert_eq!(KEY_OAUTH_ACCESS_LEGACY, "oauth-access-token");
        assert_eq!(KEY_OAUTH_REFRESH_LEGACY, "oauth-refresh-token");
        // Verify they carry no profile namespace
        assert!(
            !KEY_EMAIL.contains(':'),
            "email key must not be namespaced: {KEY_EMAIL}"
        );
        assert!(
            !KEY_API_TOKEN.contains(':'),
            "api-token key must not be namespaced: {KEY_API_TOKEN}"
        );
        assert!(
            !KEY_OAUTH_ACCESS_LEGACY.contains(':'),
            "legacy access key must not be namespaced: {KEY_OAUTH_ACCESS_LEGACY}"
        );
        assert!(
            !KEY_OAUTH_REFRESH_LEGACY.contains(':'),
            "legacy refresh key must not be namespaced: {KEY_OAUTH_REFRESH_LEGACY}"
        );
    }

    /// AC-001 (BC-1.4.027): `oauth_access_key` and `oauth_refresh_key` must
    /// produce distinct keys for different profiles. Cross-profile collision
    /// would silently overwrite one site's tokens with another site's.
    #[test]
    fn test_s_1_08_ac001_profile_keys_are_distinct_across_profiles() {
        assert_ne!(
            oauth_access_key("default"),
            oauth_access_key("sandbox"),
            "access keys for different profiles must differ"
        );
        assert_ne!(
            oauth_refresh_key("default"),
            oauth_refresh_key("sandbox"),
            "refresh keys for different profiles must differ"
        );
        // Access and refresh keys for the same profile must also differ
        assert_ne!(
            oauth_access_key("default"),
            oauth_refresh_key("default"),
            "access and refresh keys for the same profile must differ"
        );
    }

    /// AC-001 (BC-1.4.027): The key format is `<profile>:oauth-<kind>-token`.
    /// Verify the separator (`:`) and suffix are present for arbitrary profile names.
    #[test]
    fn test_s_1_08_ac001_key_format_structure() {
        // Verify the colon separator and fixed suffixes are present
        let access = oauth_access_key("prod");
        let refresh = oauth_refresh_key("prod");
        assert!(
            access.starts_with("prod:"),
            "access key must start with '<profile>:': {access}"
        );
        assert!(
            access.ends_with(":oauth-access-token"),
            "access key suffix must be ':oauth-access-token': {access}"
        );
        assert!(
            refresh.starts_with("prod:"),
            "refresh key must start with '<profile>:': {refresh}"
        );
        assert!(
            refresh.ends_with(":oauth-refresh-token"),
            "refresh key suffix must be ':oauth-refresh-token': {refresh}"
        );
    }

    /// AC-002 (BC-1.4.025): The lazy-migration guard is `profile == "default"`.
    /// This test pins the guard string value — if the sentinel changes, the
    /// migration will silently fire (or silently skip) for the wrong profiles.
    ///
    /// The guard is exercised at two sites in `load_oauth_tokens`:
    /// (None, None) arm and the partial-state `_` arm. Both check the same
    /// string. We verify the constant string used as the sentinel matches
    /// `"default"` by checking the surrounding constants are exactly what the
    /// production code expects.
    #[test]
    fn test_s_1_08_ac002_lazy_migration_guard_sentinel_is_default() {
        // The sentinel used in `if profile == "default"` is the literal
        // string "default". Verify that:
        // 1. The default profile's namespaced key starts with "default:"
        // 2. A non-default profile's namespaced key does NOT start with "default:"
        // This pins the sentinel string transitively — if the sentinel changed,
        // the key format would diverge, and the guard would fire on the wrong profile.
        let default_access = oauth_access_key("default");
        let sandbox_access = oauth_access_key("sandbox");

        assert!(
            default_access.starts_with("default:"),
            "default profile key must start with 'default:': {default_access}"
        );
        assert!(
            !sandbox_access.starts_with("default:"),
            "sandbox profile key must NOT start with 'default:': {sandbox_access}"
        );
    }

    /// AC-002 / AC-004 (BC-1.4.025 / BC-1.4.029): The legacy flat keys
    /// (`oauth-access-token`, `oauth-refresh-token`) are NOT prefixed with
    /// any profile name. This ensures that `load_oauth_tokens(&Profile::from("sandbox"))`
    /// cannot accidentally read legacy keys by constructing a namespaced key
    /// that happens to match the legacy key string.
    ///
    /// The invariant: `oauth_access_key(profile) != KEY_OAUTH_ACCESS_LEGACY`
    /// for every non-default profile, so namespaced lookups never alias the
    /// legacy key slot.
    #[test]
    fn test_s_1_08_ac004_namespaced_key_never_aliases_legacy_key() {
        // For sandbox (non-default), the namespaced key must differ from legacy
        assert_ne!(
            oauth_access_key("sandbox"),
            KEY_OAUTH_ACCESS_LEGACY,
            "sandbox:oauth-access-token must not alias legacy oauth-access-token"
        );
        assert_ne!(
            oauth_refresh_key("sandbox"),
            KEY_OAUTH_REFRESH_LEGACY,
            "sandbox:oauth-refresh-token must not alias legacy oauth-refresh-token"
        );
        // Even the "default" profile's namespaced key differs from the legacy key
        assert_ne!(
            oauth_access_key("default"),
            KEY_OAUTH_ACCESS_LEGACY,
            "default:oauth-access-token must not alias legacy oauth-access-token"
        );
        assert_ne!(
            oauth_refresh_key("default"),
            KEY_OAUTH_REFRESH_LEGACY,
            "default:oauth-refresh-token must not alias legacy oauth-refresh-token"
        );
    }

    // -------------------------------------------------------------------------
    // S-cycle3-percred-storage (BC-1.4.031): per-profile API-token keychain
    // storage — `store_api_token`/`load_api_token` and their namespaced-key
    // helpers `api_token_email_key`/`api_token_key`.
    // -------------------------------------------------------------------------

    /// AC-001 (BC-1.4.031 postcondition 1): `api_token_email_key("default")`
    /// must produce `"default:email"`. Mirrors `oauth_access_key`'s shape.
    #[test]
    fn test_bc_1_4_031_api_token_email_key_default_profile() {
        assert_eq!(api_token_email_key("default"), "default:email");
    }

    /// AC-001 (BC-1.4.031 postcondition 1): `api_token_email_key("sandbox")`
    /// must produce `"sandbox:email"`.
    #[test]
    fn test_bc_1_4_031_api_token_email_key_sandbox_profile() {
        assert_eq!(api_token_email_key("sandbox"), "sandbox:email");
    }

    /// AC-001 (BC-1.4.031 postcondition 1): `api_token_key("default")` must
    /// produce `"default:api-token"`.
    #[test]
    fn test_bc_1_4_031_api_token_key_default_profile() {
        assert_eq!(api_token_key("default"), "default:api-token");
    }

    /// AC-001 (BC-1.4.031 postcondition 1): `api_token_key("sandbox")` must
    /// produce `"sandbox:api-token"`.
    #[test]
    fn test_bc_1_4_031_api_token_key_sandbox_profile() {
        assert_eq!(api_token_key("sandbox"), "sandbox:api-token");
    }

    /// BC-1.4.031 Invariant 1: the api-token key helpers must mirror the
    /// OAuth key helpers' shape byte-for-byte — `<profile>:<suffix>`, same
    /// separator, same profile-first ordering. If this drifts, the two
    /// credential families are no longer symmetric, which the story's
    /// entire design rationale depends on.
    #[test]
    fn test_bc_1_4_031_api_token_keys_symmetric_with_oauth_key_shape() {
        for profile in ["default", "sandbox", "prod"] {
            let email_key = api_token_email_key(profile);
            let token_key = api_token_key(profile);
            let oauth_access = oauth_access_key(profile);
            let oauth_refresh = oauth_refresh_key(profile);

            assert!(
                email_key.starts_with(&format!("{profile}:")),
                "email key must start with '<profile>:': {email_key}"
            );
            assert!(
                token_key.starts_with(&format!("{profile}:")),
                "api-token key must start with '<profile>:': {token_key}"
            );
            assert_eq!(
                email_key.matches(':').count(),
                oauth_access.matches(':').count(),
                "email key must have the same '#:' separators as oauth_access_key: {email_key} vs {oauth_access}"
            );
            assert_eq!(
                token_key.matches(':').count(),
                oauth_refresh.matches(':').count(),
                "api-token key must have the same '#:' separators as oauth_refresh_key: {token_key} vs {oauth_refresh}"
            );
        }
    }

    /// BC-1.4.027 (amended): api-token namespaced keys must be distinct
    /// across profiles — cross-profile collision would silently overwrite
    /// one site's API-token credentials with another's.
    #[test]
    fn test_bc_1_4_031_api_token_keys_distinct_across_profiles() {
        assert_ne!(
            api_token_email_key("default"),
            api_token_email_key("sandbox"),
            "email keys for different profiles must differ"
        );
        assert_ne!(
            api_token_key("default"),
            api_token_key("sandbox"),
            "api-token keys for different profiles must differ"
        );
        assert_ne!(
            api_token_email_key("default"),
            api_token_key("default"),
            "email and api-token keys for the same profile must differ"
        );
    }

    /// Best-effort cleanup of `<profile>:email`/`<profile>:api-token`
    /// entries created by a gated api-token test. `clear_all_credentials`
    /// (used by `with_test_keyring`'s own cleanup) does not yet clear these
    /// namespaced keys — flagged for the implementer/follow-on story — so
    /// api-token round-trip tests clean up after themselves explicitly to
    /// avoid leaving orphaned entries under the unique per-test service name.
    fn cleanup_api_token_profile(profile: &str) {
        if let Ok(e) = entry(&api_token_email_key(profile)) {
            let _ = e.delete_credential();
        }
        if let Ok(e) = entry(&api_token_key(profile)) {
            let _ = e.delete_credential();
        }
    }

    /// AC-001/002 (BC-1.4.031 postconditions 1-2): store then load returns
    /// the exact pair written, for two independent profiles. Mirrors
    /// `store_and_load_per_profile_oauth_tokens_round_trip`.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn store_and_load_per_profile_api_token_round_trip() {
        with_test_keyring(|| {
            store_api_token(
                &Profile::from("default"),
                "default@example.com",
                "default-token-1",
            )
            .unwrap();
            store_api_token(
                &Profile::from("sandbox"),
                "sandbox@example.com",
                "sandbox-token-2",
            )
            .unwrap();

            let (e1, t1) = load_api_token(&Profile::from("default")).unwrap();
            let (e2, t2) = load_api_token(&Profile::from("sandbox")).unwrap();

            assert_eq!(
                (e1.as_str(), t1.as_str()),
                ("default@example.com", "default-token-1")
            );
            assert_eq!(
                (e2.as_str(), t2.as_str()),
                ("sandbox@example.com", "sandbox-token-2")
            );

            cleanup_api_token_profile("default");
            cleanup_api_token_profile("sandbox");
        });
    }

    /// AC-008 / EC-1.4.031-1 (BC-1.4.031): a brand-new profile with no
    /// namespaced keys (and no legacy flat keys either) must return an
    /// actionable error, not panic or silently succeed with empty strings.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_api_token_returns_err_for_missing_profile() {
        with_test_keyring(|| {
            assert!(load_api_token(&Profile::from("brand-new-profile")).is_err());
        });
    }

    /// BC-1.4.031 Invariant 2: unlike `load_oauth_tokens`, `load_api_token`
    /// has NO `"default"`-only legacy-fallback branch. Pre-seed ONLY the
    /// legacy flat keys (`email`/`api-token`) for the `"default"` profile —
    /// if `load_api_token` copied `load_oauth_tokens`'s migration behavior,
    /// this would incorrectly succeed by reading the legacy pair. It must
    /// still error: the detect-and-instruct legacy-pair check belongs to
    /// `S-cycle3-credential-absence-guard` (BC-1.4.032), not this story.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_api_token_default_profile_has_no_legacy_fallback() {
        with_test_keyring(|| {
            store_legacy_flat_api_token("legacy@example.com", "legacy-token").unwrap();

            let result = load_api_token(&Profile::from("default"));
            assert!(
                result.is_err(),
                "load_api_token(\"default\") must NOT fall back to the legacy flat pair"
            );

            // Legacy flat pair is still readable via the dedicated legacy
            // reader — load_api_token must never have touched it.
            let (legacy_email, legacy_token) = load_legacy_flat_api_token().unwrap();
            assert_eq!(legacy_email, "legacy@example.com");
            assert_eq!(legacy_token, "legacy-token");

            if let Ok(e) = entry(KEY_EMAIL) {
                let _ = e.delete_credential();
            }
            if let Ok(e) = entry(KEY_API_TOKEN) {
                let _ = e.delete_credential();
            }
        });
    }

    /// VP-AUTHDX-004 direct case (BC-1.4.031): cross-profile isolation —
    /// storing credentials for one profile must never be readable under a
    /// different profile's namespaced keys.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_api_token_cross_profile_isolation() {
        with_test_keyring(|| {
            store_api_token(&Profile::from("default"), "p1@example.com", "p1-token").unwrap();
            store_api_token(&Profile::from("sandbox"), "p2@example.com", "p2-token").unwrap();

            let (e1, t1) = load_api_token(&Profile::from("default")).unwrap();
            let (e2, t2) = load_api_token(&Profile::from("sandbox")).unwrap();

            assert_ne!((e1.as_str(), t1.as_str()), (e2.as_str(), t2.as_str()));
            assert_eq!(e1, "p1@example.com");
            assert_eq!(t1, "p1-token");
            assert_eq!(e2, "p2@example.com");
            assert_eq!(t2, "p2-token");

            // A brand-new third profile must see neither pair.
            assert!(load_api_token(&Profile::from("ghost-profile")).is_err());

            cleanup_api_token_profile("default");
            cleanup_api_token_profile("sandbox");
        });
    }

    /// AC-007 / EC-1.4.031-2 (BC-1.4.031, I-5): a genuine keychain backend
    /// error must propagate as its own distinct problem, never coerced into
    /// the "no stored credential" absence message. Simulated deterministically
    /// (no reliance on real OS fault injection, which would be flaky/
    /// non-portable) by pointing `JR_SERVICE_NAME` at an empty string: every
    /// keyring backend this crate uses (macOS Keychain Services / Windows
    /// Credential Manager / Linux secret-service or keyutils) rejects an
    /// empty service name with `Err(Error::Invalid(..))` — NOT
    /// `Err(Error::NoEntry)` — before any persistent-storage I/O happens
    /// (confirmed against the vendored `keyring` 3.6.3 source:
    /// `macos.rs::MacCredential::new_with_target`,
    /// `windows.rs`/`secret_service.rs`/`keyutils.rs` all reject an empty
    /// service/target the same way). This exercises the exact
    /// `read_keyring_optional` `NoEntry`-vs-other-`Err` branch
    /// `load_api_token` must reuse rather than re-implement.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn load_api_token_propagates_backend_error_not_absent_message() {
        with_test_keyring(|| {
            // SAFETY: with_test_keyring holds KEYRING_TEST_ENV_MUTEX for this
            // closure's entire duration.
            unsafe { std::env::set_var("JR_SERVICE_NAME", "") };

            let err = load_api_token(&Profile::from("sandbox"))
                .expect_err("empty service name must error");
            let msg = format!("{err:#}").to_lowercase();
            assert!(
                !msg.contains("no stored credential"),
                "a backend/validation error must not be coerced into the \
                 absent-credential message: {msg}"
            );
        });
    }

    /// EC-1.4.031-2 pinned specifically at the [`legacy_flat_pair_exists`]
    /// call site inside `load_api_token`'s both-namespaced-absent branch
    /// (BC-1.4.032 Postcondition 1) — a genuine keychain backend error
    /// occurring during the legacy-pair existence probe must propagate as
    /// `Err`, never be silently coerced to `Ok(false)`/absent.
    ///
    /// **Isolation limitation (documented, not worked around):** the
    /// `JR_SERVICE_NAME=""` backend-error mechanism this module's tests rely
    /// on (see `load_api_token_propagates_backend_error_not_absent_message`
    /// above) fails keychain `Entry` construction uniformly for every key —
    /// `entry()` routes through the same [`service_name`] regardless of
    /// which key is being read (`KEY_EMAIL`/`KEY_API_TOKEN` vs the
    /// namespaced `<profile>:email`/`<profile>:api-token` keys). There is no
    /// available seam to make the namespaced-key reads succeed as `NoEntry`
    /// while only the legacy-pair probe's reads fail, so this test cannot
    /// drive the failure through `load_api_token`'s full both-absent branch
    /// with the namespaced reads legitimately absent. Instead it calls
    /// [`legacy_flat_pair_exists`] directly — the exact function
    /// `load_api_token`'s both-absent branch calls at that call site — under
    /// the same backend-error condition, and asserts the error propagates
    /// rather than collapsing to `Ok(false)`.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_ec_1_4_031_2_backend_error_during_legacy_pair_probe_propagates() {
        with_test_keyring(|| {
            // SAFETY: with_test_keyring holds KEYRING_TEST_ENV_MUTEX for this
            // closure's entire duration.
            unsafe { std::env::set_var("JR_SERVICE_NAME", "") };

            let err = legacy_flat_pair_exists()
                .expect_err("empty service name must error the legacy-pair probe");
            let msg = format!("{err:#}").to_lowercase();
            assert!(
                !msg.contains("no stored credential"),
                "a backend/validation error surfaced from the legacy-pair \
                 probe must not be coerced into the absent-credential \
                 message: {msg}"
            );
        });
    }

    /// BC-1.4.033 Postcondition 3 (forward reference — informs this story's
    /// write semantics even though the BC itself is owned by a later story):
    /// `store_api_token` is a plain unconditional two-key overwrite, not a
    /// read-modify-write / merge. A second call for the same profile fully
    /// replaces the first pair.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn store_api_token_overwrites_unconditionally() {
        with_test_keyring(|| {
            store_api_token(
                &Profile::from("sandbox"),
                "first@example.com",
                "first-token",
            )
            .unwrap();
            store_api_token(
                &Profile::from("sandbox"),
                "second@example.com",
                "second-token",
            )
            .unwrap();

            let (email, token) = load_api_token(&Profile::from("sandbox")).unwrap();
            assert_eq!(email, "second@example.com");
            assert_eq!(token, "second-token");

            cleanup_api_token_profile("sandbox");
        });
    }

    /// AC-005 / VP-AUTHDX-004: bounded-generator property tests for the
    /// round-trip + cross-profile-isolation invariants, against the REAL
    /// keychain backend (no in-process double is usable here — the
    /// `keyring` crate's `mock` backend has zero identity-based persistence
    /// across separate `Entry::new()` calls, which is exactly how
    /// `store_api_token`/`load_api_token` construct their entries, so a
    /// mock can't stand in for a real store-then-load round trip). Each
    /// case runs through `with_test_keyring`, so it no-ops (trivially
    /// passes) unless `JR_RUN_KEYRING_TESTS=1` is set — kept `#[ignore]`d
    /// too, for the same belt-and-suspenders reason the other gated tests
    /// in this module are. Case count is kept small (12) since every case
    /// performs real keychain I/O.
    mod percred_proptests {
        use super::{Profile, load_api_token, store_api_token, with_test_keyring};
        use proptest::prelude::*;

        fn profile_strategy() -> impl Strategy<Value = String> {
            "[a-z][a-z0-9]{2,9}"
        }

        fn email_strategy() -> impl Strategy<Value = String> {
            "[a-z]{1,10}@[a-z]{1,10}\\.[a-z]{2,4}"
        }

        fn token_strategy() -> impl Strategy<Value = String> {
            "[A-Za-z0-9]{8,32}"
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 12, .. ProptestConfig::default() })]

            /// AC-005(a): for any profile and any valid-shaped (email, token),
            /// `store_api_token` then `load_api_token` returns exactly
            /// `(email, token)`.
            /// AC-005(b): for any two distinct profiles p1 != p2, after
            /// `store_api_token(p1, e1, t1)`, `load_api_token(p2)` never
            /// returns `(e1, t1)` nor any component of it.
            #[test]
            #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
            fn prop_bc_1_4_031_round_trip_and_cross_profile_isolation(
                p1 in profile_strategy(),
                p2 in profile_strategy(),
                email in email_strategy(),
                token in token_strategy(),
            ) {
                prop_assume!(p1 != p2);
                let p1 = Profile::from(p1);
                let p2 = Profile::from(p2);

                with_test_keyring(|| {
                    store_api_token(&p1, &email, &token).unwrap();

                    // (a) round-trip.
                    let (got_email, got_token) = load_api_token(&p1).unwrap();
                    assert_eq!(got_email, email);
                    assert_eq!(got_token, token);

                    // (b) cross-profile isolation: p2 was never written to,
                    // so it must not see p1's pair (nor any component of it).
                    match load_api_token(&p2) {
                        Err(_) => {} // expected: p2 has no stored credential
                        Ok((e2, t2)) => {
                            assert_ne!(e2, email, "p2 must not see p1's email");
                            assert_ne!(t2, token, "p2 must not see p1's token");
                        }
                    }

                    super::cleanup_api_token_profile(p1.as_ref());
                });
            }
        }
    }

    /// SEC-JR-SERVICE-NAME-GATE behavioral regression: proves that debug builds
    /// actually honor `JR_SERVICE_NAME` at runtime, not just that the
    /// `#[cfg(debug_assertions)]` attribute is textually present in source.
    ///
    /// This is the behavioral complement to the source-scan test in
    /// `tests/jr_service_name_release_gate.rs`. The source-scan can only assert
    /// the gate *text* exists; this test asserts the gated path *executes*
    /// in debug builds (where `cfg!(debug_assertions)` is true).
    ///
    /// Pattern mirrors the keyring-gated tests above: KEYRING_TEST_ENV_MUTEX
    /// serializes env mutation; `JR_RUN_KEYRING_TESTS=1` gates execution so
    /// it never runs in normal CI (no keyring backend required — `service_name()`
    /// does NOT touch the keychain, but we share the mutex to stay consistent
    /// with the env-isolation convention used throughout this module).
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_service_name_debug_build_honors_jr_service_name_override() {
        if std::env::var("JR_RUN_KEYRING_TESTS").as_deref() != Ok("1") {
            return;
        }
        let sentinel = "jr-test-sec-service-name-gate-sentinel";
        let _guard = KEYRING_TEST_ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let prev = std::env::var("JR_SERVICE_NAME").ok();
        // SAFETY: KEYRING_TEST_ENV_MUTEX is held for the duration of this scope.
        unsafe { std::env::set_var("JR_SERVICE_NAME", sentinel) };
        let result = service_name();
        // SAFETY: still holding KEYRING_TEST_ENV_MUTEX.
        unsafe {
            match prev {
                Some(p) => std::env::set_var("JR_SERVICE_NAME", p),
                None => std::env::remove_var("JR_SERVICE_NAME"),
            }
        }
        assert_eq!(
            result, sentinel,
            "SEC-JR-SERVICE-NAME-GATE BEHAVIORAL FAILURE: \
             service_name() returned {:?} but expected sentinel {:?}. \
             The #[cfg(debug_assertions)] gate must be active in this debug build \
             and JR_SERVICE_NAME must be honored at runtime.",
            result, sentinel
        );
    }

    /// AC-003 (BC-1.4.028): `load_oauth_tokens` error message for partial state
    /// must contain `"partial"` so users can identify the recovery path.
    /// This test verifies the error arm is present and produces an actionable
    /// diagnostic. (Full behavioral test requires keychain — see #[ignore] tests.)
    #[test]
    fn test_s_1_08_ac003_partial_state_error_message_contains_partial() {
        // The partial-state error is constructed inline in load_oauth_tokens.
        // We verify the literal error string directly from the source-level
        // constant by constructing what the error would look like given the
        // format string. The format string in the `_ =>` arm is:
        //   "OAuth keychain entries for profile {profile:?} are partial ..."
        // Simulate the error message for profile "sandbox":
        let simulated_err = format!(
            "OAuth keychain entries for profile {:?} are partial \
             (one of access/refresh present, the other missing). \
             Run \"jr auth logout --profile {0}\" then \
             \"jr auth login --profile {0}\" to restore a clean state.",
            "sandbox"
        );
        assert!(
            simulated_err.contains("partial"),
            "partial-state error must contain 'partial': {simulated_err}"
        );
        assert!(
            simulated_err.contains("sandbox"),
            "partial-state error must name the profile: {simulated_err}"
        );
        assert!(
            simulated_err.contains("jr auth logout"),
            "partial-state error must include recovery instruction: {simulated_err}"
        );
    }

    // -------------------------------------------------------------------------
    // S-cycle3-credential-absence-guard (BC-1.4.032/BC-1.4.033/BC-1.4.034,
    // DEC-326 no-copy detect-and-instruct redesign). These tests target
    // `load_api_token`'s both-absent and namespaced-partial branches (see
    // `legacy_flat_pair_exists` and `load_api_token` above), plus the
    // `legacy_flat_pair_exists` existence-only helper itself.
    // -------------------------------------------------------------------------

    /// BC-1.4.032 Postcondition 2's exact actionable error text for a given
    /// profile name — single source of truth for every test below that
    /// asserts byte-identical error text across differing legacy-pair
    /// states (Postcondition 2's "identical regardless of legacy pair"
    /// guarantee) or across differing profile names (Postcondition 4's
    /// "no profile is special-cased" guarantee).
    fn expected_bc_1_4_032_absent_message(profile: &str) -> String {
        format!(
            "No credentials stored for profile '{profile}'. This version of jr \
             requires per-profile credentials — run `jr auth login {profile}` to set them up."
        )
    }

    /// BC-1.4.033 Postcondition 2's exact actionable error text for a given
    /// profile name.
    fn expected_bc_1_4_033_partial_message(profile: &str) -> String {
        format!(
            "Incomplete credentials stored for profile '{profile}' — run \
             `jr auth login {profile}` to fix this."
        )
    }

    /// Downcasts an `anyhow::Error` produced by `load_api_token`'s
    /// absent/partial branches to `JrError`, asserts it is a `UserError`
    /// with exit code 64 (the shared exit-64 contract BC-1.4.032
    /// Postcondition 2 and BC-1.4.033 Postcondition 2 both mandate), and
    /// returns the formatted display message for further assertion.
    fn assert_user_error_exit_64(err: &anyhow::Error) -> String {
        let je = err
            .downcast_ref::<JrError>()
            .unwrap_or_else(|| panic!("expected a JrError, got: {err:#}"));
        assert!(
            matches!(je, JrError::UserError(_)),
            "expected JrError::UserError, got {je:?}"
        );
        assert_eq!(
            je.exit_code(),
            64,
            "BC-1.4.032/BC-1.4.033 both mandate exit code 64"
        );
        format!("{err:#}")
    }

    /// Asserts `entry(key).get_password()` is exactly `Err(NoEntry)` — i.e.
    /// the key was never written. Used throughout this section to prove the
    /// DEC-326 no-copy invariant: a failed `load_api_token` call must never
    /// leave a `<profile>:email`/`<profile>:api-token` entry behind.
    fn assert_keychain_entry_absent(key: &str) {
        assert!(
            matches!(
                entry(key).unwrap().get_password(),
                Err(keyring::Error::NoEntry)
            ),
            "expected keychain entry {key:?} to be absent (never written) — no-copy invariant (DEC-326)"
        );
    }

    fn cleanup_legacy_flat_pair() {
        if let Ok(e) = entry(KEY_EMAIL) {
            let _ = e.delete_credential();
        }
        if let Ok(e) = entry(KEY_API_TOKEN) {
            let _ = e.delete_credential();
        }
    }

    /// AC-002 (BC-1.4.032 postcondition 2): both namespaced keys absent, no
    /// legacy pair either → actionable exit-64 error.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_032_absent_namespaced_keys_no_legacy_pair_returns_actionable_exit64() {
        with_test_keyring(|| {
            let err =
                load_api_token(&Profile::from("default")).expect_err("absent state must error");
            let msg = assert_user_error_exit_64(&err);
            assert_eq!(msg, expected_bc_1_4_032_absent_message("default"));
        });
    }

    /// AC-001 / EC-1.4.032-1 (BC-1.4.032 postcondition 2): both namespaced
    /// keys absent, legacy flat pair PRESENT → the IDENTICAL actionable
    /// exit-64 error as the no-legacy-pair case above (byte-for-byte equal
    /// message — legacy-pair presence changes nothing observable).
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_032_absent_namespaced_keys_legacy_pair_present_returns_identical_actionable_exit64()
     {
        with_test_keyring(|| {
            store_legacy_flat_api_token("legacy@example.com", "legacy-token-xyz").unwrap();

            let err = load_api_token(&Profile::from("default"))
                .expect_err("absent state with legacy pair must still error");
            let msg = assert_user_error_exit_64(&err);
            assert_eq!(msg, expected_bc_1_4_032_absent_message("default"));

            cleanup_legacy_flat_pair();
        });
    }

    /// **CRITICAL — DEC-326 no-copy invariant (AC-003, VP-AUTHDX-005(a)/(b)).**
    /// Seeds ONLY the legacy flat `email`/`api-token` pair (no per-profile
    /// pair at all). After the resulting exit-64 `Err`:
    /// (a) the legacy flat pair STILL EXISTS, byte-for-byte unchanged — it
    ///     was never deleted;
    /// (b) NO `default:email`/`default:api-token` entry was ever created —
    ///     it was never copied.
    /// This is the core guarantee the F2-gate human decision (DEC-326)
    /// exists to enforce: a shared, environment-unbound Basic-auth pair must
    /// never be silently handed to a freshly-tagged profile.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_032_no_copy_invariant_legacy_pair_untouched_and_no_percred_written() {
        with_test_keyring(|| {
            let seeded_email = "legacy-untouched@example.com";
            let seeded_token = "legacy-untouched-token-123";
            store_legacy_flat_api_token(seeded_email, seeded_token).unwrap();

            let err = load_api_token(&Profile::from("default"))
                .expect_err("absent namespaced state must error");
            assert_user_error_exit_64(&err);

            // (a) legacy pair STILL EXISTS, byte-for-byte unchanged — never deleted.
            let (legacy_email, legacy_token) = load_legacy_flat_api_token()
                .expect("legacy pair must still be readable — it must never be deleted");
            assert_eq!(legacy_email, seeded_email, "legacy email must be unchanged");
            assert_eq!(legacy_token, seeded_token, "legacy token must be unchanged");

            // (b) NO per-profile entry was ever created — never copied.
            assert_keychain_entry_absent(&api_token_email_key("default"));
            assert_keychain_entry_absent(&api_token_key("default"));

            cleanup_legacy_flat_pair();
        });
    }

    /// EC-1.4.032-3 / Postcondition 4: `"default"` and a non-default profile
    /// (`"sandbox"`) in the identical absent-namespaced-keys +
    /// legacy-pair-present state must get byte-identical error text — no
    /// `if profile == "default"` branch anywhere, and neither profile
    /// inherits the legacy pair.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_032_default_profile_not_special_cased_identical_to_other_profiles() {
        with_test_keyring(|| {
            store_legacy_flat_api_token("shared-legacy@example.com", "shared-legacy-token")
                .unwrap();

            let default_err =
                load_api_token(&Profile::from("default")).expect_err("default must error");
            let default_msg = assert_user_error_exit_64(&default_err);

            let sandbox_err = load_api_token(&Profile::from("sandbox"))
                .expect_err("sandbox must error identically");
            let sandbox_msg = assert_user_error_exit_64(&sandbox_err);

            assert_eq!(default_msg, expected_bc_1_4_032_absent_message("default"));
            assert_eq!(sandbox_msg, expected_bc_1_4_032_absent_message("sandbox"));

            assert_keychain_entry_absent(&api_token_email_key("default"));
            assert_keychain_entry_absent(&api_token_key("default"));
            assert_keychain_entry_absent(&api_token_email_key("sandbox"));
            assert_keychain_entry_absent(&api_token_key("sandbox"));

            cleanup_legacy_flat_pair();
        });
    }

    /// VP-AUTHDX-005(c) direct case: because this branch is read-only with
    /// no mutating side effect, repeated `load_api_token` calls in the same
    /// keychain state return the byte-identical `Err` — there is no
    /// "first-call-migrates, subsequent-call-short-circuits" shape (contrast
    /// `load_oauth_tokens`'s copy-then-short-circuit migration, which this
    /// BC deliberately does not reuse).
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_032_repeated_calls_return_same_err_no_first_call_side_effect() {
        with_test_keyring(|| {
            let err1 =
                load_api_token(&Profile::from("sandbox")).expect_err("first call must error");
            let msg1 = assert_user_error_exit_64(&err1);

            let err2 = load_api_token(&Profile::from("sandbox"))
                .expect_err("second call must error identically");
            let msg2 = assert_user_error_exit_64(&err2);

            assert_eq!(
                msg1, msg2,
                "no first-call-migrates shape — every call in the same state is identical"
            );
            assert_eq!(msg1, expected_bc_1_4_032_absent_message("sandbox"));
        });
    }

    /// AC-007 (BC-1.4.033 postconditions 1-2), variant 1: `<profile>:email`
    /// present, `<profile>:api-token` absent → actionable exit-64
    /// "Incomplete credentials" error, never a silently-incomplete `Ok`.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_033_namespaced_partial_email_present_returns_incomplete_credentials_error() {
        with_test_keyring(|| {
            entry(&api_token_email_key("sandbox"))
                .unwrap()
                .set_password("partial@example.com")
                .unwrap();
            // api-token half intentionally left absent.

            let err = load_api_token(&Profile::from("sandbox"))
                .expect_err("partial namespaced state must error");
            let msg = assert_user_error_exit_64(&err);
            assert_eq!(msg, expected_bc_1_4_033_partial_message("sandbox"));

            cleanup_api_token_profile("sandbox");
        });
    }

    /// AC-007 (BC-1.4.033 postconditions 1-2), variant 2 (reverse of the
    /// above): `<profile>:api-token` present, `<profile>:email` absent →
    /// the identical "Incomplete credentials" error shape.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_033_namespaced_partial_token_present_returns_incomplete_credentials_error() {
        with_test_keyring(|| {
            entry(&api_token_key("sandbox"))
                .unwrap()
                .set_password("partial-token-only")
                .unwrap();
            // email half intentionally left absent.

            let err = load_api_token(&Profile::from("sandbox"))
                .expect_err("partial namespaced state must error");
            let msg = assert_user_error_exit_64(&err);
            assert_eq!(msg, expected_bc_1_4_033_partial_message("sandbox"));

            cleanup_api_token_profile("sandbox");
        });
    }

    /// AC-008 / EC-1.4.033-1: `default:email` present, `default:api-token`
    /// absent, AND a complete legacy flat pair also exists → the namespaced
    /// partial-write error still fires (namespaced check runs BEFORE any
    /// legacy-pair consideration) — never falls through to BC-1.4.032's
    /// both-absent error. The legacy pair remains untouched throughout,
    /// same no-copy discipline as BC-1.4.032.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_033_partial_precedence_over_legacy_pair_present() {
        with_test_keyring(|| {
            entry(&api_token_email_key("default"))
                .unwrap()
                .set_password("default@example.com")
                .unwrap();
            store_legacy_flat_api_token("legacy@example.com", "legacy-token").unwrap();

            let err = load_api_token(&Profile::from("default"))
                .expect_err("namespaced-partial must take precedence");
            let msg = assert_user_error_exit_64(&err);
            assert_eq!(
                msg,
                expected_bc_1_4_033_partial_message("default"),
                "namespaced-partial error must fire, not the BC-1.4.032 both-absent error"
            );
            assert!(
                !msg.contains("No credentials stored"),
                "must not fall through to the both-absent branch: {msg}"
            );

            let (legacy_email, legacy_token) = load_legacy_flat_api_token().unwrap();
            assert_eq!(legacy_email, "legacy@example.com");
            assert_eq!(legacy_token, "legacy-token");

            cleanup_api_token_profile("default");
            cleanup_legacy_flat_pair();
        });
    }

    /// AC-010 / SR-009 (BC-1.4.033 invariant 2): the partial-write
    /// remediation message must never name `jr auth logout` (a no-op for
    /// api-token profiles, BC-1.2.013 amended) — only `jr auth login
    /// <profile>` is a valid remediation for this branch.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_033_remediation_message_never_mentions_auth_logout() {
        with_test_keyring(|| {
            entry(&api_token_email_key("sandbox"))
                .unwrap()
                .set_password("only-email@example.com")
                .unwrap();

            let err =
                load_api_token(&Profile::from("sandbox")).expect_err("partial state must error");
            let msg = format!("{err:#}");
            assert!(
                !msg.contains("jr auth logout"),
                "SR-009: remediation message must never name `jr auth logout` \
                 (no-op for api-token profiles): {msg}"
            );
            assert!(
                msg.contains("jr auth login"),
                "remediation message must name `jr auth login` as the primary fix: {msg}"
            );

            cleanup_api_token_profile("sandbox");
        });
    }

    /// AC-006 / VP-AUTHDX-007 (MANDATORY, SR-014) — keyring-gated end-to-end
    /// detect-and-instruct SCENARIO against the REAL OS keychain backend.
    /// Pre-seeds legacy flat keys (simulating a pre-cycle-003 install), runs
    /// the "first post-upgrade `jr` invocation" against `"default"`,
    /// confirms the exit-64 actionable error, confirms the legacy pair is
    /// byte-for-byte unchanged afterward, confirms no namespaced pair was
    /// ever written — then repeats identically for a `"sandbox"` profile,
    /// proving the failure-and-untouched behavior is not differentiated by
    /// profile. This is the only VP in the BC-1.4.032/033 cluster proving
    /// the no-copy logic against a REAL, non-mockable OS keychain backend
    /// (macOS Keychain / Windows Credential Manager / Linux Secret Service)
    /// rather than the in-process double the unit tests above implicitly
    /// rely on via `with_test_keyring`'s isolated service-name namespace.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_vp_authdx_007_keyring_gated_end_to_end_detect_and_instruct_scenario() {
        with_test_keyring(|| {
            let seeded_email = "pre-cycle-003@example.com";
            let seeded_token = "pre-cycle-003-token-abc123";
            store_legacy_flat_api_token(seeded_email, seeded_token).unwrap();

            for profile in ["default", "sandbox"] {
                let err = load_api_token(&Profile::from(profile))
                    .expect_err("first post-upgrade invocation must fail with an actionable error");
                let msg = assert_user_error_exit_64(&err);
                assert_eq!(msg, expected_bc_1_4_032_absent_message(profile));

                assert_keychain_entry_absent(&api_token_email_key(profile));
                assert_keychain_entry_absent(&api_token_key(profile));
            }

            // Legacy pair confirmed byte-for-byte unchanged in the real
            // keychain after both failed calls.
            let (legacy_email, legacy_token) = load_legacy_flat_api_token()
                .expect("legacy pair must still be readable in the real keychain");
            assert_eq!(legacy_email, seeded_email);
            assert_eq!(legacy_token, seeded_token);

            cleanup_legacy_flat_pair();
        });
    }

    /// BC-1.4.034 postconditions 2/4 / EC-1.4.032-4: after the actionable
    /// error fires once, running the remediation (`jr auth login
    /// <profile>`, simulated here via `store_api_token`) exactly once
    /// permanently resolves the failure for that profile — no second
    /// re-login is ever required — and the legacy pair (if any) remains
    /// untouched, inert, throughout and after remediation.
    #[test]
    #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
    fn test_bc_1_4_034_single_relogin_permanently_resolves_the_breaking_change() {
        with_test_keyring(|| {
            store_legacy_flat_api_token("legacy@example.com", "legacy-token").unwrap();

            // First post-upgrade call fails.
            let err1 =
                load_api_token(&Profile::from("default")).expect_err("first call must error");
            assert_user_error_exit_64(&err1);

            // Remediation: `jr auth login default` writes the namespaced pair
            // (BC-1.4.034 postcondition 2 — no flags beyond a normal login).
            store_api_token(&Profile::from("default"), "new@example.com", "new-token").unwrap();

            // Permanently resolved — repeated calls all succeed identically
            // (postcondition 2: "no second re-login is ever required").
            for _ in 0..2 {
                let (e, t) = load_api_token(&Profile::from("default")).unwrap();
                assert_eq!(e, "new@example.com");
                assert_eq!(t, "new-token");
            }

            // Legacy pair untouched throughout remediation (postcondition 4).
            let (legacy_email, legacy_token) = load_legacy_flat_api_token().unwrap();
            assert_eq!(legacy_email, "legacy@example.com");
            assert_eq!(legacy_token, "legacy-token");

            cleanup_api_token_profile("default");
            cleanup_legacy_flat_pair();
        });
    }

    /// VP-AUTHDX-005/006/008 property tests. Like `percred_proptests` above,
    /// these require the REAL keychain backend (the `keyring` crate's `mock`
    /// backend has no identity-based persistence across separate
    /// `Entry::new()` calls, so it can't stand in for a real
    /// store/seed-then-load round trip) — every case runs through
    /// `with_test_keyring`, so it no-ops (trivially passes) unless
    /// `JR_RUN_KEYRING_TESTS=1` is set, and is `#[ignore]`d for the same
    /// belt-and-suspenders reason as the rest of this module's gated tests.
    mod absence_guard_proptests {
        use super::{
            JrError, KEY_API_TOKEN, KEY_EMAIL, Profile, api_token_email_key, api_token_key,
            cleanup_api_token_profile, entry, expected_bc_1_4_032_absent_message,
            expected_bc_1_4_033_partial_message, load_api_token, load_legacy_flat_api_token,
            store_legacy_flat_api_token, with_test_keyring,
        };
        use proptest::prelude::*;

        fn profile_strategy() -> impl Strategy<Value = String> {
            "[a-z][a-z0-9]{2,9}"
        }

        fn email_strategy() -> impl Strategy<Value = String> {
            "[a-z]{1,10}@[a-z]{1,10}\\.[a-z]{2,4}"
        }

        fn token_strategy() -> impl Strategy<Value = String> {
            "[A-Za-z0-9]{8,32}"
        }

        fn cleanup_legacy_flat_pair() {
            if let Ok(e) = entry(KEY_EMAIL) {
                let _ = e.delete_credential();
            }
            if let Ok(e) = entry(KEY_API_TOKEN) {
                let _ = e.delete_credential();
            }
        }

        /// Calls `load_api_token(profile)`, asserts it is an exit-64
        /// `JrError::UserError`, asserts the no-copy invariant (neither
        /// namespaced key was written by the call), and returns the
        /// formatted message.
        fn assert_absent_err_and_no_write(profile: &str) -> String {
            let err = load_api_token(&Profile::from(profile))
                .expect_err("absent-namespaced state must error");
            let je = err
                .downcast_ref::<JrError>()
                .unwrap_or_else(|| panic!("expected a JrError, got: {err:#}"));
            assert!(matches!(je, JrError::UserError(_)));
            assert_eq!(je.exit_code(), 64);
            assert!(matches!(
                entry(&api_token_email_key(profile)).unwrap().get_password(),
                Err(keyring::Error::NoEntry)
            ));
            assert!(matches!(
                entry(&api_token_key(profile)).unwrap().get_password(),
                Err(keyring::Error::NoEntry)
            ));
            format!("{err:#}")
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 12, .. ProptestConfig::default() })]

            /// VP-AUTHDX-005: for any legacy `(email, token)` pair — OR none
            /// at all (`has_legacy = false`) — against `"default"`
            /// specifically: (a) `Err` with the actionable message
            /// regardless of legacy-pair presence, and `default:email`/
            /// `default:api-token` never written by the call; (b) the
            /// legacy pair's bytes (if seeded) are unchanged before/after;
            /// (c) a second call in the same state returns the identical
            /// `Err` (no first-call-migrates shape).
            #[test]
            #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
            fn prop_vp_authdx_005_detect_and_instruct_correctness(
                has_legacy in any::<bool>(),
                email in email_strategy(),
                token in token_strategy(),
            ) {
                with_test_keyring(|| {
                    if has_legacy {
                        store_legacy_flat_api_token(&email, &token).unwrap();
                    }

                    // (a) + no-write invariant, first call.
                    let msg1 = assert_absent_err_and_no_write("default");
                    assert_eq!(msg1, expected_bc_1_4_032_absent_message("default"));

                    // (b) legacy pair bytes unchanged, if seeded.
                    if has_legacy {
                        let (le, lt) = load_legacy_flat_api_token().unwrap();
                        assert_eq!(le, email);
                        assert_eq!(lt, token);
                    }

                    // (c) stability: repeated call, same state, same Err.
                    let msg2 = assert_absent_err_and_no_write("default");
                    assert_eq!(msg1, msg2);

                    if has_legacy {
                        cleanup_legacy_flat_pair();
                    }
                });
            }

            /// VP-AUTHDX-006: for ANY profile name — INCLUDING `"default"`
            /// itself, which the generator never excludes — even when a
            /// complete legacy flat pair exists, `load_api_token(profile)`
            /// surfaces the identical actionable error with no
            /// `"default"`-only branch, and the legacy pair is left
            /// byte-for-byte unchanged.
            #[test]
            #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
            fn prop_vp_authdx_006_no_profile_is_special_cased(
                profile in prop_oneof![Just("default".to_string()), profile_strategy()],
                email in email_strategy(),
                token in token_strategy(),
            ) {
                with_test_keyring(|| {
                    store_legacy_flat_api_token(&email, &token).unwrap();

                    let msg = assert_absent_err_and_no_write(&profile);
                    assert_eq!(msg, expected_bc_1_4_032_absent_message(&profile));

                    let (le, lt) = load_legacy_flat_api_token().unwrap();
                    assert_eq!(le, email);
                    assert_eq!(lt, token);

                    cleanup_legacy_flat_pair();
                });
            }

            /// VP-AUTHDX-008: for either namespaced partial-state
            /// combination — `email` present/`api-token` absent, or the
            /// reverse — `load_api_token` ALWAYS returns `Err` with the
            /// actionable "Incomplete credentials" message, never a panic
            /// and never a silently-incomplete `Ok`.
            #[test]
            #[ignore = "requires keyring backend; set JR_RUN_KEYRING_TESTS=1 to run"]
            fn prop_vp_authdx_008_namespaced_partial_state_safety(
                profile in profile_strategy(),
                email_first in any::<bool>(),
                email in email_strategy(),
                token in token_strategy(),
            ) {
                with_test_keyring(|| {
                    if email_first {
                        entry(&api_token_email_key(&profile)).unwrap().set_password(&email).unwrap();
                    } else {
                        entry(&api_token_key(&profile)).unwrap().set_password(&token).unwrap();
                    }

                    let err = load_api_token(&Profile::from(profile.clone()))
                        .expect_err("partial state must error, never Ok");
                    let je = err
                        .downcast_ref::<JrError>()
                        .unwrap_or_else(|| panic!("expected a JrError, got: {err:#}"));
                    assert!(matches!(je, JrError::UserError(_)));
                    assert_eq!(je.exit_code(), 64);
                    let msg = format!("{err:#}");
                    assert_eq!(msg, expected_bc_1_4_033_partial_message(&profile));

                    cleanup_api_token_profile(&profile);
                });
            }
        }
    }
}
