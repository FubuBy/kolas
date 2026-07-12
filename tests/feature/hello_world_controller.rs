//! `HelloWorldController::index` now depends on `Inject<dyn Logger>`, so it
//! must be driven through a real `Route`/`tower::ServiceExt::oneshot` (same
//! style as `tests/feature/di_http.rs`) instead of being called as a plain
//! async function — calling it directly would have no way to satisfy the
//! extractor.
//!
//! Uses `di_http::ensure_global` rather than installing its own container:
//! `Container::install_global` is a `OnceLock` shared by the whole `feature`
//! test binary, not per file — two different containers racing for it would
//! make whichever one loses silently disappear. `di_http::ensure_global`
//! already builds on the real `kolas::app::providers::all(...)` wiring
//! (which is where `dyn Logger` comes from), so it covers this file's needs
//! too; see its doc comment for the full rationale.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use kolas::app::http::controllers::HelloWorldController;
use kolas::framework::routing::Route;
use tower::ServiceExt;

use crate::di_http;

#[tokio::test]
async fn index_returns_hello_world_payload() {
    di_http::ensure_global();
    let app = Route::new()
        .get("/", HelloWorldController::index)
        .into_router();

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["payload"], "Hello world");
}
