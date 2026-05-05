use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config directory '{path}': {source}")]
    ReadDir {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read config file '{path}': {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse config '{file}': {source}")]
    Parse {
        file: String,
        #[source]
        source: toml::de::Error,
    },

    #[error("config file '{0}' must contain a top-level table")]
    NotTable(String),
}
