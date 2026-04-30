//! Embedded `jr` Atlassian OAuth app credentials.
//!
//! `build.rs` writes `$OUT_DIR/embedded_oauth.rs` with three constants
//! (`EMBEDDED_ID`, `EMBEDDED_SECRET_XOR`, `EMBEDDED_SECRET_KEY`). When the
//! build env vars are set, the secret is XOR-obfuscated against a per-build
//! random key. This module decodes them on demand and exposes a single
//! [`embedded_oauth_app`] accessor.
//!
//! Obfuscation defeats automated secret scanners. Motivated reverse engineers
//! can still extract the plaintext from a debugger; the operational mitigation
//! is `client_secret` rotation in Atlassian Developer Console (see ADR-0006).

use std::sync::OnceLock;

// Pulls in EMBEDDED_ID, EMBEDDED_SECRET_XOR, EMBEDDED_SECRET_KEY constants
// emitted by build.rs.
include!(concat!(env!("OUT_DIR"), "/embedded_oauth.rs"));

/// Embedded OAuth app credentials. Plaintext after `decode()`; held in
/// process memory for the lifetime of the binary because `client_secret`
/// is needed for every refresh-token grant.
#[derive(Clone, PartialEq, Eq)]
pub struct EmbeddedOAuthApp {
    pub client_id: String,
    pub client_secret: String,
}

/// Manual Debug that redacts `client_secret`. Defense in depth: release
/// builds bake in real Atlassian credentials, so a stray `dbg!` or
/// `tracing::debug!("{app:?}")` must not leak the live secret to logs.
/// `client_id` is treated as non-secret and rendered verbatim (it
/// identifies the OAuth app to Atlassian's authorize endpoint, not the
/// user's session).
impl std::fmt::Debug for EmbeddedOAuthApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedOAuthApp")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .finish()
    }
}

/// Source of the OAuth app credentials used for a login or refresh. Reported
/// by `jr auth status` so users (and triagers) can tell which credentials
/// drove the live session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthAppSource {
    Flag,
    Env,
    Keychain,
    Embedded,
    Prompt,
    /// Sentinel for "no source resolved" — used only by the status display
    /// surface (`peek_oauth_app_source`) so the rendered row can read
    /// `(none)` without forcing every callsite to an `Option<OAuthAppSource>`.
    None,
}

impl OAuthAppSource {
    pub fn label(self) -> &'static str {
        match self {
            OAuthAppSource::Flag => "flag",
            OAuthAppSource::Env => "env",
            OAuthAppSource::Keychain => "keychain",
            OAuthAppSource::Embedded => "embedded",
            OAuthAppSource::Prompt => "prompt",
            OAuthAppSource::None => "(none)",
        }
    }
}

/// Decode an XOR-obfuscated secret using the per-build key. Pure function;
/// callers must supply both halves so tests can exercise it without
/// touching `OUT_DIR`.
fn decode(xored: &[u8], key: &[u8; 32]) -> Result<String, std::string::FromUtf8Error> {
    let bytes: Vec<u8> = xored
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % 32])
        .collect();
    String::from_utf8(bytes)
}

/// Construct an [`EmbeddedOAuthApp`] from raw build-emitted constants. Pure
/// function so tests can exercise both the present and absent paths without
/// rebuilding with different env vars.
fn build_embedded_app(
    id: Option<&str>,
    xor: Option<&[u8]>,
    key: Option<&[u8; 32]>,
) -> Option<EmbeddedOAuthApp> {
    let (id, xor, key) = match (id, xor, key) {
        (Some(i), Some(x), Some(k)) => (i, x, k),
        _ => return None,
    };
    // Reject empty inputs: a build pipeline that sets the env vars to ""
    // would otherwise ship a binary that posts an empty client_id to
    // Atlassian. None here means the binary falls through to BYO/prompt
    // exactly as it would for a fork build.
    if id.is_empty() || xor.is_empty() {
        return None;
    }
    let secret = decode(xor, key).ok()?;
    if secret.is_empty() {
        return None;
    }
    Some(EmbeddedOAuthApp {
        client_id: id.to_string(),
        client_secret: secret,
    })
}

/// Lazily-initialized cached embedded app. `OnceLock` ensures the XOR
/// decode happens at most once per process; the plaintext is then held
/// for the process lifetime (needed for token refreshes).
pub fn embedded_oauth_app() -> Option<&'static EmbeddedOAuthApp> {
    static APP: OnceLock<Option<EmbeddedOAuthApp>> = OnceLock::new();
    APP.get_or_init(|| build_embedded_app(EMBEDDED_ID, EMBEDDED_SECRET_XOR, EMBEDDED_SECRET_KEY))
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_round_trip_known_plaintext() {
        let plaintext = "hello-world-secret";
        let key = [42u8; 32];
        let xored: Vec<u8> = plaintext
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 32])
            .collect();

        let decoded = decode(&xored, &key).expect("valid utf-8");
        assert_eq!(decoded, plaintext);
    }

    #[test]
    fn build_embedded_app_none_when_constants_unset() {
        let app = build_embedded_app(None, None, None);
        assert_eq!(app, None);
    }

    #[test]
    fn build_embedded_app_returns_decoded_when_all_set() {
        let plaintext = "secret-xyz";
        let key = [7u8; 32];
        let xored: Vec<u8> = plaintext
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % 32])
            .collect();

        let app = build_embedded_app(Some("client-abc"), Some(&xored), Some(&key))
            .expect("all three constants present → Some");
        assert_eq!(app.client_id, "client-abc");
        assert_eq!(app.client_secret, plaintext);
    }

    #[test]
    fn build_embedded_app_none_when_any_constant_missing() {
        let key = [0u8; 32];
        // id missing
        assert_eq!(build_embedded_app(None, Some(b"x"), Some(&key)), None);
        // secret_xor missing
        assert_eq!(build_embedded_app(Some("id"), None, Some(&key)), None);
        // key missing
        assert_eq!(build_embedded_app(Some("id"), Some(b"x"), None), None);
    }

    /// Empty-string id or zero-length ciphertext must produce None — a
    /// build-pipeline misconfig that emits empty values should not ship a
    /// binary that posts empty credentials to Atlassian.
    #[test]
    fn build_embedded_app_rejects_empty_inputs() {
        let key = [0u8; 32];
        // empty id
        assert_eq!(build_embedded_app(Some(""), Some(b"x"), Some(&key)), None);
        // empty ciphertext (would decode to empty secret)
        assert_eq!(build_embedded_app(Some("id"), Some(&[]), Some(&key)), None);
    }

    /// Default test runs (no JR_BUILD_OAUTH_CLIENT_* env vars at compile time)
    /// must produce a binary where `embedded_oauth_app()` returns None. This
    /// is the fork / local-build path. Branded builds get a separate
    /// integration-test rig that sets the env vars.
    #[test]
    fn embedded_oauth_app_is_none_in_default_test_build() {
        // If this assertion ever fails in CI, the release env var is leaking
        // into test runs. Fix the workflow, not the test.
        assert!(
            embedded_oauth_app().is_none(),
            "test builds must not have embedded credentials"
        );
    }

    /// Defense in depth: `client_secret` must never appear in `Debug` output.
    /// Release builds carry a real Atlassian secret; a stray `dbg!` or
    /// log-call would otherwise leak it.
    #[test]
    fn embedded_oauth_app_debug_redacts_secret() {
        let app = EmbeddedOAuthApp {
            client_id: "visible-id".to_string(),
            client_secret: "super-secret-must-not-leak".to_string(),
        };
        let rendered = format!("{app:?}");
        assert!(
            rendered.contains("visible-id"),
            "client_id should be visible: {rendered}"
        );
        assert!(
            !rendered.contains("super-secret-must-not-leak"),
            "client_secret must not appear in Debug: {rendered}"
        );
        assert!(
            rendered.contains("<redacted>"),
            "redaction marker should be present: {rendered}"
        );
    }
}
