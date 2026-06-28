use std::collections::HashMap;

use kolas::framework::database::{
    ConnectionConfig, Database, DatabaseConfig, DatabaseError, DriverKind, PoolConfig,
};

fn sqlite_memory_config() -> DatabaseConfig {
    let mut connections = HashMap::new();
    connections.insert(
        "primary".to_string(),
        ConnectionConfig {
            driver: DriverKind::Sqlite,
            url: Some("sqlite::memory:".into()),
            host: None,
            port: None,
            database: None,
            username: None,
            password: None,
            read: Vec::new(),
            pool: PoolConfig::default(),
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
async fn connection_for_returns_sqlite_pool_and_ping_succeeds() {
    let db = Database::new(sqlite_memory_config());
    let conn = db.connection_for("primary").await.expect("must open");
    assert_eq!(conn.driver(), "sqlite");
    conn.ping().await.expect("SELECT 1 must succeed");
}

#[tokio::test]
async fn repeated_connection_for_same_name_is_idempotent() {
    let db = Database::new(sqlite_memory_config());
    for _ in 0..3 {
        let conn = db
            .connection_for("primary")
            .await
            .expect("each call must succeed");
        conn.ping().await.expect("SELECT 1");
    }
}

#[tokio::test]
async fn unknown_connection_is_reported_with_name() {
    let db = Database::new(sqlite_memory_config());
    let err = db
        .connection_for("nope")
        .await
        .expect_err("must reject unknown name");
    match err {
        DatabaseError::UnknownConnection(n) => assert_eq!(n, "nope"),
        other => panic!("expected UnknownConnection, got {other:?}"),
    }
}

#[tokio::test]
async fn driver_mismatch_is_reported() {
    let db = Database::new(sqlite_memory_config());
    let err = db
        .postgres_pool_for("primary")
        .await
        .expect_err("primary is sqlite, postgres pool must fail");
    match err {
        DatabaseError::DriverMismatch {
            name,
            expected,
            actual,
        } => {
            assert_eq!(name, "primary");
            assert_eq!(expected, "postgres");
            assert_eq!(actual, "sqlite");
        }
        other => panic!("expected DriverMismatch, got {other:?}"),
    }
}

#[tokio::test]
async fn sqlite_pool_for_returns_typed_pool() {
    let db = Database::new(sqlite_memory_config());
    let pool = db.sqlite_pool_for("primary").await.expect("sqlite pool");
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .expect("typed query must run");
}

#[tokio::test]
async fn any_pool_for_returns_driver_agnostic_pool() {
    let db = Database::new(sqlite_memory_config());
    let pool = db.any_pool_for("primary").await.expect("any pool");
    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .expect("any query must run");
}

#[tokio::test]
async fn default_connection_uses_configured_default_name() {
    let db = Database::new(sqlite_memory_config());
    let conn = db.default_connection().await.expect("default must resolve");
    assert_eq!(conn.driver(), "sqlite");
}

#[tokio::test]
async fn no_default_is_reported_when_unset() {
    let mut cfg = sqlite_memory_config();
    cfg.default = None;
    let db = Database::new(cfg);
    let err = db.default_connection().await.expect_err("no default");
    assert!(matches!(err, DatabaseError::NoDefault));
}
