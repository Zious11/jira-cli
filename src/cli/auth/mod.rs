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
pub(crate) use list::{render_list_json, render_list_table};
pub use login::{LoginArgs, handle_login, login_oauth, login_token};
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
pub use status::status;
pub use switch::handle_switch;
#[cfg(test)]
pub(crate) use switch::handle_switch_in_memory;

#[cfg(test)]
use crate::api::auth;
#[cfg(test)]
use crate::api::auth_embedded::OAuthAppSource;
#[cfg(test)]
use crate::config::Config;
#[cfg(test)]
use crate::error::JrError;

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
