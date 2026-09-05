use anyhow::Result;

use crate::config::Config;
use crate::error::JrError;
use crate::output;

use super::{
    AuthFlow, check_noninteractive_oauth_guard, chosen_flow_for_profile,
    emit_api_token_inert_on_refresh_notice, emit_oauth_deprecation_notice, login_oauth,
    login_token,
};

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

/// Bundle of CLI arguments threaded from `main.rs` to [`refresh_credentials`].
///
/// Same rationale as [`LoginArgs`](super::LoginArgs) — passing all credential
/// slots plus the flow toggle and `--profile` as positional parameters trips
/// clippy's `too_many_arguments` lint, so they're grouped into a struct that
/// also makes the call site at `main.rs` self-documenting.
pub struct RefreshArgs<'a> {
    pub profile: Option<&'a str>,
    pub oauth: bool,
    /// BC-1.2.050 Postcondition 3 (O-2/CV-2): syntactically accepted for
    /// symmetry with `LoginArgs::api_token`, but INERT on `auth refresh` —
    /// `refresh` always follows the target profile's own stored
    /// `auth_method` (BC-1.2.051). Presence emits the inert-with-notice
    /// stderr line (`emit_api_token_inert_on_refresh_notice`), stderr-only
    /// and human-mode-only, mirroring `oauth`'s BC-1.2.049 deprecation
    /// notice's output-channel rules.
    pub api_token: bool,
    pub email: Option<String>,
    pub token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub no_input: bool,
    pub output: &'a crate::cli::OutputFormat,
}

/// Refresh (re-obtain) the stored credentials for a profile.
///
/// On macOS this is the recovery path for the legacy Keychain ACL/partition
/// invalidation that occurs after `jr` is replaced at its installed path
/// (e.g., `brew upgrade`). See spec at
/// `docs/superpowers/specs/2026-04-17-keychain-prompts-207-design.md`.
///
/// **AMENDED by S-cycle3-chosen-flow-reconcile (BC-1.2.051 Invariant 2, I-6).**
/// Ordering is **relogin-then-replace**, not "clear-then-login" (the prior
/// term/behavior was self-contradicting — it admitted clearing credentials
/// BEFORE confirming a replacement was obtainable, which directly violates
/// "a failed refresh must never leave a profile in a WORSE, credential-less
/// state than before the command was run"). The corrected contract: the new
/// credential value must be obtained and confirmed usable FIRST; only then
/// does the stored credential get overwritten. A refresh that fails to
/// obtain a usable replacement (network error, cancelled interactive
/// re-prompt, EOF on stdin) must leave the existing
/// `<profile>:email`/`<profile>:api-token` (or OAuth) pair completely
/// intact and propagate the error — never a prior delete step. This is a
/// genuine logic reordering of the credential-obtain/replace sequence, not
/// a comment-only fix; see BC-1.2.051 AC-006/AC-007.
///
/// Per BC-5.38.005's self-check, the `RefreshArgs` field access, config
/// load, target-profile resolution, the BC-1.1.016 non-interactive OAuth
/// guard, the BC-1.2.049/050 deprecation/inertness notices, and the
/// URL-completeness precondition are all PRE-EXISTING control flow this
/// story left intact. Only the relogin-then-replace credential-obtain/store
/// sequence (previously the clear-then-login block) and its terminal
/// success/error output were reordered.
pub async fn refresh_credentials(args: RefreshArgs<'_>) -> Result<()> {
    // Pass `args.profile` as the CLI-flag override so a `--profile X`
    // against an unconfigured X surfaces the strict load's "unknown
    // profile" error rather than silently refreshing the active profile.
    let config = Config::load_with(args.profile)?;
    let target = args
        .profile
        .map(str::to_string)
        .unwrap_or_else(|| config.active_profile_name.to_string());
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
    // BC-1.2.051 AC-001/AC-002/AC-003/AC-004: `chosen_flow_for_profile` no
    // longer takes an `--oauth`/override argument — it resolves solely from
    // `target_profile.auth_method`. `args.oauth`/`args.api_token` are still
    // read below (guard evaluation, deprecation/inertness notices), but
    // neither reaches this call.
    let flow = chosen_flow_for_profile(&target_profile);

    // BC-1.1.016 Postcondition 3, Precondition 2b: the airtight
    // non-interactive OAuth guard, evaluated as the FIRST thing done with
    // `flow` — before the URL-completeness check below, any credential
    // clear, and any login dispatch. `chosen_flow_for_profile` resolves
    // solely from `target_profile.auth_method` (BC-1.2.051 AC-001..AC-004,
    // DEC-321) — the explicit `--oauth` flag is deliberately NOT folded
    // into this resolution on `refresh` (unlike `login`); `--oauth` never
    // reaches `chosen_flow_for_profile` at all, so it cannot override an
    // api_token-method profile's resolved flow here. This single check
    // covers only the implicit oauth-method-profile case (Precondition
    // 2b): `flow == AuthFlow::OAuth` is true exactly when the profile's
    // own `auth_method` says so. `--api-token` similarly has no override
    // power here (EC-1.1.016-3) — it never reaches `chosen_flow_for_profile`
    // either.
    check_noninteractive_oauth_guard(args.no_input, flow == AuthFlow::OAuth)?;

    // BC-1.2.049 / BC-1.2.050: stderr-only, human-mode-only notices for the
    // two flags. `--oauth`/`--api-token` are mutually exclusive at the clap
    // layer, so at most one of these fires. Reached only once the guard
    // above has passed.
    if args.oauth {
        emit_oauth_deprecation_notice(*args.output);
    }
    if args.api_token {
        emit_api_token_inert_on_refresh_notice(*args.output);
    }

    // For the api_token flow, login_token re-prompts/sets the per-profile
    // namespaced `<profile>:email`/`<profile>:api-token` pair (BC-1.4.031)
    // but doesn't write a URL. If the target profile has no
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

    // Relogin-then-replace (BC-1.2.051 Invariant 2 / I-6): obtain/confirm
    // the new credential value FIRST, then replace the stored one — never a
    // prior delete step. `login_token`/`login_oauth` already have this
    // shape internally: each resolves/obtains its new credential value
    // (`resolve_credential` for Token; the OAuth browser round-trip for
    // OAuth) and ONLY THEN persists it via `auth::store_api_token` /
    // `auth::store_oauth_tokens` — both a plain, unconditional two-key
    // `set_password` overwrite of the existing keychain entry, which is
    // itself the "atomic-in-effect store" BC-1.2.051 Invariant 2 calls for.
    // Calling them directly, with no separate clear/delete step beforehand,
    // is therefore sufficient: on failure (e.g. `resolve_credential`'s
    // no-input/missing-value check, or a failed/cancelled OAuth round-trip)
    // neither store function is ever reached, so the existing
    // <profile>:email/<profile>:api-token (or OAuth) pair is left
    // completely untouched. This replaces the prior "clear-then-login"
    // sequence (`clear_profile_oauth_pair`/`clear_all_credentials` called
    // BEFORE login), which is the exact I-6 self-contradiction this story
    // removes — it admitted deleting the working credential before a
    // replacement was confirmed obtainable.
    let login_result = match flow {
        // S-cycle4-cloud-id-correctness (BC-1.2.052 Invariant 3, AC-006):
        // `RefreshArgs` has no `--cloud-id` flag — hardcoded `None`, mirroring
        // the existing sibling `login_oauth(..., None, ...)` call below.
        // `login_token`'s tenant_info fetch still fires on every such
        // `auth refresh` invocation (fetch-on-every-invocation, not
        // override-gated) — intentional, not an oversight (ADR-0022 §2).
        AuthFlow::Token => {
            login_token(
                &target,
                args.email,
                args.token,
                None,
                args.no_input,
                *args.output,
            )
            .await
        }
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
        // Corrected I-6 framing: the old "Credentials were cleared, but…"
        // message is gone because it is no longer true — relogin-then-
        // replace never touches the existing credential until a
        // replacement has been confirmed obtainable.
        eprintln!(
            "Refresh failed; your existing credentials for {target:?} were \
             left unchanged. Run `{login_cmd}` to try again."
        );
        return Err(err);
    }

    match args.output {
        crate::cli::OutputFormat::Json => {
            let payload = output::render_json(&refresh_success_payload(flow))?;
            println!("{payload}");
        }
        crate::cli::OutputFormat::Table => {
            eprintln!("Credentials refreshed. {REFRESH_HELP_LINE}");
        }
    }

    Ok(())
}
