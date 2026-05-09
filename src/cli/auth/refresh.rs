use anyhow::{Context, Result};

use crate::api::auth;
use crate::config::Config;
use crate::error::JrError;

use super::{AuthFlow, chosen_flow_for_profile, login_oauth, login_token};

/// Post-refresh guidance shown to humans (stderr, Table mode) and embedded
/// in the JSON payload (`next_step`). Click "Always Allow" on the keychain
/// write prompts so future commands run silently.
const REFRESH_HELP_LINE: &str = "If prompted to allow keychain access, choose \"Always Allow\" so future commands run silently.";

/// Build the `--output json` success payload. Extracted for unit-testing the
/// shape (status key, auth_method label, next_step guidance) without needing
/// to drive the full login flow.
pub(crate) fn refresh_success_payload(flow: AuthFlow) -> serde_json::Value {
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
            login_oauth(
                &target,
                args.client_id,
                args.client_secret,
                None,
                args.no_input,
            )
            .await
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
