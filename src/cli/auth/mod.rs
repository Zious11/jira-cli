mod keychain;
mod list;
mod login;
mod logout;
mod refresh;
mod remove;
mod status;
mod switch;

#[cfg(test)]
pub(crate) use keychain::{OAUTH_APP_HINT, resolve_oauth_app_credentials_for_test};
pub(crate) use keychain::{resolve_credential, resolve_oauth_app_credentials};
pub use list::handle_list;
#[cfg(test)]
pub(crate) use list::{render_env_column, render_list_json, render_list_table};
pub use login::{
    LoginArgs, clear_outgoing_mechanism_on_switch, handle_login, login_oauth, login_token,
    prompt_auth_method_picker,
};
#[cfg(test)]
pub(crate) use login::{prepare_login_target, resolve_oauth_scopes};
pub use logout::handle_logout;
#[cfg(test)]
pub(crate) use logout::resolve_logout_target;
#[cfg(test)]
pub(crate) use refresh::refresh_success_payload;
pub use refresh::{RefreshArgs, refresh_credentials};
pub use remove::handle_remove;
#[cfg(test)]
pub(crate) use remove::handle_remove_in_memory;
#[cfg(test)]
pub(crate) use status::peek_oauth_app_source_for_test;
#[cfg(test)]
pub(crate) use status::render_env_line;
pub use status::status;
pub use switch::handle_switch;
#[cfg(test)]
pub(crate) use switch::handle_switch_in_memory;

use anyhow::Result;

#[cfg(test)]
use crate::api::auth;
#[cfg(test)]
use crate::api::auth_embedded::OAuthAppSource;
#[cfg(test)]
use crate::config::Config;
use crate::error::JrError;

/// BC-1.1.016 Postcondition 2: fixed stderr string for the airtight
/// non-interactive OAuth guard — a literal constant, no value
/// interpolation. Cited verbatim by both `handle_login` and
/// `refresh_credentials`.
pub const NONINTERACTIVE_OAUTH_GUARD_MESSAGE: &str =
    "OAuth requires an interactive terminal; use --api-token for non-interactive auth.";

/// BC-1.1.016: precondition check for the airtight non-interactive OAuth
/// guard, shared by `handle_login` (Precondition 2a: explicit `--oauth`)
/// and `refresh_credentials` (Preconditions 2a/2b folded together, since
/// [`chosen_flow_for_profile`] already resolves the implicit
/// oauth-method-profile `refresh` case into `AuthFlow::OAuth`).
///
/// `no_input`: the caller's already-resolved non-interactive trigger state
/// (`--no-input` set, OR stdin is not a TTY).
/// `oauth_selected`: whether OAuth is the mechanism this invocation would
/// otherwise select — `handle_login` passes `args.oauth`; `refresh_credentials`
/// passes `flow == AuthFlow::OAuth`.
///
/// MUST be called as a PRECONDITION — before any network call, callback-
/// listener bind, or browser-open attempt — in both callers (Postcondition
/// 3). Never a timeout on, or best-effort cancellation of, an already-started
/// flow. Returns `Err(JrError::UserError(NONINTERACTIVE_OAUTH_GUARD_MESSAGE))`
/// (exit 64) when `no_input && oauth_selected`; `Ok(())` otherwise.
pub fn check_noninteractive_oauth_guard(no_input: bool, oauth_selected: bool) -> Result<()> {
    if no_input && oauth_selected {
        return Err(JrError::UserError(NONINTERACTIVE_OAUTH_GUARD_MESSAGE.to_string()).into());
    }
    Ok(())
}

/// BC-1.2.049 Postcondition 2: emit `--oauth`'s deprecation notice —
/// stderr-only, human-output-mode-only. Gated on OUTPUT FORMAT, not
/// TTY-ness (EC-1.2.049-1: never emitted under `--output json`, regardless
/// of interactivity).
pub fn emit_oauth_deprecation_notice(output: crate::cli::OutputFormat) {
    if matches!(output, crate::cli::OutputFormat::Table) {
        eprintln!(
            "--oauth is deprecated: the interactive login picker now defaults \
             to OAuth 2.0. Prefer running the picker (drop this flag), or pass \
             --api-token to select the other mechanism explicitly."
        );
    }
}

/// BC-1.2.050 Postcondition 3 (O-2/CV-2): emit `--api-token`'s
/// inert-on-`refresh` notice. Same output-channel rules as
/// [`emit_oauth_deprecation_notice`] (stderr-only, human-output-mode-only),
/// worded for inertness rather than deprecation — `--api-token` itself is
/// not deprecated, only its effect on `refresh` specifically is a no-op.
pub fn emit_api_token_inert_on_refresh_notice(output: crate::cli::OutputFormat) {
    if matches!(output, crate::cli::OutputFormat::Table) {
        eprintln!(
            "--api-token has no effect on 'auth refresh' — the profile's own \
             stored auth method is always used to refresh credentials."
        );
    }
}

/// Build the verb-aligned `--output json` success payload for the four auth
/// subcommands that mutate profile state (login, switch, logout, remove).
///
/// The shape `{"profile", "action", "ok": true}` is the canonical contract
/// documented in `docs/specs/json-output-shapes.md`. Kept separate from
/// `refresh_success_payload` because `auth refresh` is a re-authentication
/// trigger with its own richer payload — see json-output-shapes.md for
/// the rationale.
fn auth_json_response(profile: &str, action: &str) -> serde_json::Value {
    serde_json::json!({
        "profile": profile,
        "action": action,
        "ok": true,
    })
}

/// Which auth flow `jr auth refresh` should dispatch to.
///
/// `pub(crate)` so sibling shards (`refresh.rs`) and test helpers
/// (`refresh_success_payload`) can reference it without hitting the
/// `private-interfaces` lint. Not part of the public library API surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthFlow {
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

#[cfg(test)]
mod tests;
