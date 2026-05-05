use kolas::app::http::controllers::hello_world_controller::HelloPayload;

#[test]
fn serializes_to_expected_json_shape() {
    let payload = HelloPayload {
        payload: "Hello world".to_string(),
    };

    let json = serde_json::to_string(&payload).expect("HelloPayload must serialize");

    assert_eq!(json, r#"{"payload":"Hello world"}"#);
}

#[test]
fn preserves_arbitrary_payload_string() {
    let payload = HelloPayload {
        payload: "тест 🦀".to_string(),
    };

    let json = serde_json::to_string(&payload).expect("HelloPayload must serialize");

    assert!(json.contains("тест"));
    assert!(json.contains("🦀"));
}
