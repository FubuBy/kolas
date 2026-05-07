use std::future::Future;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::middleware::{Next, from_fn};
use axum::response::Response;

/// Application middleware: runs before the matched handler and can read or mutate the request.
///
/// Implement this on a unit struct, or pass an `async fn(Request, Next) -> Response` — function
/// items implement [`Middleware`] via a blanket implementation.
pub trait Middleware: Send + Sync + 'static {
    fn handle(&self, request: Request, next: Next) -> impl Future<Output = Response> + Send;
}

impl<F, Fut> Middleware for F
where
    F: Fn(Request, Next) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response> + Send + 'static,
{
    fn handle(&self, request: Request, next: Next) -> impl Future<Output = Response> + Send {
        (self)(request, next)
    }
}

/// Applies middleware to the entire router (all current and future routes, including fallbacks).
pub(crate) fn apply_layer<M: Middleware>(router: Router, middleware: M) -> Router {
    let middleware = Arc::new(middleware);
    router.layer(from_fn(move |req: Request, next: Next| {
        let middleware = middleware.clone();
        async move { middleware.handle(req, next).await }
    }))
}

/// Applies middleware only to routes **already** registered on this router.
///
/// Routes added after this call will **not** receive this layer. Axum panics if the router has no
/// routes yet — register at least one route before calling [`apply_route_layer`].
pub(crate) fn apply_route_layer<M: Middleware>(router: Router, middleware: M) -> Router {
    let middleware = Arc::new(middleware);
    router.route_layer(from_fn(move |req: Request, next: Next| {
        let middleware = middleware.clone();
        async move { middleware.handle(req, next).await }
    }))
}
