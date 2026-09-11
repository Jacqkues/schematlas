use serde::Serialize;
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Agent(String),
    #[error("The operation was declined or cancelled.")]
    Declined,
    #[error("{0}")]
    Validation(String),
    #[error("The project or source no longer exists.")]
    NotFound,
    #[error("Database operation failed. Check the address, credentials, TLS options, and catalog permissions.")]
    Database(#[from] sqlx::Error),
    #[error("SQL Server connection or catalog query failed. Check the address, credentials, TLS options, and permissions.")]
    Mssql(#[from] tiberius::error::Error),
    #[error("The local file could not be read or written. Check its location and permissions.")]
    Io(#[from] std::io::Error),
    #[error("Invalid JSON document: {0}")]
    Json(#[from] serde_json::Error),
    #[error("The database did not respond within 30 seconds.")]
    Timeout,
    #[error(
        "Reconnect this source to refresh it. Credentials are kept only for this app session."
    )]
    Reconnect,
}
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
pub type Result<T> = std::result::Result<T, AppError>;
