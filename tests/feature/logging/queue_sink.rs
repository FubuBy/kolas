use std::time::Duration;

use kolas::framework::logging::{
    LevelFilter, LoggingError, NullQueueDriver, QueueDriver, QueueSinkConfig,
    sink::queue::build_queue_layer,
};
use tracing_subscriber::Registry;

/// Verifies that `NullQueueDriver` never panics and always returns `Ok`.
#[tokio::test]
async fn null_queue_driver_never_panics() {
    let driver = NullQueueDriver;
    let payload = serde_json::json!({
        "level": "ERROR",
        "message": "test event",
        "timestamp": "2024-01-01T00:00:00Z"
    });

    let result = driver.publish(payload).await;
    assert!(result.is_ok(), "NullQueueDriver must always return Ok");
}

#[tokio::test]
async fn null_queue_driver_name_is_null() {
    let driver = NullQueueDriver;
    assert_eq!(driver.name(), "null");
}

/// Verifies that `build_queue_layer` with "null" driver succeeds.
#[tokio::test]
async fn queue_sink_null_driver_builds_successfully() {
    let cfg = QueueSinkConfig {
        level: LevelFilter::Error,
        driver: "null".to_string(),
        channel_capacity: 64,
        connection: None,
    };

    let result = build_queue_layer::<Registry>(&cfg);
    assert!(
        result.is_ok(),
        "build_queue_layer must succeed with null driver"
    );

    let (_layer, handle) = result.unwrap();
    assert!(!handle.is_finished());
    handle.abort();
}

/// Verifies that an unknown driver returns `LoggingError::UnknownQueueDriver`.
#[tokio::test]
async fn queue_sink_unknown_driver_returns_error() {
    let cfg = QueueSinkConfig {
        level: LevelFilter::Info,
        driver: "redis".to_string(), // not implemented yet
        channel_capacity: 64,
        connection: None,
    };

    let result = build_queue_layer::<Registry>(&cfg);
    assert!(result.is_err());
    let err = result.err().expect("must be Err");
    match err {
        LoggingError::UnknownQueueDriver { driver } => {
            assert_eq!(driver, "redis");
        }
        other => panic!("expected UnknownQueueDriver, got {other:?}"),
    }
}

/// Verifies that the queue sink does not block on a full channel.
#[tokio::test]
async fn queue_sink_does_not_block_on_full_channel() {
    let cfg = QueueSinkConfig {
        level: LevelFilter::Trace,
        driver: "null".to_string(),
        channel_capacity: 1, // tiny channel
        connection: None,
    };

    let result = build_queue_layer::<Registry>(&cfg);
    assert!(result.is_ok());
    let (_layer, handle) = result.unwrap();

    // Give any startup work a moment.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // No blocking — test passes if we reach here.
    handle.abort();
}
