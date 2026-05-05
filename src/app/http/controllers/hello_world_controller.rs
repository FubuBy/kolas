use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HelloPayload {
    pub payload: String,
}

pub struct HelloWorldController;

impl HelloWorldController {
    pub async fn index() -> Json<HelloPayload> {
        Json(HelloPayload {
            payload: "Hello world".to_string(),
        })
    }
}
