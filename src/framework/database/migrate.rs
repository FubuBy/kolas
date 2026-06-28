use std::path::Path;

use sqlx::migrate::{Migrate, Migrator};

use super::error::DatabaseError;
use super::manager::{Connection, Database};

/// Applies pending migrations from `dir` against the given connection.
///
/// Missing or empty directory is reported as a `MigrateError` by sqlx —
/// we surface it as-is to keep the boot-time failure mode honest.
pub async fn migrate(connection_name: &str, dir: impl AsRef<Path>) -> Result<(), DatabaseError> {
    let migrator = Migrator::new(dir.as_ref()).await?;
    let conn = Database::connection(connection_name).await?;
    run_against(&migrator, &conn).await
}

/// Applies migrations against an explicit Database instance — needed by
/// tests that don't install a global manager.
pub async fn migrate_with(
    db: &Database,
    connection_name: &str,
    dir: impl AsRef<Path>,
) -> Result<(), DatabaseError> {
    let migrator = Migrator::new(dir.as_ref()).await?;
    let conn = db.connection_for(connection_name).await?;
    run_against(&migrator, &conn).await
}

/// Applies migrations against the default connection.
pub async fn migrate_default(dir: impl AsRef<Path>) -> Result<(), DatabaseError> {
    let name = Database::global().config().default_name()?.to_string();
    migrate(&name, dir).await
}

/// Rolls back the last applied migration against the default connection.
pub async fn rollback_default(dir: impl AsRef<Path>) -> Result<(), DatabaseError> {
    let name = Database::global().config().default_name()?.to_string();
    rollback(&name, dir).await
}

/// Rolls back the last applied migration against the given connection.
pub async fn rollback(connection_name: &str, dir: impl AsRef<Path>) -> Result<(), DatabaseError> {
    let migrator = Migrator::new(dir.as_ref()).await?;
    let conn = Database::connection(connection_name).await?;

    let versions = last_applied_versions(&conn, 2).await?;
    let target = match versions.len() {
        0 => {
            println!("Nothing to roll back.");
            return Ok(());
        }
        1 => 0,
        _ => versions[1],
    };

    run_undo_against(&migrator, &conn, target).await
}

async fn run_against(migrator: &Migrator, conn: &Connection) -> Result<(), DatabaseError> {
    match conn {
        Connection::Postgres(p) => migrator.run(p).await?,
        Connection::MySql(p) => migrator.run(p).await?,
        Connection::Sqlite(p) => migrator.run(p).await?,
    }
    Ok(())
}

async fn run_undo_against(
    migrator: &Migrator,
    conn: &Connection,
    target: i64,
) -> Result<(), DatabaseError> {
    match conn {
        Connection::Postgres(p) => migrator.undo(p, target).await?,
        Connection::MySql(p) => migrator.undo(p, target).await?,
        Connection::Sqlite(p) => migrator.undo(p, target).await?,
    }
    Ok(())
}

/// Returns up to `limit` applied migration versions, newest first.
///
/// Uses sqlx's public `Migrate` API rather than querying the internal
/// `_sqlx_migrations` table directly. `ensure_migrations_table` makes a
/// never-migrated database return an empty vec instead of erroring; any real
/// database error is propagated so a failed rollback never masquerades as
/// "nothing to do".
async fn last_applied_versions(conn: &Connection, limit: usize) -> Result<Vec<i64>, DatabaseError> {
    let mut applied = match conn {
        Connection::Postgres(p) => {
            let mut c = p.acquire().await?;
            c.ensure_migrations_table().await?;
            c.list_applied_migrations().await?
        }
        Connection::MySql(p) => {
            let mut c = p.acquire().await?;
            c.ensure_migrations_table().await?;
            c.list_applied_migrations().await?
        }
        Connection::Sqlite(p) => {
            let mut c = p.acquire().await?;
            c.ensure_migrations_table().await?;
            c.list_applied_migrations().await?
        }
    };
    applied.sort_by(|a, b| b.version.cmp(&a.version));
    applied.truncate(limit);
    Ok(applied.into_iter().map(|m| m.version).collect())
}
