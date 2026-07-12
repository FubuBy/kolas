use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use tokio::sync::OnceCell;

use super::container::Container;
use super::entry::{BoxFuture, Entry, Factory, erase};
use super::error::DiError;
use super::key::ServiceKey;

/// Fluent builder for a [`Container`]. Registrations are collected as a
/// plain list and only frozen into the container's lookup map by
/// [`ContainerBuilder::build`] — which, mirroring `Database::install_global`
/// ("no sockets opened"), never executes a single factory.
///
/// Two independent kinds of registration:
/// - **Singular** (`singleton`/`transient`/`scoped`, optionally `_tagged`):
///   exactly one implementation is bound per `(T, tag)` — a tag *selects*
///   among mutually exclusive alternatives (e.g. a primary vs. a secondary
///   database connection).
/// - **Multibinding** (`multibind`/`multibind_factory`/`multibind_transient`/
///   `multibind_scoped`, optionally `_group`): any number of independent
///   implementations of `T` are collected into a named group and resolved
///   together as a `Vec<Arc<T>>` — the `Multibinder` pattern (see Guice's
///   `Multibinder`/`MapBinder`, or .NET's `IEnumerable<TService>` resolution
///   of several `AddSingleton<TService, Impl>` calls). Deliberately named
///   and typed apart from tags (`multibind*`/`*_group`, never `*_tagged`) so
///   "select one" and "collect all" can never be confused at a call site.
pub struct ContainerBuilder {
    entries: Vec<(ServiceKey, Entry)>,
    multibindings: Vec<(ServiceKey, Entry)>,
}

impl ContainerBuilder {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            multibindings: Vec::new(),
        }
    }

    /// Registers an already-built value as a singleton. No factory, no
    /// async — for values that cost nothing to construct (config snapshots,
    /// plain value objects, test doubles handed in directly by a test).
    pub fn singleton<T>(self, value: Arc<T>) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.with_value(None, value)
    }

    /// Same as [`singleton`](Self::singleton), tagged.
    pub fn singleton_tagged<T>(self, tag: &'static str, value: Arc<T>) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.with_value(Some(tag), value)
    }

    /// Registers a singleton built lazily, at most once for the life of the
    /// process, via an async factory. The factory receives the container
    /// itself (as `Arc<Container>`) so it can resolve other registered
    /// collaborators. Framework facades (`Database`, `Config`) are called
    /// directly inside the factory body, not resolved through the
    /// container — they are pre-existing process-wide facades, not DI
    /// registrations.
    pub fn singleton_factory<T, F, Fut>(self, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_entry::<T, F, Fut>(None, factory, |factory| Entry::Singleton {
            cell: OnceCell::new(),
            factory,
        })
    }

    /// Same as [`singleton_factory`](Self::singleton_factory), tagged.
    pub fn singleton_factory_tagged<T, F, Fut>(self, tag: &'static str, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_entry::<T, F, Fut>(Some(tag), factory, |factory| Entry::Singleton {
            cell: OnceCell::new(),
            factory,
        })
    }

    /// Registers a factory that runs on every `resolve` call — no caching.
    /// For cheap, stateless, or deliberately-fresh-every-call constructions
    /// (a request-id generator, a test double that must not leak state
    /// between calls).
    pub fn transient<T, F, Fut>(self, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_entry::<T, F, Fut>(None, factory, |factory| Entry::Transient { factory })
    }

    /// Same as [`transient`](Self::transient), tagged.
    pub fn transient_tagged<T, F, Fut>(self, tag: &'static str, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_entry::<T, F, Fut>(Some(tag), factory, |factory| Entry::Transient { factory })
    }

    /// Registers a factory that is built once **per HTTP request** and
    /// reused for the rest of that request (see
    /// `dev_docs/di/architecture.md`, "Жизненный цикл scope"). Resolving
    /// outside an active request (no `ScopeMiddleware` in the stack, or
    /// resolving from a console command/scheduler task) fails with
    /// `Err(DiError::ScopeNotActive)`.
    pub fn scoped<T, F, Fut>(self, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_entry::<T, F, Fut>(None, factory, |factory| Entry::Scoped { factory })
    }

    /// Same as [`scoped`](Self::scoped), tagged.
    pub fn scoped_tagged<T, F, Fut>(self, tag: &'static str, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_entry::<T, F, Fut>(Some(tag), factory, |factory| Entry::Scoped { factory })
    }

    /// Adds one member to the default (ungrouped) multibinding set for `T` —
    /// an already-built value, with no factory and no caching decision to
    /// make (there's nothing to build). See the type-level doc comment for
    /// how this differs from a tagged singular registration.
    pub fn multibind<T>(self, value: Arc<T>) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.with_multibind_value(None, value)
    }

    /// Same as [`multibind`](Self::multibind), added to the named `group`
    /// instead of the default group.
    pub fn multibind_group<T>(self, group: &'static str, value: Arc<T>) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.with_multibind_value(Some(group), value)
    }

    /// Adds a member to the default multibinding set for `T`, built lazily
    /// and cached — at most once per process, independently of every other
    /// member of the same group — via `tokio::sync::OnceCell`, exactly like
    /// [`singleton_factory`](Self::singleton_factory).
    pub fn multibind_factory<T, F, Fut>(self, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_multibind_entry::<T, F, Fut>(None, factory, |factory| Entry::Singleton {
            cell: OnceCell::new(),
            factory,
        })
    }

    /// Same as [`multibind_factory`](Self::multibind_factory), added to the
    /// named `group` instead of the default group.
    pub fn multibind_factory_group<T, F, Fut>(self, group: &'static str, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_multibind_entry::<T, F, Fut>(Some(group), factory, |factory| Entry::Singleton {
            cell: OnceCell::new(),
            factory,
        })
    }

    /// Adds a member to the default multibinding set for `T`, rebuilt on
    /// every `resolve_all`/`resolve_all_group` call — no caching, like
    /// [`transient`](Self::transient).
    pub fn multibind_transient<T, F, Fut>(self, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_multibind_entry::<T, F, Fut>(None, factory, |factory| Entry::Transient {
            factory,
        })
    }

    /// Same as [`multibind_transient`](Self::multibind_transient), added to
    /// the named `group` instead of the default group.
    pub fn multibind_transient_group<T, F, Fut>(self, group: &'static str, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_multibind_entry::<T, F, Fut>(Some(group), factory, |factory| Entry::Transient {
            factory,
        })
    }

    /// Adds a member to the default multibinding set for `T`, built at most
    /// once per HTTP request, like [`scoped`](Self::scoped). Every member of
    /// the group gets its own per-request cache slot — resolving the group
    /// twice within one request reuses each member's own built value, not
    /// just the group's.
    pub fn multibind_scoped<T, F, Fut>(self, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_multibind_entry::<T, F, Fut>(None, factory, |factory| Entry::Scoped { factory })
    }

    /// Same as [`multibind_scoped`](Self::multibind_scoped), added to the
    /// named `group` instead of the default group.
    pub fn multibind_scoped_group<T, F, Fut>(self, group: &'static str, factory: F) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        self.with_multibind_entry::<T, F, Fut>(Some(group), factory, |factory| Entry::Scoped {
            factory,
        })
    }

    /// Freezes registrations into a resolvable container. No factory runs
    /// here — building is pure bookkeeping.
    pub fn build(self) -> Arc<Container> {
        let entries: HashMap<ServiceKey, Entry> = self.entries.into_iter().collect();

        let mut groups: HashMap<ServiceKey, Vec<Entry>> = HashMap::new();
        for (key, entry) in self.multibindings {
            groups.entry(key).or_default().push(entry);
        }

        Container::from_entries(entries, groups)
    }

    // === Internals ===

    fn with_value<T>(mut self, tag: Option<&'static str>, value: Arc<T>) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.entries
            .push((ServiceKey::of::<T>(tag), Entry::Value(erase(value))));
        self
    }

    fn with_entry<T, F, Fut>(
        mut self,
        tag: Option<&'static str>,
        factory: F,
        wrap: impl FnOnce(Factory) -> Entry,
    ) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        let erased = erase_factory::<T, F, Fut>(factory);
        self.entries.push((ServiceKey::of::<T>(tag), wrap(erased)));
        self
    }

    fn with_multibind_value<T>(mut self, group: Option<&'static str>, value: Arc<T>) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.multibindings
            .push((ServiceKey::of::<T>(group), Entry::Value(erase(value))));
        self
    }

    fn with_multibind_entry<T, F, Fut>(
        mut self,
        group: Option<&'static str>,
        factory: F,
        wrap: impl FnOnce(Factory) -> Entry,
    ) -> Self
    where
        T: ?Sized + Send + Sync + 'static,
        F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
    {
        let erased = erase_factory::<T, F, Fut>(factory);
        self.multibindings
            .push((ServiceKey::of::<T>(group), wrap(erased)));
        self
    }
}

impl Default for ContainerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Wraps a typed provider factory into the type-erased [`Factory`] shape
/// every [`Entry`] variant stores.
fn erase_factory<T, F, Fut>(factory: F) -> Factory
where
    T: ?Sized + Send + Sync + 'static,
    F: Fn(Arc<Container>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Arc<T>, DiError>> + Send + 'static,
{
    Arc::new(move |container: Arc<Container>| {
        let fut = factory(container);
        Box::pin(async move { fut.await.map(erase::<T>) })
            as BoxFuture<Result<Arc<dyn Any + Send + Sync>, DiError>>
    })
}
