use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database connection '{0}' is not defined in config")]
    UnknownConnection(String),

    #[error("database connection '{name}' is missing required config field '{field}'")]
    MissingField { name: String, field: String },

    #[error("database connection '{name}' uses unsupported driver '{driver}'")]
    UnsupportedDriver { name: String, driver: String },

    #[error(
        "driver mismatch for connection '{name}': expected {expected}, actual driver is {actual}"
    )]
    DriverMismatch {
        name: String,
        expected: &'static str,
        actual: &'static str,
    },

    #[error("database default connection is not configured (set `database.default` in config)")]
    NoDefault,

    #[error(
        "Database is not initialized; call Database::install_global() in bootstrap::app::run()"
    )]
    NotInitialized,

    #[error("Database global singleton was already installed")]
    AlreadyInstalled,

    #[error("failed to parse database connection url for '{name}': {source}")]
    InvalidUrl {
        name: String,
        #[source]
        source: sqlx::Error,
    },

    #[error(transparent)]
    Pool(#[from] sqlx::Error),

    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
}
