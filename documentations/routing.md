# Routing under the hood

`Route` is a thin builder over `axum::Router`. Verbs like `.get` delegate to `Router::route`. `.middleware` and `.layer` delegate to Axum's `Router::layer`; `.route_middleware` maps to `Router::route_layer`. In `src/routes/api.rs`, **tower-http** layers are chained on the same `Route` builder as application middleware, then `.into_router()` produces the `Router` passed to `axum::serve` in `bootstrap/server.rs` (`HttpServer::run`). Anything Axum supports (extractors, state, error responses) remains available inside controller methods.

See also: [Add a controller and register its route](controllers.md), [Add HTTP middleware](middleware.md).

[← Back to readme](../readme.md)
