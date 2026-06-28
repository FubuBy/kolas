//! Translates `ConnectionConfig` (pure data from TOML) into SQLx-specific
//! objects. This is the only place in the database layer that knows
//! per-driver details — URL schemes, builder methods, what counts as a
//! required field.
//!
//! Three paths leave this module:
//! * `pg_options` / `mysql_options` / `sqlite_options` — typed
//!   `*ConnectOptions` for `Database::postgres / mysql / sqlite`. SQLx
//!   handles its own per-driver defaults (e.g. port `5432` / `3306`),
//!   so this module never hard-codes those.
//! * `any_url` — composes a URL for `sqlx::AnyPool::connect_lazy`, which
//!   only accepts a URL string. Ports are omitted when unset — SQLx parses
//!   the URL and applies the driver default.
//!
//! When the user supplies an explicit `url` field, all paths use it
//! verbatim (parsed through SQLx `FromStr` for the typed paths, passed
//! through unchanged for `any_url`).

use std::str::FromStr;

use sqlx::mysql::MySqlConnectOptions;
use sqlx::postgres::PgConnectOptions;
use sqlx::sqlite::SqliteConnectOptions;

use super::config::{ConnectionConfig, DriverKind};
use super::error::DatabaseError;

/// Builds an `AnyPool` connection URL from a `ConnectionConfig`.
///
/// Resolution order:
/// 1. If `url` is set explicitly — return it as-is (caller is responsible
///    for url-encoding special characters).
/// 2. For Postgres / MySQL — compose `<scheme>://[user[:pass]@]host[:port]/database`
///    using whatever fields are present. The port is **omitted** when not
///    set in the config; SQLx applies the driver default on parse.
/// 3. For SQLite without `url` — `MissingField { field: "url" }`.
pub fn any_url(name: &str, cfg: &ConnectionConfig) -> Result<String, DatabaseError> {
    if let Some(url) = &cfg.url {
        return Ok(url.clone());
    }

    match cfg.driver {
        DriverKind::Sqlite => Err(missing(name, "url")),
        DriverKind::Postgres | DriverKind::Mysql => {
            let host = cfg.host.as_deref().ok_or_else(|| missing(name, "host"))?;
            let database = cfg
                .database
                .as_deref()
                .ok_or_else(|| missing(name, "database"))?;

            let user = cfg.username.as_deref().unwrap_or("");
            let pass = cfg.password.as_deref().unwrap_or("");
            let userinfo = match (user.is_empty(), pass.is_empty()) {
                (true, _) => String::new(),
                (false, true) => format!("{user}@"),
                (false, false) => format!("{user}:{pass}@"),
            };
            let port_part = match cfg.port {
                Some(p) => format!(":{p}"),
                None => String::new(),
            };
            Ok(format!(
                "{scheme}://{userinfo}{host}{port_part}/{database}",
                scheme = cfg.driver.as_str(),
            ))
        }
    }
}

pub fn pg_options(name: &str, cfg: &ConnectionConfig) -> Result<PgConnectOptions, DatabaseError> {
    if let Some(url) = &cfg.url {
        return PgConnectOptions::from_str(url).map_err(|source| DatabaseError::InvalidUrl {
            name: name.to_string(),
            source,
        });
    }

    let host = cfg.host.as_deref().ok_or_else(|| missing(name, "host"))?;
    let database = cfg
        .database
        .as_deref()
        .ok_or_else(|| missing(name, "database"))?;

    let mut opts = PgConnectOptions::new().host(host).database(database);
    if let Some(port) = cfg.port {
        opts = opts.port(port);
    }
    if let Some(user) = cfg.username.as_deref() {
        if !user.is_empty() {
            opts = opts.username(user);
        }
    }
    if let Some(pass) = cfg.password.as_deref() {
        if !pass.is_empty() {
            opts = opts.password(pass);
        }
    }
    Ok(opts)
}

pub fn mysql_options(
    name: &str,
    cfg: &ConnectionConfig,
) -> Result<MySqlConnectOptions, DatabaseError> {
    if let Some(url) = &cfg.url {
        return MySqlConnectOptions::from_str(url).map_err(|source| DatabaseError::InvalidUrl {
            name: name.to_string(),
            source,
        });
    }

    let host = cfg.host.as_deref().ok_or_else(|| missing(name, "host"))?;
    let database = cfg
        .database
        .as_deref()
        .ok_or_else(|| missing(name, "database"))?;

    let mut opts = MySqlConnectOptions::new().host(host).database(database);
    if let Some(port) = cfg.port {
        opts = opts.port(port);
    }
    if let Some(user) = cfg.username.as_deref() {
        if !user.is_empty() {
            opts = opts.username(user);
        }
    }
    if let Some(pass) = cfg.password.as_deref() {
        if !pass.is_empty() {
            opts = opts.password(pass);
        }
    }
    Ok(opts)
}

pub fn sqlite_options(
    name: &str,
    cfg: &ConnectionConfig,
) -> Result<SqliteConnectOptions, DatabaseError> {
    // SQLite has no host/port/credentials in our model — only `url` makes sense.
    let url = cfg.url.as_deref().ok_or_else(|| missing(name, "url"))?;
    SqliteConnectOptions::from_str(url).map_err(|source| DatabaseError::InvalidUrl {
        name: name.to_string(),
        source,
    })
}

fn missing(name: &str, field: &str) -> DatabaseError {
    DatabaseError::MissingField {
        name: name.to_string(),
        field: field.to_string(),
    }
}
