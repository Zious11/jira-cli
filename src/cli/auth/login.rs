use anyhow::{Context, Result};

use crate::api::auth;
use crate::api::auth_embedded::OAuthAppSource;
use crate::cli::OutputFormat;
use crate::config::{Config, global_config_path};
use crate::error::JrError;
use crate::output;

use super::keychain::{ENV_API_TOKEN, ENV_EMAIL};
use super::{auth_json_response, resolve_credential, resolve_oauth_app_credentials};

/// Pick the OAuth scope string: user override from the *target* profile's
/// `oauth_scopes` if set, else the compiled-in default. Trims and collapses
/// interior whitespace so multi-line TOML strings encode cleanly. Empty or
/// whitespace-only overrides are a configuration error.
///
/// Takes a `&ProfileConfig` (not a `&Config`) so callers like `login_oauth`
/// can pass the profile they're actually targeting; reading `Config`'s
/// active profile would silently return the wrong scopes when
/// `jr auth login --profile X` runs against a non-active X.
pub(crate) fn resolve_oauth_scopes(profile: &crate::config::ProfileConfig) -> Result<String> {
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
        if crate::api::auth_embedded::embedded_oauth_app_present() {
            eprintln!("OAuth 2.0: by default, official jr binaries use the embedded \"jr\" app.");
            eprintln!("To use your own OAuth app instead, pass --client-id and --client-secret,");
            eprintln!("or set JR_OAUTH_CLIENT_ID and JR_OAUTH_CLIENT_SECRET.\n");
        } else {
            eprintln!(
                "OAuth 2.0: this build has no embedded OAuth app (likely a fork or source build)."
            );
            eprintln!("Pass --client-id and --client-secret,");
            eprintln!("or set JR_OAUTH_CLIENT_ID and JR_OAUTH_CLIENT_SECRET.\n");
        }
    }

    let (client_id, client_secret, source) =
        resolve_oauth_app_credentials(client_id, client_secret, no_input)?;

    // Embedded credentials get the registered fixed callback. Every other
    // source is BYO and stays on the historical dynamic-port flow — the
    // user has registered their own callback URL.
    let strategy = match source {
        OAuthAppSource::Embedded => crate::api::auth::RedirectUriStrategyRequest::Fixed(
            crate::api::auth::EMBEDDED_CALLBACK_PORT,
        ),
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
    pub output: OutputFormat,
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
        login_oauth(&target, args.client_id, args.client_secret, args.no_input).await?;
    } else {
        login_token(&target, args.email, args.token, args.no_input).await?;
    }
    if matches!(args.output, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&auth_json_response(&target, "login"))
                .expect("auth JSON response serialization cannot fail")
        );
    }
    Ok(())
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
pub(crate) fn prepare_login_target(
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
