use axum::Router;

use crate::app::http::controllers::HelloWorldController;
use crate::app::http::middleware::TrimStrings;
use crate::framework::routing::Route;

pub fn routes() -> Router {
    Route::new()
        .get("/", HelloWorldController::index)
        .get("/hello", HelloWorldController::index)
        .middleware(TrimStrings)
        .into_router()
}
