use thiserror::Error;

#[derive(Error, Debug)]
pub enum JrError {
    #[error("Not authenticated. Run \"jr auth login\" to connect.")]
    NotAuthenticated,

    #[error("Could not reach {0} — check your connection")]
    NetworkError(String),

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("{0}")]
    ConfigError(String),

    #[error("{0}")]
    UserError(String),

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
}
