#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    #[error("no sinks configured in logging.toml — logging is disabled")]
    NoSinks,

    #[error("subscriber already initialized")]
    AlreadyInitialized,

    #[error("invalid file sink path '{path}': {source}")]
    FilePathError {
        path: String,
        source: std::io::Error,
    },

    #[error("database sink: connection '{connection}' not found in database config")]
    UnknownConnection { connection: String },

    #[error("unknown queue driver '{driver}'")]
    UnknownQueueDriver { driver: String },

    #[error(
        "database sink: invalid table name '{table}' (only [A-Za-z0-9_] allowed, must be non-empty)"
    )]
    InvalidTableName { table: String },
}
