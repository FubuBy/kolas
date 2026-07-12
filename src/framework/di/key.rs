use std::any::TypeId;
use std::sync::Arc;

/// Identifies a registration: the target type (as `Arc<T>`, so a `dyn Trait`
/// target and a concrete-type target use the same mechanism) plus an
/// optional tag for the "several implementations of one trait" case.
///
/// Deviation from `dev_docs/di/api-contracts.md`: `tag` is `Option<String>`
/// here, not `Option<&'static str>`. `Container::resolve_tagged`/
/// `Scope::resolve_for` need to build a lookup key from a caller-supplied
/// `tag: &str` that is not necessarily `'static` (see `api-contracts.md`'s
/// own `resolve_tagged<T>(&self, tag: &str)` signature) — an owned `String`
/// lets every lookup site build a key by value instead of requiring a
/// `'static` borrow (which would force leaking memory per unique tag).
/// `ServiceKey` is `pub(crate)` and explicitly called out in
/// `api-contracts.md` as "not part of the contract", so this internal
/// representation choice does not change any public signature.
#[derive(Clone, Eq, PartialEq, Hash)]
pub(crate) struct ServiceKey {
    type_id: TypeId,
    tag: Option<String>,
}

impl ServiceKey {
    /// Builds the key for `T` under an optional tag. Keyed on
    /// `TypeId::of::<Arc<T>>()` (not `TypeId::of::<T>()`) — this is what
    /// allows `T` to be a `dyn Trait`: `Arc<dyn Trait>` is a concrete,
    /// `Sized`, `'static` type even though `T` itself is unsized, so it can
    /// be stored behind `Arc<dyn Any + Send + Sync>` and recovered again via
    /// `downcast_ref`.
    pub(crate) fn of<T: ?Sized + Send + Sync + 'static>(tag: Option<&str>) -> Self {
        Self {
            type_id: TypeId::of::<Arc<T>>(),
            tag: tag.map(str::to_string),
        }
    }
}
