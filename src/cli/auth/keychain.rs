use anyhow::{Context, Result};
use dialoguer::{Input, Password};

use crate::api::auth_embedded::{OAuthAppSource, embedded_oauth_app};
use crate::error::JrError;

/// Environment variable names for the four auth credentials.
///
/// Flag > env > prompt precedence is implemented by [`resolve_credential`].
/// Callers pass the matching `flag_name` so error messages can cite both
/// names verbatim.
pub(crate) const ENV_EMAIL: &str = "JR_EMAIL";
pub(crate) const ENV_API_TOKEN: &str = "JR_API_TOKEN";
pub(crate) const ENV_OAUTH_CLIENT_ID: &str = "JR_OAUTH_CLIENT_ID";
pub(crate) const ENV_OAUTH_CLIENT_SECRET: &str = "JR_OAUTH_CLIENT_SECRET";

/// Hint for OAuth client_id / client_secret errors so first-time agents
/// discover they must create an OAuth app before passing credentials.
pub(crate) const OAUTH_APP_HINT: &str =
    "Create one at https://developer.atlassian.com/console/myapps/.";

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
/// Flag and env are pair-gated: both halves must be present to use that
/// source. Providing only one of the two values is an explicit error
/// rather than a fall-through — the resolver returns `JrError::UserError`
/// citing both flag/env names so a user who forgot the second half learns
/// what's missing. Empty strings are treated as absent.
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

    // Detect any flag/env presence WITHOUT touching the keychain.
    // Using "any half present" (not "both halves") ensures we skip the
    // keychain probe when a partial flag/env pair will hard-error in
    // `_for_test` — a locked keychain must not mask the real error
    // (the user forgetting one flag, or providing only one env var).
    // This also means an explicit `--client-id`/`--client-secret` pair
    // wins even when the keychain backend is unavailable.
    let any_flag_present = flag_id.as_deref().is_some_and(|s| !s.is_empty())
        || flag_secret.as_deref().is_some_and(|s| !s.is_empty());
    let any_env_present = env_id.is_some() || env_secret.is_some();

    let keychain = if any_flag_present || any_env_present {
        // Flag or env will resolve OR hard-error first — never reaches
        // the keychain layer in `_for_test`. Skip the read entirely.
        None
    } else {
        // Single-pass keychain read; combines probe + load to avoid
        // double I/O and double keychain prompts on platforms that
        // prompt per access.
        crate::api::auth::try_load_oauth_app_credentials()?
    };

    // Defer the XOR decode: only materialize the embedded plaintext
    // `client_secret` when no higher-precedence source resolves.
    // BYO users (flag/env/keychain) never trigger the embedded decode —
    // including the partial-flag and partial-env cases that will hard-error
    // in `_for_test` before reaching the embedded layer. Gate on
    // `any_*_present` (not the pair-complete check) so a user passing
    // `--client-id` without `--client-secret` doesn't silently materialize
    // the embedded plaintext just to be told they forgot the second flag.
    let embedded = if any_flag_present || any_env_present || keychain.is_some() {
        None
    } else {
        embedded_oauth_app().map(|a| (a.client_id.clone(), a.client_secret.clone()))
    };

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
pub(crate) fn resolve_oauth_app_credentials_for_test(
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
