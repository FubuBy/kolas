use std::path::Path;

use sqlx::migrate::Migrator;

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

async fn run_against(migrator: &Migrator, conn: &Connection) -> Result<(), DatabaseError> {
    match conn {
        Connection::Postgres(p) => migrator.run(p).await?,
        Connection::MySql(p) => migrator.run(p).await?,
        Connection::Sqlite(p) => migrator.run(p).await?,
    }
    Ok(())
}
