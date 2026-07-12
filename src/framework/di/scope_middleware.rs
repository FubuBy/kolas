use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::framework::http::middleware::Middleware;

use super::scope::Scope;

/// Establishes a fresh [`Scope`] for the duration of one request. Register
/// it on the application router via `Route::middleware(ScopeMiddleware)` if
/// the application has at least one `scoped`/`scoped_tagged` registration —
/// without it, resolving a `Scoped` type during a request fails with
/// `Err(DiError::ScopeNotActive)`.
#[derive(Clone, Default)]
pub struct ScopeMiddleware;

impl Middleware for ScopeMiddleware {
    async fn handle(&self, request: Request, next: Next) -> Response {
        let scope = Scope::new();
        scope.enter(next.run(request)).await
    }
}
