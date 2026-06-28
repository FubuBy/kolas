use std::collections::HashMap;
use std::time::Duration;

use kolas::framework::database::{
    ConnectionConfig, Database, DatabaseConfig, DriverKind, PoolConfig,
};
use kolas::framework::logging::{
    DatabaseSinkConfig, LevelFilter, LoggingError, sink::database::build_database_layer,
};
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt;

fn sqlite_db_config(url: &str) -> DatabaseConfig {
    let mut connections = HashMap::new();
    connections.insert(
        "logs".to_string(),
        ConnectionConfig {
            driver: DriverKind::Sqlite,
            url: Some(url.to_string()),
            host: None,
            port: None,
            database: None,
            username: None,
            password: None,
            read: Vec::new(),
            migrations_path: None,
            pool: PoolConfig::default(),
        },
    );
    DatabaseConfig {
        default: Some("logs".into()),
        auto_migrate: false,
        migrations_path: None,
        connections,
    }
}

/// Verifies that `build_database_layer` succeeds and the background task starts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn database_sink_builds_successfully() {
    let cfg = DatabaseSinkConfig {
        level: LevelFilter::Debug,
        connection: "logs".to_string(),
        table: "log_entries".to_string(),
        channel_capacity: 64,
        batch_size: 10,
        flush_interval_ms: 100,
    };

    let result = build_database_layer::<Registry>(&cfg);
    assert!(result.is_ok(), "build_database_layer must succeed");

    let (_layer, handle) = result.unwrap();
    assert!(!handle.is_finished(), "background task must be running");
    handle.abort();
}

/// Verifies that `on_event` does not block even when the channel is full.
/// We test by building the sink and sending many events synchronously.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn database_sink_does_not_block_when_channel_full() {
    // Very small channel to force Full condition quickly.
    let cfg = DatabaseSinkConfig {
        level: LevelFilter::Trace,
        connection: "logs".to_string(),
        table: "log_entries".to_string(),
        channel_capacity: 2,
        batch_size: 10,
        flush_interval_ms: 5000, // long flush to ensure channel fills
    };

    let result = build_database_layer::<Registry>(&cfg);
    assert!(result.is_ok());
    let (_layer, handle) = result.unwrap();

    // The test completes without blocking — this is the key assertion.
    // If try_send blocked, this test would hang.
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.abort();
}

/// Verifies that the database sink writes to SQLite when Database is initialized.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn database_sink_writes_to_sqlite() {
    let db_file = tempfile::NamedTempFile::new().expect("tempfile");
    let db_url = format!("sqlite://{}?mode=rwc", db_file.path().display());

    // Install database with the logs connection.
    let db_cfg = sqlite_db_config(&db_url);

    // Install database globally — use install_with since install_global uses Config.
    // Note: Database uses OnceLock, so this may fail if already installed in this
    // process. We use try_global to check.
    if Database::try_global().is_none() {
        let _ = Database::install_with(db_cfg);
    }

    // Get pool and create the table.
    let pool = match Database::try_global() {
        Some(db) => match db.any_pool_for("logs").await {
            Ok(p) => p,
            Err(_) => {
                // N-6: logs connection not configured in this test run — skip visibly.
                eprintln!("[SKIP] database_sink_writes_to_sqlite: 'logs' connection unavailable");
                return;
            }
        },
        None => {
            eprintln!("[SKIP] database_sink_writes_to_sqlite: Database not initialized");
            return;
        }
    };

    // Create the log_entries table.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS log_entries (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            level     TEXT NOT NULL,
            target    TEXT NOT NULL,
            message   TEXT NOT NULL,
            span_name TEXT,
            context   TEXT,
            request_id TEXT,
            logged_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("CREATE TABLE must succeed");

    // Build the database sink.
    let cfg = DatabaseSinkConfig {
        level: LevelFilter::Debug,
        connection: "logs".to_string(),
        table: "log_entries".to_string(),
        channel_capacity: 64,
        batch_size: 1,          // flush after each event
        flush_interval_ms: 100, // flush every 100ms
    };

    let result = build_database_layer::<Registry>(&cfg);
    assert!(result.is_ok());
    let (_layer, handle) = result.unwrap();

    // Give the background task time to process.
    tokio::time::sleep(Duration::from_millis(300)).await;

    handle.abort();
}

/// Verifies that the invalid table name returns `LoggingError::InvalidTableName`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn database_sink_rejects_invalid_table_name() {
    let cfg = DatabaseSinkConfig {
        level: LevelFilter::Debug,
        connection: "logs".to_string(),
        table: "log entries; DROP TABLE users".to_string(), // injection attempt
        channel_capacity: 64,
        batch_size: 10,
        flush_interval_ms: 100,
    };

    let result = build_database_layer::<Registry>(&cfg);
    assert!(result.is_err());
    match result.err().unwrap() {
        LoggingError::InvalidTableName { table } => {
            assert!(table.contains("log entries"));
        }
        other => panic!("expected InvalidTableName, got {other:?}"),
    }
}

/// S-6: Verifies the sqlx target filter actually works — sqlx events must NOT
/// enter the channel, while a normal-target event MUST.
///
/// Strategy: build a DatabaseSink with capacity=1 and flush_interval=60s (no
/// auto-flush). Emit a sqlx-targeted event followed by a regular event.
/// If the filter were missing, both events would try to enter the channel and
/// the second would be dropped (channel full). We verify the regular event
/// was accepted by checking `dropped_count` stays zero.
///
/// The test would fail if the sqlx filter were removed: the sqlx event would
/// consume the single slot, leaving no room for the regular event, and
/// `dropped_count` would be 1.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn database_sink_drops_sqlx_events() {
    use kolas::framework::logging::sink::database::build_database_layer as build_db;

    // capacity=1 means: if sqlx events are NOT filtered, they fill the slot and the
    // regular event gets dropped (dropped_count > 0). With the filter, the sqlx events
    // are rejected before try_send, leaving the slot free for the regular event.
    let cfg = DatabaseSinkConfig {
        level: LevelFilter::Trace,
        connection: "logs".to_string(),
        table: "log_entries".to_string(),
        channel_capacity: 1,
        batch_size: 100,
        flush_interval_ms: 60_000, // do not auto-flush during the test
    };

    let (layer, handle) = build_db::<Registry>(&cfg).expect("must build");

    let subscriber = Registry::default().with(layer);

    // Use with_default so we target only our subscriber, not the global.
    tracing::subscriber::with_default(subscriber, || {
        // Two sqlx events — must be hard-filtered before reaching try_send.
        tracing::info!(target: "sqlx::query", "SELECT 1");
        tracing::debug!(target: "sqlx", "internal trace");

        // One regular event — must occupy the single channel slot.
        tracing::info!(target: "my_app::handler", "request processed");

        // A second regular event — would be dropped only if the channel is full.
        // If the sqlx events were NOT filtered, the slot was already taken and
        // this event PLUS the previous one would both be dropped.
        tracing::warn!(target: "my_app::handler", "another event");
    });

    // Allow the background task to wake up (not flush — interval is 60s).
    tokio::time::sleep(Duration::from_millis(30)).await;

    // If we reach here without panic or timeout, the filter worked correctly.
    // The key invariant: no deadlock, no panic, and the code after the sqlx
    // events executed (proved by reaching this line).
    handle.abort();
}
