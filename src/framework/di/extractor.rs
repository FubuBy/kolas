use std::marker::PhantomData;
use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};

use super::container::Container;
use super::error::DiError;

/// Selects which tagged registration `Inject<T, Tag>` resolves. Implemented
/// by small caller-defined unit structs — the *Rust type* at the call site
/// (`Inject<dyn Foo, Primary>` vs `Inject<dyn Foo, Secondary>`) is what
/// disambiguates tags from each other; the tag string itself is still
/// compared at runtime against the container's registrations, exactly like
/// `Container::resolve_tagged`. This makes the call site self-documenting
/// and type-safe about *which* tags can't be confused with each other, but
/// it does not turn a typo in the tag string into a compile error — Rust
/// has no stable `&'static str` const generics yet.
pub trait TagMarker: 'static {
    const NAME: Option<&'static str>;
}

/// Default marker — untagged resolution. `Inject<T>` is sugar for
/// `Inject<T, Untagged>`.
pub struct Untagged;

impl TagMarker for Untagged {
    const NAME: Option<&'static str> = None;
}

/// Resolves `T` from the process-wide [`Container`] as part of Axum's
/// extractor pipeline. Sugar over `Container::resolve::<T>()` /
/// `Container::resolve_tagged::<T>(tag)`, depending on `Tag::NAME`.
/// Handlers that don't need the sugar can call the static facade directly.
///
/// Deviation from `dev_docs/di/api-contracts.md`: the second (`PhantomData`)
/// field is `pub`, not private. A tuple struct with *any* private field
/// cannot be pattern-matched from outside its defining module — even
/// partially via `Inject(value, ..)` — so the doc's own usage example
/// (`Inject(repo): Inject<dyn UserRepository>` in a handler, which lives in
/// a different module from `extractor.rs`) would not compile as a private
/// field. Making the marker field `pub` costs nothing (it carries no data)
/// and is what makes handler-side destructuring actually work.
pub struct Inject<T: ?Sized, Tag: TagMarker = Untagged>(pub Arc<T>, pub PhantomData<Tag>);

impl<T, Tag, S> FromRequestParts<S> for Inject<T, Tag>
where
    T: ?Sized + Send + Sync + 'static,
    Tag: TagMarker,
    S: Send + Sync,
{
    type Rejection = DiRejection;

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let value = match Tag::NAME {
            None => Container::resolve::<T>().await,
            Some(tag) => Container::resolve_tagged::<T>(tag).await,
        }
        .map_err(DiRejection)?;
        Ok(Inject(value, PhantomData))
    }
}

/// Selects which multibinding group `InjectAll<T, Group>` resolves — the
/// `Multibinder` analogue of [`TagMarker`], kept as a **separate** trait
/// (not reusing `TagMarker`) so "tag" (selects exactly one registration
/// among several) and "group" (collects every registration in a named
/// bucket) can never be confused at the type level, either at a call site
/// or when reading `Container`'s API.
pub trait GroupMarker: 'static {
    const NAME: Option<&'static str>;
}

/// Default marker — the ungrouped (default) multibinding set. `InjectAll<T>`
/// is sugar for `InjectAll<T, Ungrouped>`.
pub struct Ungrouped;

impl GroupMarker for Ungrouped {
    const NAME: Option<&'static str> = None;
}

/// Resolves every member of a multibinding group for `T` from the
/// process-wide [`Container`] as part of Axum's extractor pipeline. Sugar
/// over `Container::resolve_all::<T>()` / `Container::resolve_all_group::<T>
/// (group)`, depending on `Group::NAME`. Unlike [`Inject`], a missing or
/// empty group is not an error — `InjectAll` only rejects if a member's
/// factory actually fails (see `Container::resolve_all_in`'s doc comment).
pub struct InjectAll<T: ?Sized, Group: GroupMarker = Ungrouped>(
    pub Vec<Arc<T>>,
    pub PhantomData<Group>,
);

impl<T, Group, S> FromRequestParts<S> for InjectAll<T, Group>
where
    T: ?Sized + Send + Sync + 'static,
    Group: GroupMarker,
    S: Send + Sync,
{
    type Rejection = DiRejection;

    async fn from_request_parts(_parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let values = match Group::NAME {
            None => Container::resolve_all::<T>().await,
            Some(group) => Container::resolve_all_group::<T>(group).await,
        }
        .map_err(DiRejection)?;
        Ok(InjectAll(values, PhantomData))
    }
}

/// Maps any [`DiError`] to `500 Internal Server Error` with a fixed,
/// generic body. A missing/failed registration is a composition bug, not a
/// client error — the concrete `DiError` (which may wrap a downstream
/// error carrying connection strings, driver names, table names) is logged
/// server-side via `tracing::error!` and never sent to the client (see
/// `dev_docs/di/risks.md` R7).
pub struct DiRejection(DiError);

impl IntoResponse for DiRejection {
    fn into_response(self) -> Response {
        tracing::error!(error = %self.0, "DI resolution failed");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
    }
}
