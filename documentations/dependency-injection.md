# Dependency injection

`framework::di` gives applications a way to register dependencies — values or async factories — under a trait or concrete type, and resolve an `Arc<T>` anywhere in the code (controllers, other providers' factories) without threading a pool or config through every call site by hand. The framework core has no opinion about *how* you organize dependencies (ports/adapters, layers, a plain flat list of services) — from `framework::di`'s point of view there is only `T` and `Arc<T>`.

## 1. Register providers under `src/app/providers/`

A provider is a plain function `fn register(builder: ContainerBuilder) -> ContainerBuilder` that binds a trait to an implementation. **A provider file contains wiring only — no trait or `impl` block.** Define the trait and its implementation as ordinary application code, wherever that belongs in your app (a `services/` module, a `domain`/`infra` split for a layered app, or — for something this small — its own top-level file); the provider only imports and binds them. This keeps `src/app/providers/` a flat, readable list of "what is bound to what" instead of a dumping ground for every trait and struct in the app — mixing type definitions into the wiring file is exactly the kind of single-responsibility violation this convention exists to avoid, even though the framework itself has no opinion on where those types live.

```rust
// src/app/greeter.rs — the trait and its implementation, NOT in providers/
pub trait Greeter: Send + Sync {   // Send + Sync is required — see §5
    fn greet(&self) -> String;
}

pub struct EnglishGreeter;
impl Greeter for EnglishGreeter {
    fn greet(&self) -> String { "Hello".to_string() }
}
```

```rust
// src/app/providers/greeter_provider.rs — wiring only
use std::sync::Arc;
use kolas::framework::di::ContainerBuilder;
use crate::app::greeter::{Greeter, EnglishGreeter};

pub fn register(builder: ContainerBuilder) -> ContainerBuilder {
    builder.singleton_factory::<dyn Greeter, _, _>(|_container| async {
        Ok(Arc::new(EnglishGreeter) as Arc<dyn Greeter>)
    })
}
```

Then fold it into `src/app/providers/mod.rs` — the only file that needs to change when a new provider is added, mirroring `app/console/commands/mod.rs`'s `all()`:

```rust
pub mod greeter_provider;

use crate::framework::di::ContainerBuilder;

pub fn all(builder: ContainerBuilder) -> ContainerBuilder {
    greeter_provider::register(builder)
    // additional providers fold in here
}
```

## 2. Bootstrap wiring

Both `bootstrap/app.rs` and `bootstrap/console.rs` build the container from `app::providers::all(...)` and install it as a process-wide singleton, **after** `Database::install_global()` and **before** anything that might resolve a registration:

```rust
Database::install_global()?;
Container::install_global(app::providers::all(ContainerBuilder::new()).build())?;
```

`ContainerBuilder::build()` never executes a factory — same philosophy as `Database::install_global()` never opening a socket. `cargo run` boots even if a provider's downstream dependency (a database, an external service) is currently unavailable — see §6.

## 3. Resolving: the `Container` facade and the `Inject<T, Tag>` extractor

Anywhere in application code:

```rust
let greeter = Container::resolve::<dyn Greeter>().await?;
```

In a controller, `Inject<T, Tag = Untagged>` is sugar over the same call, wired up as an Axum extractor:

```rust
use kolas::framework::di::Inject;

pub async fn hello(Inject(greeter, ..): Inject<dyn Greeter>) -> impl IntoResponse {
    greeter.greet()
}
```

A missing registration, a failed factory, a cyclic dependency, or a missing scope (see §4) never panics — `Container::resolve`/`resolve_tagged` return `Err(DiError)`, and `Inject`'s rejection (`DiRejection`) maps any `DiError` to a `500` with a **fixed, generic body** — the actual error (which can carry internal detail such as connection strings or driver names) is logged server-side via `tracing::error!` and never sent to the client.

### Tagged registrations

When more than one implementation of a trait is registered, disambiguate with a tag — a string at registration time, and a small marker type implementing `TagMarker` at the call site:

```rust
use kolas::framework::di::TagMarker;

pub struct Primary;
impl TagMarker for Primary { const NAME: Option<&'static str> = Some("primary"); }

pub struct Secondary;
impl TagMarker for Secondary { const NAME: Option<&'static str> = Some("secondary"); }
```

```rust
builder
    .singleton_tagged::<dyn Repo>("primary", primary_repo)
    .singleton_tagged::<dyn Repo>("secondary", secondary_repo)
```

```rust
pub async fn merge(
    Inject(primary, ..): Inject<dyn Repo, Primary>,
    Inject(secondary, ..): Inject<dyn Repo, Secondary>,
) -> impl IntoResponse { /* ... */ }
```

Untagged and tagged registrations of the same trait are independent — there is no implicit fallback in either direction.

### Multibinder: collecting several implementations

A tag *selects exactly one* implementation among several — the container gives you back one `Arc<T>`. Sometimes what you actually want is the opposite: register any number of independent implementations of a trait and get **all of them back together**, e.g. a list of validators, exporters, or health-checks that each register themselves independently and run as a batch. That's `multibind`/`multibind_factory`/`multibind_transient`/`multibind_scoped` (optionally `_group`) plus `resolve_all`/`resolve_all_group` — the same pattern as Guice's `Multibinder` or resolving `IEnumerable<TService>` in .NET DI. It is **not** spelled with `tag`/`tagged` anywhere, on purpose — "select one" and "collect all" are different operations and are never expressed with the same word in this API:

Every registration method has a `multibind` counterpart — grouped below by lifecycle, default group first, then the `_group` variant:

```rust
// multibind / multibind_group — already-built values, no factory
builder
    .multibind::<dyn Plugin>(Arc::new(MetricsPlugin))
    .multibind_group::<dyn Plugin>("admin", Arc::new(AuditPlugin));
```

```rust
// multibind_factory / multibind_factory_group — built lazily, cached at most
// once per member (independently of every other member of the same group)
builder
    .multibind_factory::<dyn Plugin, _, _>(|_container| async {
        Ok(Arc::new(MetricsPlugin) as Arc<dyn Plugin>)
    })
    .multibind_factory_group::<dyn Plugin, _, _>("admin", |_container| async {
        Ok(Arc::new(AuditPlugin) as Arc<dyn Plugin>)
    });
```

```rust
// multibind_transient / multibind_transient_group — every member rebuilt on
// every resolve_all/resolve_all_group call
builder
    .multibind_transient::<dyn Plugin, _, _>(|_container| async {
        Ok(Arc::new(EphemeralPlugin::new()) as Arc<dyn Plugin>)
    })
    .multibind_transient_group::<dyn Plugin, _, _>("admin", |_container| async {
        Ok(Arc::new(EphemeralPlugin::new()) as Arc<dyn Plugin>)
    });
```

```rust
// multibind_scoped / multibind_scoped_group — every member built at most
// once per HTTP request, requires ScopeMiddleware (see §4)
builder
    .multibind_scoped::<dyn Plugin, _, _>(|_container| async {
        Ok(Arc::new(RequestScopedPlugin::new()) as Arc<dyn Plugin>)
    })
    .multibind_scoped_group::<dyn Plugin, _, _>("admin", |_container| async {
        Ok(Arc::new(RequestScopedPlugin::new()) as Arc<dyn Plugin>)
    });
```

```rust
let plugins = Container::resolve_all::<dyn Plugin>().await?; // Vec<Arc<dyn Plugin>>, registration order
let admin_plugins = Container::resolve_all_group::<dyn Plugin>("admin").await?;
```

In a controller, `InjectAll<T, Group = Ungrouped>` is the `Multibinder` counterpart of `Inject<T, Tag>` — same shape, deliberately built on its own `GroupMarker` trait rather than reusing `TagMarker`, so a group can never be mistaken for a tag at the type level:

```rust
use kolas::framework::di::InjectAll;

pub async fn run_all(InjectAll(plugins, ..): InjectAll<dyn Plugin>) -> impl IntoResponse {
    for plugin in &plugins { plugin.run(); }
}
```

Named groups (`multibind_group`/`multibind_factory_group`/.../`resolve_all_group`/`InjectAll<T, SomeGroup>` with a `GroupMarker` impl) let several independent collections of the same trait coexist, exactly like tags do for singular registrations — but a group and a tag of the same string never collide with each other, even for the same trait, because they live in separate maps inside `Container`.

Unlike a singular `resolve`, an empty or never-registered multibinding group is **not an error** — `resolve_all`/`resolve_all_group` return `Ok(vec![])`, since a plugin list with zero plugins is a perfectly valid state, not a missing-wiring bug. If any member's factory does fail, that error is returned immediately (the rest of the group is not resolved).

## 4. Three lifecycles

| Method | Built | Notes |
|---|---|---|
| `singleton` / `singleton_tagged` | once, eagerly (value handed in already built) | for cheap value objects, config snapshots, test doubles |
| `singleton_factory` / `singleton_factory_tagged` | once per process, lazily, on first `resolve` | async factory; guaranteed to run at most once even under concurrent first resolves (`tokio::sync::OnceCell`) |
| `transient` / `transient_tagged` | on every `resolve` | no caching — for cheap or deliberately-fresh-every-call constructions |
| `scoped` / `scoped_tagged` | once per HTTP request | requires `ScopeMiddleware` on the route |

Calling code never needs to know which lifecycle a type was registered under — `Container::resolve::<T>()` / `Inject<T, Tag>` behave identically regardless. Every lifecycle also has a Multibinder counterpart (`multibind`, `multibind_factory`, `multibind_transient`, `multibind_scoped`, each with a `_group` variant — see §3) — a member of a multibinding group builds under exactly the same rule its lifecycle describes, independently of every other member of the same group.

An example of every singular registration method, untagged and tagged:

```rust
// singleton / singleton_tagged — already-built value, no factory, nothing lazy
builder
    .singleton::<dyn Repo>(Arc::new(InMemoryRepo::new()))
    .singleton_tagged::<dyn Repo>("secondary", Arc::new(InMemoryRepo::new()));
```

```rust
// singleton_factory / singleton_factory_tagged — built lazily via an async
// factory, cached at most once for the life of the process
builder
    .singleton_factory::<dyn Repo, _, _>(|_container| async {
        let pool = Database::postgres("main")
            .await
            .map_err(DiError::factory_failed::<dyn Repo>)?;
        Ok(Arc::new(PostgresRepo::new(pool)) as Arc<dyn Repo>)
    })
    .singleton_factory_tagged::<dyn Repo, _, _>("secondary", |_container| async {
        let pool = Database::postgres("secondary")
            .await
            .map_err(DiError::factory_failed::<dyn Repo>)?;
        Ok(Arc::new(PostgresRepo::new(pool)) as Arc<dyn Repo>)
    });
```

```rust
// transient / transient_tagged — rebuilt on every resolve, no caching
builder
    .transient::<dyn RequestId, _, _>(|_container| async {
        Ok(Arc::new(RequestId::generate()) as Arc<dyn RequestId>)
    })
    .transient_tagged::<dyn RequestId, _, _>("short", |_container| async {
        Ok(Arc::new(RequestId::short()) as Arc<dyn RequestId>)
    });
```

```rust
// scoped / scoped_tagged — built at most once per HTTP request; requires
// ScopeMiddleware on the route that resolves it (below)
builder
    .scoped::<dyn UnitOfWork, _, _>(|_container| async {
        Ok(Arc::new(UnitOfWork::new()) as Arc<dyn UnitOfWork>)
    })
    .scoped_tagged::<dyn UnitOfWork, _, _>("audit", |_container| async {
        Ok(Arc::new(UnitOfWork::new()) as Arc<dyn UnitOfWork>)
    });
```

**`Scoped` requires `ScopeMiddleware`.** If the application registers at least one `scoped`/`scoped_tagged` type, register the middleware on the route that resolves it:

```rust
Route::new()
    .get("/orders", OrderController::create)
    .middleware(ScopeMiddleware)   // <-- required for Scoped resolution to succeed
    .into_router()
```

Without it, resolving a `Scoped` type (from a request, a console command, or a scheduler task — none of which have an HTTP request scope) returns `Err(DiError::ScopeNotActive)`, never a panic.

**Ordering matters if you combine `ScopeMiddleware` with other app-level middleware.** Per the "first `.middleware()` call is innermost, last is outermost" rule documented for `routes/api.rs`, register `ScopeMiddleware` **last** among your `.middleware(...)` calls if any *other* middleware also needs to resolve a `Scoped` type — otherwise that other middleware runs before the scope is established and its resolve fails with `ScopeNotActive`. (The handler and its extractors don't care about this ordering — they always run deepest, inside every middleware's `next.run(...)`.) See `dev_docs/di/architecture.md`'s "Bootstrap wiring" section for the same note.

**Do not store a Scoped resolve result beyond the current request.** The `Arc<T>` you get back from a Scoped resolve is an ordinary `Arc` — nothing stops you from stashing it in a `static`, a `Singleton`'s field, or a global collection, but doing so quietly turns a per-request value into a de-facto process-wide singleton with state leaking across requests. Scoped is for data that is *semantically* request-bound (a per-request unit-of-work, a request-scoped metrics accumulator, the current authenticated user) — use it only for the duration of the request that resolved it.

**Scope does not follow `tokio::spawn`.** `Scope` propagates via `tokio::task_local!`, which is visible anywhere within the same task's call tree but does **not** cross into a separately spawned task — the same limitation `tracing`'s span context has. If a handler does `tokio::spawn(async move { ... })` and the spawned task needs a Scoped dependency, resolve the value (or values) you need *before* calling `tokio::spawn`, and move the already-resolved `Arc<T>` into the spawned task:

```rust
pub async fn export(Inject(unit_of_work, ..): Inject<dyn UnitOfWork>) -> impl IntoResponse {
    let unit_of_work = Arc::clone(&unit_of_work);
    tokio::spawn(async move {
        // use `unit_of_work` here — it's an ordinary `Arc<T>` now, no
        // ambient scope needed.
    });
    // ...
}
```

Relying on ambient `Container::resolve`/`Inject` *inside* the spawned task will fail with `ScopeNotActive` — there is no public way to read "the scope currently active for this request" from arbitrary code, so resolve-before-spawn is the only supported pattern.

## 5. Traits used with DI need `Send + Sync`

Anything stored behind `Arc<dyn Trait>` and shared across async tasks must satisfy `Send + Sync`. Declare DI-bound traits accordingly:

```rust
pub trait Greeter: Send + Sync {
    fn greet(&self) -> String;
}
```

Forgetting the supertraits surfaces as a compile error at the registration call site (`Arc<dyn Greeter>: Send + Sync` not satisfied) — not a runtime surprise. The same applies to whatever state a factory closure captures: capture `Arc<Mutex<_>>`/`Arc<RwLock<_>>`, not `Rc<RefCell<_>>`.

## 6. Laziness composes with `Database`'s laziness

A provider factory that calls `Database::postgres("main").await` is exactly as lazy as `Database` itself: nothing opens until the *first* resolve of that registration, which itself only happens on first use. This means a clean `cargo run` does **not** prove a registered service's downstream dependency (a database, an external API) actually works — it only proves the wiring compiles and nothing eagerly executed. See [`dev_docs/architecture/database.md`](../dev_docs/architecture/database.md) for the same nuance as it applies to `Database` alone. If you want registration-wiring mistakes caught in CI rather than in production on first use, add an app-owned integration test that builds the real `app::providers::all(...)` and resolves every type your application actually depends on.

## 7. Testing: substituting a fake for a registered trait

Any dependency resolved through the container — not just I/O-bound ones — benefits from being bound to a trait instead of called as a static facade, because a test can register a fake implementation with zero production-code changes. `framework::logging`'s `Logger`/`TracingLogger` (wired up in `src/app/providers/logger_provider.rs`) is a small, realistic example: `Logger` is a trait, `TracingLogger` (the default binding) forwards to the `tracing` macros, and application code depends on `Arc<dyn Logger>` instead of calling `tracing::info!` directly. `Logger` lives in the framework core, next to `Logging` (the subscriber installer it composes with), because it's a generic, reusable abstraction — not something specific to this application's domain; only the *binding* (`logger_provider.rs`) is app-level.

```rust
// tests/unit/logger.rs
use std::sync::{Arc, Mutex};

use kolas::framework::di::ContainerBuilder;
use kolas::framework::logging::Logger;

struct FakeLogger { messages: Arc<Mutex<Vec<String>>> }
impl Logger for FakeLogger {
    fn info(&self, message: &str) {
        self.messages.lock().unwrap().push(format!("INFO {message}"));
    }
    // ...debug/warn/error omitted for brevity
}

let captured = Arc::new(Mutex::new(Vec::new()));
let container = ContainerBuilder::new()
    .singleton::<dyn Logger>(Arc::new(FakeLogger { messages: Arc::clone(&captured) }))
    .build();

let logger = container.resolve_in::<dyn Logger>().await.unwrap();
logger.info("service started");
assert_eq!(&captured.lock().unwrap()[..], &["INFO service started"]);
```

No global state, no `tracing` subscriber to install or intercept, no static method to shadow — the test builds its own small `Container` with only the binding it needs and asserts directly on what the fake captured. This is the same reason `singleton`/`singleton_factory` accept a trait object rather than the framework prescribing one concrete type: production code binds `TracingLogger`, tests bind whatever they need.

[← Back to readme](../readme.md)
