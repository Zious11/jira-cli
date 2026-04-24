use thiserror::Error;

#[derive(Error, Debug)]
pub enum JrError {
    #[error("Not authenticated. Run \"jr auth login\" to connect.")]
    NotAuthenticated,

    #[error(
        "Insufficient token scope: {message}\n\n\
         The Atlassian API gateway rejects granular-scoped personal tokens on POST \
         requests (while PUT/GET succeed). Workarounds:\n  \
         • Use a classic token with \"write:jira-work\" scope instead of granular scopes, or\n  \
         • Try OAuth 2.0 (run \"jr auth login --oauth\") — may avoid this bug, not verified\n\n\
         See https://github.com/Zious11/jira-cli/issues/185 for details."
    )]
    InsufficientScope { message: String },

    #[error("Could not reach {0} — check your connection")]
    NetworkError(String),

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

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
            JrError::NotAuthenticated => 2,
            JrError::InsufficientScope { .. } => 2,
            JrError::ConfigError(_) => 78,
            JrError::UserError(_) => 64,
            JrError::Interrupted => 130,
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
                message: "Unauthorized; scope does not match".into()
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn insufficient_scope_display_includes_workarounds() {
        let err = JrError::InsufficientScope {
            message: "Unauthorized; scope does not match".into(),
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
}
