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
///
/// `cloud_id_override` (S-cycle4-cloud-id-correctness, BC-1.2.052) is
/// symmetric with [`login_oauth`]'s existing parameter of the same name.
/// Ordered fallback chain (ADR-0022 §2), applied on EVERY invocation, not
/// only brand-new-profile creation (BC-1.2.052 Invariant 3, BC-1.2.053):
/// 1. `cloud_id_override` supplied → used directly, fetch skipped, value
///    persisted (BC-1.2.052 Postcondition 1).
/// 2. Otherwise, [`crate::api::jira::tenant::fetch_cloud_id`] is attempted
///    against the profile's `url` (already resolved by
///    `prepare_login_target` before this function runs).
/// 3. On fetch failure, soft-fail: `p.cloud_id` is left untouched and a
///    single `eprintln!` diagnostic is emitted in Table (human) mode only
///    (BC-1.2.052 Postcondition 3) — see [`resolve_and_apply_cloud_id`]'s
///    own doc comment for the output-channel contract.
///
/// `output` selects the output-channel contract for the soft-fail
/// diagnostic — passed straight through to [`resolve_and_apply_cloud_id`].
///
/// Call sites (BC-1.2.052 Invariant 3 — exactly three): `handle_login`
/// (`auth login`, threads `args.output`), `jr init`'s API-token branch
/// (hardcoded `OutputFormat::Table` — `jr init` is inherently interactive),
/// and `refresh_credentials` (`jr auth refresh`, threads `*args.output`;
/// `cloud_id_override` stays hardcoded `None` — no `--cloud-id` flag on
/// `RefreshArgs`).
pub async fn login_token(
    profile: &str,
    email: Option<String>,
    token: Option<String>,
    cloud_id_override: Option<&str>,
    no_input: bool,
    output: OutputFormat,
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

    resolve_and_apply_cloud_id(&mut config, profile, cloud_id_override, output).await;

    config.save_global()?;

    eprintln!("Credentials stored in keychain.");
    Ok(())
}

/// `cloud_id` acquisition fallback chain shared by every `login_token` call
/// site (S-cycle4-cloud-id-correctness, BC-1.2.052/053, ADR-0022 §2/§3).
///
/// Mutates `config`'s entry for `profile` in place; does NOT call
/// `config.save_global()` — the caller ([`login_token`]) persists once,
/// alongside the `auth_method`/`default_profile` writes already made in the
/// same load/save cycle.
///
/// Never returns an error — every failure mode (missing override, fetch
/// failure of any shape, non-`https://` `site_url`) is a soft-fail per
/// BC-1.2.052 Postcondition 3/Invariant 2: `p.cloud_id` is left as it
/// already was, and a single `eprintln!` diagnostic is emitted — but ONLY in
/// Table (human) output mode, gated on `matches!(output, OutputFormat::Table)`.
/// Under `--output json` the diagnostic is suppressed entirely: nothing is
/// written to stderr, and the return value reflects that (`None`, not the
/// would-be message) so a caller/test can't mistake "suppressed" for
/// "printed". This mirrors the established sibling convention in this same
/// module — [`super::emit_oauth_deprecation_notice`] and
/// [`super::emit_api_token_inert_on_refresh_notice`] gate their stderr
/// notices the same way — and matches this function's own long-standing doc
/// claim of being "human-mode-only," which prior to this change was
/// documented but not actually implemented (ADV MED, S-cycle4-cloud-id-
/// correctness). This is the single implementation both the
/// override-precedence (AC-001), fetch-success-overwrite (AC-005), and
/// mechanism-switch refresh-not-clear (AC-007, BC-1.2.053) acceptance
/// criteria share — no separate "mechanism switch" detection code exists
/// anywhere (BC-1.2.053 Invariant 1).
///
/// Returns the exact text of the `eprintln!` diagnostic when one was
/// ACTUALLY EMITTED — i.e. only on a fetch-failure/no-URL soft-fail path
/// AND `output == OutputFormat::Table` (`None` on the override path, the
/// fetch-success path, and any soft-fail path under `OutputFormat::Json`,
/// none of which print anything) — a testability seam only (ADV LOW-1):
/// production callers ([`login_token`]) ignore it, since the diagnostic has
/// already reached stderr as a side effect by the time this returns (when
/// it returns `Some` at all). This lets tests assert on the exact soft-fail
/// wording (preserved-vs-none, ADV LOW-4) AND on JSON-mode suppression
/// (ADV MED) without needing to capture the real stderr file descriptor.
///
/// Structural note (ADV MED-B, S-cycle4-cloud-id-correctness fix burst):
/// the would-be diagnostic (`diag: Option<String>`, computed independently
/// of `output` on the no-URL/fetch-failure paths) and the actual emit
/// (`eprintln!`) are decided by a SINGLE tail gate — `let emit = if
/// matches!(output, OutputFormat::Table) { diag } else { None };` — rather
/// than two separate `if matches!(output, OutputFormat::Table) { eprintln!
/// …; return Some(msg) }` blocks (one per soft-fail branch, as this
/// function had prior to this fix burst). This makes the return value a
/// faithful-by-construction proxy for what actually reached stderr: there
/// is exactly one `eprintln!(` call in this function's body (down from
/// two), and it is reached if and only if `emit` — the value this function
/// returns — is `Some`. A hand-edit can no longer hoist an `eprintln!`
/// above the mode gate on just one branch while leaving the other, or the
/// return-value tests, apparently intact — both branches now share the one
/// gate. This is also why a dedicated in-process real-stderr-fd capture
/// test is unnecessary here: there is no crate already in this repo for
/// that (checked — no `gag`-style dependency, and `assert_cmd`/`predicates`
/// are subprocess-level tools that cannot reach a module-private `async
/// fn`), so stderr silence under `--output json` is guaranteed
/// STRUCTURALLY by this single-gate design instead, pinned by two
/// independent things: the return-value assertions on this function
/// (`test_adv_med_json_mode_fetch_failure_suppresses_diagnostic` et al.,
/// below) and the source-level guard confirming no stdout leak exists
/// (`test_adv_low1_resolve_cloud_id_source_has_no_stdout_macro_or_write`).
async fn resolve_and_apply_cloud_id(
    config: &mut Config,
    profile: &str,
    cloud_id_override: Option<&str>,
    output: OutputFormat,
) -> Option<String> {
    if let Some(override_value) = cloud_id_override {
        // AC-001 (BC-1.2.052 Postcondition 1): explicit override takes
        // precedence — the fetch is never attempted — and is written,
        // symmetric with a fetch-success write.
        let p = config
            .global
            .profiles
            .entry(profile.to_string())
            .or_default();
        p.cloud_id = Some(override_value.to_string());
        return None;
    }

    let site_url = config
        .global
        .profiles
        .get(profile)
        .and_then(|p| p.url.clone());

    // `diag` is the diagnostic that WOULD be shown, computed independently
    // of `output` — the single tail gate below is the only place `output`
    // decides anything.
    let diag: Option<String> = match site_url {
        None => {
            // No URL to fetch against at all — soft-fail, leave cloud_id
            // as-is.
            Some(format!(
                "warning: could not look up cloud_id for profile {profile:?} — no URL configured."
            ))
        }
        Some(site_url) => {
            // Captured before the fetch so the soft-fail diagnostic
            // (below) can distinguish "a prior value survives untouched"
            // from "there was never one to begin with" (ADV LOW-4) —
            // BC-1.2.053 Invariant 3: on `auth refresh` this fetch fires
            // on every invocation, so a transient failure must not read
            // as an error when the existing value is in fact preserved
            // intact.
            let had_existing_cloud_id = config
                .global
                .profiles
                .get(profile)
                .is_some_and(|p| p.cloud_id.is_some());

            match crate::api::jira::tenant::fetch_cloud_id(&site_url).await {
                Ok(cloud_id) => {
                    // AC-005 / AC-007 (BC-1.2.052 Postcondition 5,
                    // BC-1.2.053 Postcondition 1): fetch success
                    // overwrites p.cloud_id unconditionally, even a stale
                    // OAuth-era value.
                    let p = config
                        .global
                        .profiles
                        .entry(profile.to_string())
                        .or_default();
                    p.cloud_id = Some(cloud_id);
                    None
                }
                Err(err) => {
                    // AC-003 / AC-007 (BC-1.2.052 Postcondition 3,
                    // BC-1.2.053 Postcondition 2): soft-fail — p.cloud_id
                    // is left as it already was (None stays None; a prior
                    // value survives untouched). Never abort login_token.
                    Some(if had_existing_cloud_id {
                        format!(
                            "warning: could not refresh cloud_id for profile {profile:?} ({err}); keeping the existing value."
                        )
                    } else {
                        format!(
                            "warning: could not look up cloud_id for profile {profile:?}: {err}"
                        )
                    })
                }
            }
        }
    };

    // Single decision point (ADV MED-B): human-mode-only (BC-1.2.052
    // Postcondition 3) — suppressed entirely under `--output json`,
    // matching the sibling notice convention (`emit_oauth_deprecation_notice`
    // et al.). `eprintln!` fires IFF this function returns `Some` — there is
    // no way for the two to desync.
    let emit = if matches!(output, OutputFormat::Table) {
        diag
    } else {
        None
    };
    if let Some(m) = &emit {
        eprintln!("{m}");
    }
    emit
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
    /// Passed through to `login_oauth` on the OAuth branch and, as of
    /// S-cycle4-cloud-id-correctness (BC-1.2.052 Postcondition 1), to
    /// `login_token` on the API-token branch too — previously silently
    /// dropped there.
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

    // BC-1.1.013 EC-1.1.013-2 / BC-1.1.014 EC-1.1.014-4 (M-1), AMENDED by
    // FIX-F5-login-switch (relogin-then-replace ordering, mirroring I-6 /
    // BC-1.2.051's fix to `refresh_credentials`): a mechanism-switching
    // re-declaration must obtain and STORE the new mechanism's credentials
    // FIRST, and only clear the outgoing mechanism's now-orphaned
    // credentials AFTER that has succeeded. The old "clear-then-login"
    // ordering (clearing before dispatch) left a profile credential-less if
    // the new login failed (browser cancel, network error, a missing
    // `--no-input` value) — strictly worse than the pre-command state. See
    // `clear_outgoing_mechanism_on_switch`'s doc comment for why the clear
    // step itself was also narrowed to per-kind (never
    // `auth::clear_profile_creds`, which would delete the credentials this
    // reorder just stored).
    let switching = current_auth_method
        .as_deref()
        .is_some_and(|current| current != new_method);

    // PR #771 review Finding B-1 (BC-1.4.039): for a profile with NO prior
    // `auth_method` on record (a brand-new profile), persist the SELECTED
    // mechanism now, BEFORE attempting the flow below — not only after it
    // succeeds. Without this, a brand-new profile whose OAuth login fails
    // partway through (e.g. Site 1's `DpapiFallbackFailed` honest-fail case
    // in `src/api/auth.rs`) is left with `auth_method: None`, which `jr
    // auth logout` treats as an api-token profile ("nothing to log out") —
    // exactly the wrong outcome for the cleanup command that failure
    // message recommends as the default remediation. Scoped to the
    // no-prior-method case only (`should_mark_auth_method_before_attempt`
    // returns `false` whenever `switching` above would be `true`) — a
    // mechanism SWITCH must still record the new mechanism only after a
    // successful login, or a failed switch would mislabel a profile whose
    // PRIOR mechanism's credentials are still valid and working.
    //
    // PR #771 fresh-context re-review Finding NEW-1: `current_auth_method
    // == None` alone is an unsafe proxy for "nothing to protect" — a
    // profile migrated from the legacy `[instance]` config shape can carry
    // `auth_method: None` while STILL holding working credentials under
    // some label. Probe the keychain before pre-marking so that case isn't
    // mislabelled-then-broken by a failing switch. The probe is only
    // performed when `current_auth_method` is `None` — when it's `Some(_)`,
    // `should_mark_auth_method_before_attempt` short-circuits to `false`
    // regardless of this value, so probing there would be a wasted (and, on
    // some platforms, OS-prompting) keychain round-trip.
    let has_stored_credentials = if current_auth_method.is_none() {
        auth::profile_has_stored_credentials(&Profile::from(target.clone()))?
    } else {
        false
    };
    if should_mark_auth_method_before_attempt(
        current_auth_method.as_deref(),
        has_stored_credentials,
    ) {
        config.global = mark_auth_method_if_new(
            config.global,
            &target,
            current_auth_method.as_deref(),
            new_method,
            has_stored_credentials,
        );
        config.save_global()?;
    }

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
        // BC-1.2.052 Postcondition 1: `--cloud-id` was previously silently
        // dropped on the API-token branch (`login_oauth`'s sibling call
        // above already threads it through). Symmetric fix.
        login_token(
            &target,
            args.email,
            args.token,
            args.cloud_id.as_deref(),
            args.no_input,
            args.output,
        )
        .await?;
    }

    // Reached only when the login above succeeded — a failed login returns
    // early via `?` and this line, and therefore the outgoing-credential
    // clear, never runs. The SHOULD-level stderr notice is scoped to the
    // non-interactive switch case only — the interactive picker interaction
    // already makes the switch visible to the user.
    clear_outgoing_mechanism_on_switch(
        &Profile::from(target.clone()),
        current_auth_method.as_deref(),
        new_method,
        args.no_input && switching,
    )?;

    if matches!(args.output, OutputFormat::Json) {
        println!(
            "{}",
            output::render_json(&auth_json_response(&target, "login"))?
        );
    }
    Ok(())
}

/// PR #771 review Finding B-1 (BC-1.4.039): decide whether [`handle_login`]
/// should pre-mark the target profile's `auth_method` as the SELECTED
/// mechanism BEFORE attempting that mechanism's flow, rather than only
/// recording it after a successful login.
///
/// Scoped to a profile with NO established `auth_method` on record AND no
/// working credentials stored under any label yet — i.e. a genuinely
/// brand-new profile, or one whose prior login never completed. Returns
/// `false` whenever the profile already has a WORKING, different mechanism
/// (`current_auth_method` is `Some(_)`): this is `FIX-F5-login-switch`'s
/// "relogin-then-replace" territory, where flipping the label eagerly would
/// mislabel the profile as the NEW mechanism (with no credentials yet)
/// while the OLD mechanism's still-valid credentials remain in place — a
/// subsequent ordinary command would then try (and fail) to authenticate
/// with the missing new mechanism instead of the still-working old one.
///
/// **`has_stored_credentials` (PR #771 fresh-context re-review Finding
/// NEW-1):** `current_auth_method.is_none()` alone is an UNSAFE proxy for
/// "brand-new profile, nothing to protect" — `None` is also the state of a
/// profile migrated from the legacy `[instance]` config shape
/// (`crate::config::migrate_legacy_global`), which can carry `auth_method:
/// None` while STILL holding a working, already-namespaced credential pair
/// in the keychain (see [`crate::api::auth::profile_has_stored_credentials`]
/// for exactly what "working" means here). Without this parameter, a
/// failing mechanism switch on such a profile would persist the NEW
/// mechanism's label with no credentials behind it, silently orphaning the
/// profile's still-working OLD credentials — the same "relogin-then-replace"
/// failure mode `current_auth_method` alone already protects against for a
/// non-`None` label, just reached via the `None` blind spot instead. Pass
/// the caller's [`crate::api::auth::profile_has_stored_credentials`] probe
/// result here; this function stays pure and takes it as a plain `bool` so
/// it remains disk/keychain-I/O-free and trivially unit-testable.
///
/// For a genuinely brand-new profile (`current_auth_method` is `None` AND
/// `has_stored_credentials` is `false`) there is nothing to protect: no
/// working credentials exist under any label yet, so marking the intended
/// mechanism early only helps `jr auth logout`/`jr auth remove` correctly
/// recognize the profile if the login attempt fails partway through (e.g.
/// at the credential-store step — the exact scenario BC-1.4.039's Site-1
/// honest-fail message's cleanup recommendation targets).
pub(crate) fn should_mark_auth_method_before_attempt(
    current_auth_method: Option<&str>,
    has_stored_credentials: bool,
) -> bool {
    current_auth_method.is_none() && !has_stored_credentials
}

/// Applies [`should_mark_auth_method_before_attempt`]'s decision to a
/// `GlobalConfig`: when the target profile currently has no `auth_method`
/// on record AND no working credentials stored under any label
/// (`has_stored_credentials == false`), sets it to `method` now. No-op
/// (returns `global` unchanged modulo the entry always existing) whenever
/// the profile already has an established, different mechanism, OR already
/// holds working credentials under some label despite the `None` record
/// (PR #771 re-review Finding NEW-1) — see the sibling function's doc
/// comment for why. Pure over `GlobalConfig` and `has_stored_credentials`,
/// mirroring [`prepare_login_target`]'s disk-I/O-free testability — callers
/// compute the keychain probe themselves and pass the result in.
pub(crate) fn mark_auth_method_if_new(
    mut global: crate::config::GlobalConfig,
    target: &str,
    current_auth_method: Option<&str>,
    method: &str,
    has_stored_credentials: bool,
) -> crate::config::GlobalConfig {
    if should_mark_auth_method_before_attempt(current_auth_method, has_stored_credentials) {
        global
            .profiles
            .entry(target.to_string())
            .or_default()
            .auth_method = Some(method.to_string());
    }
    global
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

/// BC-1.1.013 EC-1.1.013-2 / BC-1.1.014 EC-1.1.014-4 (M-1), AMENDED by
/// FIX-F5-login-switch (relogin-then-replace ordering): clear the OUTGOING
/// mechanism's per-profile credentials once an `auth login` invocation
/// (interactive re-declaration OR non-interactive mechanism switch) has
/// ALREADY persisted `new_method`'s credentials onto `profile`, and that
/// differs from the profile's PRIOR `auth_method`.
///
/// **Caller contract (data-loss fix, mirrors I-6 / BC-1.2.051's fix to
/// `refresh_credentials`):** this function MUST be called only AFTER
/// `login_oauth`/`login_token` has returned `Ok`, never before. The old
/// "clear-then-login" ordering called this BEFORE dispatching the new
/// login, so a failed login (browser cancel, network error, a missing
/// `--no-input` value) left the profile with the outgoing mechanism's
/// credentials already deleted and no replacement ever stored — worse than
/// the profile's state before the command ran. `handle_login` now calls
/// `login_oauth`/`login_token` first and only reaches this call on success;
/// a failed login returns early via `?` and this function is never invoked,
/// so the prior credentials stay completely intact.
///
/// A SAME-mechanism re-declaration, or a first-time declaration
/// (`current_auth_method` is `None`), is a caller-side no-op — the existing
/// `store_api_token`/`store_oauth_tokens` write already overwrites in
/// place, so callers must not invoke this function in that case.
///
/// **Clears ONLY the outgoing mechanism's pair — NOT both kinds.** Reuses
/// [`crate::api::auth::clear_profile_oauth_pair`] /
/// [`crate::api::auth::clear_profile_api_token_pair`], the SAME per-kind
/// primitives [`crate::api::auth::clear_profile_creds`] is itself built
/// from (ADR-0020 §Decision 7 / BC-1.2.014) — so per-kind clearing is still
/// not re-implemented inline here, just dispatched to the single relevant
/// kind instead of both. This is load-bearing under the new ordering:
/// because this call now runs AFTER the new mechanism's credentials are
/// already stored, calling the combined `clear_profile_creds` (which clears
/// BOTH the OAuth pair AND the API-token pair unconditionally) would delete
/// the credentials this function's caller just persisted. Dispatching on
/// `outgoing` alone keeps the newly-stored `new_method` pair untouched.
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

    // Dispatch to the per-kind clear matching ONLY the outgoing mechanism —
    // never the combined clear_profile_creds, which would also delete the
    // new_method credentials this function's caller (handle_login) has
    // already stored by the time this runs. An unrecognized `outgoing`
    // value (e.g. a hand-edited config) has nothing of ours to clear.
    let clear_result = match outgoing {
        "oauth" => auth::clear_profile_oauth_pair(profile),
        "api_token" => auth::clear_profile_api_token_pair(profile),
        _ => Ok(()),
    };
    clear_result.with_context(|| {
        format!(
            "failed to clear profile {:?}'s outgoing '{outgoing}' credentials \
             after switching to '{new_method}'",
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

/// S-cycle4-cloud-id-correctness — Two-Step Red Gate TESTS (step 2 of 2) for
/// `resolve_and_apply_cloud_id`'s override/fetch/soft-fail fallback chain
/// (BC-1.2.052/053, ADR-0022 §2/§3, VP-AUTHDX-019/020).
///
/// `resolve_and_apply_cloud_id` is module-private — this inline test module
/// is the ONLY place these behaviors can be exercised WITHOUT going through
/// the real OS keychain (`login_token`'s `auth::store_api_token` write).
/// This is exactly the "config-layer verification without a real credential
/// store" AC-008/VP-AUTHDX-020 calls for: an in-memory `Config`/
/// `ProfileConfig` plus `wiremock`, no keychain touched anywhere below.
///
/// ## `JR_TENANT_INFO_URL` seam — REQUIRED, NOT YET IMPLEMENTED
///
/// `fetch_cloud_id` (ADR-0022 §1, `src/api/jira/tenant.rs`) REQUIRES
/// `site_url` to start with `https://` — a real security invariant (Pass-4
/// adversarial review Finding #4: closes an on-path plaintext
/// wrong-tenant-`cloudId` vector), not a testing inconvenience to relax.
/// `wiremock` 0.6.5 has NO HTTPS/TLS support (verified directly against its
/// public `MockServerBuilder` API: only `.listener()` and
/// `.disable_request_recording()` exist — no TLS/cert configuration of any
/// kind), so a genuine 200-plus-`cloudId` response can never be produced by
/// pointing `fetch_cloud_id` at a real `wiremock` server while honoring the
/// `https://` requirement literally: the TLS handshake against a plaintext
/// server fails before any HTTP semantics are ever exchanged. This is a
/// hard technical constraint of the available tooling, not a design choice
/// made by this test suite, and it is NOT solvable by adding a new
/// dependency (this story's own Library & Framework Requirements table
/// says no new dependency is introduced).
///
/// The tests below that need a fetch *success* therefore assume
/// `fetch_cloud_id` gains a debug-only `JR_TENANT_INFO_URL` env var
/// override: when set, the ACTUAL GET request goes to
/// `format!("{}/_edge/tenant_info", env::var("JR_TENANT_INFO_URL").unwrap())`
/// instead of `format!("{}/_edge/tenant_info", site_url)`, while `site_url`
/// itself is STILL what the `https://`-prefix precondition check validates.
/// This is the same "override the actual network target while the logical
/// argument still drives validation" shape as the already-established
/// `JR_BASE_URL` (`src/config.rs::Config::base_url`) and
/// `JR_ACCESSIBLE_RESOURCES_URL` (`src/api/auth.rs::oauth_login`) seams —
/// see CLAUDE.md's "AI Agent Notes" section for the full family. It should
/// be gated `#[cfg(debug_assertions)]` exactly like its siblings, with a
/// matching CLAUDE.md entry and a `tests/*_release_gate.rs` pin added in
/// the SAME commit, per this codebase's own documented convention for new
/// `JR_*` test seams.
///
/// Until this seam is added, every test below fails via the current
/// `todo!()` panic (Red Gate, step 2 — this is the required state right
/// now). Once `resolve_and_apply_cloud_id`/`fetch_cloud_id` are implemented
/// literally per ADR-0022's reference code WITHOUT this seam, the
/// success-dependent tests will continue to fail (a TLS handshake error,
/// not `Ok`) rather than flip green — flagged prominently in this story's
/// Test Writer report as the single highest-priority open question, not
/// left for a future reader to silently rediscover. The failure-path tests
/// (soft-fail / preserve-on-failure) need no such seam and are fully
/// self-contained: any `http://`/scheme-less `site_url` deterministically
/// triggers `fetch_cloud_id`'s own https-only precondition skip, which is
/// itself a legitimate, spec-documented failure shape (EC-1.2.052-2) from
/// this caller's point of view.
#[cfg(test)]
mod cloud_id_fallback_chain_tests {
    use std::collections::BTreeMap;
    use std::sync::OnceLock;

    use tokio::sync::Mutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::cli::OutputFormat;
    use crate::config::{Config, GlobalConfig, ProfileConfig, ProjectConfig};

    use super::resolve_and_apply_cloud_id;

    /// Serializes access to the process-global `JR_TENANT_INFO_URL` env var
    /// across this module's tests — mirrors `src/config.rs`'s `ENV_MUTEX`
    /// pattern for `JR_BASE_URL`, but async-aware (`tokio::sync::Mutex`, not
    /// `std::sync::Mutex`) since the guard is held across `.await` points
    /// here — `clippy::await_holding_lock` requires this. Only this
    /// module's tests touch this env var, so a module-local guard is
    /// sufficient (no cross-file races).
    fn env_mutex() -> &'static Mutex<()> {
        static M: OnceLock<Mutex<()>> = OnceLock::new();
        M.get_or_init(|| Mutex::new(()))
    }

    /// Build a minimal, fully in-memory `Config` with exactly one profile —
    /// no disk I/O, no keychain, matching AC-008's config-layer contract.
    fn make_config(profile_name: &str, url: Option<&str>, cloud_id: Option<&str>) -> Config {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            profile_name.to_string(),
            ProfileConfig {
                url: url.map(str::to_string),
                cloud_id: cloud_id.map(str::to_string),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        Config {
            global: GlobalConfig {
                default_profile: Some(profile_name.to_string()),
                profiles,
                ..GlobalConfig::default()
            },
            project: ProjectConfig::default(),
            active_profile_name: profile_name.into(),
        }
    }

    fn cloud_id_of<'a>(config: &'a Config, profile_name: &str) -> Option<&'a str> {
        config
            .global
            .profiles
            .get(profile_name)
            .and_then(|p| p.cloud_id.as_deref())
    }

    /// AC-001 (BC-1.2.052 Postcondition 1, VP-AUTHDX-019): an explicit
    /// `--cloud-id` override takes precedence — the fetch is never
    /// attempted (proven by pointing `url` at a value that could never
    /// resolve a real fetch) — and the override value is WRITTEN to
    /// `p.cloud_id`, symmetric with a fetch-success write, not merely used
    /// in-memory to skip the fetch.
    #[tokio::test]
    async fn test_ac_001_explicit_override_takes_precedence_and_is_written() {
        let mut config = make_config("sandbox", Some("not-a-real-url"), None);

        resolve_and_apply_cloud_id(
            &mut config,
            "sandbox",
            Some("override-uuid-123"),
            OutputFormat::Table,
        )
        .await;

        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("override-uuid-123"),
            "AC-001: explicit --cloud-id override must overwrite p.cloud_id \
             even though the profile's url could never resolve a real fetch"
        );
    }

    /// Multi-profile boundary pin (CLAUDE.md "Multi-profile boundary":
    /// "Cross-profile cache leakage is a correctness bug, not a UX issue"
    /// — the same principle applies to config writes, not just caches):
    /// `resolve_and_apply_cloud_id` must write `cloud_id` onto the NAMED
    /// `profile` argument it was called with, never onto
    /// `config.active_profile_name`, when the two differ. Builds a config
    /// with TWO distinct profiles — "prod" is the ACTIVE profile, "sandbox"
    /// is the TARGET profile passed explicitly to the function (as
    /// `jr auth login --profile sandbox` would do while some other profile
    /// is active) — and asserts the write landed only on "sandbox" while
    /// "prod" (including its own pre-existing `cloud_id`) is left
    /// completely untouched. Uses the override path (no network/seam
    /// needed) purely to isolate this from any fetch-success/failure
    /// behavior already covered by the tests above.
    #[tokio::test]
    async fn test_resolve_cloud_id_writes_named_profile_not_active_profile() {
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "prod".to_string(),
            ProfileConfig {
                url: Some("https://prod.example.atlassian.net".to_string()),
                cloud_id: Some("prod-untouched-uuid".to_string()),
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        profiles.insert(
            "sandbox".to_string(),
            ProfileConfig {
                url: Some("not-a-real-url".to_string()),
                cloud_id: None,
                auth_method: Some("api_token".into()),
                ..ProfileConfig::default()
            },
        );
        let mut config = Config {
            global: GlobalConfig {
                default_profile: Some("prod".to_string()),
                profiles,
                ..GlobalConfig::default()
            },
            project: ProjectConfig::default(),
            // The ACTIVE profile is "prod" — deliberately different from
            // the "sandbox" profile named in the call below.
            active_profile_name: "prod".into(),
        };

        resolve_and_apply_cloud_id(
            &mut config,
            "sandbox",
            Some("sandbox-override-uuid"),
            OutputFormat::Table,
        )
        .await;

        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("sandbox-override-uuid"),
            "the write must land on the NAMED profile argument (\"sandbox\"), \
             regardless of which profile is active"
        );
        assert_eq!(
            cloud_id_of(&config, "prod"),
            Some("prod-untouched-uuid"),
            "the ACTIVE profile (\"prod\") must be completely untouched — a \
             write keyed on `active_profile_name` instead of the `profile` \
             argument would corrupt it here"
        );
        assert_eq!(
            config.active_profile_name, "prod",
            "resolve_and_apply_cloud_id must not mutate active_profile_name itself"
        );
    }

    /// AC-001 (Pass-2 adversarial review Finding #8): the override REPLACES
    /// a pre-existing value too, not merely a brand-new-profile
    /// None -> Some transition.
    #[tokio::test]
    async fn test_ac_001_explicit_override_replaces_existing_cloud_id() {
        let mut config = make_config("sandbox", Some("not-a-real-url"), Some("old-uuid"));

        resolve_and_apply_cloud_id(
            &mut config,
            "sandbox",
            Some("override-uuid-456"),
            OutputFormat::Table,
        )
        .await;

        assert_eq!(cloud_id_of(&config, "sandbox"), Some("override-uuid-456"));
    }

    /// AC-003 (BC-1.2.052 Postcondition 3, Invariant 2; VP-AUTHDX-019): on
    /// fetch failure (here: the https-only precondition skip, EC-1.2.052-2
    /// — the cheapest deterministic way to force an `Err` from
    /// `fetch_cloud_id` with zero network dependency) — a brand-new
    /// profile's `p.cloud_id` stays `None`; the function never aborts its
    /// caller.
    #[tokio::test]
    async fn test_ac_003_soft_fail_leaves_new_profile_cloud_id_none() {
        let mut config = make_config("sandbox", Some("http://not-https.example.net"), None);

        resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            None,
            "AC-003: a brand-new profile's cloud_id must stay None on fetch failure"
        );
    }

    /// AC-003, existing-profile half: a PRIOR value survives a fetch
    /// failure completely untouched (not merely None -> None).
    #[tokio::test]
    async fn test_ac_003_soft_fail_leaves_existing_cloud_id_untouched() {
        let mut config = make_config(
            "sandbox",
            Some("http://not-https.example.net"),
            Some("prior-uuid"),
        );

        resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        assert_eq!(cloud_id_of(&config, "sandbox"), Some("prior-uuid"));
    }

    /// EC-1.2.053-1: mechanism switch, fetch fails, and the profile never
    /// had a `cloud_id` to begin with -> preserved AS `None` (not `""`, not
    /// a panic) — Postcondition 2's "preserve" applies uniformly regardless
    /// of whether the preserved value is itself present or absent.
    #[tokio::test]
    async fn test_ec_1_2_053_1_preserve_none_on_failure_is_still_none() {
        let mut config = make_config("sandbox", Some("http://not-https.example.net"), None);

        resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        assert_eq!(cloud_id_of(&config, "sandbox"), None);
    }

    /// AC-005 / VP-AUTHDX-019 (BC-1.2.052 Postcondition 5): fetch SUCCESS
    /// overwrites `p.cloud_id` unconditionally and this function's caller
    /// (`login_token`) is expected to persist it via the normal
    /// `Config::save_global()` path (this test itself stays at the
    /// config-layer / does not touch disk).
    ///
    /// Depends on the `JR_TENANT_INFO_URL` seam documented in this module's
    /// header doc comment.
    #[tokio::test]
    async fn test_ac_005_fetch_success_overwrites_cloud_id() {
        let _guard = env_mutex().lock().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/_edge/tenant_info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "cloudId": "fetched-uuid-789"
            })))
            .mount(&server)
            .await;
        // SAFETY: env_mutex is held for this whole scope; no other test in
        // this module reads/writes JR_TENANT_INFO_URL concurrently.
        unsafe {
            std::env::set_var("JR_TENANT_INFO_URL", server.uri());
        }

        let mut config = make_config("sandbox", Some("https://plausible-site.example"), None);
        resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        unsafe {
            std::env::remove_var("JR_TENANT_INFO_URL");
        }

        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("fetched-uuid-789"),
            "AC-005: fetch success must overwrite p.cloud_id unconditionally"
        );
    }

    /// AC-007 (BC-1.2.053 Postcondition 1, VP-AUTHDX-020): the
    /// mechanism-switch scenario's SUCCESS branch — a stale OAuth-era
    /// `cloud_id` is unconditionally overwritten on fetch success. No
    /// switch-specific code exists (Invariant 1) — this exercises the exact
    /// same `resolve_and_apply_cloud_id` codepath as
    /// `test_ac_005_fetch_success_overwrites_cloud_id` above, just
    /// pre-seeded with a stale value matching BC-1.2.053's own precondition
    /// shape. Same `JR_TENANT_INFO_URL` seam dependency.
    #[tokio::test]
    async fn test_ac_007_mechanism_switch_overwrites_stale_cloud_id_on_fetch_success() {
        let _guard = env_mutex().lock().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/_edge/tenant_info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "cloudId": "fresh-uuid-after-switch"
            })))
            .mount(&server)
            .await;
        unsafe {
            std::env::set_var("JR_TENANT_INFO_URL", server.uri());
        }

        let mut config = make_config(
            "sandbox",
            Some("https://plausible-site.example"),
            Some("stale-oauth-era-uuid"),
        );
        resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        unsafe {
            std::env::remove_var("JR_TENANT_INFO_URL");
        }

        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("fresh-uuid-after-switch"),
            "AC-007/BC-1.2.053 Postcondition 1: fetch success must overwrite \
             even a stale OAuth-era cloud_id"
        );
    }

    /// AC-007 (BC-1.2.053 Postcondition 2, VP-AUTHDX-020): the
    /// mechanism-switch scenario's FAILURE branch — a stale value is
    /// PRESERVED, never bare-cleared, on fetch failure. Fully testable
    /// without any new seam — the https-skip is itself a legitimate
    /// `fetch_cloud_id` failure from this caller's point of view.
    #[tokio::test]
    async fn test_ac_007_mechanism_switch_preserves_stale_cloud_id_on_fetch_failure() {
        let mut config = make_config(
            "sandbox",
            Some("http://not-https.example.net"),
            Some("stale-oauth-era-uuid"),
        );

        resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("stale-oauth-era-uuid"),
            "AC-007/BC-1.2.053 Postcondition 2: a stale value must be \
             preserved on fetch failure, never bare-cleared"
        );
    }

    /// ADV LOW-1 / LOW-4: pins the soft-fail OBSERVABLE that no prior test
    /// asserted — the exact wording of the diagnostic when a PRIOR
    /// `cloud_id` survives a fetch failure untouched (BC-1.2.053 Invariant
    /// 3, the `auth refresh` fetch-every-invocation case where a
    /// transient failure must read as "kept", not as a bare error).
    /// `resolve_and_apply_cloud_id` still succeeds (soft-fail — no panic,
    /// no `Result::Err` bubbled to the caller) and hands the diagnostic
    /// back via its return value so this can be asserted without a real
    /// stderr-fd capture; production emits the identical text via
    /// `eprintln!` (stderr only — this function's body contains no bare
    /// `print!`/`println!` macro call and no reference to `stdout`,
    /// verified by
    /// `test_adv_low1_resolve_cloud_id_source_has_no_stdout_macro_or_write`
    /// below), so the returned text and what a real invocation prints to
    /// stderr are one and the same string.
    #[tokio::test]
    async fn test_adv_low4_preserved_cloud_id_message_names_existing_value_kept() {
        let mut config = make_config(
            "sandbox",
            Some("http://not-https.example.net"),
            Some("prior-uuid"),
        );

        let diagnostic =
            resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        let msg =
            diagnostic.expect("ADV LOW-1: a fetch failure must emit a diagnostic and hand it back");
        assert!(
            msg.contains("keeping the existing value"),
            "ADV LOW-4: the message must convey the prior cloud_id was KEPT, \
             not read as a bare error, when a value already existed. Got: {msg}"
        );
        assert!(
            msg.contains("\"sandbox\""),
            "the message must name the affected profile. Got: {msg}"
        );
        assert!(
            !msg.contains("no URL configured"),
            "the no-URL-configured wording must not appear here — a URL WAS \
             configured, just non-https. Got: {msg}"
        );
        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("prior-uuid"),
            "the prior value must actually survive untouched, matching the \
             message's claim"
        );
    }

    /// ADV LOW-1 / LOW-4 sibling: when there was NEVER a prior `cloud_id`,
    /// the ORIGINAL "could not look up" wording is correct as-is (there is
    /// nothing to report as "kept") and must be left unchanged — this test
    /// guards against the preserved-value wording bleeding into the
    /// absent-value case.
    #[tokio::test]
    async fn test_adv_low4_absent_cloud_id_message_keeps_original_lookup_wording() {
        let mut config = make_config("sandbox", Some("http://not-https.example.net"), None);

        let diagnostic =
            resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        let msg =
            diagnostic.expect("ADV LOW-1: a fetch failure must emit a diagnostic and hand it back");
        assert!(
            msg.starts_with("warning: could not look up cloud_id for profile \"sandbox\":"),
            "ADV LOW-4: with no prior value, the original (non-'kept') \
             wording is correct and must be preserved verbatim. Got: {msg}"
        );
        assert!(
            !msg.contains("keeping the existing value"),
            "there is no existing value to keep — this phrasing must not \
             appear. Got: {msg}"
        );
        assert_eq!(cloud_id_of(&config, "sandbox"), None);
    }

    /// AC-001 / fetch-success: the return-value seam (added for ADV LOW-1)
    /// must stay `None` on the two paths that print nothing, so a future
    /// caller can't mistake "printed nothing" for "printed an empty
    /// string".
    #[tokio::test]
    async fn test_adv_low1_no_diagnostic_returned_on_override_or_fetch_success() {
        let mut config = make_config("sandbox", Some("not-a-real-url"), None);
        let diagnostic = resolve_and_apply_cloud_id(
            &mut config,
            "sandbox",
            Some("override-uuid"),
            OutputFormat::Table,
        )
        .await;
        assert_eq!(
            diagnostic, None,
            "the explicit-override path prints nothing, so nothing should be returned"
        );

        let _guard = env_mutex().lock().await;
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/_edge/tenant_info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "cloudId": "fetched-uuid"
            })))
            .mount(&server)
            .await;
        unsafe {
            std::env::set_var("JR_TENANT_INFO_URL", server.uri());
        }
        let mut config2 = make_config("sandbox", Some("https://plausible-site.example"), None);
        let diagnostic2 =
            resolve_and_apply_cloud_id(&mut config2, "sandbox", None, OutputFormat::Table).await;
        unsafe {
            std::env::remove_var("JR_TENANT_INFO_URL");
        }
        assert_eq!(
            diagnostic2, None,
            "a successful fetch prints nothing, so nothing should be returned"
        );
    }

    /// ADV MED (S-cycle4-cloud-id-correctness fix burst, BC-1.2.052
    /// Postcondition 3): pins the Table-mode HALF of the human-mode-only
    /// contract explicitly and by name — a fetch failure under
    /// `OutputFormat::Table` must both emit the diagnostic (`Some(..)`
    /// returned, matching what a real invocation actually writes to
    /// stderr) AND leave the prior `cloud_id` preserved untouched. This is
    /// the positive control for
    /// `test_adv_med_json_mode_fetch_failure_suppresses_diagnostic` below —
    /// together the pair proves the gate actually branches on `output`
    /// rather than always emitting or always suppressing.
    #[tokio::test]
    async fn test_adv_med_table_mode_fetch_failure_emits_diagnostic() {
        let mut config = make_config(
            "sandbox",
            Some("http://not-https.example.net"),
            Some("prior-uuid"),
        );

        let diagnostic =
            resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        let msg = diagnostic.expect(
            "ADV MED: Table mode must emit and return a soft-fail diagnostic \
             on a fetch failure",
        );
        assert!(
            msg.starts_with("warning: could not refresh cloud_id for profile \"sandbox\"")
                && msg.contains("keeping the existing value"),
            "ADV MED: Table-mode diagnostic must use the preserved-value \
             wording (a prior cloud_id existed). Got: {msg}"
        );
        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("prior-uuid"),
            "the prior cloud_id must survive the fetch failure untouched"
        );
    }

    /// ADV MED (S-cycle4-cloud-id-correctness fix burst, BC-1.2.052
    /// Postcondition 3, VP-AUTHDX-019): the negative control — the SAME
    /// fetch-failure scenario as
    /// `test_adv_med_table_mode_fetch_failure_emits_diagnostic` above, but
    /// under `OutputFormat::Json`. The diagnostic must be fully suppressed
    /// (`None` returned — nothing was printed to stderr, matching the
    /// sibling convention on `emit_oauth_deprecation_notice`/
    /// `emit_api_token_inert_on_refresh_notice`), while login-level
    /// behavior is completely unaffected: the function still soft-fails
    /// rather than aborting, and the prior `cloud_id` still survives
    /// untouched. Before this fix burst, `resolve_and_apply_cloud_id` had
    /// no `output` parameter at all and always emitted — this test would
    /// not have compiled, let alone passed.
    #[tokio::test]
    async fn test_adv_med_json_mode_fetch_failure_suppresses_diagnostic() {
        let mut config = make_config(
            "sandbox",
            Some("http://not-https.example.net"),
            Some("prior-uuid"),
        );

        let diagnostic =
            resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Json).await;

        assert_eq!(
            diagnostic, None,
            "ADV MED: JSON mode must suppress the soft-fail diagnostic \
             entirely — nothing printed, so nothing returned"
        );
        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            Some("prior-uuid"),
            "suppressing the diagnostic must not change the soft-fail \
             preserve-on-failure behavior — the prior cloud_id still \
             survives untouched"
        );
    }

    /// ADV MED sibling: the no-URL-configured soft-fail branch (distinct
    /// code path from the fetch-failure branch above) must ALSO suppress
    /// its diagnostic under JSON mode, on a brand-new profile with no prior
    /// `cloud_id` to preserve.
    #[tokio::test]
    async fn test_adv_med_json_mode_no_url_configured_suppresses_diagnostic() {
        let mut config = make_config("sandbox", None, None);

        let diagnostic =
            resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Json).await;

        assert_eq!(
            diagnostic, None,
            "ADV MED: JSON mode must suppress the no-URL-configured \
             diagnostic too, not just the fetch-failure one"
        );
        assert_eq!(cloud_id_of(&config, "sandbox"), None);
    }

    /// ADV MED-A (S-cycle4-cloud-id-correctness fix burst): the positive
    /// control for `test_adv_med_json_mode_no_url_configured_suppresses_diagnostic`
    /// above — no prior test asserted the Table-mode (human-mode) HALF of
    /// the no-URL-configured branch, nor its exact wording. Mirrors
    /// `test_adv_med_table_mode_fetch_failure_emits_diagnostic`'s role for
    /// the fetch-failure branch: together with its JSON-mode sibling, this
    /// proves the no-URL-configured gate actually branches on `output`
    /// rather than always emitting or always suppressing.
    #[tokio::test]
    async fn test_adv_med_table_mode_no_url_configured_emits_diagnostic() {
        let mut config = make_config("sandbox", None, None);

        let diagnostic =
            resolve_and_apply_cloud_id(&mut config, "sandbox", None, OutputFormat::Table).await;

        assert_eq!(
            diagnostic,
            Some(
                "warning: could not look up cloud_id for profile \"sandbox\" — \
                 no URL configured."
                    .to_string()
            ),
            "ADV MED-A: Table mode must emit and return the exact \
             no-URL-configured diagnostic wording"
        );
        assert_eq!(
            cloud_id_of(&config, "sandbox"),
            None,
            "the no-URL-configured soft-fail must leave cloud_id untouched (None)"
        );
    }

    /// Counts occurrences of `pattern` (e.g. `"println!("` or `"print!("`)
    /// in `body` that are NOT immediately preceded by the byte `'e'` — i.e.
    /// occurrences that are not actually part of `eprintln!(`/`eprint!(`.
    /// This lets a single scan tell a genuinely bare stdout macro call
    /// apart from its `e`-prefixed stderr sibling, since (for example)
    /// `"println!("` is a literal substring of `"eprintln!("` starting one
    /// byte in.
    fn count_bare_macro_calls(body: &str, pattern: &str) -> usize {
        let bytes = body.as_bytes();
        let mut count = 0;
        let mut search_from = 0;
        while let Some(rel) = body[search_from..].find(pattern) {
            let abs = search_from + rel;
            let preceded_by_e = abs > 0 && bytes[abs - 1] == b'e';
            if !preceded_by_e {
                count += 1;
            }
            search_from = abs + pattern.len();
        }
        count
    }

    /// ADV LOW-1 (stderr-only channel, structural proof); ADV MED-2
    /// (strengthened, renamed from `test_adv_low1_no_stdout_writes_in_source`
    /// — that name asserted a guarantee ("no stdout writes") the original
    /// body did not actually check): every diagnostic this function can
    /// ever emit goes through `eprintln!`. This scan of the function's own
    /// source text rejects every stdout-writing shape this repo's
    /// `println!(`-only original guard would have missed: a bare
    /// `println!(` or `print!(` call (checked via `count_bare_macro_calls`,
    /// which excludes matches that are actually part of `eprintln!(`/
    /// `eprint!(`), AND any textual reference to `stdout` at all — which
    /// additionally catches `write!(std::io::stdout(), …)`,
    /// `writeln!(std::io::stdout(), …)`, and
    /// `std::io::stdout().write_all(…)`, none of which contain a bare
    /// `print!`/`println!` macro call for the first check to see. This is
    /// a source-level structural pin, in the same spirit as this repo's
    /// other reject-don't-parse text guards (see CLAUDE.md's CI Gate
    /// history), chosen because `resolve_and_apply_cloud_id` is a private
    /// fn with no in-process real-fd stderr capture available without a
    /// new dependency or an unrelated refactor.
    ///
    /// Verified (locally, then reverted — not left in the tree) that this
    /// guard actually fails on each of the vectors it claims to reject: a
    /// temporary `print!("x");` or `writeln!(std::io::stdout(), "x").ok();`
    /// inserted into `resolve_and_apply_cloud_id`'s body flips this test
    /// from green to a failing assertion before the edit is reverted.
    #[test]
    fn test_adv_low1_resolve_cloud_id_source_has_no_stdout_macro_or_write() {
        let src = include_str!("login.rs");
        let start = src
            .find("async fn resolve_and_apply_cloud_id(")
            .expect("resolve_and_apply_cloud_id must exist in this file");
        // The function ends at the first top-level `\n}\n` after its start;
        // scanning to the next `\n/// Run the OAuth 2.0` doc comment (the
        // next item in this file) is a safe, simple bound.
        let end = src[start..]
            .find("/// Run the OAuth 2.0 (3LO) login flow")
            .map(|i| start + i)
            .expect("the next item after resolve_and_apply_cloud_id must exist");
        let body = &src[start..end];
        let eprintln_count = body.matches("eprintln!(").count();
        assert_eq!(
            eprintln_count, 1,
            "sanity: expected exactly ONE eprintln! call in this function's \
             body (ADV MED-B, S-cycle4-cloud-id-correctness fix burst: both \
             soft-fail branches — no-URL-configured, fetch-failure — now \
             funnel through a single tail gate rather than each carrying \
             its own eprintln!/return pair) — either the test's start/end \
             source markers have drifted, or the single-gate structure was \
             reverted to two separate gates"
        );

        // Neither a bare `println!(` nor a bare `print!(` (stdout) may
        // appear inside this function's body — each must be zero, not
        // merely "no more than the eprintln count" (that looser check is
        // what ADV MED-2 is replacing).
        let bare_println_count = count_bare_macro_calls(body, "println!(");
        assert_eq!(
            bare_println_count, 0,
            "a bare stdout `println!(` call was found in \
             resolve_and_apply_cloud_id — this would leak a diagnostic onto \
             stdout, breaking the Output-channels convention (stdout stays \
             clean, including under --output json)"
        );
        let bare_print_count = count_bare_macro_calls(body, "print!(");
        assert_eq!(
            bare_print_count, 0,
            "a bare stdout `print!(` call was found in \
             resolve_and_apply_cloud_id — this would leak a diagnostic onto \
             stdout, breaking the Output-channels convention (stdout stays \
             clean, including under --output json)"
        );

        // No reference to `stdout` at all (case-sensitive) may appear in
        // this function's body — this closes the ADV MED-2 gap left by the
        // two macro-call checks above: `write!(std::io::stdout(), …)`,
        // `writeln!(std::io::stdout(), …)`, and
        // `std::io::stdout().write_all(…)` are all real stdout-leak shapes
        // that contain no bare `print!`/`println!` macro invocation.
        assert!(
            !body.contains("stdout"),
            "a reference to `stdout` was found in resolve_and_apply_cloud_id \
             — this function must write diagnostics to stderr only (via \
             `eprintln!`), never touch stdout by any mechanism (macro call, \
             `write!`/`writeln!`, or `.write_all(…)`)"
        );
    }
}
