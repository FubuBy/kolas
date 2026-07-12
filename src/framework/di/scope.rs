use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use tokio::sync::{OnceCell, RwLock};

use super::container::Container;
use super::entry::{Factory, downcast};
use super::error::DiError;
use super::key::ServiceKey;

tokio::task_local! {
    /// The `Scope` for the HTTP request currently being handled by this
    /// task. Set by [`super::scope_middleware::ScopeMiddleware`] for the
    /// duration of `next.run(request)` and cleared automatically (dropped)
    /// when that future completes — plain RAII over a `tokio::task_local!`.
    ///
    /// Does not propagate across `tokio::spawn` — see `dev_docs/di/risks.md`
    /// R11.
    pub(crate) static CURRENT_SCOPE: Arc<Scope>;
}

/// Per-slot build guard: one `OnceCell` per `ServiceKey`, so a concurrent
/// resolve of the *same* key within one scope still builds exactly once.
type Slot = Arc<OnceCell<Arc<dyn Any + Send + Sync>>>;

/// Cache of `Scoped` values for a single HTTP request. Created by
/// `ScopeMiddleware` at the start of request handling, dropped — along with
/// everything cached inside it — when the request finishes.
pub struct Scope {
    cache: RwLock<HashMap<ServiceKey, Slot>>,
}

impl Scope {
    pub fn new() -> Arc<Scope> {
        Arc::new(Scope {
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// Runs `fut` with `self` installed as [`CURRENT_SCOPE`] for its
    /// duration — the exact mechanism `ScopeMiddleware` uses internally
    /// (`CURRENT_SCOPE.scope(...)`).
    ///
    /// Deliberately `pub(crate)`, not `pub` — `02-plan.md`'s "Out of scope"
    /// section is explicit: "no manual scope entry API beyond `Scope::new`
    /// used by `ScopeMiddleware`". Making this public would let application
    /// handler code call `Scope::new().enter(fut)` mid-request, silently
    /// establishing a second, disconnected scope that shadows the one
    /// `ScopeMiddleware` already installed for that request, with no error
    /// — exactly the footgun that constraint rules out (`06-review.md` S1).
    /// `ScopeMiddleware::handle` is the one production call site; the
    /// `#[cfg(test)] mod tests` below is the other, exercising
    /// `Scoped`-lifecycle resolution without booting Axum — it lives here,
    /// in-crate, rather than in `tests/unit/di.rs` (which only sees the
    /// public `kolas::*` surface and therefore cannot reach a `pub(crate)`
    /// method) for exactly that reason.
    pub(crate) async fn enter<F: Future>(self: &Arc<Self>, fut: F) -> F::Output {
        CURRENT_SCOPE.scope(Arc::clone(self), fut).await
    }

    /// Resolves a `Scoped` registration against this scope's cache: builds
    /// the value at most once per scope, via the same "insert an empty slot
    /// under a short write-lock, then build off-lock through the slot's own
    /// `OnceCell`" pattern `Database` uses for its pool cache (see
    /// `dev_docs/di/risks.md` R12). Not part of the public contract — only
    /// `Container::resolve` calls this when it finds an `Entry::Scoped`.
    pub(crate) async fn resolve_for<T: ?Sized + Send + Sync + 'static>(
        &self,
        key: ServiceKey,
        factory: &Factory,
        container: Arc<Container>,
    ) -> Result<Arc<T>, DiError> {
        let slot = {
            let cache = self.cache.read().await;
            cache.get(&key).cloned()
        };

        let slot = match slot {
            Some(slot) => slot,
            None => {
                let mut cache = self.cache.write().await;
                cache
                    .entry(key)
                    .or_insert_with(|| Arc::new(OnceCell::new()))
                    .clone()
            }
        };

        let any = slot
            .get_or_try_init(|| async { factory(container).await })
            .await?;
        Ok(downcast::<T>(any))
    }
}

#[cfg(test)]
mod tests {
    //! Scope-lifecycle assertions, moved in-crate from `tests/unit/di.rs`
    //! (S1 fix, `06-review.md`): they need [`Scope::enter`], which is
    //! `pub(crate)` on purpose (see its doc comment) and therefore
    //! invisible to `tests/unit/di.rs`, which compiles as a separate crate
    //! seeing only the public `kolas::*` surface. Living here instead
    //! satisfies the same test coverage without reopening the public
    //! manual-scope-entry API the plan explicitly ruled out.
    //!
    //! Test doubles mirror the `Counted`/`CountedValue`/`counted_factory`
    //! pattern used throughout `tests/unit/di.rs`.

    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use crate::framework::di::ContainerBuilder;

    use super::*;

    trait Counted: Send + Sync {
        fn build_id(&self) -> usize;
    }

    struct CountedValue(usize);

    impl Counted for CountedValue {
        fn build_id(&self) -> usize {
            self.0
        }
    }

    type CountedFuture =
        Pin<Box<dyn std::future::Future<Output = Result<Arc<dyn Counted>, DiError>> + Send>>;

    fn counted_factory(
        builds: Arc<AtomicUsize>,
    ) -> impl Fn(Arc<Container>) -> CountedFuture + Send + Sync + 'static {
        move |_container| {
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                let id = builds.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(Arc::new(CountedValue(id)) as Arc<dyn Counted>)
            })
        }
    }

    #[tokio::test]
    async fn scoped_resolve_returns_same_instance_within_one_scope() {
        let builds = Arc::new(AtomicUsize::new(0));
        let container = ContainerBuilder::new()
            .scoped::<dyn Counted, _, _>(counted_factory(Arc::clone(&builds)))
            .build();

        let scope = Scope::new();
        let (first, second) = scope
            .enter(async {
                let first = container.resolve_in::<dyn Counted>().await.unwrap();
                let second = container.resolve_in::<dyn Counted>().await.unwrap();
                (first, second)
            })
            .await;

        assert_eq!(first.build_id(), second.build_id());
        assert_eq!(builds.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn scoped_resolve_returns_different_instances_across_two_scopes() {
        let builds = Arc::new(AtomicUsize::new(0));
        let container = ContainerBuilder::new()
            .scoped::<dyn Counted, _, _>(counted_factory(Arc::clone(&builds)))
            .build();

        let scope_a = Scope::new();
        let first = scope_a
            .enter(async { container.resolve_in::<dyn Counted>().await.unwrap() })
            .await;

        let scope_b = Scope::new();
        let second = scope_b
            .enter(async { container.resolve_in::<dyn Counted>().await.unwrap() })
            .await;

        assert_ne!(first.build_id(), second.build_id());
        assert_eq!(builds.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn concurrent_scoped_resolve_within_one_scope_builds_once() {
        let builds = Arc::new(AtomicUsize::new(0));
        let container = ContainerBuilder::new()
            .scoped::<dyn Counted, _, _>(move |_container| {
                let builds = Arc::clone(&builds);
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    let id = builds.fetch_add(1, Ordering::SeqCst) + 1;
                    Ok(Arc::new(CountedValue(id)) as Arc<dyn Counted>)
                })
            })
            .build();

        let scope = Scope::new();
        let (first, second) = scope
            .enter(async {
                tokio::join!(
                    container.resolve_in::<dyn Counted>(),
                    container.resolve_in::<dyn Counted>(),
                )
            })
            .await;

        let first = first.unwrap();
        let second = second.unwrap();
        assert_eq!(first.build_id(), second.build_id());
    }
}
