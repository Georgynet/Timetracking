use serde::Serialize;

/// All Tauri commands return `Result<T, AppError>`. `AppError` serializes to a plain
/// string for the frontend (via `Serialize`), and its `Display` impl is guaranteed to
/// never contain secrets (see module docs on `jira::models::JiraError`).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("{0}")]
    Jira(#[from] crate::jira::models::JiraError),

    #[error("secret storage error: {0}")]
    Secret(#[from] crate::secrets::keyring_store::SecretError),

    #[error("{0}")]
    Timer(#[from] crate::timer::engine::TimerError),

    #[error("{0}")]
    Workday(#[from] crate::workday::engine::WorkdayError),

    #[error("{0}")]
    Stats(#[from] crate::stats::engine::StatsError),

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    NotFound(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
