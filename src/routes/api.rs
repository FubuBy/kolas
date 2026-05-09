use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::app::http::controllers::HelloWorldController;
use crate::app::http::middleware::TrimStrings;
use crate::framework::routing::Route;

/// HTTP router: **application** middleware (`TrimStrings`) + **infrastructure** Tower layers.
///
/// Order on the builder: routes and `middleware` first, then `layer` calls. On the wire (outer →
/// inner for the request): **trace → CORS → compression →** `TrimStrings` → handlers.
///
/// `CorsLayer::permissive()` is for local development; tighten origins and headers for production.
pub fn routes() -> Router {
    Route::new()
        .get("/", HelloWorldController::index)
        .get("/hello", HelloWorldController::index)
        .middleware(TrimStrings)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .into_router()
}
