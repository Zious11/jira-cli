use anyhow::{Context, Result};
use dialoguer::{Input, Password};

use crate::api::auth;
use crate::config::Config;
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

/// Decide which login flow to run based on config + explicit override.
///
/// Order of precedence:
/// 1. `oauth_override = true` → always OAuth (user passed `--oauth`).
/// 2. Config `auth_method == "oauth"` → OAuth.
/// 3. Anything else (including unset) → Token. Matches the `api_token`
///    default that `JiraClient::from_config` applies when no method is set.
fn chosen_flow(config: &Config, oauth_override: bool) -> AuthFlow {
    if oauth_override {
        return AuthFlow::OAuth;
    }
    match config.global.instance.auth_method.as_deref() {
        Some("oauth") => AuthFlow::OAuth,
        _ => AuthFlow::Token,
    }
}

/// Resolve email and API token (flag → env → prompt), then store in keychain.
pub async fn login_token(
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
    eprintln!("Credentials stored in keychain.");
    Ok(())
}

/// Run the OAuth 2.0 (3LO) login flow and persist site configuration.
///
/// Credentials resolved via flag → env → prompt, so CI/agent workflows can
/// pipe them in without a TTY.
pub async fn login_oauth(
    client_id: Option<String>,
    client_secret: Option<String>,
    no_input: bool,
) -> Result<()> {
    if !no_input {
        eprintln!("OAuth 2.0 requires your own Atlassian OAuth app.");
        eprintln!("Create one at: https://developer.atlassian.com/console/myapps/\n");
    }

    let client_id = resolve_credential(
        client_id,
        ENV_OAUTH_CLIENT_ID,
        "--client-id",
        "OAuth Client ID",
        false,
        no_input,
        Some(OAUTH_APP_HINT),
    )?;
    let client_secret = resolve_credential(
        client_secret,
        ENV_OAUTH_CLIENT_SECRET,
        "--client-secret",
        "OAuth Client Secret",
        true,
        no_input,
        Some(OAUTH_APP_HINT),
    )?;

    // Store OAuth app credentials in keychain
    crate::api::auth::store_oauth_app_credentials(&client_id, &client_secret)?;

    let result = crate::api::auth::oauth_login(&client_id, &client_secret).await?;

    let mut config = Config::load().unwrap_or_default();
    config.global.instance.url = Some(result.site_url);
    config.global.instance.cloud_id = Some(result.cloud_id);
    config.global.instance.auth_method = Some("oauth".into());
    config.save_global()?;

    output::print_success(&format!("Authenticated with {}", result.site_name));
    Ok(())
}

/// Show authentication status: instance URL, auth method, credential availability.
pub async fn status() -> Result<()> {
    let config = Config::load()?;

    let url = config
        .global
        .instance
        .url
        .as_deref()
        .unwrap_or("(not configured)");
    println!("Instance:    {url}");

    let method = config
        .global
        .instance
        .auth_method
        .as_deref()
        .unwrap_or("(not configured)");
    println!("Auth method: {method}");

    let creds_ok = auth::load_api_token().is_ok();
    if creds_ok {
        println!("Credentials: stored in keychain");
    } else {
        println!("Credentials: not found");
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
pub async fn refresh_credentials(
    oauth_override: bool,
    email: Option<String>,
    token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    no_input: bool,
    output: &crate::cli::OutputFormat,
) -> Result<()> {
    let config = Config::load()?;
    let flow = chosen_flow(&config, oauth_override);

    auth::clear_credentials().context(
        "failed to clear stored credentials before refresh — keychain may still hold stale entries",
    )?;

    let login_result = match flow {
        AuthFlow::Token => login_token(email, token, no_input).await,
        AuthFlow::OAuth => login_oauth(client_id, client_secret, no_input).await,
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

    match output {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, GlobalConfig, InstanceConfig};

    fn config_with_auth_method(method: Option<&str>) -> Config {
        Config {
            global: GlobalConfig {
                instance: InstanceConfig {
                    url: Some("https://example.atlassian.net".into()),
                    cloud_id: None,
                    org_id: None,
                    auth_method: method.map(str::to_string),
                },
                ..Default::default()
            },
            project: Default::default(),
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
    // Env-reading tests use per-test env var names to avoid races with
    // parallel test threads. `EnvGuard` removes the var on drop so a panic
    // mid-test doesn't leak state to later tests in the same process.

    struct EnvGuard(&'static str);

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            // SAFETY: test-local keys (all prefixed `_JR_TEST_`), never read
            // by production code. The Drop impl unsets the same key.
            unsafe {
                std::env::set_var(key, value);
            }
            EnvGuard(key)
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: matches the test-local key set in `EnvGuard::set`.
            unsafe {
                std::env::remove_var(self.0);
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
}
