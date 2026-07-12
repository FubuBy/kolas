//! Feature tests for `Inject<T, Tag>` and `ScopeMiddleware` wired into a real
//! (small) `Route`, driven via `tower::ServiceExt::oneshot` — same style as
//! `tests/feature/trim_strings.rs`.
//!
//! `Container::install_global` is a `OnceLock` (like `Config`/`Database`) —
//! settable only **once per test binary**, not once per file. Every file
//! under `tests/feature/` compiles into the same `feature` test binary, so
//! [`ensure_global`] must be the **one** function in the whole binary that
//! ever calls `Container::install_global` — it builds on top of the real
//! `kolas::app::providers::all(...)` wiring (so `dyn Logger`, resolved by
//! `hello_world_controller.rs`, is present) plus every fixture the tests in
//! *this* file need: an untagged `Greeter`, a `Repo` tagged
//! `"primary"`/`"secondary"`, a `PartialRepo` tagged only `"primary"` (so a
//! "missing tag" request has something to fail on), a `Scoped`
//! `RequestScoped`, and no registration at all for `Missing` (so the
//! "missing registration" test has something unregistered to ask for). Any
//! other file in this binary that needs the container calls this same
//! `ensure_global` (see `tests/feature/hello_world_controller.rs`) instead of
//! building its own — two different containers racing for the same
//! `OnceLock` would make the loser's registrations silently vanish. Each
//! test still builds its **own local** `Route`/router (with or without
//! `ScopeMiddleware`, as the scenario needs).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Json;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use kolas::app::providers;
use kolas::framework::di::{
    Container, ContainerBuilder, Inject, InjectAll, ScopeMiddleware, TagMarker,
};
use kolas::framework::routing::Route;
use serde::Serialize;
use tower::ServiceExt;

// --- shared global container (OQ3) ------------------------------------------

trait Greeter: Send + Sync {
    fn greet(&self) -> String;
}
struct FakeGreeter;
impl Greeter for FakeGreeter {
    fn greet(&self) -> String {
        "Hello".to_string()
    }
}

trait Repo: Send + Sync {
    fn label(&self) -> &'static str;
}
struct FakeRepo(&'static str);
impl Repo for FakeRepo {
    fn label(&self) -> &'static str {
        self.0
    }
}

/// Registered *only* under `"primary"` — exists purely so a request for
/// `Inject<dyn PartialRepo, Secondary>` has a real, existing trait with a
/// missing tag to fail on (as opposed to a wholly unregistered type, which
/// `Missing` below already covers).
trait PartialRepo: Send + Sync {}
struct FakePartialRepo;
impl PartialRepo for FakePartialRepo {}

/// Never registered in [`ensure_global`] — the target for the "missing
/// registration" scenario.
trait Missing: Send + Sync {}

trait RequestScoped: Send + Sync {
    fn id(&self) -> usize;
}
struct ScopedValue(usize);
impl RequestScoped for ScopedValue {
    fn id(&self) -> usize {
        self.0
    }
}

/// Multibound (`multibind`, not tagged) — several independent
/// implementations collected under `InjectAll<dyn Plugin>`.
trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
}
struct FakePlugin(&'static str);
impl Plugin for FakePlugin {
    fn name(&self) -> &'static str {
        self.0
    }
}

struct Primary;
impl TagMarker for Primary {
    const NAME: Option<&'static str> = Some("primary");
}

struct Secondary;
impl TagMarker for Secondary {
    const NAME: Option<&'static str> = Some("secondary");
}

pub(crate) fn ensure_global() {
    let container = providers::all(ContainerBuilder::new())
        .singleton::<dyn Greeter>(Arc::new(FakeGreeter))
        .singleton_tagged::<dyn Repo>("primary", Arc::new(FakeRepo("primary-repo")))
        .singleton_tagged::<dyn Repo>("secondary", Arc::new(FakeRepo("secondary-repo")))
        .singleton_tagged::<dyn PartialRepo>("primary", Arc::new(FakePartialRepo))
        .scoped::<dyn RequestScoped, _, _>(|_container| async {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            Ok(
                Arc::new(ScopedValue(NEXT_ID.fetch_add(1, Ordering::SeqCst)))
                    as Arc<dyn RequestScoped>,
            )
        })
        .multibind::<dyn Plugin>(Arc::new(FakePlugin("alpha")))
        .multibind::<dyn Plugin>(Arc::new(FakePlugin("beta")))
        .build();
    // Idempotent by design: only the first call across the whole test binary
    // actually installs anything; every later call's `AlreadyInstalled` is
    // expected and ignored.
    let _ = Container::install_global(container);
}

// --- payloads ----------------------------------------------------------------

#[derive(Serialize, serde::Deserialize)]
struct GreetingPayload {
    greeting: String,
}

#[derive(Serialize, serde::Deserialize)]
struct CombinedPayload {
    greeting: String,
    repo: String,
}

#[derive(Serialize, serde::Deserialize)]
struct TaggedPayload {
    primary: String,
    secondary: String,
}

#[derive(Serialize, serde::Deserialize)]
struct ScopedPayload {
    id: usize,
}

#[derive(Serialize, serde::Deserialize)]
struct TwoScopedPayload {
    first: usize,
    second: usize,
}

#[derive(Serialize, serde::Deserialize)]
struct PluginsPayload {
    names: Vec<String>,
}

// --- handlers ------------------------------------------------------------------

async fn greeter_handler(Inject(greeter, ..): Inject<dyn Greeter>) -> Json<GreetingPayload> {
    Json(GreetingPayload {
        greeting: greeter.greet(),
    })
}

async fn missing_handler(Inject(_missing, ..): Inject<dyn Missing>) -> StatusCode {
    StatusCode::OK
}

async fn combined_handler(
    Inject(greeter, ..): Inject<dyn Greeter>,
    Inject(repo, ..): Inject<dyn Repo, Primary>,
) -> Json<CombinedPayload> {
    Json(CombinedPayload {
        greeting: greeter.greet(),
        repo: repo.label().to_string(),
    })
}

async fn tagged_handler(
    Inject(primary, ..): Inject<dyn Repo, Primary>,
    Inject(secondary, ..): Inject<dyn Repo, Secondary>,
) -> Json<TaggedPayload> {
    Json(TaggedPayload {
        primary: primary.label().to_string(),
        secondary: secondary.label().to_string(),
    })
}

async fn partial_repo_missing_tag_handler(
    Inject(_repo, ..): Inject<dyn PartialRepo, Secondary>,
) -> StatusCode {
    StatusCode::OK
}

async fn scoped_handler(Inject(value, ..): Inject<dyn RequestScoped>) -> Json<ScopedPayload> {
    Json(ScopedPayload { id: value.id() })
}

async fn double_scoped_handler(
    Inject(first, ..): Inject<dyn RequestScoped>,
    Inject(second, ..): Inject<dyn RequestScoped>,
) -> Json<TwoScopedPayload> {
    Json(TwoScopedPayload {
        first: first.id(),
        second: second.id(),
    })
}

async fn plugins_handler(InjectAll(plugins, ..): InjectAll<dyn Plugin>) -> Json<PluginsPayload> {
    Json(PluginsPayload {
        names: plugins.iter().map(|p| p.name().to_string()).collect(),
    })
}

// --- helpers -------------------------------------------------------------------

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

async fn body_string(res: axum::response::Response) -> String {
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

// --- 1 ---------------------------------------------------------------------

#[tokio::test]
async fn inject_extractor_resolves_registered_service() {
    ensure_global();
    let app = Route::new().get("/greet", greeter_handler).into_router();

    let res = app.oneshot(get("/greet")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: GreetingPayload = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(body.greeting, "Hello");
}

// --- 2 ---------------------------------------------------------------------

#[tokio::test]
async fn inject_extractor_missing_registration_returns_500() {
    ensure_global();
    let app = Route::new().get("/missing", missing_handler).into_router();

    let res = app.oneshot(get("/missing")).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = body_string(res).await;
    assert!(!body.contains("Missing"));
    assert!(!body.contains("NotRegistered"));
}

// --- 3 ---------------------------------------------------------------------

#[tokio::test]
async fn inject_extractor_does_not_block_on_unrelated_registrations() {
    ensure_global();
    let app = Route::new()
        .get("/combined", combined_handler)
        .into_router();

    let res = app.oneshot(get("/combined")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: CombinedPayload = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(body.greeting, "Hello");
    assert_eq!(body.repo, "primary-repo");
}

// --- 4 ---------------------------------------------------------------------

#[tokio::test]
async fn tagged_inject_resolves_correct_binding() {
    ensure_global();
    let app = Route::new().get("/tagged", tagged_handler).into_router();

    let res = app.oneshot(get("/tagged")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: TaggedPayload = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(body.primary, "primary-repo");
    assert_eq!(body.secondary, "secondary-repo");
}

// --- 5 ---------------------------------------------------------------------

#[tokio::test]
async fn tagged_inject_missing_tag_returns_500_not_panic() {
    ensure_global();
    let app = Route::new()
        .get("/partial", partial_repo_missing_tag_handler)
        .into_router();

    let res = app.oneshot(get("/partial")).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// --- 6 ---------------------------------------------------------------------

#[tokio::test]
async fn scope_middleware_gives_different_instances_across_two_requests() {
    ensure_global();
    let app = Route::new()
        .get("/scoped", scoped_handler)
        .middleware(ScopeMiddleware)
        .into_router();

    let res1 = app.clone().oneshot(get("/scoped")).await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let body1: ScopedPayload = serde_json::from_str(&body_string(res1).await).unwrap();

    let res2 = app.oneshot(get("/scoped")).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let body2: ScopedPayload = serde_json::from_str(&body_string(res2).await).unwrap();

    assert_ne!(body1.id, body2.id);
}

// --- 7 ---------------------------------------------------------------------

#[tokio::test]
async fn scope_middleware_gives_same_instance_within_one_request_with_two_resolves() {
    ensure_global();
    let app = Route::new()
        .get("/double-scoped", double_scoped_handler)
        .middleware(ScopeMiddleware)
        .into_router();

    let res = app.oneshot(get("/double-scoped")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: TwoScopedPayload = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(body.first, body.second);
}

// --- 8 ---------------------------------------------------------------------

#[tokio::test]
async fn resolving_scoped_type_without_scope_middleware_returns_500() {
    ensure_global();
    // Same handler as test 6/7, deliberately without `ScopeMiddleware`.
    let app = Route::new().get("/scoped", scoped_handler).into_router();

    let res = app.oneshot(get("/scoped")).await.unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

// --- 9 ---------------------------------------------------------------------

#[tokio::test]
async fn inject_all_resolves_every_multibound_plugin() {
    ensure_global();
    let app = Route::new().get("/plugins", plugins_handler).into_router();

    let res = app.oneshot(get("/plugins")).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: PluginsPayload = serde_json::from_str(&body_string(res).await).unwrap();
    assert_eq!(body.names, vec!["alpha", "beta"]);
}
