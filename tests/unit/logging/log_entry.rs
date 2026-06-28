use kolas::framework::logging::LogEntry;

/// Helper: serialize a LogEntry and check JSON fields.
fn to_json(entry: &LogEntry) -> serde_json::Value {
    serde_json::to_value(entry).expect("LogEntry must serialize")
}

#[test]
fn log_entry_serializes_required_fields() {
    let entry = LogEntry {
        timestamp: chrono::Utc::now(),
        level: "INFO".to_string(),
        target: "my_app::handler".to_string(),
        message: "hello world".to_string(),
        fields: serde_json::Value::Object(serde_json::Map::new()),
        span_name: None,
    };

    let json = to_json(&entry);
    assert_eq!(json["level"], "INFO");
    assert_eq!(json["target"], "my_app::handler");
    assert_eq!(json["message"], "hello world");
    assert!(json["timestamp"].is_string());
    assert!(json.get("span_name").is_some());
}

#[test]
fn log_entry_serializes_optional_span_name() {
    let entry_with = LogEntry {
        timestamp: chrono::Utc::now(),
        level: "DEBUG".to_string(),
        target: "t".to_string(),
        message: "m".to_string(),
        fields: serde_json::json!({}),
        span_name: Some("my-span".to_string()),
    };
    let entry_without = LogEntry {
        span_name: None,
        ..entry_with.clone()
    };

    assert_eq!(to_json(&entry_with)["span_name"], "my-span");
    assert!(to_json(&entry_without)["span_name"].is_null());
}

#[test]
fn log_entry_serializes_structured_fields() {
    let fields = serde_json::json!({
        "user_id": 42,
        "path": "/api/users",
        "success": true
    });
    let entry = LogEntry {
        timestamp: chrono::Utc::now(),
        level: "INFO".to_string(),
        target: "t".to_string(),
        message: "request".to_string(),
        fields: fields.clone(),
        span_name: None,
    };

    let json = to_json(&entry);
    assert_eq!(json["fields"]["user_id"], 42);
    assert_eq!(json["fields"]["path"], "/api/users");
    assert_eq!(json["fields"]["success"], true);
}

#[test]
fn log_entry_clone_is_independent() {
    let original = LogEntry {
        timestamp: chrono::Utc::now(),
        level: "WARN".to_string(),
        target: "t".to_string(),
        message: "original".to_string(),
        fields: serde_json::json!({}),
        span_name: None,
    };
    let mut cloned = original.clone();
    cloned.message = "modified".to_string();

    assert_eq!(original.message, "original");
    assert_eq!(cloned.message, "modified");
}
