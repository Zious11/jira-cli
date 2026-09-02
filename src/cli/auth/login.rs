use anyhow::{Context, Result};
use dialoguer::Select;

use crate::api::auth;
use crate::api::auth_embedded::OAuthAppSource;
use crate::cli::OutputFormat;
use crate::config::{Config, global_config_path};
use crate::error::JrError;
use crate::output;
use crate::profile::Profile;

use super::keychain::{ENV_API_TOKEN, ENV_EMAIL};
use super::{
    auth_json_response, check_noninteractive_oauth_guard, emit_oauth_deprecation_notice,
    resolve_credential, resolve_oauth_app_credentials,
};

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
/// `auth_method = "api_token"` after a successful login. As of
/// S-cycle3-percred-storage (BC-1.4.031), the keychain entry for API token +
/// email is namespaced per profile (`<profile>:email` / `<profile>:api-token`
/// via [`auth::store_api_token`]) — symmetric with the existing per-profile
/// OAuth token namespacing, not shared/flat across profiles.
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

    auth::store_api_token(&crate::profile::Profile::from(profile), &email, &token)?;

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
    cloud_id_override: Option<&str>,
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

    let result = crate::api::auth::oauth_login(
        profile,
        &client_id,
        &client_secret,
        &scopes,
        strategy,
        cloud_id_override,
        no_input,
    )
    .await?;

    // Persist site info to the named profile under [profiles.<name>], not
    // the legacy [instance] block. Reload to pick up any mutations made
    // earlier in the login flow (e.g., by `prepare_login_target`). Same
    // lenient-load rationale as the earlier reload above.
    // Capture fields before moving into config.
    let site_name = result.site_name.clone();
    let site_url = result.site_url.clone();
    let cloud_id_stored = result.cloud_id.clone();

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

    output::print_success(&format!(
        "Authenticated with {site_name} ({site_url}) [cloudId: {cloud_id_stored}]"
    ));
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
    /// BC-1.2.050: select `api_token` directly, skipping the BC-1.1.013
    /// interactive OAuth-default picker. Mutually exclusive with `oauth`
    /// (enforced by clap `conflicts_with` at the CLI layer — see
    /// `AuthCommand::Login` in `src/cli/mod.rs`).
    pub api_token: bool,
    pub email: Option<String>,
    pub token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    /// Cloud ID override for multi-org disambiguation (--cloud-id flag).
    /// Only meaningful when `oauth` is true; passed through to `login_oauth`.
    pub cloud_id: Option<String>,
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
    // BC-1.1.016 Postcondition 3: the airtight non-interactive OAuth guard
    // is evaluated as the FIRST statement in this function — before
    // `Config::load_lenient_with` (which could itself block on a malformed
    // config file), any prompt, any credential resolution, and any HTTP or
    // browser code. Precondition 2a: explicit `--oauth` under a
    // non-interactive trigger. The picker path (BC-1.1.013) can never
    // select OAuth non-interactively — SR-012's precedence skips the
    // picker entirely whenever `args.no_input` is set — so checking only
    // `args.oauth` here covers every way this invocation could otherwise
    // reach OAuth under `--no-input`.
    check_noninteractive_oauth_guard(args.no_input, args.oauth)?;

    // BC-1.2.049 Postcondition 2: `--oauth` is a deprecated-but-accepted
    // alias. The guard above already ruled out the non-interactive case,
    // so reaching here means this is a functional (interactive, or
    // explicit non-interactive api-token-incompatible) `--oauth` use —
    // stderr-only, human-mode-only (EC-1.2.049-1).
    if args.oauth {
        emit_oauth_deprecation_notice(args.output);
    }

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
        .unwrap_or(config.active_profile_name.as_ref());
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
        config.active_profile_name.as_ref(),
    )?;
    config.global = global;
    config.save_global()?;

    // Capture the target profile's CURRENT auth_method (if any) before it
    // gets overwritten by login_oauth/login_token below — needed for the
    // EC-1.1.013-2/EC-1.1.014-4 re-declaration credential-clear decision.
    // `prepare_login_target` only touches `url`/`default_profile`, so this
    // still reflects whatever was on disk prior to this invocation.
    let current_auth_method = config
        .global
        .profiles
        .get(&target)
        .and_then(|p| p.auth_method.clone());

    // SR-012 precedence (BC-1.1.013 Invariant 2): explicit flag (tier 1) >
    // BC-1.1.014 non-interactive default (tier 2) > BC-1.1.013 interactive
    // OAuth-default picker (tier 3). Each tier is consulted only when every
    // tier above it does not apply.
    let oauth_selected = if args.oauth {
        true
    } else if args.api_token {
        false
    } else if args.no_input {
        // BC-1.1.014 Precondition 1 (SR-010-corrected): only `--no-input`/
        // non-TTY trigger the non-interactive default — env var presence
        // alone does not (EC-1.1.014-3, AC-006).
        false
    } else {
        prompt_auth_method_picker()?
    };
    let new_method = if oauth_selected { "oauth" } else { "api_token" };

    // BC-1.1.013 EC-1.1.013-2 / BC-1.1.014 EC-1.1.014-4 (M-1): a
    // mechanism-switching re-declaration clears the outgoing mechanism's
    // credentials before/alongside writing the new mechanism's. The
    // SHOULD-level stderr notice is scoped to the non-interactive switch
    // case only — the interactive picker interaction already makes the
    // switch visible to the user.
    let switching = current_auth_method
        .as_deref()
        .is_some_and(|current| current != new_method);
    clear_outgoing_mechanism_on_switch(
        &Profile::from(target.clone()),
        current_auth_method.as_deref(),
        new_method,
        args.no_input && switching,
    )?;

    if oauth_selected {
        login_oauth(
            &target,
            args.client_id,
            args.client_secret,
            args.cloud_id.as_deref(),
            args.no_input,
        )
        .await?;
    } else {
        login_token(&target, args.email, args.token, args.no_input).await?;
    }
    if matches!(args.output, OutputFormat::Json) {
        println!(
            "{}",
            output::render_json(&auth_json_response(&target, "login"))?
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

/// CWE-400 (MED-1, pre-PR review): independent, `no_input`-blind terminal
/// check used immediately before [`prompt_auth_method_picker`] would invoke
/// `dialoguer::Select::interact()`.
///
/// `handle_login`'s tier-3 picker branch is normally gated by `args.no_input`
/// alone, which is itself normally set correctly by `src/main.rs`'s
/// auto-`--no-input` flip on non-TTY stdin. But that flip has a documented
/// exception: `JR_OAUTH_CODE` (an OAuth-callback test seam, ungated by
/// `#[cfg(debug_assertions)]`) suppresses the flip so a test harness can pipe
/// stdin while still driving interactive selection. That means a *release*
/// build with `JR_OAUTH_CODE` set and non-TTY stdin can reach `handle_login`
/// with `no_input == false` even though there is no real terminal — which
/// would otherwise let the picker call `Select::interact()` and hang a
/// non-interactive session (CWE-400: uncontrolled resource consumption).
///
/// This function performs its OWN terminal detection rather than trusting
/// the caller's `no_input`, so the picker is unreachable-by-construction
/// without a real interactive terminal — regardless of `no_input`, and
/// regardless of `JR_OAUTH_CODE`. It deliberately mirrors `src/main.rs`'s
/// `JR_STDIN_IS_TTY` debug-only override (so interactive tests that force
/// `JR_STDIN_IS_TTY=1` still reach the picker) but does NOT mirror
/// `main.rs`'s `JR_OAUTH_CODE` exception — that exception is exactly the gap
/// this check exists to close.
fn stdin_is_interactive_tty() -> bool {
    use std::io::IsTerminal;
    #[cfg(debug_assertions)]
    let forced = std::env::var("JR_STDIN_IS_TTY")
        .map(|v| v == "1")
        .unwrap_or(false);
    #[cfg(not(debug_assertions))]
    let forced = false;
    forced || std::io::stdin().is_terminal()
}

/// BC-1.1.013: creation-time OAuth-default picker for `jr auth login`'s bare
/// interactive path. Per SR-012's mechanism-selection precedence
/// (BC-1.1.013 Invariant 2), this is tier 3 — reached only when neither an
/// explicit `--oauth`/`--api-token` flag (tier 1, BC-1.2.049/BC-1.2.050) nor
/// a non-interactive trigger (tier 2, BC-1.1.014) applies.
///
/// Must mirror `jr init`'s picker (`src/cli/init.rs::handle`, read-only
/// reference for this story) byte-for-byte: same items (`"OAuth 2.0
/// (recommended)"`, `"API Token"`), same `dialoguer::Select` with
/// `.default(0)`. Returns `true` when OAuth is selected, `false` for API
/// token — matching `LoginArgs::oauth`'s shape so a caller can reuse the
/// existing `if oauth { login_oauth(..) } else { login_token(..) }`
/// dispatch unchanged.
///
/// CWE-400 (MED-1, pre-PR review): before touching `dialoguer::Select`, this
/// independently confirms stdin is a real interactive terminal via
/// [`stdin_is_interactive_tty`] — NOT via the caller's `no_input` value. When
/// stdin is not a real TTY, this returns `Ok(false)` (the same token-first
/// default BC-1.1.014's non-interactive tier already selects) instead of
/// calling `Select::interact()`, so the picker can never hang a
/// non-interactive session no matter how it is reached.
pub fn prompt_auth_method_picker() -> Result<bool> {
    if !stdin_is_interactive_tty() {
        return Ok(false);
    }
    let auth_methods = ["OAuth 2.0 (recommended)", "API Token"];
    let choice = Select::new()
        .with_prompt("Authentication method")
        .items(auth_methods)
        .default(0)
        .interact()
        .context("failed to prompt for authentication method")?;
    Ok(choice == 0)
}

/// BC-1.1.013 EC-1.1.013-2 / BC-1.1.014 EC-1.1.014-4 (M-1): clear the
/// OUTGOING mechanism's per-profile credentials when an `auth login`
/// invocation (interactive re-declaration OR non-interactive mechanism
/// switch) is about to persist `new_method` onto `profile` and that differs
/// from the profile's CURRENT `auth_method` — before or alongside writing
/// the new mechanism's credentials.
///
/// A SAME-mechanism re-declaration, or a first-time declaration
/// (`current_auth_method` is `None`), is a caller-side no-op — the existing
/// `store_api_token`/`store_oauth_tokens` write already overwrites in
/// place, so callers must not invoke this function in that case.
///
/// MUST reuse [`crate::api::auth::clear_profile_creds`]'s existing per-kind
/// branches (OAuth-pair AND API-token-pair deletion, ADR-0020 §Decision 7 /
/// BC-1.2.014) — the O-1/SR-011 requirement is explicit that per-kind
/// clearing must not be re-implemented inline here.
///
/// `emit_switch_notice` gates BC-1.1.014 EC-1.1.014-4's SHOULD-level
/// informational stderr line for a NON-interactive mechanism switch (e.g.
/// `"Profile '<profile>' auth method changed from 'oauth' to
/// 'api_token'."`). The interactive re-declaration path (BC-1.1.013
/// EC-1.1.013-2) should pass `false` — the picker interaction itself
/// already makes the switch visible to the user.
pub fn clear_outgoing_mechanism_on_switch(
    profile: &Profile,
    current_auth_method: Option<&str>,
    new_method: &str,
    emit_switch_notice: bool,
) -> Result<()> {
    let Some(outgoing) = current_auth_method else {
        // First-time declaration — nothing to clear, caller-side no-op per
        // this function's own contract.
        return Ok(());
    };
    if outgoing == new_method {
        // Same-mechanism re-declaration — the ordinary store_* overwrite
        // path handles this; no separate clear step (AC-004).
        return Ok(());
    }

    // Reuse clear_profile_creds's existing per-kind branches (OAuth-pair
    // AND API-token-pair deletion) rather than re-implementing per-kind
    // clearing inline — the O-1/SR-011 requirement this function's doc
    // comment cites explicitly. clear_profile_creds clears BOTH kinds
    // unconditionally, so it's correct regardless of which specific
    // mechanism is the outgoing one.
    auth::clear_profile_creds(profile).with_context(|| {
        format!(
            "failed to clear profile {:?}'s outgoing '{outgoing}' credentials \
             before switching to '{new_method}'",
            profile.as_ref()
        )
    })?;

    if emit_switch_notice {
        eprintln!(
            "Profile '{}' auth method changed from '{outgoing}' to '{new_method}'.",
            profile.as_ref()
        );
    }

    Ok(())
}
