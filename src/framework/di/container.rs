use std::collections::HashMap;
use std::sync::{Arc, OnceLock, Weak};

use super::entry::{Entry, downcast};
use super::error::DiError;
use super::key::ServiceKey;
use super::scope::CURRENT_SCOPE;

static GLOBAL: OnceLock<Arc<Container>> = OnceLock::new();

tokio::task_local! {
    /// The immutable chain of `ServiceKey`s currently being resolved *along
    /// this specific resolution path* — the ordered ancestors that led to
    /// the current `resolve` call (a factory calling `Container::resolve`
    /// for a collaborator counts as an ancestor of that nested call).
    /// Checked before entering any factory — a key already present in the
    /// chain means a genuine cycle, returned as `DiError::CircularDependency`
    /// instead of recursing into a stack overflow or an `OnceCell` deadlock.
    ///
    /// Deliberately **immutable and rebound, not mutated in place**: every
    /// nested `resolve` clones the ambient chain, appends its own key, and
    /// installs the result via `RESOLVING_STACK.scope(new_chain, ...)` for
    /// the duration of *its own* resolve future only — see
    /// `resolve_with_tag`. This matters because two sibling branches
    /// resolved concurrently under a common ancestor factory (e.g.
    /// `tokio::join!(resolve::<B>(), resolve::<C>())` inside a
    /// `singleton_factory`) are polled on the same task: with a single
    /// mutable `RefCell<Vec<_>>` shared by both branches, whichever branch
    /// reached a common leaf dependency second would see it already pushed
    /// by the other and report a **false** `CircularDependency` on an
    /// ordinary diamond dependency graph. Cloning the chain per branch
    /// instead of mutating one shared cell fixes that — see the M1 fix in
    /// `06-review.md` and the
    /// `concurrent_sibling_branches_sharing_a_dependency_is_not_a_false_cycle`
    /// regression test in `tests/unit/di.rs`.
    ///
    /// Logically separate from `CURRENT_SCOPE` (`scope.rs`) — both use the
    /// same Tokio primitive, for different purposes (see
    /// `dev_docs/di/risks.md` R2 vs R11).
    static RESOLVING_STACK: Vec<(ServiceKey, &'static str)>;
}

/// The DI container: a frozen map of registrations, resolved by type
/// (+ optional tag), plus a separate frozen map of **multibindings** —
/// named groups of registrations of the same type, resolved together as a
/// `Vec<Arc<T>>` (see `resolve_all_in`/`resolve_all_group_in`). The two maps
/// are independent: a singular (possibly tagged) registration for `T` and a
/// multibinding group for `T` never collide, even if a tag and a group
/// happen to share the same string. Built once via `ContainerBuilder::build`
/// and never mutated afterward.
pub struct Container {
    self_ref: Weak<Container>,
    entries: HashMap<ServiceKey, Entry>,
    groups: HashMap<ServiceKey, Vec<Entry>>,
}

impl Container {
    /// Builds the `Arc<Container>` self-handle. Only `ContainerBuilder`
    /// calls this — construction (freezing registrations, wiring the
    /// `Weak` self-reference) belongs to the builder's contract, not to
    /// arbitrary code holding a `HashMap`.
    pub(crate) fn from_entries(
        entries: HashMap<ServiceKey, Entry>,
        groups: HashMap<ServiceKey, Vec<Entry>>,
    ) -> Arc<Container> {
        Arc::new_cyclic(|weak| Container {
            self_ref: weak.clone(),
            entries,
            groups,
        })
    }

    // === Instance API — used directly in tests, and by the static facade ===
    //
    // Named `resolve_in`/`resolve_tagged_in` rather than sharing the bare
    // `resolve`/`resolve_tagged` name with the static facade below: an
    // inherent method and a receiver-less associated function with the
    // *same name* in the same `impl` block do not coexist in Rust (E0592,
    // "duplicate definitions") — there is no overloading by receiver.
    // `dev_docs/di/api-contracts.md` calls this out as a "trivial nit" with
    // exactly this documented fallback, so that's what's implemented here.

    /// Resolves the default (untagged) registration for `T`. Behaves
    /// identically regardless of whether `T` was registered as a
    /// `Singleton`, `Transient`, or `Scoped` entry — the lifecycle is
    /// decided at registration time, not at the call site.
    pub async fn resolve_in<T>(&self) -> Result<Arc<T>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.resolve_with_tag::<T>(None).await
    }

    /// Resolves the `tag`-registered entry for `T`.
    pub async fn resolve_tagged_in<T>(&self, tag: &str) -> Result<Arc<T>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.resolve_with_tag::<T>(Some(tag)).await
    }

    /// `true` if a (default, untagged) registration exists for `T`. Never
    /// builds anything — a pure lookup, useful for optional dependencies.
    pub fn contains<T>(&self) -> bool
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.entries.contains_key(&ServiceKey::of::<T>(None))
    }

    /// Resolves every member of the default (ungrouped) **multibinding**
    /// set for `T` — the `Multibinder` pattern (see `ContainerBuilder::
    /// multibind`/`multibind_factory`/`multibind_transient`/
    /// `multibind_scoped`): several independent implementations of the same
    /// trait, collected together, as opposed to a *tag* which selects
    /// exactly one among several. Returns `Ok(vec![])` — never
    /// `Err(DiError::NotRegistered)` — if nothing was ever multibound for
    /// `T`; an empty collection is a valid outcome for a Multibinder-style
    /// registration, not a missing-wiring bug. Members resolve in
    /// registration order. If any member's factory fails, that error is
    /// returned immediately and the rest of the group is not resolved
    /// (fail-fast, mirroring `Iterator::collect::<Result<_, _>>()`).
    pub async fn resolve_all_in<T>(&self) -> Result<Vec<Arc<T>>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.resolve_group::<T>(None).await
    }

    /// Same as [`resolve_all_in`](Self::resolve_all_in), for a named group —
    /// see `ContainerBuilder::multibind_group` and friends. Independent from
    /// the default group and from any tagged/untagged singular registration
    /// of `T`, even if `group` happens to equal a tag string used elsewhere.
    pub async fn resolve_all_group_in<T>(&self, group: &str) -> Result<Vec<Arc<T>>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        self.resolve_group::<T>(Some(group)).await
    }

    // === Static facade — primary entry point for controllers/commands ===

    /// Registers this container as the process-wide singleton. Called once,
    /// in bootstrap, after `Database::install_global()` — factories that
    /// call `Database::*` internally need the DB facade already installed.
    pub fn install_global(container: Arc<Container>) -> Result<(), DiError> {
        GLOBAL.set(container).map_err(|_| DiError::AlreadyInstalled)
    }

    /// Returns the process-wide container. Panics if `install_global` has
    /// not been called yet (same contract as `Config::global()` /
    /// `Database::global()`).
    pub fn global() -> &'static Arc<Container> {
        GLOBAL.get().expect(
            "Container is not initialized; call Container::install_global(...) in bootstrap",
        )
    }

    /// Shortcut for `Container::global().resolve_in::<T>().await`.
    pub async fn resolve<T>() -> Result<Arc<T>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        Self::global().resolve_in::<T>().await
    }

    /// Shortcut for `Container::global().resolve_tagged_in::<T>(tag).await`.
    pub async fn resolve_tagged<T>(tag: &str) -> Result<Arc<T>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        Self::global().resolve_tagged_in::<T>(tag).await
    }

    /// Shortcut for `Container::global().resolve_all_in::<T>().await`.
    pub async fn resolve_all<T>() -> Result<Vec<Arc<T>>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        Self::global().resolve_all_in::<T>().await
    }

    /// Shortcut for `Container::global().resolve_all_group_in::<T>(group).await`.
    pub async fn resolve_all_group<T>(group: &str) -> Result<Vec<Arc<T>>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        Self::global().resolve_all_group_in::<T>(group).await
    }

    // === Internals ===

    fn handle(&self) -> Arc<Container> {
        self.self_ref
            .upgrade()
            .expect("container dropped while resolving")
    }

    async fn resolve_with_tag<T>(&self, tag: Option<&str>) -> Result<Arc<T>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        let key = ServiceKey::of::<T>(tag);
        let type_name = std::any::type_name::<T>();

        // The ambient chain of ancestors on *this* resolution path — empty
        // for a fresh top-level call, or whatever chain an enclosing
        // factory's `resolve` call handed down. Cloned rather than
        // borrowed, so pushing onto it below never mutates whatever value
        // a concurrently-polled sibling branch is holding onto (see the
        // `RESOLVING_STACK` doc comment).
        let ancestors = RESOLVING_STACK
            .try_with(|chain| chain.clone())
            .unwrap_or_default();

        if let Some(cycle) = cycle_through(&ancestors, &key, type_name) {
            return Err(DiError::CircularDependency { type_name, cycle });
        }

        let mut chain = ancestors;
        chain.push((key.clone(), type_name));

        RESOLVING_STACK
            .scope(chain, self.resolve_uncycled::<T>(key, tag))
            .await
    }

    async fn resolve_uncycled<T>(
        &self,
        key: ServiceKey,
        tag: Option<&str>,
    ) -> Result<Arc<T>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        let entry = self
            .entries
            .get(&key)
            .ok_or_else(|| DiError::NotRegistered {
                type_name: std::any::type_name::<T>(),
                tag: tag.map(str::to_string),
            })?;

        self.resolve_entry::<T>(&key, entry).await
    }

    /// Resolves every member of the multibinding group `group` for `T`
    /// (`None` = the default group). Returns `Ok(vec![])` if the group is
    /// empty or was never registered — see `resolve_all_in`'s doc comment
    /// for why that's the correct behavior, unlike a singular `resolve`.
    async fn resolve_group<T>(&self, group: Option<&str>) -> Result<Vec<Arc<T>>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        let group_key = ServiceKey::of::<T>(group);
        let Some(members) = self.groups.get(&group_key) else {
            return Ok(Vec::new());
        };
        let type_name = std::any::type_name::<T>();

        // Same immutable-chain cycle guard as `resolve_with_tag`, pushed
        // once for the whole group — this catches a member's factory
        // trying to `resolve_all`/`resolve_all_group` this exact group
        // again. It does not need to distinguish individual members from
        // each other: a real cycle through *any* member still routes back
        // through this same group key.
        let ancestors = RESOLVING_STACK
            .try_with(|chain| chain.clone())
            .unwrap_or_default();
        if let Some(cycle) = cycle_through(&ancestors, &group_key, type_name) {
            return Err(DiError::CircularDependency { type_name, cycle });
        }
        let mut chain = ancestors;
        chain.push((group_key.clone(), type_name));

        let group_label = group.unwrap_or("");
        RESOLVING_STACK
            .scope(chain, async {
                let mut resolved = Vec::with_capacity(members.len());
                for (index, entry) in members.iter().enumerate() {
                    // Each member needs its own `ServiceKey` distinct from
                    // `group_key` and from every other member — otherwise a
                    // `Scoped` member's per-request cache slot (keyed by
                    // `ServiceKey` in `Scope::resolve_for`) would collide
                    // with its siblings', and all members would resolve to
                    // whichever one's factory won the race. The
                    // `__multibind` prefix makes collision with a
                    // user-chosen tag/group string astronomically unlikely;
                    // even if it happened, it could only affect a Scoped
                    // member's cache identity, never the singular/tagged
                    // resolution path (a completely separate map).
                    let member_key =
                        ServiceKey::of::<T>(Some(&format!("__multibind:{group_label}:{index}")));
                    resolved.push(self.resolve_entry::<T>(&member_key, entry).await?);
                }
                Ok(resolved)
            })
            .await
    }

    /// Dispatches a single already-looked-up [`Entry`] to its lifecycle's
    /// build strategy. Shared by singular resolution (`resolve_uncycled`)
    /// and multibinding resolution (`resolve_group`) — the lifecycle
    /// mechanics (`OnceCell` for `Singleton`, no caching for `Transient`,
    /// `Scope::resolve_for` for `Scoped`) don't care whether `key` came from
    /// a singular registration or a synthesized per-member group key.
    async fn resolve_entry<T>(&self, key: &ServiceKey, entry: &Entry) -> Result<Arc<T>, DiError>
    where
        T: ?Sized + Send + Sync + 'static,
    {
        match entry {
            Entry::Value(any) => Ok(downcast::<T>(any)),
            Entry::Singleton { cell, factory } => {
                let container = self.handle();
                let any = cell
                    .get_or_try_init(|| async { factory(container).await })
                    .await?;
                Ok(downcast::<T>(any))
            }
            Entry::Transient { factory } => {
                let container = self.handle();
                let any = factory(container).await?;
                Ok(downcast::<T>(&any))
            }
            Entry::Scoped { factory } => {
                let scope = CURRENT_SCOPE
                    .try_with(Arc::clone)
                    .map_err(|_| DiError::scope_not_active::<T>())?;
                let container = self.handle();
                scope
                    .resolve_for::<T>(key.clone(), factory, container)
                    .await
            }
        }
    }
}

/// If `key` is already present in `ancestors`, returns the human-readable
/// cycle description (`A -> B -> ... -> key`); otherwise `None`.
fn cycle_through(
    ancestors: &[(ServiceKey, &'static str)],
    key: &ServiceKey,
    type_name: &'static str,
) -> Option<String> {
    if ancestors.iter().any(|(k, _)| k == key) {
        let mut names: Vec<&str> = ancestors.iter().map(|(_, name)| *name).collect();
        names.push(type_name);
        Some(names.join(" -> "))
    } else {
        None
    }
}
