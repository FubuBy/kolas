use axum::Json;
use serde::Serialize;

use crate::framework::di::Inject;
use crate::framework::logging::Logger;

#[derive(Serialize)]
pub struct HelloPayload {
    pub payload: String,
}

pub struct HelloWorldController;

impl HelloWorldController {
    pub async fn index(Inject(logger, ..): Inject<dyn Logger>) -> Json<HelloPayload> {
        logger.info("HelloWorldController::index called");
        Json(HelloPayload {
            payload: "Hello world".to_string(),
        })
    }
}
