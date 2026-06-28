use std::collections::HashMap;

use serde::Deserialize;

use super::error::DatabaseError;

/// Default location of file-based migrations, used when `migrations_path` is
/// not set in `database.toml`. Relative to the process working directory.
pub const DEFAULT_MIGRATIONS_PATH: &str = "./database/migrations";

/// Database driver kinds the framework recognizes.
///
/// MS SQL Server is intentionally not represented here: no Rust ORM / SQL
/// toolkit supports it natively. See `dev_docs/database/improvements.md`
/// (item 9) for the path forward if MS SQL is ever required.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DriverKind {
    Postgres,
    Mysql,
    Sqlite,
}

impl DriverKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DriverKind::Postgres => "postgres",
            DriverKind::Mysql => "mysql",
            DriverKind::Sqlite => "sqlite",
        }
    }
}

/// Per-connection pool tuning. All fields are optional and fall back to
/// SQLx defaults when missing.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PoolConfig {
    pub max: Option<u32>,
    pub min: Option<u32>,
    pub acquire_timeout_ms: Option<u64>,
    pub idle_timeout_ms: Option<u64>,
    pub max_lifetime_ms: Option<u64>,
}

/// One entry under `[connections.<name>]` in `config/database.toml`.
///
/// This is a pure data type: it describes *what* the user wrote in TOML,
/// nothing more. Translating these fields into SQLx connect options or
/// connection URLs lives in `driver.rs`, where driver-specific knowledge
/// (port defaults, URL composition, SQLx option builders) is concentrated.
#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionConfig {
    pub driver: DriverKind,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    /// Names of other connections to be used as read-replicas.
    /// Parsed in this iteration; runtime W/R routing is future work
    /// (see `dev_docs/database/improvements.md`, item 6).
    #[serde(default)]
    pub read: Vec<String>,
    #[serde(default)]
    pub pool: PoolConfig,
}

/// Whole `database` section of the merged configuration.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub auto_migrate: bool,
    #[serde(default)]
    pub migrations_path: Option<String>,
    #[serde(default)]
    pub connections: HashMap<String, ConnectionConfig>,
}

impl DatabaseConfig {
    /// Returns the configured `migrations_path` or the default location.
    pub fn migrations_path(&self) -> &str {
        self.migrations_path
            .as_deref()
            .unwrap_or(DEFAULT_MIGRATIONS_PATH)
    }

    /// Looks up a connection by name.
    pub fn connection(&self, name: &str) -> Result<&ConnectionConfig, DatabaseError> {
        self.connections
            .get(name)
            .ok_or_else(|| DatabaseError::UnknownConnection(name.to_string()))
    }

    /// Returns the configured default connection name or `NoDefault`.
    pub fn default_name(&self) -> Result<&str, DatabaseError> {
        self.default.as_deref().ok_or(DatabaseError::NoDefault)
    }
}
