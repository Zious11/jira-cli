use anyhow::{Context, Result};
use dialoguer::{Input, Password};

use crate::api::auth;
use crate::api::auth_embedded::{OAuthAppSource, embedded_oauth_app};
use crate::config::{Config, global_config_path};
use crate::error::JrError;
use crate::output;

/// Environment variable names for the four auth credentials.
///
/// Flag > env > prompt precedence is implemented by [`resolve_credential`].
/// Callers pass the matching `flag_name` so error messages can cite both
/// names verbatim.
pub(crate) const ENV_EMAIL: &str = "JR_EMAIL";
pub(crate) const ENV_API_TOKEN: &str = "JR_API_TOKEN";
pub(crate) const ENV_OAUTH_CLIENT_ID: &str = "JR_OAUTH_CLIENT_ID";
pub(crate) const ENV_OAUTH_CLIENT_SECRET: &str = "JR_OAUTH_CLIENT_SECRET";

/// Resolve a credential value via flag → env → TTY prompt, or error under
/// `--no-input`.
///
/// Order of precedence:
/// 1. `flag_value` (explicit CLI arg wins).
/// 2. `env::var(env_name)` if non-empty.
/// 3. If `no_input` is true, return a `JrError::UserError` naming the flag
///    and env var so scripts/agents can recover. `hint` — if supplied —
///    is appended to the error so first-time agents learn *where to obtain*
///    the credential, not just how to pass it (relevant for OAuth where
///    users must first create an app at developer.atlassian.com).
/// 4. Otherwise, prompt interactively. `is_password` chooses between
///    `dialoguer::Password` (masked) and `Input` (visible).
///
/// Empty env values are ignored so an accidentally-exported-but-unset var
/// doesn't silently substitute for real input.
pub(crate) fn resolve_credential(
    flag_value: Option<String>,
    env_name: &str,
    flag_name: &str,
    prompt_label: &str,
    is_password: bool,
    no_input: bool,
    hint: Option<&str>,
) -> Result<String> {
    if let Some(v) = flag_value.filter(|v| !v.is_empty()) {
        return Ok(v);
    }
    if let Ok(v) = std::env::var(env_name)
        && !v.is_empty()
    {
        return Ok(v);
    }
    if no_input {
        let base = format!("{prompt_label} is required. Provide {flag_name} or set ${env_name}.");
        let msg = match hint {
            Some(h) => format!("{base} {h}"),
            None => base,
        };
        return Err(JrError::UserError(msg).into());
    }
    if is_password {
        Password::new()
            .with_prompt(prompt_label)
            .interact()
            .with_context(|| format!("failed to read {prompt_label}"))
    } else {
        Input::new()
            .with_prompt(prompt_label)
            .interact_text()
            .with_context(|| format!("failed to read {prompt_label}"))
    }
}

/// Resolve the OAuth app credentials for a `login --oauth` invocation.
/// Returns `(client_id, client_secret, source)`. The `source` flows into
/// `jr auth status` so users can tell which path drove the session.
///
/// Order: flag → env → keychain → embedded → prompt.
///
/// Flag and env are pair-gated: both halves must be present, otherwise the
/// resolver falls through (avoids sending a half-empty pair to Atlassian).
pub(crate) fn resolve_oauth_app_credentials(
    flag_id: Option<String>,
    flag_secret: Option<String>,
    no_input: bool,
) -> Result<(String, String, OAuthAppSource)> {
    let env_id = std::env::var(ENV_OAUTH_CLIENT_ID)
        .ok()
        .filter(|s| !s.is_empty());
    let env_secret = std::env::var(ENV_OAUTH_CLIENT_SECRET)
        .ok()
        .filter(|s| !s.is_empty());
    let keychain = crate::api::auth::load_oauth_app_credentials().ok();
    let embedded = embedded_oauth_app().map(|a| (a.client_id.clone(), a.client_secret.clone()));

    resolve_oauth_app_credentials_for_test(
        flag_id,
        flag_secret,
        env_id,
        env_secret,
        keychain,
        embedded,
        no_input,
    )
}

/// Pure resolution function — accepts every potential source as an argument
/// so unit tests can exercise the precedence chain without mutating env vars
/// or the keychain.
//
// 7 parameters is deliberate: each layer of the resolution chain
// (flag/env/keychain/embedded) must be independently substitutable.
// Grouping into a struct would obscure the precedence semantics.
#[allow(clippy::too_many_arguments)]
fn resolve_oauth_app_credentials_for_test(
    flag_id: Option<String>,
    flag_secret: Option<String>,
    env_id: Option<String>,
    env_secret: Option<String>,
    keychain: Option<(String, String)>,
    embedded: Option<(String, String)>,
    no_input: bool,
) -> Result<(String, String, OAuthAppSource)> {
    // Flag pair: must be all-or-nothing. A user passing only one half
    // (e.g., --client-id alone) almost certainly meant BYO and forgot the
    // other flag — silently falling through to embedded would surprise
    // them. Hard-error with a specific recovery message instead.
    let flag_id_present = flag_id.as_deref().map(|s| !s.is_empty()).unwrap_or(false);
    let flag_secret_present = flag_secret
        .as_deref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    match (flag_id_present, flag_secret_present) {
        (true, true) => {
            return Ok((flag_id.unwrap(), flag_secret.unwrap(), OAuthAppSource::Flag));
        }
        (true, false) => {
            return Err(JrError::UserError(
                "--client-id was provided without --client-secret. \
                 Both flags must be supplied together for OAuth bring-your-own-app login."
                    .to_string(),
            )
            .into());
        }
        (false, true) => {
            return Err(JrError::UserError(
                "--client-secret was provided without --client-id. \
                 Both flags must be supplied together for OAuth bring-your-own-app login."
                    .to_string(),
            )
            .into());
        }
        (false, false) => {} // fall through to env layer
    }

    // Env pair: same all-or-nothing rule.
    let env_id_present = env_id.as_deref().map(|s| !s.is_empty()).unwrap_or(false);
    let env_secret_present = env_secret
        .as_deref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    match (env_id_present, env_secret_present) {
        (true, true) => {
            return Ok((env_id.unwrap(), env_secret.unwrap(), OAuthAppSource::Env));
        }
        (true, false) => {
            return Err(JrError::UserError(
                "JR_OAUTH_CLIENT_ID is set but JR_OAUTH_CLIENT_SECRET is not. \
                 Both env vars must be set together for OAuth bring-your-own-app login."
                    .to_string(),
            )
            .into());
        }
        (false, true) => {
            return Err(JrError::UserError(
                "JR_OAUTH_CLIENT_SECRET is set but JR_OAUTH_CLIENT_ID is not. \
                 Both env vars must be set together for OAuth bring-your-own-app login."
                    .to_string(),
            )
            .into());
        }
        (false, false) => {} // fall through to keychain layer
    }

    if let Some((i, s)) = keychain {
        return Ok((i, s, OAuthAppSource::Keychain));
    }
    if let Some((i, s)) = embedded {
        return Ok((i, s, OAuthAppSource::Embedded));
    }
    if no_input {
        return Err(JrError::UserError(
            "OAuth app credentials are required. Provide --client-id and --client-secret, \
             or set JR_OAUTH_CLIENT_ID and JR_OAUTH_CLIENT_SECRET. This binary was not \
             built with embedded credentials."
                .to_string(),
        )
        .into());
    }
    // Fall back to the existing interactive prompt path. Re-enter
    // resolve_credential for each so the existing UX (masked input,
    // hint, retry) is preserved verbatim.
    let id = resolve_credential(
        None,
        ENV_OAUTH_CLIENT_ID,
        "--client-id",
        "OAuth Client ID",
        false,
        false,
        Some(OAUTH_APP_HINT),
    )?;
    let secret = resolve_credential(
        None,
        ENV_OAUTH_CLIENT_SECRET,
        "--client-secret",
        "OAuth Client Secret",
        true,
        false,
        Some(OAUTH_APP_HINT),
    )?;
    Ok((id, secret, OAuthAppSource::Prompt))
}

/// Hint for OAuth client_id / client_secret errors so first-time agents
/// discover they must create an OAuth app before passing credentials.
const OAUTH_APP_HINT: &str = "Create one at https://developer.atlassian.com/console/myapps/.";

/// Which auth flow `jr auth refresh` should dispatch to.
///
/// Internal detail of the `refresh` command; kept module-private so it
/// isn't part of the crate's public library API surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthFlow {
    Token,
    OAuth,
}

impl AuthFlow {
    /// Canonical string form used in config (`auth_method`) and in the
    /// `--output json` success payload. Single source of truth for the label
    /// so a future rename (e.g., `"api_token"` → `"basic"`) has one edit site.
    fn label(self) -> &'static str {
        match self {
            AuthFlow::Token => "api_token",
            AuthFlow::OAuth => "oauth",
        }
    }
}

/// Decide which login flow to run for the **active** profile + explicit
/// override.
///
/// Today this is only exercised by unit tests (production callers like
/// `refresh_credentials` need the target profile, not the active one, and
/// use [`chosen_flow_for_profile`] directly). It's kept as a thin wrapper
/// so a future caller that genuinely wants the active profile has a
/// labeled entry point — `#[cfg(test)]` because adding it without a real
/// caller would just be dead code.
///
/// Order of precedence:
/// 1. `oauth_override = true` → always OAuth (user passed `--oauth`).
/// 2. Active profile `auth_method == "oauth"` → OAuth.
/// 3. Anything else (including unset) → Token. Matches the `api_token`
///    default that `JiraClient::from_config` applies when no method is set.
#[cfg(test)]
fn chosen_flow(config: &Config, oauth_override: bool) -> AuthFlow {
    chosen_flow_for_profile(&config.active_profile(), oauth_override)
}

/// Decide which login flow to run based on a specific profile + explicit
/// override. Use this when the caller has already resolved the target
/// profile and that profile may differ from the active one (refresh,
/// per-target dispatch).
fn chosen_flow_for_profile(
    profile: &crate::config::ProfileConfig,
    oauth_override: bool,
) -> AuthFlow {
    if oauth_override {
        return AuthFlow::OAuth;
    }
    match profile.auth_method.as_deref() {
        Some("oauth") => AuthFlow::OAuth,
        _ => AuthFlow::Token,
    }
}

/// Pick the OAuth scope string: user override from the *target* profile's
/// `oauth_scopes` if set, else the compiled-in default. Trims and collapses
/// interior whitespace so multi-line TOML strings encode cleanly. Empty or
/// whitespace-only overrides are a configuration error.
///
/// Takes a `&ProfileConfig` (not a `&Config`) so callers like `login_oauth`
/// can pass the profile they're actually targeting; reading `Config`'s
/// active profile would silently return the wrong scopes when
/// `jr auth login --profile X` runs against a non-active X.
fn resolve_oauth_scopes(profile: &crate::config::ProfileConfig) -> Result<String> {
    match profile.oauth_scopes.as_deref() {
        None => Ok(auth::DEFAULT_OAUTH_SCOPES.to_string()),
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

/// Resolve email and API token (flag → env → prompt), then store in keychain.
///
/// `profile` names which entry under `[profiles]` should record the
/// `auth_method = "api_token"` after a successful login. The keychain entry
/// for API token + email is shared across profiles today (one-pair-per-host
/// keyring layout); the profile name only affects config persistence.
pub async fn login_token(
    profile: &str,
    email: Option<String>,
    token: Option<String>,
    no_input: bool,
) -> Result<()> {
    let email = resolve_credential(
        email,
        ENV_EMAIL,
        "--email",
        "Jira email",
        false,
        no_input,
        None,
    )?;
    let token = resolve_credential(
        token,
        ENV_API_TOKEN,
        "--token",
        "API token",
        true,
        no_input,
        None,
    )?;

    auth::store_api_token(&email, &token)?;

    // Persist the profile's auth_method so subsequent runs know which flow
    // to use. URL is set by `prepare_login_target` before this point, so
    // we only touch auth_method here.
    //
    // Use `load_lenient` (not `load`) for the same reason `handle_login`
    // does: this function may be invoked while creating a brand-new profile
    // whose name doesn't yet appear in `[profiles]`, and the resolved
    // active profile (e.g., from `JR_PROFILE`) might not exist either.
    // A strict reload here would re-trigger the unknown-active-profile
    // check mid-flight and abort a login that's intentionally creating
    // its target.
    let mut config = Config::load_lenient_with(Some(profile))?;
    let p = config
        .global
        .profiles
        .entry(profile.to_string())
        .or_default();
    p.auth_method = Some("api_token".into());
    // If `default_profile` is unset (legacy / fresh config / refresh
    // creating a non-"default" profile on a brand-new install), promote
    // the target so the next strict `Config::load()` doesn't error trying
    // to resolve the literal "default" against an empty profiles map.
    // `handle_login` does this via `prepare_login_target`; callers that
    // bypass that helper (notably `refresh_credentials`) need the same
    // safeguard here.
    if config.global.default_profile.is_none() {
        config.global.default_profile = Some(profile.to_string());
    }
    config.save_global()?;

    eprintln!("Credentials stored in keychain.");
    Ok(())
}

/// Run the OAuth 2.0 (3LO) login flow and persist site configuration.
///
/// Credentials resolved via flag → env → prompt, so CI/agent workflows can
/// pipe them in without a TTY. `profile` names the target profile under
/// `[profiles]`; OAuth tokens are stored under namespaced keychain entries
/// (`<profile>:oauth-*-token`) so multiple sites can coexist.
pub async fn login_oauth(
    profile: &str,
    client_id: Option<String>,
    client_secret: Option<String>,
    no_input: bool,
) -> Result<()> {
    if !no_input {
        eprintln!("OAuth 2.0: by default, official jr binaries use the embedded \"jr\" app.");
        eprintln!("To use your own OAuth app instead, pass --client-id and --client-secret,");
        eprintln!("or set JR_OAUTH_CLIENT_ID and JR_OAUTH_CLIENT_SECRET.\n");
    }

    let (client_id, client_secret, source) =
        resolve_oauth_app_credentials(client_id, client_secret, no_input)?;

    // Embedded credentials get the registered fixed callback. Every other
    // source is BYO and stays on the historical dynamic-port flow — the
    // user has registered their own callback URL.
    let strategy = match source {
        OAuthAppSource::Embedded => crate::api::auth::RedirectUriStrategyRequest::Fixed(53682),
        _ => crate::api::auth::RedirectUriStrategyRequest::Dynamic,
    };

    // Resolve config and scopes BEFORE persisting credentials — a bad
    // [profiles.<name>].oauth_scopes (empty/whitespace-only) must fail fast,
    // not leave new client_id/client_secret in the keychain alongside a
    // login that never succeeded.
    let config_path = global_config_path();
    // Use `load_lenient` (not `load`) so a `JR_PROFILE` pointing at an
    // unconfigured profile, or a target profile that doesn't yet exist,
    // can't trip the strict active-profile existence check mid-login.
    let config = Config::load_lenient_with(Some(profile)).map_err(|err| {
        JrError::ConfigError(format!(
            "Failed to load config: {err:#}\n\n\
             Fix or remove the file referenced above. Global config: {config_path}; \
             per-project overrides come from `.jr.toml` in the current directory or any parent.",
            config_path = config_path.display()
        ))
    })?;
    let target_profile = config
        .global
        .profiles
        .get(profile)
        .cloned()
        .unwrap_or_default();
    let scopes = resolve_oauth_scopes(&target_profile)?;

    // Persist user-provided OAuth app creds to keychain so subsequent
    // refreshes use the same app. Embedded credentials are NOT persisted —
    // they re-decode from the binary every launch and would only pollute
    // the keychain for the inevitable rotation cycle.
    if !matches!(source, OAuthAppSource::Embedded) {
        crate::api::auth::store_oauth_app_credentials(&client_id, &client_secret)?;
    }

    let result =
        crate::api::auth::oauth_login(profile, &client_id, &client_secret, &scopes, strategy)
            .await?;

    // Persist site info to the named profile under [profiles.<name>], not
    // the legacy [instance] block. Reload to pick up any mutations made
    // earlier in the login flow (e.g., by `prepare_login_target`). Same
    // lenient-load rationale as the earlier reload above.
    let mut config = Config::load_lenient_with(Some(profile))?;
    let p = config
        .global
        .profiles
        .entry(profile.to_string())
        .or_default();
    p.url = Some(result.site_url);
    p.cloud_id = Some(result.cloud_id);
    p.auth_method = Some("oauth".into());
    // Same default_profile safeguard as login_token — `refresh_credentials`
    // can reach this path on a fresh install, and we must never leave
    // `default_profile = None` when [profiles] is non-empty (the next
    // strict `Config::load()` would error trying to resolve "default"
    // against a profiles map that doesn't contain it).
    if config.global.default_profile.is_none() {
        config.global.default_profile = Some(profile.to_string());
    }
    config.save_global()?;

    output::print_success(&format!("Authenticated with {}", result.site_name));
    Ok(())
}

/// Bundle of CLI arguments threaded from `main.rs` to [`handle_login`].
///
/// Grouped into a struct because the orchestrator needs all four credential
/// slots (two API-token, two OAuth) plus profile/URL/flow toggles, which
/// trips clippy's `too_many_arguments` lint when passed as positional
/// parameters. The struct also makes the call site at `main.rs` self-
/// documenting.
pub struct LoginArgs {
    pub profile: Option<String>,
    pub url: Option<String>,
    pub oauth: bool,
    pub email: Option<String>,
    pub token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub no_input: bool,
}

/// Orchestrate `jr auth login`: ensure the target profile exists with the
/// requested URL, then dispatch to the API-token or OAuth flow. Wraps the
/// pure logic in [`prepare_login_target`] so `main.rs` only needs one call
/// to thread the new `--profile` / `--url` flags through.
///
/// Wraps a load failure in `JrError::ConfigError` (exit 78) so a malformed
/// `config.toml` surfaces as an actionable error instead of dropping to
/// `Config::default()` and overwriting the user's broken-but-recoverable
/// file (#258).
pub async fn handle_login(args: LoginArgs) -> Result<()> {
    let config_path = global_config_path();
    // `load_lenient` skips the active-profile existence check so
    // `jr auth login --profile newprof --url ...` can create the profile
    // on first use. Every other command keeps the strict `Config::load()`.
    //
    // Pass `args.profile.as_deref()` as the cli-flag override so the
    // resolved active profile reflects the subcommand's `--profile` rather
    // than relying on env-var seams (which are unsound under #[tokio::main]).
    let mut config = Config::load_lenient_with(args.profile.as_deref()).map_err(|err| {
        JrError::ConfigError(format!(
            "Failed to load config: {err:#}\n\n\
             Fix or remove the file referenced above. Global config: {config_path}; \
             per-project overrides come from `.jr.toml` in the current directory or any parent.",
            config_path = config_path.display()
        ))
    })?;

    // Defensive: when the user is creating a NEW profile interactively and
    // didn't pass `--url`, prompt for it instead of silently creating a
    // URL-less profile that fails confusingly on the next command. Done in
    // the orchestrator (not in `prepare_login_target`) so that pure helper
    // stays trivially unit-testable without a TTY.
    let target_for_check = args
        .profile
        .as_deref()
        .unwrap_or(&config.active_profile_name);
    // Prompt for URL whenever the target profile lacks one — both the
    // brand-new-profile case AND the existing-but-URL-less case (e.g.,
    // a hand-edited or migrated profile with status `unset`). Without
    // this, `jr auth login --profile <existing-no-url>` interactively
    // would leave the profile URL-less and fail confusingly on the
    // next command.
    let target_has_url = config
        .global
        .profiles
        .get(target_for_check)
        .and_then(|p| p.url.as_deref())
        .is_some();
    let url_resolved: Option<String> = if let Some(u) = args.url.as_deref() {
        Some(u.to_string())
    } else if !args.no_input && !target_has_url {
        let prompt: String = dialoguer::Input::new()
            .with_prompt(format!(
                "Jira instance URL for profile {target_for_check:?} \
                 (e.g., https://yourorg.atlassian.net)"
            ))
            .interact_text()
            .context("failed to read Jira instance URL")?;
        Some(prompt)
    } else {
        None
    };

    let (global, target) = prepare_login_target(
        config.global,
        args.profile.as_deref(),
        url_resolved.as_deref(),
        args.no_input,
        &config.active_profile_name,
    )?;
    config.global = global;
    config.save_global()?;
    if args.oauth {
        login_oauth(&target, args.client_id, args.client_secret, args.no_input).await
    } else {
        login_token(&target, args.email, args.token, args.no_input).await
    }
}

/// Pure logic for ensuring a target profile exists with the given URL.
/// Returns `(updated_global, resolved_profile_name)`.
///
/// - When `profile_arg` is `Some`, that name is validated and used as the
///   target. Otherwise we fall back to `active_profile_name`, which the
///   caller has already resolved through the full precedence chain
///   (`--profile` flag > `JR_PROFILE` env > `default_profile` field >
///   `"default"`). Reading `default_profile` directly here would drop the
///   flag and env layers and silently target the wrong profile.
/// - When `url_arg` is `Some`, the profile's URL is overwritten (with the
///   trailing slash trimmed for canonical form).
/// - When creating a new profile under `--no-input`, a URL is required so
///   non-interactive agents can't accidentally create empty profiles.
/// - If `default_profile` is unset (legacy / fresh config), the resolved
///   target is promoted to the default so a follow-up `jr` invocation
///   keeps targeting it.
pub(super) fn prepare_login_target(
    mut global: crate::config::GlobalConfig,
    profile_arg: Option<&str>,
    url_arg: Option<&str>,
    no_input: bool,
    active_profile_name: &str,
) -> Result<(crate::config::GlobalConfig, String)> {
    let target = match profile_arg {
        Some(name) => {
            crate::config::validate_profile_name(name)?;
            name.to_string()
        }
        None => active_profile_name.to_string(),
    };

    let entry = global.profiles.entry(target.clone()).or_default();

    if let Some(url) = url_arg {
        entry.url = Some(url.trim_end_matches('/').to_string());
    } else if entry.url.is_none() && no_input {
        // Both "brand-new profile" and "existing profile with no URL"
        // hit this path — under --no-input we can't prompt for the
        // missing URL, so error out with the expected recovery flag.
        return Err(JrError::UserError(
            "--url required when the target profile has no URL configured".into(),
        )
        .into());
    }

    if global.default_profile.is_none() {
        global.default_profile = Some(target.clone());
    }

    Ok((global, target))
}

/// Inspect — without consuming or modifying — which source would supply
/// OAuth app credentials on the next `refresh_oauth_token` call. Mirrors
/// the resolver order in `api/auth.rs::resolve_refresh_app_credentials`.
///
/// On keychain probe failure (locked keychain, permission denied) emits
/// a stderr warning and falls through to the next source in the chain.
/// The status row may therefore display `embedded` when the keychain is
/// merely temporarily inaccessible — that's defensible for a status
/// surface (display non-blocking, keep `auth status` usable) but it
/// diverges from `resolve_refresh_app_credentials`, which hard-errors on
/// the same condition. The stderr warning is the user's signal that the
/// row may be incomplete.
fn peek_oauth_app_source() -> OAuthAppSource {
    let keychain_present = match crate::api::auth::probe_oauth_app_credentials() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "warning: could not probe keychain for OAuth app credentials ({e:#}); \
                 status report may be incomplete."
            );
            false
        }
    };
    let embedded_present = crate::api::auth_embedded::embedded_oauth_app().is_some();
    peek_oauth_app_source_for_test(keychain_present, embedded_present)
}

/// Pure helper for testing the precedence chain. Match the runtime
/// resolver: keychain wins, embedded falls back, otherwise returns
/// `OAuthAppSource::None` (the explicit sentinel variant for "no source
/// resolved", not Rust's `Option::None`).
fn peek_oauth_app_source_for_test(
    keychain_present: bool,
    embedded_present: bool,
) -> OAuthAppSource {
    if keychain_present {
        return OAuthAppSource::Keychain;
    }
    if embedded_present {
        return OAuthAppSource::Embedded;
    }
    OAuthAppSource::None
}

/// Show authentication status: instance URL, auth method, credential availability.
///
/// When `profile_arg` is `Some`, reports for that profile. Otherwise reports
/// for the active profile (resolved via the usual flag → env → config →
/// "default" precedence chain at `Config::load` time).
pub async fn status(profile_arg: Option<&str>) -> Result<()> {
    // `profile_arg` is the explicit per-subcommand override (`--profile`
    // on `auth status`); when absent we still let Config::load apply the
    // standard precedence chain (env > default_profile > "default").
    // Passing `profile_arg` here also doubles as the CLI-flag override
    // for `Config::load_with`, ensuring a `jr auth status --profile X`
    // against an unconfigured X surfaces a clear "unknown profile" error
    // from the strict load instead of silently falling back to the
    // active profile.
    let config = Config::load_with(profile_arg)?;
    let target = profile_arg
        .map(str::to_string)
        .unwrap_or_else(|| config.active_profile_name.clone());
    crate::config::validate_profile_name(&target)?;

    // Special-case: fresh install with no profiles yet AND no explicit
    // `--profile` was passed. `jr auth status` is a legitimate probe
    // used by setup scripts / CI / agents to detect first-run state.
    // Erroring here would block that probe — the user hasn't configured
    // anything yet, so "unknown profile" would be misleading.
    //
    // BUT if the user explicitly named a profile via `--profile X`, take
    // the strict path below — they're asserting X exists, and silently
    // succeeding with a generic "no profiles configured" message would
    // hide the mismatch. Matches the strict behavior of switch/remove/
    // logout for explicit profile targets.
    if config.global.profiles.is_empty() && profile_arg.is_none() {
        eprintln!(
            "No profiles configured. Run `jr init` or \
             `jr auth login --profile <NAME>` to set up."
        );
        return Ok(());
    }

    // Refuse to "succeed" against a profile the user never configured —
    // matches the strict behavior of switch/remove/logout. Without this,
    // `jr auth status --profile typo` printed "(not configured)" for
    // every field and exited 0, hiding the typo.
    if !config.global.profiles.contains_key(&target) {
        let known: Vec<&str> = config.global.profiles.keys().map(String::as_str).collect();
        return Err(JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() {
                "(none)".into()
            } else {
                known.join(", ")
            }
        ))
        .into());
    }

    let profile = config.global.profiles.get(&target);
    let url = profile
        .and_then(|p| p.url.as_deref())
        .unwrap_or("(not configured)");
    println!("Profile:     {target}");
    println!("Instance:    {url}");

    let method = profile
        .and_then(|p| p.auth_method.as_deref())
        .unwrap_or("(not configured)");
    println!("Auth method: {method}");

    // Credential probe: API-token creds are shared (one per host); OAuth
    // tokens are per-profile and namespaced by the profile name.
    let creds_ok = match method {
        "oauth" => auth::load_oauth_tokens(&target).is_ok(),
        _ => auth::load_api_token().is_ok(),
    };
    if creds_ok {
        println!("Credentials: stored in keychain");
    } else {
        println!("Credentials: not found");
    }

    // Report which OAuth app credentials would be used for the next refresh.
    // This is the *future* source — same resolver as `refresh_oauth_token`.
    if method == "oauth" {
        let source = peek_oauth_app_source();
        println!("OAuth app:   {}", source.label());
    }

    Ok(())
}

/// Post-refresh guidance shown to humans (stderr, Table mode) and embedded
/// in the JSON payload (`next_step`). Click "Always Allow" on the keychain
/// write prompts so future commands run silently.
const REFRESH_HELP_LINE: &str = "If prompted to allow keychain access, choose \"Always Allow\" so future commands run silently.";

/// Build the `--output json` success payload. Extracted for unit-testing the
/// shape (status key, auth_method label, next_step guidance) without needing
/// to drive the full login flow.
fn refresh_success_payload(flow: AuthFlow) -> serde_json::Value {
    serde_json::json!({
        "status": "refreshed",
        "auth_method": flow.label(),
        "next_step": REFRESH_HELP_LINE,
    })
}

/// Clear all stored credentials and re-run the login flow so the current
/// binary re-registers as the creator of fresh keychain entries.
///
/// On macOS this is the recovery path for the legacy Keychain ACL/partition
/// invalidation that occurs after `jr` is replaced at its installed path
/// (e.g., `brew upgrade`). See spec at
/// `docs/superpowers/specs/2026-04-17-keychain-prompts-207-design.md`.
///
/// Ordering is clear-then-login. If the login step fails (e.g., EOF on stdin,
/// network error during OAuth), the user is warned that credentials are gone
/// and told exactly which `jr auth login` invocation will restore them,
/// before the error is propagated.
/// Bundle of CLI arguments threaded from `main.rs` to [`refresh_credentials`].
///
/// Same rationale as [`LoginArgs`] — passing all credential slots plus the
/// flow toggle and `--profile` as positional parameters trips clippy's
/// `too_many_arguments` lint, so they're grouped into a struct that also
/// makes the call site at `main.rs` self-documenting.
pub struct RefreshArgs<'a> {
    pub profile: Option<&'a str>,
    pub oauth: bool,
    pub email: Option<String>,
    pub token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub no_input: bool,
    pub output: &'a crate::cli::OutputFormat,
}

pub async fn refresh_credentials(args: RefreshArgs<'_>) -> Result<()> {
    // Pass `args.profile` as the CLI-flag override so a `--profile X`
    // against an unconfigured X surfaces the strict load's "unknown
    // profile" error rather than silently refreshing the active profile.
    let config = Config::load_with(args.profile)?;
    let target = args
        .profile
        .map(str::to_string)
        .unwrap_or_else(|| config.active_profile_name.clone());
    crate::config::validate_profile_name(&target)?;
    // Inspect the target profile's auth method (not the active profile's)
    // so `jr auth refresh --profile X` against a non-active X dispatches
    // the right flow. Missing entries default to api_token, matching the
    // login-time default.
    let target_profile = config
        .global
        .profiles
        .get(&target)
        .cloned()
        .unwrap_or_default();
    let flow = chosen_flow_for_profile(&target_profile, args.oauth);

    // For the api_token flow, login_token re-prompts/sets the SHARED
    // api-token but doesn't write a URL. If the target profile has no
    // URL configured (fresh install / hand-edited profile with status
    // `unset`), refresh would succeed in keychain terms while leaving
    // the profile unusable for any actual API call. Refuse upfront with
    // a recovery hint to use `jr auth login --profile X --url ...`
    // instead. The OAuth flow goes through oauth_login which fetches
    // accessible-resources and writes its own URL/cloud_id, so it
    // doesn't have this gap.
    if flow == AuthFlow::Token && target_profile.url.is_none() {
        return Err(JrError::UserError(format!(
            "profile {target:?} has no URL configured. Use \
             \"jr auth login --profile {target} --url <https://...>\" \
             instead of refresh — refresh assumes the profile is already \
             set up and only rotates credentials."
        ))
        .into());
    }

    // Clear-only-what-this-flow-refreshes:
    //
    // - OAuth refresh rotates the per-profile <profile>:oauth-*-token
    //   entries; the shared keys (email, api-token, oauth_client_id,
    //   oauth_client_secret) belong to other profiles too and must not
    //   be wiped.
    // - API-token refresh re-prompts the email + api-token, and the
    //   shared api-token IS the credential being refreshed — so the
    //   #207-style "wipe-then-relogin" path is correct here.
    match flow {
        AuthFlow::OAuth => auth::clear_profile_creds(&target).context(
            "failed to clear stored OAuth tokens before refresh — keychain may still hold stale entries",
        )?,
        AuthFlow::Token => auth::clear_all_credentials(&[target.as_str()]).context(
            "failed to clear stored credentials before refresh — keychain may still hold stale entries",
        )?,
    }

    let login_result = match flow {
        AuthFlow::Token => login_token(&target, args.email, args.token, args.no_input).await,
        AuthFlow::OAuth => {
            login_oauth(&target, args.client_id, args.client_secret, args.no_input).await
        }
    };

    if let Err(err) = login_result {
        let login_cmd = match flow {
            AuthFlow::Token => "jr auth login",
            AuthFlow::OAuth => "jr auth login --oauth",
        };
        eprintln!(
            "Credentials were cleared, but the login flow did not complete. \
             Run `{login_cmd}` to restore access."
        );
        return Err(err);
    }

    match args.output {
        crate::cli::OutputFormat::Json => {
            let payload = serde_json::to_string_pretty(&refresh_success_payload(flow))
                .context("failed to serialize refresh success payload as JSON")?;
            println!("{payload}");
        }
        crate::cli::OutputFormat::Table => {
            eprintln!("Credentials refreshed. {REFRESH_HELP_LINE}");
        }
    }

    Ok(())
}

/// Pure resolver for `jr auth logout`. Defaults to the active profile when
/// the user passes no `--profile`. Kept module-private and split out so the
/// CLI default behavior is unit-testable without filesystem or keychain.
pub(super) fn resolve_logout_target(
    _global: &crate::config::GlobalConfig,
    profile_arg: Option<&str>,
    active: &str,
) -> String {
    profile_arg.unwrap_or(active).to_string()
}

/// `jr auth logout [--profile <name>]` — clear OAuth tokens for the target
/// profile. The profile entry in `config.toml` is left in place so a follow-up
/// `jr auth login --profile <name>` re-authenticates without losing site
/// metadata. The shared API-token credential is intentionally NOT cleared
/// (it's keyed by host, not profile, so wiping it would log every profile
/// out of API-token mode).
pub async fn handle_logout(profile_arg: Option<&str>) -> anyhow::Result<()> {
    let config = crate::config::Config::load_with(profile_arg)?;
    let target = resolve_logout_target(&config.global, profile_arg, &config.active_profile_name);
    crate::config::validate_profile_name(&target)?;
    if !config.global.profiles.contains_key(&target) {
        let known: Vec<&str> = config.global.profiles.keys().map(String::as_str).collect();
        return Err(JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() {
                "(none)".into()
            } else {
                known.join(", ")
            }
        ))
        .into());
    }
    crate::api::auth::clear_profile_creds(&target)?;
    crate::output::print_success(&format!("Logged out of profile {target:?}"));
    Ok(())
}

/// Pure logic for `jr auth remove` — separated for testing without filesystem
/// or keychain. Returns the mutated `GlobalConfig` with `target` removed from
/// `profiles`. Refuses to remove the active profile (caller must switch first)
/// or unknown profiles. The cache directory and per-profile OAuth tokens are
/// cleared by [`handle_remove`] after the in-memory mutation succeeds; this
/// function only owns the config-shape transition.
pub(super) fn handle_remove_in_memory(
    mut global: crate::config::GlobalConfig,
    target: &str,
    active: &str,
) -> anyhow::Result<crate::config::GlobalConfig> {
    crate::config::validate_profile_name(target)?;
    if !global.profiles.contains_key(target) {
        let known: Vec<&str> = global.profiles.keys().map(String::as_str).collect();
        return Err(JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() {
                "(none)".into()
            } else {
                known.join(", ")
            }
        ))
        .into());
    }
    if target == active {
        return Err(JrError::UserError(format!(
            "cannot remove active profile {target:?}; switch first with \"jr auth switch <other>\""
        ))
        .into());
    }
    // Also refuse if `target` is the persisted default_profile, even when
    // not the *current* active (e.g., `jr --profile sandbox auth remove
    // default` where active=sandbox but default_profile=default). Removing
    // the profile that default_profile points to leaves config.toml in a
    // broken state — strict Config::load() afterward would error with
    // "active profile 'default' not in [profiles]" until the user manually
    // edits the file.
    if global.default_profile.as_deref() == Some(target) {
        return Err(JrError::UserError(format!(
            "cannot remove profile {target:?}: it is the default_profile in config. \
             Switch the default first with \"jr auth switch <other>\"."
        ))
        .into());
    }
    global.profiles.remove(target);
    Ok(global)
}

/// `jr auth remove <name>` — permanently delete a profile.
///
/// Order of operations:
/// 1. Confirm with the user (skipped under `--no-input`).
/// 2. Mutate config in-memory via [`handle_remove_in_memory`] (validates name,
///    refuses active profile, refuses unknown profile).
/// 3. Persist config first so a subsequent keychain/cache failure can't
///    leave the profile listed in `config.toml` after its credentials are
///    gone.
/// 4. Best-effort wipe of per-profile OAuth tokens and cache directory; both
///    are intentionally non-fatal — a missing keychain entry or cache dir is
///    the expected steady state for an already-cleaned profile, not an error.
pub async fn handle_remove(
    target: &str,
    no_input: bool,
    cli_profile: Option<&str>,
) -> anyhow::Result<()> {
    let mut config = Config::load_with(cli_profile)?;
    crate::config::validate_profile_name(target)?;

    // Pre-validate against a clone before prompting so a typo or
    // unremovable target (active profile, default_profile target) doesn't
    // make the user click through a confirmation dialog only to error
    // afterward. The actual mutation runs below against the real config.
    let _ = handle_remove_in_memory(config.global.clone(), target, &config.active_profile_name)?;

    if !no_input {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "Permanently remove profile {target:?}? \
                 This deletes its config entry, cache, and OAuth tokens. \
                 Shared credentials remain."
            ))
            .default(false)
            .interact()?;
        if !confirm {
            crate::output::print_warning("Aborted.");
            return Ok(());
        }
    }

    config.global = handle_remove_in_memory(config.global, target, &config.active_profile_name)?;
    config.save_global()?;

    // Config entry is gone — that's the persistent state. The keychain
    // and cache cleanup is best-effort: failures here (permission denied,
    // locked keychain, IO) shouldn't unwind the config write, but the
    // user does need to know they have leftover state to clean up
    // manually. Surface as warnings; report overall success.
    if let Err(e) = crate::api::auth::clear_profile_creds(target) {
        crate::output::print_warning(&format!(
            "removed config entry but failed to clear OAuth tokens for {target:?}: {e}. \
             Remove the entries manually via your OS keychain UI."
        ));
    }
    if let Err(e) = crate::cache::clear_profile_cache(target) {
        let cache_path = crate::cache::cache_dir(target);
        crate::output::print_warning(&format!(
            "removed config entry but failed to clear cache for {target:?}: {e}. \
             Remove {} manually if disk space matters.",
            cache_path.display()
        ));
    }
    crate::output::print_success(&format!("Removed profile {target:?}"));
    Ok(())
}

/// Pure logic for `jr auth switch` — separated for testing without filesystem.
pub(super) fn handle_switch_in_memory(
    mut global: crate::config::GlobalConfig,
    target: &str,
) -> Result<crate::config::GlobalConfig> {
    crate::config::validate_profile_name(target)?;
    if !global.profiles.contains_key(target) {
        let known: Vec<&str> = global.profiles.keys().map(String::as_str).collect();
        return Err(JrError::UserError(format!(
            "unknown profile: {target}; known: {}",
            if known.is_empty() {
                "(none)".into()
            } else {
                known.join(", ")
            }
        ))
        .into());
    }
    global.default_profile = Some(target.to_string());
    Ok(global)
}

/// `jr auth switch <name>` — set the default profile in `config.toml`.
pub async fn handle_switch(target: &str, cli_profile: Option<&str>) -> Result<()> {
    let mut config = Config::load_with(cli_profile)?;
    config.global = handle_switch_in_memory(config.global, target)?;
    config.save_global()?;
    output::print_success(&format!("Active profile set to {target:?}"));
    Ok(())
}

/// Render the table-form output of `jr auth list`. The active profile is
/// marked with a leading `*`; others get a leading space so column widths
/// stay stable across rows. Status today is a coarse "do we have a URL on
/// file?" check — credential-store probing comes in Task 13.
pub(super) fn render_list_table(global: &crate::config::GlobalConfig, active: &str) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for (name, p) in &global.profiles {
        let marker = if name == active { "*" } else { " " };
        let auth = p.auth_method.as_deref().unwrap_or("?");
        let url = p.url.as_deref().unwrap_or("(unset)");
        // STATUS reflects CONFIG presence (URL on file), not credential
        // presence. `unset` is more accurate than the old `no-creds` label,
        // which suggested the keychain was missing entries when in reality
        // the profile entry simply lacks a URL.
        let status = if p.url.is_some() {
            "configured"
        } else {
            "unset"
        };
        rows.push(vec![
            format!("{marker} {name}"),
            url.to_string(),
            auth.to_string(),
            status.to_string(),
        ]);
    }
    crate::output::render_table(&["NAME", "URL", "AUTH", "STATUS"], &rows)
}

/// Render the `--output json` form of `jr auth list`: an array of profile
/// objects keyed by name, with `active: true` on exactly one entry.
pub(super) fn render_list_json(
    global: &crate::config::GlobalConfig,
    active: &str,
) -> Result<String> {
    let arr: Vec<serde_json::Value> = global
        .profiles
        .iter()
        .map(|(name, p)| {
            serde_json::json!({
                "name": name,
                "url": &p.url,
                "auth_method": &p.auth_method,
                "status": if p.url.is_some() { "configured" } else { "unset" },
                "active": name == active,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&arr)?)
}

/// `jr auth list` — print every configured profile, marking the active one.
pub async fn handle_list(
    output: &crate::cli::OutputFormat,
    cli_profile: Option<&str>,
) -> Result<()> {
    let config = Config::load_with(cli_profile)?;
    let rendered = match output {
        crate::cli::OutputFormat::Table => {
            render_list_table(&config.global, &config.active_profile_name)
        }
        crate::cli::OutputFormat::Json => {
            render_list_json(&config.global, &config.active_profile_name)?
        }
    };
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, GlobalConfig, ProfileConfig};

    fn config_with_auth_method(method: Option<&str>) -> Config {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                url: Some("https://example.atlassian.net".into()),
                auth_method: method.map(str::to_string),
                ..ProfileConfig::default()
            },
        );
        Config {
            global: GlobalConfig {
                default_profile: Some("default".into()),
                profiles,
                ..Default::default()
            },
            project: Default::default(),
            active_profile_name: "default".into(),
        }
    }

    #[test]
    fn chosen_flow_defaults_to_token_when_unset() {
        let config = config_with_auth_method(None);
        assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
    }

    #[test]
    fn chosen_flow_uses_token_for_explicit_api_token() {
        let config = config_with_auth_method(Some("api_token"));
        assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
    }

    #[test]
    fn chosen_flow_uses_oauth_when_config_says_so() {
        let config = config_with_auth_method(Some("oauth"));
        assert_eq!(chosen_flow(&config, false), AuthFlow::OAuth);
    }

    #[test]
    fn chosen_flow_oauth_override_wins_over_config() {
        let config = config_with_auth_method(Some("api_token"));
        assert_eq!(chosen_flow(&config, true), AuthFlow::OAuth);
    }

    /// Regression: refresh against a non-active profile must dispatch the
    /// flow stored on THAT profile's auth_method, not the active profile's.
    /// `chosen_flow(&Config, _)` always reads the active profile, which
    /// silently picked the wrong flow when active=api_token but the refresh
    /// target=oauth (or vice-versa). `chosen_flow_for_profile` takes the
    /// resolved target profile so callers like `refresh_credentials` can
    /// thread the right ProfileConfig in.
    #[test]
    fn chosen_flow_for_profile_inspects_passed_profile_not_active() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".into(),
            ProfileConfig {
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        profiles.insert(
            "sandbox".into(),
            ProfileConfig {
                auth_method: Some("oauth".into()),
                ..ProfileConfig::default()
            },
        );
        let config = Config {
            global: GlobalConfig {
                default_profile: Some("default".into()),
                profiles,
                ..GlobalConfig::default()
            },
            project: Default::default(),
            active_profile_name: "default".into(),
        };
        // chosen_flow without override returns Token (active is api_token)
        assert_eq!(chosen_flow(&config, false), AuthFlow::Token);
        // chosen_flow_for_profile against sandbox returns OAuth even though
        // the active profile is api_token — proves the resolver looks at
        // the passed profile, not the active one.
        let sandbox = config.global.profiles["sandbox"].clone();
        assert_eq!(chosen_flow_for_profile(&sandbox, false), AuthFlow::OAuth);
    }

    #[test]
    fn auth_flow_labels_match_config_and_json_conventions() {
        assert_eq!(AuthFlow::Token.label(), "api_token");
        assert_eq!(AuthFlow::OAuth.label(), "oauth");
    }

    #[test]
    fn refresh_payload_pins_token_shape() {
        let payload = refresh_success_payload(AuthFlow::Token);
        assert_eq!(payload["status"], "refreshed");
        assert_eq!(payload["auth_method"], "api_token");
        assert!(
            payload["next_step"]
                .as_str()
                .unwrap()
                .contains("Always Allow"),
            "next_step should guide the user to click Always Allow, got: {}",
            payload["next_step"]
        );
    }

    #[test]
    fn refresh_payload_pins_oauth_shape() {
        let payload = refresh_success_payload(AuthFlow::OAuth);
        assert_eq!(payload["status"], "refreshed");
        assert_eq!(payload["auth_method"], "oauth");
    }

    // ── resolve_credential ───────────────────────────────────────────
    //
    // Env-reading tests must serialize process-environment mutation across
    // parallel test threads. `std::env::set_var` / `remove_var` are unsafe
    // in Rust 2024 because concurrent env access (even on different keys)
    // is UB — C's getenv/setenv aren't thread-safe. `EnvGuard` holds
    // `ENV_LOCK` for its full lifetime and removes the var on drop so a
    // panic mid-test doesn't leak state to later tests in the same
    // process. Matches the pattern in src/config.rs::ENV_MUTEX.

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let lock = ENV_LOCK.lock().unwrap();
            // SAFETY: test env mutation is serialized by ENV_LOCK, held for
            // this guard's lifetime. The Drop impl unsets the same
            // test-local key before releasing the lock.
            unsafe {
                std::env::set_var(key, value);
            }
            EnvGuard { key, _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: matches the test-local key set in `EnvGuard::set`
            // while `_lock` is still held by this `EnvGuard`.
            unsafe {
                std::env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn resolve_credential_prefers_flag_over_env() {
        let _guard = EnvGuard::set("_JR_TEST_PREFERS_FLAG", "from-env");
        let got = resolve_credential(
            Some("from-flag".into()),
            "_JR_TEST_PREFERS_FLAG",
            "--email",
            "Jira email",
            false,
            true,
            None,
        )
        .unwrap();
        assert_eq!(got, "from-flag");
    }

    #[test]
    fn resolve_credential_falls_back_to_env_when_flag_absent() {
        let _guard = EnvGuard::set("_JR_TEST_FALLS_BACK", "from-env");
        let got = resolve_credential(
            None,
            "_JR_TEST_FALLS_BACK",
            "--email",
            "Jira email",
            false,
            true,
            None,
        )
        .unwrap();
        assert_eq!(got, "from-env");
    }

    #[test]
    fn resolve_credential_ignores_empty_flag_and_env() {
        // Empty values should fall through to the no_input error path.
        let _guard = EnvGuard::set("_JR_TEST_EMPTY", "");
        let err = resolve_credential(
            Some(String::new()),
            "_JR_TEST_EMPTY",
            "--email",
            "Jira email",
            false,
            true,
            None,
        )
        .unwrap_err();
        assert!(
            err.downcast_ref::<JrError>()
                .is_some_and(|e| matches!(e, JrError::UserError(_))),
            "Expected JrError::UserError for empty inputs, got: {err}"
        );
    }

    #[test]
    fn resolve_credential_no_input_errors_when_missing() {
        // resolve_credential reads env via std::env::var — hold ENV_LOCK to
        // serialize against set/remove calls in sibling tests.
        let _lock = ENV_LOCK.lock().unwrap();
        let err = resolve_credential(
            None,
            "_JR_TEST_UNSET_MISSING",
            "--email",
            "Jira email",
            false,
            true,
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            err.downcast_ref::<JrError>()
                .is_some_and(|e| matches!(e, JrError::UserError(_))),
            "Expected JrError::UserError, got: {err}"
        );
        assert!(
            msg.contains("--email") && msg.contains("$_JR_TEST_UNSET_MISSING"),
            "Error should cite both flag and env var: {msg}"
        );
    }

    #[test]
    fn resolve_credential_oauth_hint_appears_in_error() {
        // Same env-read serialization as the test above.
        let _lock = ENV_LOCK.lock().unwrap();
        let err = resolve_credential(
            None,
            "_JR_TEST_UNSET_OAUTH",
            "--client-id",
            "OAuth Client ID",
            false,
            true,
            Some(OAUTH_APP_HINT),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("developer.atlassian.com/console/myapps"),
            "OAuth error should cite dev console URL: {msg}"
        );
    }

    fn profile_with_oauth_scopes(scopes: Option<&str>) -> ProfileConfig {
        ProfileConfig {
            oauth_scopes: scopes.map(String::from),
            ..ProfileConfig::default()
        }
    }

    #[test]
    fn resolve_oauth_scopes_none_returns_default() {
        let p = profile_with_oauth_scopes(None);
        assert_eq!(
            resolve_oauth_scopes(&p).unwrap(),
            auth::DEFAULT_OAUTH_SCOPES
        );
    }

    #[test]
    fn resolve_oauth_scopes_trims_and_collapses_whitespace() {
        let p = profile_with_oauth_scopes(Some(
            "  read:issue:jira   write:comment:jira\n\toffline_access  ",
        ));
        assert_eq!(
            resolve_oauth_scopes(&p).unwrap(),
            "read:issue:jira write:comment:jira offline_access"
        );
    }

    #[test]
    fn resolve_oauth_scopes_empty_string_is_config_error() {
        let p = profile_with_oauth_scopes(Some(""));
        let err = resolve_oauth_scopes(&p).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("oauth_scopes is empty"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn resolve_oauth_scopes_whitespace_only_is_config_error() {
        let p = profile_with_oauth_scopes(Some("   \n\t  "));
        let err = resolve_oauth_scopes(&p).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("oauth_scopes is empty"),
            "unexpected error: {msg}"
        );
    }

    /// Regression: `resolve_oauth_scopes` must read the *passed* profile,
    /// not anything off a `Config`. `login_oauth(profile, ...)` may target
    /// a non-active profile and used to resolve scopes from the active
    /// profile, silently returning the wrong scope list.
    #[test]
    fn resolve_oauth_scopes_inspects_passed_profile_not_active() {
        let custom = ProfileConfig {
            oauth_scopes: Some("custom:scope offline_access".into()),
            ..ProfileConfig::default()
        };
        assert_eq!(
            resolve_oauth_scopes(&custom).unwrap(),
            "custom:scope offline_access"
        );
    }

    /// The default scope literal is a backward-compatibility contract for
    /// every user who hasn't opted into `oauth_scopes`. A typo that drops
    /// `offline_access` would silently break refresh tokens for everyone.
    #[test]
    fn default_oauth_scopes_is_the_classic_set_with_offline_access() {
        assert_eq!(
            auth::DEFAULT_OAUTH_SCOPES,
            "read:jira-work write:jira-work read:jira-user offline_access"
        );
    }

    #[test]
    fn resolve_logout_target_defaults_to_active() {
        let global = crate::config::GlobalConfig::default();
        assert_eq!(resolve_logout_target(&global, None, "default"), "default");
        assert_eq!(
            resolve_logout_target(&global, Some("sandbox"), "default"),
            "sandbox"
        );
    }

    #[test]
    fn switch_to_unknown_profile_returns_error() {
        let result = handle_switch_in_memory(GlobalConfig::default(), "ghost");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("unknown profile"), "got: {msg}");
        assert!(msg.contains("ghost"), "got: {msg}");
    }

    #[test]
    fn switch_to_known_profile_mutates_default_profile() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert("sandbox".to_string(), ProfileConfig::default());
        let global = GlobalConfig {
            default_profile: Some("default".into()),
            profiles,
            ..GlobalConfig::default()
        };
        let mutated = handle_switch_in_memory(global, "sandbox").unwrap();
        assert_eq!(mutated.default_profile.as_deref(), Some("sandbox"));
    }

    #[test]
    fn remove_active_profile_returns_error() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            crate::config::ProfileConfig::default(),
        );
        let global = crate::config::GlobalConfig {
            default_profile: Some("default".into()),
            profiles,
            ..crate::config::GlobalConfig::default()
        };
        let result = handle_remove_in_memory(global, "default", "default");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("cannot remove active"), "got: {msg}");
    }

    #[test]
    fn remove_unknown_profile_returns_error() {
        let global = crate::config::GlobalConfig {
            default_profile: Some("default".into()),
            ..crate::config::GlobalConfig::default()
        };
        let result = handle_remove_in_memory(global, "ghost", "default");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("unknown profile"), "got: {msg}");
    }

    #[test]
    fn remove_existing_non_active_profile_succeeds() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            crate::config::ProfileConfig::default(),
        );
        profiles.insert(
            "sandbox".to_string(),
            crate::config::ProfileConfig::default(),
        );
        let global = crate::config::GlobalConfig {
            default_profile: Some("default".into()),
            profiles,
            ..crate::config::GlobalConfig::default()
        };
        let mutated = handle_remove_in_memory(global, "sandbox", "default").unwrap();
        assert!(!mutated.profiles.contains_key("sandbox"));
        assert!(mutated.profiles.contains_key("default"));
    }

    fn three_profile_fixture() -> GlobalConfig {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                url: Some("https://acme.atlassian.net".into()),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        profiles.insert(
            "sandbox".to_string(),
            ProfileConfig {
                url: Some("https://acme-sandbox.atlassian.net".into()),
                auth_method: Some("oauth".into()),
                cloud_id: Some("xyz-789".into()),
                ..ProfileConfig::default()
            },
        );
        profiles.insert(
            "staging".to_string(),
            ProfileConfig {
                url: Some("https://acme-staging.atlassian.net".into()),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        GlobalConfig {
            default_profile: Some("default".into()),
            profiles,
            ..GlobalConfig::default()
        }
    }

    #[test]
    fn list_table_snapshot() {
        let global = three_profile_fixture();
        let rendered = render_list_table(&global, "default");
        insta::assert_snapshot!(rendered);
    }

    #[test]
    fn list_json_shape() {
        let global = three_profile_fixture();
        let json = render_list_json(&global, "default").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().expect("array");
        assert_eq!(arr.len(), 3);
        let active: Vec<&serde_json::Value> = arr
            .iter()
            .filter(|p| p["active"].as_bool() == Some(true))
            .collect();
        assert_eq!(active.len(), 1, "exactly one active");
        assert_eq!(active[0]["name"], "default");
    }

    #[test]
    fn login_create_new_profile_no_input_requires_url() {
        let global = crate::config::GlobalConfig::default();
        let result = prepare_login_target(global, Some("sandbox"), None, true, "default");
        assert!(result.is_err());
        let msg = format!("{:#}", result.unwrap_err());
        assert!(msg.contains("--url required"), "got: {msg}");
    }

    #[test]
    fn login_create_new_profile_with_url_succeeds() {
        let global = crate::config::GlobalConfig::default();
        let (mutated, target) = prepare_login_target(
            global,
            Some("sandbox"),
            Some("https://sandbox.example"),
            true,
            "default",
        )
        .unwrap();
        assert_eq!(target, "sandbox");
        assert_eq!(
            mutated.profiles["sandbox"].url.as_deref(),
            Some("https://sandbox.example")
        );
    }

    #[test]
    fn login_existing_profile_with_url_updates_url() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "default".to_string(),
            crate::config::ProfileConfig {
                url: Some("https://old.example".into()),
                ..crate::config::ProfileConfig::default()
            },
        );
        let global = crate::config::GlobalConfig {
            default_profile: Some("default".into()),
            profiles,
            ..crate::config::GlobalConfig::default()
        };
        let (mutated, target) = prepare_login_target(
            global,
            Some("default"),
            Some("https://new.example"),
            true,
            "default",
        )
        .unwrap();
        assert_eq!(target, "default");
        assert_eq!(
            mutated.profiles["default"].url.as_deref(),
            Some("https://new.example")
        );
    }

    /// Regression: when `--profile` is omitted, fallback uses the active
    /// profile name (which encodes flag > env > config), NOT the
    /// `default_profile` config field — using the latter ignores the
    /// `JR_PROFILE` env / `--profile` global flag.
    #[test]
    fn login_falls_back_to_active_profile_name_not_default_profile_field() {
        let mut profiles = std::collections::BTreeMap::new();
        profiles.insert(
            "from-env".into(),
            crate::config::ProfileConfig {
                url: Some("https://from-env.example".into()),
                ..crate::config::ProfileConfig::default()
            },
        );
        let global = crate::config::GlobalConfig {
            default_profile: Some("from-config".into()),
            profiles,
            ..crate::config::GlobalConfig::default()
        };
        let (_mutated, target) =
            prepare_login_target(global, None, Some("https://x.example"), true, "from-env")
                .unwrap();
        assert_eq!(
            target, "from-env",
            "must follow active_profile_name, not default_profile field"
        );
    }

    /// Resolution order: flag → env → keychain → embedded → prompt.
    /// Flag wins even when env is set.
    #[test]
    fn resolve_oauth_app_credentials_flag_wins() {
        let (id, secret, source) = resolve_oauth_app_credentials_for_test(
            Some("flag-id".into()),
            Some("flag-secret".into()),
            None, // env_id
            None, // env_secret
            None, // keychain
            None, // embedded
            true, // no_input
        )
        .expect("flag path must succeed");
        assert_eq!(id, "flag-id");
        assert_eq!(secret, "flag-secret");
        assert_eq!(source, crate::api::auth_embedded::OAuthAppSource::Flag);
    }

    #[test]
    fn resolve_oauth_app_credentials_env_wins_over_keychain() {
        let (id, secret, source) = resolve_oauth_app_credentials_for_test(
            None,
            None,
            Some("env-id".into()),
            Some("env-secret".into()),
            Some(("kc-id".into(), "kc-secret".into())),
            None,
            true,
        )
        .unwrap();
        assert_eq!(
            (id.as_str(), secret.as_str(), source),
            (
                "env-id",
                "env-secret",
                crate::api::auth_embedded::OAuthAppSource::Env
            )
        );
    }

    #[test]
    fn resolve_oauth_app_credentials_keychain_wins_over_embedded() {
        let (id, _, source) = resolve_oauth_app_credentials_for_test(
            None,
            None,
            None,
            None,
            Some(("kc-id".into(), "kc-secret".into())),
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        )
        .unwrap();
        assert_eq!(id, "kc-id");
        assert_eq!(source, crate::api::auth_embedded::OAuthAppSource::Keychain);
    }

    #[test]
    fn resolve_oauth_app_credentials_embedded_when_no_user_input() {
        let (id, secret, source) = resolve_oauth_app_credentials_for_test(
            None,
            None,
            None,
            None,
            None,
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        )
        .unwrap();
        assert_eq!(
            (id.as_str(), secret.as_str(), source),
            (
                "embed-id",
                "embed-secret",
                crate::api::auth_embedded::OAuthAppSource::Embedded
            )
        );
    }

    #[test]
    fn resolve_oauth_app_credentials_no_input_errors_when_all_absent() {
        let err = resolve_oauth_app_credentials_for_test(None, None, None, None, None, None, true)
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("OAuth"), "got: {msg}");
        assert!(
            msg.contains("--client-id") || msg.contains("JR_OAUTH_CLIENT_ID"),
            "error must cite the BYO escape hatch: {msg}"
        );
    }

    #[test]
    fn resolve_oauth_app_credentials_partial_flag_id_errors() {
        let err = resolve_oauth_app_credentials_for_test(
            Some("partial-id".into()),
            None, // missing flag_secret
            None,
            None,
            None,
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--client-id"), "got: {msg}");
        assert!(msg.contains("--client-secret"), "got: {msg}");
    }

    #[test]
    fn resolve_oauth_app_credentials_partial_flag_secret_errors() {
        let err = resolve_oauth_app_credentials_for_test(
            None,
            Some("partial-secret".into()),
            None,
            None,
            None,
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--client-id"), "got: {msg}");
        assert!(msg.contains("--client-secret"), "got: {msg}");
    }

    #[test]
    fn resolve_oauth_app_credentials_partial_env_id_errors() {
        let err = resolve_oauth_app_credentials_for_test(
            None,
            None,
            Some("env-id".into()),
            None, // missing env_secret
            None,
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("JR_OAUTH_CLIENT_ID"), "got: {msg}");
        assert!(msg.contains("JR_OAUTH_CLIENT_SECRET"), "got: {msg}");
    }

    #[test]
    fn resolve_oauth_app_credentials_partial_env_secret_errors() {
        let err = resolve_oauth_app_credentials_for_test(
            None,
            None,
            None,
            Some("env-secret".into()),
            None,
            Some(("embed-id".into(), "embed-secret".into())),
            true,
        )
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("JR_OAUTH_CLIENT_ID"), "got: {msg}");
        assert!(msg.contains("JR_OAUTH_CLIENT_SECRET"), "got: {msg}");
    }

    /// `jr` deliberately does NOT reject mixed classic+granular scopes,
    /// unknown scope names, or missing `offline_access` — Atlassian returns
    /// `invalid_scope` at token exchange per the spec's "Out of scope"
    /// section. Locks this so a future refactor that starts "helping" with
    /// client-side validation fails visibly.
    #[test]
    fn resolve_oauth_scopes_does_not_validate_scope_shape() {
        let inputs = [
            "read:jira-work read:issue:jira",           // classic + granular mix
            "read:issue:jira write:issue:jira",         // no offline_access
            "totally-made-up-scope another-fake-scope", // unknown scopes
            "offline_access",                           // only offline_access
        ];
        for raw in inputs {
            let p = profile_with_oauth_scopes(Some(raw));
            let result = resolve_oauth_scopes(&p).unwrap_or_else(|e| {
                panic!("resolve_oauth_scopes must pass {raw:?} through unchanged, got error: {e:#}")
            });
            assert_eq!(result, raw, "input {raw:?} must pass through unchanged");
        }
    }

    #[test]
    fn peek_oauth_app_source_keychain_wins() {
        assert_eq!(
            peek_oauth_app_source_for_test(true, true),
            OAuthAppSource::Keychain
        );
        assert_eq!(
            peek_oauth_app_source_for_test(true, false),
            OAuthAppSource::Keychain
        );
    }

    #[test]
    fn peek_oauth_app_source_embedded_when_no_keychain() {
        assert_eq!(
            peek_oauth_app_source_for_test(false, true),
            OAuthAppSource::Embedded
        );
    }

    #[test]
    fn peek_oauth_app_source_none_when_nothing_resolves() {
        assert_eq!(
            peek_oauth_app_source_for_test(false, false),
            OAuthAppSource::None
        );
    }
}
