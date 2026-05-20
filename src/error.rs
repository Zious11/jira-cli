use thiserror::Error;

/// Hint for Basic-auth (API-token) users who receive a 401 on a JSM path.
///
/// Used by `handle_jsm_create` (BC-3.8.014) and `require_service_desk`
/// (BC-X.8.006) when `client.is_oauth_auth() == false`. Single source of
/// truth — both sites reference this constant; never duplicate the string.
///
/// Verbatim from F2 PRD delta CANONICAL block (adversary-pass-4 F-04).
/// Must NOT contain OAuth-scope language, `write:servicedesk-request`,
/// or `jr auth refresh`.
pub(crate) const API_TOKEN_EXPIRY_HINT: &str = "Your API token may be expired or revoked. Regenerate it at\n\
https://id.atlassian.com/manage-profile/security/api-tokens\n\
then run `jr auth login` to re-store the credentials.";

#[derive(Error, Debug)]
pub enum JrError {
    #[error("Not authenticated. {hint}")]
    NotAuthenticated { hint: String },

    #[error(
        "Insufficient token scope: {message}\n\n\
         The Atlassian API gateway rejects granular-scoped personal tokens on POST \
         requests (while PUT/GET succeed). Workarounds:\n  \
         • Use a classic token with \"{scope_hint}\" scope instead of granular scopes, or\n  \
         • Try OAuth 2.0 (run \"jr auth login --oauth\") — may avoid this bug, not verified\n\n\
         See https://github.com/Zious11/jira-cli/issues/185 for details.",
        scope_hint = required_scope.as_deref().filter(|s| !s.is_empty()).unwrap_or("write:jira-work")
    )]
    InsufficientScope {
        message: String,
        required_scope: Option<String>,
    },

    #[error("Could not reach {0} — check your connection")]
    NetworkError(String),

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    /// Caller-supplied deadline exceeded. Distinct from `ApiError(status=429)`
    /// because a deadline-exceeded error is NOT an Atlassian-server response —
    /// it is a client-side timeout signal. Mixing it with `ApiError(429)` was
    /// misleading: scripts grepping for "429" to detect rate-limit pressure
    /// would false-positive on deadline-exceeded, and the entry-point check
    /// produces this error without any HTTP request being issued at all.
    ///
    /// Exit code 124 (POSIX `timeout(1)` convention). External CLI precedent:
    /// kubectl, gh, aws-cli, doctl, fly — all use a dedicated variant for
    /// client-side deadlines rather than overloading a 4xx HTTP code.
    ///
    /// `remaining_ms` is the budget remaining when the error was produced
    /// (always 0ms at the threshold by construction; included for parity with
    /// the in-loop error message format).
    ///
    /// Closes audit-followup #333 (introduced in commit fix(bulk): clamp
    /// 429-retry sleep by caller's deadline).
    #[error("Deadline exceeded: {message}")]
    DeadlineExceeded { remaining_ms: u64, message: String },

    #[error("{0}")]
    ConfigError(String),

    #[error("{0}")]
    UserError(String),

    /// Invariant violation / "should never happen" bug. Prefix the message with
    /// "Internal error:" at call sites so the formatted output self-describes
    /// as a bug. Exit code 1 (default), distinguished from UserError (64) and
    /// ConfigError (78) so callers matching on `JrError` can tell "we have a
    /// bug" apart from "user did something wrong".
    #[error("{0}")]
    Internal(String),

    #[error("Interrupted")]
    Interrupted,

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl JrError {
    pub fn exit_code(&self) -> i32 {
        match self {
            JrError::NotAuthenticated { .. } => 2,
            JrError::InsufficientScope { .. } => 2,
            JrError::ConfigError(_) => 78,
            JrError::UserError(_) => 64,
            JrError::Interrupted => 130,
            JrError::DeadlineExceeded { .. } => 124,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_error_exit_code() {
        assert_eq!(JrError::ConfigError("test".into()).exit_code(), 78);
    }

    #[test]
    fn user_error_exit_code() {
        assert_eq!(JrError::UserError("test".into()).exit_code(), 64);
    }

    #[test]
    fn internal_error_exit_code_is_one() {
        assert_eq!(JrError::Internal("bug".into()).exit_code(), 1);
    }

    #[test]
    fn internal_error_display_passthrough() {
        assert_eq!(
            JrError::Internal("Internal error: x not found".into()).to_string(),
            "Internal error: x not found"
        );
    }

    #[test]
    fn config_error_display_no_prefix() {
        assert_eq!(
            JrError::ConfigError("No board_id configured.".into()).to_string(),
            "No board_id configured."
        );
    }

    #[test]
    fn user_error_display_passthrough() {
        assert_eq!(
            JrError::UserError("Invalid selection".into()).to_string(),
            "Invalid selection"
        );
    }

    #[test]
    fn insufficient_scope_exit_code() {
        assert_eq!(
            JrError::InsufficientScope {
                message: "Unauthorized; scope does not match".into(),
                required_scope: None,
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn deadline_exceeded_exit_code_is_124() {
        // POSIX timeout(1) convention. Distinct from rate-limit (1) so scripts
        // can react differently to "your deadline ran out" vs "server is
        // rate-limiting you". Closes #333 C-2.
        assert_eq!(
            JrError::DeadlineExceeded {
                remaining_ms: 0,
                message: "test".into()
            }
            .exit_code(),
            124
        );
    }

    #[test]
    fn deadline_exceeded_display_format() {
        // Display uses the "Deadline exceeded:" prefix so operators grepping
        // stderr for the word "deadline" find both this variant's text and
        // the in-loop message string (which also contains the word).
        assert_eq!(
            JrError::DeadlineExceeded {
                remaining_ms: 0,
                message: "Caller-supplied deadline already expired at send entry".into()
            }
            .to_string(),
            "Deadline exceeded: Caller-supplied deadline already expired at send entry"
        );
    }

    #[test]
    fn insufficient_scope_display_includes_workarounds() {
        let err = JrError::InsufficientScope {
            message: "Unauthorized; scope does not match".into(),
            required_scope: None,
        };
        let s = err.to_string();
        assert!(s.contains("Insufficient token scope"), "got: {s}");
        assert!(
            s.contains("Unauthorized; scope does not match"),
            "raw message should be included: {s}"
        );
        assert!(s.contains("write:jira-work"), "workaround missing: {s}");
        assert!(s.contains("OAuth 2.0"), "workaround missing: {s}");
        assert!(
            s.contains("github.com/Zious11/jira-cli/issues/185"),
            "issue link missing: {s}"
        );
    }

    #[test]
    fn test_insufficient_scope_display_uses_required_scope_when_some() {
        let err = JrError::InsufficientScope {
            message: "Unauthorized; scope does not match".into(),
            required_scope: Some("write:servicedesk-request".into()),
        };
        let s = format!("{err}");
        assert!(
            s.contains("write:servicedesk-request"),
            "Display should contain the call-site-supplied scope name; got: {s}"
        );
        assert!(
            !s.contains("write:jira-work"),
            "Display should NOT contain the fallback scope when required_scope is Some; got: {s}"
        );
    }

    #[test]
    fn test_insufficient_scope_display_empty_some_falls_back() {
        let err = JrError::InsufficientScope {
            message: "Unauthorized; scope does not match".into(),
            required_scope: Some(String::new()),
        };
        let s = format!("{err}");
        assert!(
            s.contains("write:jira-work"),
            "Display should fall back to write:jira-work when required_scope is Some(empty); got: {s}"
        );
    }

    /// AC-2 (BC-3.8.014 postcondition 1 / BC-X.8.006 postcondition 1):
    /// `API_TOKEN_EXPIRY_HINT` must contain expiry/revocation language, the
    /// api-tokens management URL, and `jr auth login`; must NOT contain any
    /// OAuth scope name (no `write:servicedesk-request`, no `jr auth refresh`).
    #[test]
    fn test_api_token_expiry_hint_contains_required_text() {
        assert!(
            API_TOKEN_EXPIRY_HINT.contains("expired or revoked"),
            "API_TOKEN_EXPIRY_HINT must contain 'expired or revoked'; got: {API_TOKEN_EXPIRY_HINT:?}"
        );
        assert!(
            API_TOKEN_EXPIRY_HINT.contains("id.atlassian.com/manage-profile/security/api-tokens"),
            "API_TOKEN_EXPIRY_HINT must contain the api-tokens management URL; got: {API_TOKEN_EXPIRY_HINT:?}"
        );
        assert!(
            API_TOKEN_EXPIRY_HINT.contains("jr auth login"),
            "API_TOKEN_EXPIRY_HINT must contain 'jr auth login'; got: {API_TOKEN_EXPIRY_HINT:?}"
        );
    }

    /// AC-2 negative assertion: `API_TOKEN_EXPIRY_HINT` must NOT contain OAuth
    /// scope language — it is the Basic-auth hint, not an OAuth hint.
    #[test]
    fn test_api_token_expiry_hint_excludes_oauth_scope_language() {
        assert!(
            !API_TOKEN_EXPIRY_HINT.contains("write:servicedesk-request"),
            "API_TOKEN_EXPIRY_HINT must NOT contain OAuth scope 'write:servicedesk-request'; got: {API_TOKEN_EXPIRY_HINT:?}"
        );
        assert!(
            !API_TOKEN_EXPIRY_HINT.contains("jr auth refresh"),
            "API_TOKEN_EXPIRY_HINT must NOT contain 'jr auth refresh'; got: {API_TOKEN_EXPIRY_HINT:?}"
        );
    }
}
