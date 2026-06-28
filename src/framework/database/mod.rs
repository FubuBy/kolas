//! Relational-database layer built on top of SQLx 0.8.
//!
//! Provides a `Database` manager with named connections (declared in
//! `config/database.toml`), lazy pool initialization, and both a
//! driver-agnostic `AnyPool` path and typed `Pool<Postgres|MySql|Sqlite>`
//! accessors for compile-time-checked queries.
//!
//! See `dev_docs/architecture/database.md` for design rationale and
//! `dev_docs/database/improvements.md` for the backlog.

mod config;
mod driver;
mod error;
mod manager;
mod migrate;

pub use config::{
    ConnectionConfig, DEFAULT_MIGRATIONS_PATH, DatabaseConfig, DriverKind, PoolConfig,
};
pub use driver::{any_url, mysql_options, pg_options, sqlite_options};
pub use error::DatabaseError;
pub use manager::{Connection, Database};
pub use migrate::{migrate, migrate_default, migrate_with};
