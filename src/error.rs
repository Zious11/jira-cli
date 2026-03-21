use thiserror::Error;

#[derive(Error, Debug)]
// TODO: remove #[allow(dead_code)] once subcommands are implemented
#[allow(dead_code)]
pub enum JrError {
    #[error("Not authenticated. Run \"jr auth login\" to connect.")]
    NotAuthenticated,

    #[error("Could not reach {0} — check your connection")]
    NetworkError(String),

    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("Configuration error: {0}")]
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
