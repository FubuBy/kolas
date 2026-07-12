//! Unit tests for `framework::di`: `Container` / `ContainerBuilder` / `Scope`
//! in isolation — no Axum, no `Container::install_global`, no process-wide
//! state. Each test builds its own local `Arc<Container>` via
//! `ContainerBuilder::new()...build()`. Test doubles are hand-written
//! structs implementing the trait under test (`CounterCommand` in
//! `tests/unit/schedule.rs` is the same pattern) — no mock library.
//!
//! Note on naming: the instance resolve methods are `resolve_in` /
//! `resolve_tagged_in`, not `resolve`/`resolve_tagged` — an inherent method
//! and a receiver-less associated function can't share a name in the same
//! `impl` block (E0592), so `Container`'s static facade keeps `resolve` /
//! `resolve_tagged` and the instance API is named `resolve_in` /
//! `resolve_tagged_in`, per the documented fallback in
//! `dev_docs/di/api-contracts.md`.
//!
//! Note on scope-lifecycle coverage: `scoped_resolve_returns_same_instance_
//! within_one_scope`, `scoped_resolve_returns_different_instances_across_
//! two_scopes`, and `concurrent_scoped_resolve_within_one_scope_builds_once`
//! (originally here) were moved into an in-crate `#[cfg(test)] mod tests` in
//! `src/framework/di/scope.rs` (S1 fix, `06-review.md`) — they need
//! `Scope::enter`, which is `pub(crate)`, not `pub`, and this file only sees
//! the public `kolas::*` surface. `scoped_resolve_outside_scope_returns_
//! scope_not_active_error` stays here since it never needs `Scope::enter`.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use kolas::framework::di::{Container, ContainerBuilder, DiError};

/// A named plugin — the fixture used by the Multibinder tests below (§15+).
/// Distinct from `Repo`/`Greeter` so it's never confused with the tagged
/// ("select exactly one") fixtures used by the earlier tests.
trait Plugin: Send + Sync {
    fn name(&self) -> &'static str;
}

struct FakePlugin(&'static str);

impl Plugin for FakePlugin {
    fn name(&self) -> &'static str {
        self.0
    }
}

// --- shared test fixtures ---------------------------------------------------

trait Greeter: Send + Sync {
    fn greet(&self) -> String;
}

struct FakeGreeter(&'static str);

impl Greeter for FakeGreeter {
    fn greet(&self) -> String {
        self.0.to_string()
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

/// A build-counted service: every factory call increments `builds` and
/// returns a fresh instance carrying the count it was built at, so tests can
/// distinguish "same instance" from "rebuilt".
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
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Arc<dyn Counted>, DiError>> + Send>>;

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

trait TraitA: Send + Sync {}
struct AImpl;
impl TraitA for AImpl {}

trait TraitB: Send + Sync {}
struct BImpl;
impl TraitB for BImpl {}

// --- 1 -----------------------------------------------------------------------

#[tokio::test]
async fn resolve_returns_registered_singleton_value() {
    let container = ContainerBuilder::new()
        .singleton::<dyn Greeter>(Arc::new(FakeGreeter("hi")))
        .build();

    let greeter = container.resolve_in::<dyn Greeter>().await.unwrap();
    assert_eq!(greeter.greet(), "hi");
}

// --- 2 -----------------------------------------------------------------------

#[tokio::test]
async fn resolve_unregistered_type_returns_not_registered_error() {
    let container = ContainerBuilder::new().build();

    let err = container.resolve_in::<dyn Greeter>().await.err().unwrap();
    assert!(matches!(err, DiError::NotRegistered { .. }));
    assert!(err.to_string().contains("Greeter"));
}

// --- 3 -----------------------------------------------------------------------

#[tokio::test]
async fn singleton_factory_builds_exactly_once() {
    let builds = Arc::new(AtomicUsize::new(0));
    let container = ContainerBuilder::new()
        .singleton_factory::<dyn Counted, _, _>(counted_factory(Arc::clone(&builds)))
        .build();

    let first = container.resolve_in::<dyn Counted>().await.unwrap();
    let second = container.resolve_in::<dyn Counted>().await.unwrap();

    assert_eq!(first.build_id(), 1);
    assert_eq!(second.build_id(), 1);
    assert_eq!(builds.load(Ordering::SeqCst), 1);
}

// --- 4 -----------------------------------------------------------------------

#[tokio::test]
async fn concurrent_first_resolve_builds_exactly_once() {
    let builds = Arc::new(AtomicUsize::new(0));
    let container = ContainerBuilder::new()
        .singleton_factory::<dyn Counted, _, _>(move |_container| {
            let builds = Arc::clone(&builds);
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(5)).await;
                let id = builds.fetch_add(1, Ordering::SeqCst) + 1;
                Ok(Arc::new(CountedValue(id)) as Arc<dyn Counted>)
            })
        })
        .build();

    let mut handles = Vec::new();
    for _ in 0..20 {
        let container = Arc::clone(&container);
        handles.push(tokio::spawn(async move {
            container
                .resolve_in::<dyn Counted>()
                .await
                .unwrap()
                .build_id()
        }));
    }

    let mut ids = Vec::new();
    for handle in handles {
        ids.push(handle.await.unwrap());
    }

    assert!(ids.iter().all(|id| *id == 1));
}

// --- 5 -----------------------------------------------------------------------

#[tokio::test]
async fn transient_factory_rebuilds_every_resolve() {
    let builds = Arc::new(AtomicUsize::new(0));
    let container = ContainerBuilder::new()
        .transient::<dyn Counted, _, _>(counted_factory(Arc::clone(&builds)))
        .build();

    let first = container.resolve_in::<dyn Counted>().await.unwrap();
    let second = container.resolve_in::<dyn Counted>().await.unwrap();
    let third = container.resolve_in::<dyn Counted>().await.unwrap();

    assert_eq!(first.build_id(), 1);
    assert_eq!(second.build_id(), 2);
    assert_eq!(third.build_id(), 3);
}

// --- 6 -----------------------------------------------------------------------

#[tokio::test]
async fn tagged_registrations_are_independent() {
    let container = ContainerBuilder::new()
        .singleton_tagged::<dyn Repo>("primary", Arc::new(FakeRepo("primary-repo")))
        .singleton_tagged::<dyn Repo>("secondary", Arc::new(FakeRepo("secondary-repo")))
        .build();

    let primary = container
        .resolve_tagged_in::<dyn Repo>("primary")
        .await
        .unwrap();
    let secondary = container
        .resolve_tagged_in::<dyn Repo>("secondary")
        .await
        .unwrap();

    assert_eq!(primary.label(), "primary-repo");
    assert_eq!(secondary.label(), "secondary-repo");
}

// --- 7 -----------------------------------------------------------------------

#[tokio::test]
async fn untagged_resolve_ignores_tagged_registrations() {
    let container = ContainerBuilder::new()
        .singleton_tagged::<dyn Repo>("primary", Arc::new(FakeRepo("primary-repo")))
        .build();

    let err = container.resolve_in::<dyn Repo>().await.err().unwrap();
    assert!(matches!(err, DiError::NotRegistered { .. }));
}

// --- 8 -----------------------------------------------------------------------

#[tokio::test]
async fn circular_dependency_is_detected_not_deadlocked() {
    let container = ContainerBuilder::new()
        .singleton_factory::<dyn TraitA, _, _>(|container| async move {
            let _b = container.resolve_in::<dyn TraitB>().await?;
            Ok(Arc::new(AImpl) as Arc<dyn TraitA>)
        })
        .singleton_factory::<dyn TraitB, _, _>(|container| async move {
            let _a = container.resolve_in::<dyn TraitA>().await?;
            Ok(Arc::new(BImpl) as Arc<dyn TraitB>)
        })
        .build();

    let result = tokio::time::timeout(Duration::from_secs(2), container.resolve_in::<dyn TraitA>())
        .await
        .expect("resolve must not hang");

    assert!(matches!(result, Err(DiError::CircularDependency { .. })));
}

// --- 9 -----------------------------------------------------------------------

#[tokio::test]
async fn factory_error_is_wrapped_with_type_name() {
    let container = ContainerBuilder::new()
        .singleton_factory::<dyn Repo, _, _>(|_container| async {
            let source = io::Error::other("disk on fire");
            Err(DiError::factory_failed::<dyn Repo>(source))
        })
        .build();

    let err = container.resolve_in::<dyn Repo>().await.err().unwrap();
    let message = err.to_string();
    assert!(message.contains("disk on fire"));
    assert!(message.contains("Repo"));
}

// --- 10 ----------------------------------------------------------------------

#[tokio::test]
async fn contains_reports_registration_without_building() {
    let builds = Arc::new(AtomicUsize::new(0));
    let container = ContainerBuilder::new()
        .singleton_factory::<dyn Repo, _, _>({
            let builds = Arc::clone(&builds);
            move |_container| {
                let builds = Arc::clone(&builds);
                Box::pin(async move {
                    builds.fetch_add(1, Ordering::SeqCst);
                    Ok(Arc::new(FakeRepo("built")) as Arc<dyn Repo>)
                })
            }
        })
        .build();

    assert!(container.contains::<dyn Repo>());
    assert_eq!(builds.load(Ordering::SeqCst), 0);
}

// --- 11 ----------------------------------------------------------------------

#[tokio::test]
async fn different_traits_with_same_tag_do_not_collide() {
    let container = ContainerBuilder::new()
        .singleton_tagged::<dyn TraitA>("x", Arc::new(AImpl))
        .singleton_tagged::<dyn TraitB>("x", Arc::new(BImpl))
        .build();

    assert!(container.resolve_tagged_in::<dyn TraitA>("x").await.is_ok());
    assert!(container.resolve_tagged_in::<dyn TraitB>("x").await.is_ok());
}

// --- 12 ----------------------------------------------------------------------

#[tokio::test]
async fn scoped_resolve_outside_scope_returns_scope_not_active_error() {
    let container = ContainerBuilder::new()
        .scoped::<dyn Counted, _, _>(counted_factory(Arc::new(AtomicUsize::new(0))))
        .build();

    let err = container.resolve_in::<dyn Counted>().await.err().unwrap();
    assert!(matches!(err, DiError::ScopeNotActive { .. }));
}

// --- 13 ----------------------------------------------------------------------

/// OQ2: empirically, `tokio::sync::OnceCell::get_or_try_init` leaves the
/// cell uninitialized when the init closure returns `Err` — the `Err` path
/// never calls `set_value`, so the next call retries the factory instead of
/// sticking with the failure forever (confirmed by reading
/// `tokio-1.52.2/src/sync/once_cell.rs`'s `get_or_try_init`).
#[tokio::test]
async fn singleton_factory_error_is_retryable_on_next_resolve() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let container = ContainerBuilder::new()
        .singleton_factory::<dyn Repo, _, _>(move |_container| {
            let attempts = Arc::clone(&attempts);
            Box::pin(async move {
                let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt == 1 {
                    Err(DiError::factory_failed::<dyn Repo>(io::Error::other(
                        "transient failure",
                    )))
                } else {
                    Ok(Arc::new(FakeRepo("recovered")) as Arc<dyn Repo>)
                }
            })
        })
        .build();

    let first = container.resolve_in::<dyn Repo>().await;
    assert!(first.is_err());

    let second = container.resolve_in::<dyn Repo>().await.unwrap();
    assert_eq!(second.label(), "recovered");
}

// --- 14 (M1 regression) -------------------------------------------------------

trait DiamondA: Send + Sync {}
struct DiamondAImpl;
impl DiamondA for DiamondAImpl {}

trait DiamondB: Send + Sync {}
struct DiamondBImpl;
impl DiamondB for DiamondBImpl {}

trait DiamondC: Send + Sync {}
struct DiamondCImpl;
impl DiamondC for DiamondCImpl {}

trait DiamondD: Send + Sync {}
struct DiamondDImpl;
impl DiamondD for DiamondDImpl {}

/// M1 regression (`06-review.md`): `A` resolves `B` and `C` *concurrently*
/// via `tokio::join!`, and both `B` and `C` resolve a shared leaf `D` — an
/// ordinary diamond dependency graph, not a cycle (`D` never references
/// itself or anything above it). Before the fix, the cycle detector's
/// resolving-stack was a single `RefCell<Vec<_>>` shared by both branches;
/// `D`'s factory sleeping just long enough to force the two branches to
/// overlap in time reproduced a **false** `DiError::CircularDependency`
/// (`D -> D`) purely because one branch's push was still on the shared
/// stack when the other branch checked it. After the fix (an immutable
/// chain cloned and rebound per branch instead of one mutable shared cell),
/// this must resolve successfully.
#[tokio::test]
async fn concurrent_sibling_branches_sharing_a_dependency_is_not_a_false_cycle() {
    let container = ContainerBuilder::new()
        .singleton_factory::<dyn DiamondA, _, _>(|container| async move {
            let (b, c) = tokio::join!(
                container.resolve_in::<dyn DiamondB>(),
                container.resolve_in::<dyn DiamondC>(),
            );
            b?;
            c?;
            Ok(Arc::new(DiamondAImpl) as Arc<dyn DiamondA>)
        })
        .singleton_factory::<dyn DiamondB, _, _>(|container| async move {
            let _d = container.resolve_in::<dyn DiamondD>().await?;
            Ok(Arc::new(DiamondBImpl) as Arc<dyn DiamondB>)
        })
        .singleton_factory::<dyn DiamondC, _, _>(|container| async move {
            let _d = container.resolve_in::<dyn DiamondD>().await?;
            Ok(Arc::new(DiamondCImpl) as Arc<dyn DiamondC>)
        })
        .transient::<dyn DiamondD, _, _>(|_container| async {
            // Forces `B`'s and `C`'s resolutions of `D` to overlap in time
            // instead of running start-to-finish back to back — without an
            // await point here, `tokio::join!` would never get a chance to
            // interleave the two branches, and the bug this test guards
            // against wouldn't reproduce.
            tokio::time::sleep(Duration::from_millis(5)).await;
            Ok(Arc::new(DiamondDImpl) as Arc<dyn DiamondD>)
        })
        .build();

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        container.resolve_in::<dyn DiamondA>(),
    )
    .await
    .expect("resolve must not hang");

    assert!(
        result.is_ok(),
        "diamond dependency incorrectly reported as circular: {:?}",
        result.err()
    );
}

// --- 15: Multibinder ---------------------------------------------------------

#[tokio::test]
async fn multibind_resolves_all_registered_members_in_order() {
    let container = ContainerBuilder::new()
        .multibind::<dyn Plugin>(Arc::new(FakePlugin("first")))
        .multibind::<dyn Plugin>(Arc::new(FakePlugin("second")))
        .multibind::<dyn Plugin>(Arc::new(FakePlugin("third")))
        .build();

    let plugins = container.resolve_all_in::<dyn Plugin>().await.unwrap();

    let names: Vec<&str> = plugins.iter().map(|p| p.name()).collect();
    assert_eq!(names, ["first", "second", "third"]);
}

// --- 16 ------------------------------------------------------------------------

#[tokio::test]
async fn multibind_group_is_independent_from_default_group() {
    let container = ContainerBuilder::new()
        .multibind::<dyn Plugin>(Arc::new(FakePlugin("default-a")))
        .multibind_group::<dyn Plugin>("admin", Arc::new(FakePlugin("admin-a")))
        .multibind_group::<dyn Plugin>("admin", Arc::new(FakePlugin("admin-b")))
        .build();

    let default_group = container.resolve_all_in::<dyn Plugin>().await.unwrap();
    let admin_group = container
        .resolve_all_group_in::<dyn Plugin>("admin")
        .await
        .unwrap();

    assert_eq!(
        default_group.iter().map(|p| p.name()).collect::<Vec<_>>(),
        ["default-a"]
    );
    assert_eq!(
        admin_group.iter().map(|p| p.name()).collect::<Vec<_>>(),
        ["admin-a", "admin-b"]
    );
}

// --- 17 ------------------------------------------------------------------------

#[tokio::test]
async fn resolve_all_returns_empty_vec_when_nothing_registered() {
    let container = ContainerBuilder::new().build();

    // Unlike a singular `resolve`, an empty/never-registered multibinding
    // group is a valid outcome (`Ok(vec![])`), not `Err(NotRegistered)` — a
    // plugin list with zero plugins is not a wiring bug.
    let plugins = container.resolve_all_in::<dyn Plugin>().await.unwrap();
    assert!(plugins.is_empty());
}

// --- 18 ------------------------------------------------------------------------

#[tokio::test]
async fn multibind_factory_builds_each_member_at_most_once() {
    let builds_a = Arc::new(AtomicUsize::new(0));
    let builds_b = Arc::new(AtomicUsize::new(0));
    let container = ContainerBuilder::new()
        .multibind_factory::<dyn Counted, _, _>(counted_factory(Arc::clone(&builds_a)))
        .multibind_factory::<dyn Counted, _, _>(counted_factory(Arc::clone(&builds_b)))
        .build();

    let first_pass = container.resolve_all_in::<dyn Counted>().await.unwrap();
    let second_pass = container.resolve_all_in::<dyn Counted>().await.unwrap();

    assert_eq!(builds_a.load(Ordering::SeqCst), 1);
    assert_eq!(builds_b.load(Ordering::SeqCst), 1);
    assert_eq!(
        first_pass.iter().map(|c| c.build_id()).collect::<Vec<_>>(),
        second_pass.iter().map(|c| c.build_id()).collect::<Vec<_>>()
    );
}

// --- 19 ------------------------------------------------------------------------

#[tokio::test]
async fn multibind_transient_rebuilds_every_resolve_all_call() {
    let builds = Arc::new(AtomicUsize::new(0));
    let container = ContainerBuilder::new()
        .multibind_transient::<dyn Counted, _, _>(counted_factory(Arc::clone(&builds)))
        .build();

    container.resolve_all_in::<dyn Counted>().await.unwrap();
    container.resolve_all_in::<dyn Counted>().await.unwrap();

    assert_eq!(builds.load(Ordering::SeqCst), 2);
}

// --- 20 ------------------------------------------------------------------------

/// A member's factory resolving the exact same group again must be reported
/// as `CircularDependency`, not deadlock or overflow the stack — the same
/// guarantee singular `resolve` gives (test §8), extended to group
/// resolution (`resolve_group`'s own chain push in `container.rs`).
#[tokio::test]
async fn multibind_self_referential_group_is_detected_not_deadlocked() {
    let container = ContainerBuilder::new()
        .multibind_factory::<dyn Plugin, _, _>(|container| async move {
            let _siblings = container.resolve_all_in::<dyn Plugin>().await?;
            Ok(Arc::new(FakePlugin("recursive")) as Arc<dyn Plugin>)
        })
        .build();

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        container.resolve_all_in::<dyn Plugin>(),
    )
    .await
    .expect("resolve_all must not hang");

    assert!(matches!(result, Err(DiError::CircularDependency { .. })));
}
