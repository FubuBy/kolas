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
    /// Migrations directory for this connection. When set it overrides the
    /// global `[database] migrations_path`, letting each database (e.g. a
    /// Postgres primary and a MySQL analytics store) own a separate set of
    /// migration files with independent versioning.
    #[serde(default)]
    pub migrations_path: Option<String>,
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
    /// Returns the global `migrations_path` or the default location.
    pub fn migrations_path(&self) -> &str {
        self.migrations_path
            .as_deref()
            .unwrap_or(DEFAULT_MIGRATIONS_PATH)
    }

    /// Resolves the migrations directory for a single connection.
    ///
    /// Precedence: the connection's own `migrations_path`, then the global
    /// `[database] migrations_path`, then the built-in default. Connections
    /// that don't set their own path share the global directory (the
    /// single-database default); multi-database setups give each connection
    /// its own `migrations_path`.
    pub fn migrations_path_for(&self, name: &str) -> String {
        self.connections
            .get(name)
            .and_then(|c| c.migrations_path.as_deref())
            .unwrap_or_else(|| self.migrations_path())
            .to_string()
    }

    /// Resolves which connection a migration command targets: the explicitly
    /// requested one, or the configured default when none is given.
    pub fn resolve_connection(&self, requested: Option<&str>) -> Result<String, DatabaseError> {
        match requested {
            Some(name) => Ok(name.to_string()),
            None => Ok(self.default_name()?.to_string()),
        }
    }

    /// Connections to migrate when `auto_migrate` is enabled: the default
    /// connection (if configured) plus every connection that declares its own
    /// `migrations_path`. Returns `(connection_name, migrations_path)` pairs,
    /// de-duplicated by connection name and ordered deterministically (default
    /// first, then the rest alphabetically).
    pub fn auto_migrate_targets(&self) -> Vec<(String, String)> {
        let mut names: Vec<String> = Vec::new();
        if let Ok(default) = self.default_name() {
            names.push(default.to_string());
        }

        let mut extra: Vec<&String> = self
            .connections
            .iter()
            .filter(|(_, c)| c.migrations_path.is_some())
            .map(|(name, _)| name)
            .collect();
        extra.sort();
        for name in extra {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }

        names
            .into_iter()
            .map(|name| {
                let path = self.migrations_path_for(&name);
                (name, path)
            })
            .collect()
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
