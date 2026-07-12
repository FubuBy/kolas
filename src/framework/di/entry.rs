use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::OnceCell;

use super::container::Container;
use super::error::DiError;

/// A boxed, `'static`, `Send` future — the shape every type-erased factory
/// call returns.
pub(crate) type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// A type-erased provider factory. Takes the container (so it can resolve
/// other registrations to build its own value) and produces an
/// `Arc<dyn Any + Send + Sync>` whose concrete payload is always `Arc<T>`
/// for the `T` the factory was registered under — see [`erase`]/[`downcast`].
pub(crate) type Factory = Arc<
    dyn Fn(Arc<Container>) -> BoxFuture<Result<Arc<dyn Any + Send + Sync>, DiError>> + Send + Sync,
>;

/// A single registration's recipe. Frozen once `ContainerBuilder::build()`
/// runs — no variant is ever added or removed after that point.
pub(crate) enum Entry {
    /// An already-built value, registered via `singleton`/`singleton_tagged`.
    Value(Arc<dyn Any + Send + Sync>),
    /// Built lazily, at most once per process, via `OnceCell::get_or_try_init`.
    Singleton {
        cell: OnceCell<Arc<dyn Any + Send + Sync>>,
        factory: Factory,
    },
    /// Built on every `resolve` call — no caching.
    Transient { factory: Factory },
    /// Built at most once per HTTP request, via the current [`super::scope::Scope`].
    Scoped { factory: Factory },
}

/// Type-erases a freshly-built `Arc<T>` for storage inside an `Entry`.
///
/// `Arc<T>` (for `T: ?Sized + Send + Sync + 'static`) is itself a `Sized`,
/// `'static` value — a fat/thin smart pointer struct — so it satisfies
/// `Any` regardless of whether `T` itself is unsized (e.g. `dyn Trait`).
/// Wrapping it in a fresh `Arc` gives an `Arc<dyn Any + Send + Sync>` whose
/// payload, as far as `Any` is concerned, is exactly `Arc<T>` — recovered by
/// [`downcast`] via `downcast_ref::<Arc<T>>()`.
pub(crate) fn erase<T: ?Sized + Send + Sync + 'static>(
    value: Arc<T>,
) -> Arc<dyn Any + Send + Sync> {
    Arc::new(value)
}

/// Recovers the `Arc<T>` wrapped by [`erase`].
///
/// Panics only if `ServiceKey` matching let a mismatched entry through —
/// an internal invariant violation, not a possible outcome of ordinary
/// registration/resolve usage (the same class of "cannot actually happen"
/// panic as `Container`'s own `self_ref.upgrade().expect(...)`).
///
/// This is the third of exactly three accepted defensive/invariant panics
/// in `framework::di`, alongside `Container::global()`'s panic-on-
/// uninstalled (mirrors `Config`/`Database`'s `global()` contract) and
/// `Container`'s `self_ref.upgrade().expect(...)` (the container cannot be
/// dropped while a resolve is in flight). Given `TypeId` uniqueness, this
/// one is very unlikely to ever fire through any normal registration/
/// resolve path — noted explicitly here so it isn't rediscovered as an
/// undisclosed panic (`06-review.md` N2).
pub(crate) fn downcast<T: ?Sized + Send + Sync + 'static>(
    any: &Arc<dyn Any + Send + Sync>,
) -> Arc<T> {
    any.downcast_ref::<Arc<T>>()
        .cloned()
        .expect("DI internal invariant violated: ServiceKey matched an entry of a different type")
}
