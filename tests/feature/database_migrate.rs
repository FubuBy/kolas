use std::collections::HashMap;
use std::path::Path;

use kolas::framework::database::{
    ConnectionConfig, Database, DatabaseConfig, DriverKind, PoolConfig, migrate_with,
};
use tempfile::TempDir;

fn sqlite_file_config(path: &Path) -> DatabaseConfig {
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let mut connections = HashMap::new();
    connections.insert(
        "primary".to_string(),
        ConnectionConfig {
            driver: DriverKind::Sqlite,
            url: Some(url),
            host: None,
            port: None,
            database: None,
            username: None,
            password: None,
            read: Vec::new(),
            migrations_path: None,
            // Single connection avoids contention on a single sqlite file
            // when migrations and follow-up queries run sequentially.
            pool: PoolConfig {
                max: Some(1),
                ..Default::default()
            },
        },
    );
    DatabaseConfig {
        default: Some("primary".into()),
        auto_migrate: false,
        migrations_path: None,
        connections,
    }
}

#[tokio::test]
async fn migrate_with_applies_pending_sql_files() {
    let migrations_dir = TempDir::new().expect("migrations tempdir");
    let db_dir = TempDir::new().expect("db tempdir");
    let db_path = db_dir.path().join("test.sqlite");

    std::fs::write(
        migrations_dir.path().join("0001_init.sql"),
        "CREATE TABLE pets (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
    )
    .expect("write migration");

    let db = Database::new(sqlite_file_config(&db_path));
    migrate_with(&db, "primary", migrations_dir.path())
        .await
        .expect("migrations must apply");

    let pool = db
        .sqlite_pool_for("primary")
        .await
        .expect("sqlite pool after migration");

    sqlx::query("INSERT INTO pets (name) VALUES ('Mochi')")
        .execute(&pool)
        .await
        .expect("insert into migrated table");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pets")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn migrate_with_is_idempotent_on_repeated_runs() {
    let migrations_dir = TempDir::new().expect("migrations tempdir");
    let db_dir = TempDir::new().expect("db tempdir");
    let db_path = db_dir.path().join("test.sqlite");

    std::fs::write(
        migrations_dir.path().join("0001_init.sql"),
        "CREATE TABLE pets (id INTEGER PRIMARY KEY);",
    )
    .expect("write migration");

    let db = Database::new(sqlite_file_config(&db_path));
    migrate_with(&db, "primary", migrations_dir.path())
        .await
        .expect("first run");
    migrate_with(&db, "primary", migrations_dir.path())
        .await
        .expect("second run must be a no-op");
}
