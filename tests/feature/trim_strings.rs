use std::collections::HashMap;

use axum::Json;
use axum::body::{Body, to_bytes};
use axum::extract::Query;
use axum::http::{Request, StatusCode};
use kolas::app::http::middleware::TrimStrings;
use kolas::framework::routing::Route;
use tower::ServiceExt;

async fn echo_json(body: String) -> String {
    body
}

async fn echo_form(body: String) -> String {
    body
}

async fn echo_query(Query(map): Query<HashMap<String, String>>) -> Json<HashMap<String, String>> {
    Json(map)
}

fn app_global_trim() -> axum::Router {
    Route::new()
        .post("/echo-json", echo_json)
        .post("/echo-form", echo_form)
        .get("/echo-query", echo_query)
        .middleware(TrimStrings)
        .into_router()
}

fn app_route_middleware_split() -> axum::Router {
    Route::new()
        .post("/a", echo_json)
        .route_middleware(TrimStrings)
        .post("/b", echo_json)
        .into_router()
}

#[tokio::test]
async fn post_json_body_is_trimmed() {
    let app = app_global_trim();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo-json")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"  john "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], br#"{"name":"john"}"#);
}

#[tokio::test]
async fn password_field_is_preserved_in_json() {
    let app = app_global_trim();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo-json")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"password":"  secret  "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], br#"{"password":"  secret  "}"#);
}

#[tokio::test]
async fn post_form_body_is_trimmed() {
    let app = app_global_trim();
    let body = "name=%20%20john%20%20";
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo-form")
                .header(
                    "content-type",
                    "application/x-www-form-urlencoded; charset=utf-8",
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let pairs: Vec<(String, String)> = serde_urlencoded::from_str(&text).unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0, "name");
    assert_eq!(pairs[0].1, "john");
}

#[tokio::test]
async fn password_field_is_preserved_in_form() {
    let app = app_global_trim();
    let body = "password=%20%20s%20%20";
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo-form")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    let pairs: Vec<(String, String)> = serde_urlencoded::from_str(&text).unwrap();
    assert_eq!(pairs[0].1, "  s  ");
}

#[tokio::test]
async fn query_string_is_trimmed_on_get() {
    let app = app_global_trim();
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/echo-query?q=%20%20hello%20%20&lang=en")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let map: HashMap<String, String> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(map.get("q").map(String::as_str), Some("hello"));
    assert_eq!(map.get("lang").map(String::as_str), Some("en"));
}

#[tokio::test]
async fn password_query_param_is_preserved() {
    let app = app_global_trim();
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/echo-query?password=%20%20s%20%20")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let map: HashMap<String, String> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(map.get("password").map(String::as_str), Some("  s  "));
}

#[tokio::test]
async fn non_json_non_form_body_passes_through() {
    let app = app_global_trim();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo-json")
                .header("content-type", "text/plain")
                .body(Body::from("  hi  "))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes[..], b"  hi  ");
}

#[tokio::test]
async fn empty_body_passes_through() {
    let app = app_global_trim();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/echo-json")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test]
async fn route_middleware_applies_only_to_existing_routes() {
    let app = app_route_middleware_split();

    let res_a = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/a")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"x":"  y "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_a.status(), StatusCode::OK);
    let bytes_a = to_bytes(res_a.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes_a[..], br#"{"x":"y"}"#);

    let res_b = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/b")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"x":"  y "}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_b.status(), StatusCode::OK);
    let bytes_b = to_bytes(res_b.into_body(), usize::MAX).await.unwrap();
    assert_eq!(&bytes_b[..], br#"{"x":"  y "}"#);
}
