use axum::Router;

use crate::app::http::controllers::HelloWorldController;
use crate::framework::routing::Route;

pub fn routes() -> Router {
    Route::new()
        .get("/", HelloWorldController::index)
        .get("/hello", HelloWorldController::index)
        .into_router()
}
