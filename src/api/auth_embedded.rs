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

#[allow(unused_imports)]
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/embedded_oauth.rs"));

/// Embedded OAuth app credentials. Plaintext after `decode()`; held in
/// process memory for the lifetime of the binary because `client_secret`
/// is needed for every refresh-token grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedOAuthApp {
    pub client_id: String,
    pub client_secret: String,
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
// Task 4 will consume this from `build_embedded_app()`. Until then only
// the unit test exercises it, so suppress the dead-code lint locally
// rather than hide the helper behind `#[cfg(test)]` (which would force
// us to delete and re-add it next commit).
#[allow(dead_code)]
fn decode(xored: &[u8], key: &[u8; 32]) -> Result<String, std::string::FromUtf8Error> {
    let bytes: Vec<u8> = xored
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % 32])
        .collect();
    String::from_utf8(bytes)
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
}
