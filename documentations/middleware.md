# Add HTTP middleware

Middleware runs **before** your handler. Convention: **library / infrastructure** → `Route::layer(...)` (`tower-http`, other `Layer` types); **your policy** → `Route::middleware(...)` with the `Middleware` trait (Axum `middleware::from_fn` under the hood).

## 1. Implement middleware (struct)

Create `src/app/http/middleware/<name>.rs` (or extend `trim_strings.rs` as a template). A typical middleware is a unit struct with an `async fn handle(&self, request, next) -> Response`:

```rust
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use kolas::framework::http::middleware::Middleware;

#[derive(Clone, Default)]
pub struct MyMiddleware;

impl Middleware for MyMiddleware {
    async fn handle(&self, request: Request, next: Next) -> Response {
        // Inspect or mutate `request` here if needed.
        next.run(request).await
    }
}
```

Expose it from `src/app/http/middleware/mod.rs` with `pub mod my_middleware;` and `pub use my_middleware::MyMiddleware;`.

## 2. Async function middleware (blanket impl)

Function items such as `async fn(req: Request, next: Next) -> Response` also implement `Middleware`, so you can pass them directly to the route builder:

```rust
async fn noop(req: Request, next: Next) -> Response {
    next.run(req).await
}

// ...
Route::new()
    .get("/hello", HelloWorldController::index)
    .middleware(noop)
```

Note: ordinary `|req, next| async move { ... }` closures are often `FnOnce` (they consume `Request`), so they do **not** always satisfy the blanket `Fn` bound. Prefer a named `async fn` or a unit struct.

## 3. Register on the `Route` builder

Open `src/routes/api.rs` and chain:

| Method | Typical use | Effect |
|--------|-------------|--------|
| `.layer(L)` | `CompressionLayer`, `CorsLayer`, `TraceLayer`, … | Same as `axum::Router::layer`: applies to the whole router built so far (and routes you add **after** this call). |
| `.middleware(M)` | Types implementing `Middleware` | Same stack position as other global layers; use for app-specific `from_fn` middleware. |
| `.route_middleware(M)` | Scoped auth, etc. | Same as `Router::route_layer`: only routes registered **before** this call. At least one route required or Axum panics. |

Example (matches the project skeleton — app middleware first, then tower-http; on the wire: trace → CORS → compression → `TrimStrings` → handlers):

```rust
use kolas::app::http::middleware::TrimStrings;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

Route::new()
    .get("/", HelloWorldController::index)
    .middleware(TrimStrings)
    .layer(CompressionLayer::new())
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
    .into_router()
```

## 4. What the sample `TrimStrings` middleware does

| Source | When | Behaviour |
|--------|------|-----------|
| Query string | Any HTTP method | Trims decoded parameter values (via `serde_urlencoded`). On parse failure the query is left unchanged. |
| JSON body | `POST` / `PUT` / `PATCH`, `Content-Type: application/json` | Recursively trims string values in `serde_json::Value`. Invalid JSON leaves the body untouched. |
| Form body | Same methods, `application/x-www-form-urlencoded` | Trims each value. Invalid form bodies are left untouched. |
| Keys `password`, `password_confirmation` | JSON, form, query | Values are **not** trimmed. |
| Other bodies | e.g. `multipart/*`, `text/plain` | Body is not buffered or modified. |

Internal design notes: `dev_docs/architecture/middleware.md`.

[← Back to readme](../readme.md)
