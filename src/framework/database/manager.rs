use std::collections::HashMap;
use std::sync::{Once, OnceLock};
use std::time::Duration;

use sqlx::AnyPool;
use sqlx::any::AnyPoolOptions;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::RwLock;

use super::config::{ConnectionConfig, DatabaseConfig, DriverKind, PoolConfig};
use super::driver::{any_url, mysql_options, pg_options, sqlite_options};
use super::error::DatabaseError;
use crate::framework::config::Config;

/// A live database connection pool, tagged with its driver.
///
/// Cloning is cheap — `sqlx::Pool` is internally `Arc`-based.
#[derive(Debug, Clone)]
pub enum Connection {
    Postgres(sqlx::Pool<sqlx::Postgres>),
    MySql(sqlx::Pool<sqlx::MySql>),
    Sqlite(sqlx::Pool<sqlx::Sqlite>),
}

impl Connection {
    pub fn driver(&self) -> &'static str {
        match self {
            Connection::Postgres(_) => "postgres",
            Connection::MySql(_) => "mysql",
            Connection::Sqlite(_) => "sqlite",
        }
    }

    /// `SELECT 1` against the underlying pool. Useful for health checks
    /// and as a smoke test.
    pub async fn ping(&self) -> Result<(), sqlx::Error> {
        match self {
            Connection::Postgres(p) => sqlx::query("SELECT 1").execute(p).await.map(|_| ()),
            Connection::MySql(p) => sqlx::query("SELECT 1").execute(p).await.map(|_| ()),
            Connection::Sqlite(p) => sqlx::query("SELECT 1").execute(p).await.map(|_| ()),
        }
    }
}

static GLOBAL: OnceLock<Database> = OnceLock::new();
static ANY_DRIVERS_INSTALLED: Once = Once::new();

/// Holds named database pools and resolves them lazily on first access.
///
/// Two parallel caches:
/// * `typed` — for `Database::connection / postgres / mysql / sqlite` —
///   keeps strongly-typed `sqlx::Pool<T>` so `query!` / `query_as!` work.
/// * `any` — for `Database::any` — keeps `sqlx::AnyPool` for code that
///   doesn't care about the underlying driver.
///
/// Each name can have entries in both caches (different pool objects, same
/// configured DSN). This is fine: a pool is just a holder for connections,
/// nothing forces a single one per DSN.
pub struct Database {
    config: DatabaseConfig,
    typed: RwLock<HashMap<String, Connection>>,
    any: RwLock<HashMap<String, AnyPool>>,
}

impl Database {
    /// Builds an empty manager from an explicit `DatabaseConfig`. No pools
    /// are opened. Use this directly in tests; production code should use
    /// `install_global()`.
    pub fn new(config: DatabaseConfig) -> Self {
        Self {
            config,
            typed: RwLock::new(HashMap::new()),
            any: RwLock::new(HashMap::new()),
        }
    }

    /// Reads the `database` section from the global `Config` and installs
    /// the manager as a process-wide singleton. Idempotent only in the
    /// sense that subsequent calls return `AlreadyInstalled`.
    pub fn install_global() -> Result<&'static Database, DatabaseError> {
        let cfg: DatabaseConfig = Config::try_get("database").unwrap_or_default();
        Self::install_with(cfg)
    }

    /// Installs an explicit `DatabaseConfig` as a process-wide singleton.
    /// Mostly useful for tests that bypass file-based `Config`.
    pub fn install_with(config: DatabaseConfig) -> Result<&'static Database, DatabaseError> {
        GLOBAL
            .set(Self::new(config))
            .map_err(|_| DatabaseError::AlreadyInstalled)?;
        Ok(Self::global())
    }

    /// Returns the process-wide manager. Panics if `install_global()` was
    /// not called yet.
    pub fn global() -> &'static Self {
        GLOBAL.get().expect(
            "Database is not initialized; call Database::install_global() in bootstrap::app::run()",
        )
    }

    pub fn try_global() -> Option<&'static Self> {
        GLOBAL.get()
    }

    pub fn config(&self) -> &DatabaseConfig {
        &self.config
    }

    // === Instance API (used in tests and via the static facade) ===

    /// Returns (and lazily opens) the typed connection for `name`.
    pub async fn connection_for(&self, name: &str) -> Result<Connection, DatabaseError> {
        {
            let typed = self.typed.read().await;
            if let Some(c) = typed.get(name) {
                return Ok(c.clone());
            }
        }

        let cfg = self.config.connection(name)?;
        let new_conn = build_typed(name, cfg)?;

        let mut typed = self.typed.write().await;
        Ok(typed.entry(name.to_string()).or_insert(new_conn).clone())
    }

    /// Returns (and lazily opens) the `AnyPool` for `name`.
    pub async fn any_pool_for(&self, name: &str) -> Result<AnyPool, DatabaseError> {
        {
            let any = self.any.read().await;
            if let Some(p) = any.get(name) {
                return Ok(p.clone());
            }
        }

        let cfg = self.config.connection(name)?;
        let url = any_url(name, cfg)?;
        install_any_drivers_once();
        let new_pool = configure_any_options(&cfg.pool)
            .connect_lazy(&url)
            .map_err(|source| DatabaseError::InvalidUrl {
                name: name.to_string(),
                source,
            })?;

        let mut any = self.any.write().await;
        Ok(any.entry(name.to_string()).or_insert(new_pool).clone())
    }

    /// Resolves the configured default connection name and opens its pool.
    pub async fn default_connection(&self) -> Result<Connection, DatabaseError> {
        let name = self.config.default_name()?.to_string();
        self.connection_for(&name).await
    }

    pub async fn postgres_pool_for(
        &self,
        name: &str,
    ) -> Result<sqlx::Pool<sqlx::Postgres>, DatabaseError> {
        match self.connection_for(name).await? {
            Connection::Postgres(p) => Ok(p),
            other => Err(DatabaseError::DriverMismatch {
                name: name.to_string(),
                expected: "postgres",
                actual: other.driver(),
            }),
        }
    }

    pub async fn mysql_pool_for(
        &self,
        name: &str,
    ) -> Result<sqlx::Pool<sqlx::MySql>, DatabaseError> {
        match self.connection_for(name).await? {
            Connection::MySql(p) => Ok(p),
            other => Err(DatabaseError::DriverMismatch {
                name: name.to_string(),
                expected: "mysql",
                actual: other.driver(),
            }),
        }
    }

    pub async fn sqlite_pool_for(
        &self,
        name: &str,
    ) -> Result<sqlx::Pool<sqlx::Sqlite>, DatabaseError> {
        match self.connection_for(name).await? {
            Connection::Sqlite(p) => Ok(p),
            other => Err(DatabaseError::DriverMismatch {
                name: name.to_string(),
                expected: "sqlite",
                actual: other.driver(),
            }),
        }
    }

    // === Static facade — primary entry point for application code ===

    /// Shortcut for `Database::global().connection_for(name).await`.
    pub async fn connection(name: &str) -> Result<Connection, DatabaseError> {
        Self::global().connection_for(name).await
    }

    /// Shortcut for `Database::global().default_connection().await`.
    pub async fn default() -> Result<Connection, DatabaseError> {
        Self::global().default_connection().await
    }

    /// Driver-agnostic pool (URL-scheme picks the driver at runtime).
    /// Useful when the calling code doesn't need `query!` / `query_as!`.
    pub async fn any(name: &str) -> Result<AnyPool, DatabaseError> {
        Self::global().any_pool_for(name).await
    }

    pub async fn postgres(name: &str) -> Result<sqlx::Pool<sqlx::Postgres>, DatabaseError> {
        Self::global().postgres_pool_for(name).await
    }

    pub async fn mysql(name: &str) -> Result<sqlx::Pool<sqlx::MySql>, DatabaseError> {
        Self::global().mysql_pool_for(name).await
    }

    pub async fn sqlite(name: &str) -> Result<sqlx::Pool<sqlx::Sqlite>, DatabaseError> {
        Self::global().sqlite_pool_for(name).await
    }
}

fn install_any_drivers_once() {
    ANY_DRIVERS_INSTALLED.call_once(|| {
        sqlx::any::install_default_drivers();
    });
}

fn build_typed(name: &str, cfg: &ConnectionConfig) -> Result<Connection, DatabaseError> {
    match cfg.driver {
        DriverKind::Postgres => {
            let opts = pg_options(name, cfg)?;
            let pool = configure_pg_options(&cfg.pool).connect_lazy_with(opts);
            Ok(Connection::Postgres(pool))
        }
        DriverKind::Mysql => {
            let opts = mysql_options(name, cfg)?;
            let pool = configure_mysql_options(&cfg.pool).connect_lazy_with(opts);
            Ok(Connection::MySql(pool))
        }
        DriverKind::Sqlite => {
            let opts = sqlite_options(name, cfg)?;
            let pool = configure_sqlite_options(&cfg.pool).connect_lazy_with(opts);
            Ok(Connection::Sqlite(pool))
        }
    }
}

fn configure_pg_options(p: &PoolConfig) -> PgPoolOptions {
    let mut o = PgPoolOptions::new();
    if let Some(v) = p.max {
        o = o.max_connections(v);
    }
    if let Some(v) = p.min {
        o = o.min_connections(v);
    }
    if let Some(ms) = p.acquire_timeout_ms {
        o = o.acquire_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.idle_timeout_ms {
        o = o.idle_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.max_lifetime_ms {
        o = o.max_lifetime(Duration::from_millis(ms));
    }
    o
}

fn configure_mysql_options(p: &PoolConfig) -> MySqlPoolOptions {
    let mut o = MySqlPoolOptions::new();
    if let Some(v) = p.max {
        o = o.max_connections(v);
    }
    if let Some(v) = p.min {
        o = o.min_connections(v);
    }
    if let Some(ms) = p.acquire_timeout_ms {
        o = o.acquire_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.idle_timeout_ms {
        o = o.idle_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.max_lifetime_ms {
        o = o.max_lifetime(Duration::from_millis(ms));
    }
    o
}

fn configure_sqlite_options(p: &PoolConfig) -> SqlitePoolOptions {
    let mut o = SqlitePoolOptions::new();
    if let Some(v) = p.max {
        o = o.max_connections(v);
    }
    if let Some(v) = p.min {
        o = o.min_connections(v);
    }
    if let Some(ms) = p.acquire_timeout_ms {
        o = o.acquire_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.idle_timeout_ms {
        o = o.idle_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.max_lifetime_ms {
        o = o.max_lifetime(Duration::from_millis(ms));
    }
    o
}

fn configure_any_options(p: &PoolConfig) -> AnyPoolOptions {
    let mut o = AnyPoolOptions::new();
    if let Some(v) = p.max {
        o = o.max_connections(v);
    }
    if let Some(v) = p.min {
        o = o.min_connections(v);
    }
    if let Some(ms) = p.acquire_timeout_ms {
        o = o.acquire_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.idle_timeout_ms {
        o = o.idle_timeout(Duration::from_millis(ms));
    }
    if let Some(ms) = p.max_lifetime_ms {
        o = o.max_lifetime(Duration::from_millis(ms));
    }
    o
}
