use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::uri::PathAndQuery;
use axum::http::{Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::framework::http::middleware::Middleware;

const SKIP_FIELDS: &[&str] = &["password", "password_confirmation"];
const MAX_BODY: usize = 2 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct TrimStrings;

impl Middleware for TrimStrings {
    async fn handle(&self, mut request: Request, next: Next) -> Response {
        if let Some(new_uri) = trim_query_in_uri(request.uri()) {
            *request.uri_mut() = new_uri;
        }

        if !matches!(
            request.method(),
            &Method::POST | &Method::PUT | &Method::PATCH
        ) {
            return next.run(request).await;
        }

        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let (parts, body) = request.into_parts();
        let bytes = match to_bytes(body, MAX_BODY).await {
            Ok(b) => b,
            Err(_) => {
                return (StatusCode::PAYLOAD_TOO_LARGE, "request body too large").into_response();
            }
        };

        let new_body = match content_type.as_deref() {
            Some(ct) if ct.starts_with("application/json") && !bytes.is_empty() => {
                match serde_json::from_slice::<Value>(&bytes) {
                    Ok(mut v) => {
                        trim_value(&mut v, None);
                        Body::from(serde_json::to_vec(&v).expect("re-serialize json"))
                    }
                    Err(_) => Body::from(bytes),
                }
            }
            Some(ct)
                if ct.starts_with("application/x-www-form-urlencoded") && !bytes.is_empty() =>
            {
                match serde_urlencoded::from_bytes::<Vec<(String, String)>>(&bytes) {
                    Ok(mut pairs) => {
                        trim_form_pairs(&mut pairs);
                        Body::from(serde_urlencoded::to_string(&pairs).expect("re-serialize form"))
                    }
                    Err(_) => Body::from(bytes),
                }
            }
            _ => Body::from(bytes),
        };

        next.run(Request::from_parts(parts, new_body)).await
    }
}

/// Recursively trims JSON string values; skips values under keys listed in `SKIP_FIELDS`.
pub fn trim_value(value: &mut Value, parent_key: Option<&str>) {
    match value {
        Value::String(s) => {
            if !parent_key.is_some_and(|k| SKIP_FIELDS.contains(&k)) {
                *s = s.trim().to_string();
            }
        }
        Value::Array(items) => {
            for v in items.iter_mut() {
                trim_value(v, parent_key);
            }
        }
        Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                trim_value(v, Some(k.as_str()));
            }
        }
        _ => {}
    }
}

/// Trims form field values except `password` and `password_confirmation`.
pub fn trim_form_pairs(pairs: &mut [(String, String)]) {
    for (k, v) in pairs.iter_mut() {
        if !SKIP_FIELDS.contains(&k.as_str()) {
            *v = v.trim().to_string();
        }
    }
}

/// Trims query parameter values; returns the original string if parsing fails.
pub fn trim_query_string(q: &str) -> String {
    let mut pairs: Vec<(String, String)> = match serde_urlencoded::from_str(q) {
        Ok(p) => p,
        Err(_) => return q.to_string(),
    };
    trim_form_pairs(&mut pairs);
    serde_urlencoded::to_string(&pairs).unwrap_or_else(|_| q.to_string())
}

fn trim_query_in_uri(uri: &Uri) -> Option<Uri> {
    let q = uri.query()?;
    let new_q = trim_query_string(q);
    if new_q == q {
        return None;
    }
    let path = uri.path();
    let path_and_query = if new_q.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{new_q}")
    };
    let pq: PathAndQuery = path_and_query.parse().ok()?;
    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(pq);
    Uri::from_parts(parts).ok()
}
