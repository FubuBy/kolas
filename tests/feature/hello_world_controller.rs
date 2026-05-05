use kolas::app::http::controllers::HelloWorldController;

#[tokio::test]
async fn index_returns_hello_world_payload() {
    let response = HelloWorldController::index().await;
    assert_eq!(response.0.payload, "Hello world");
}
