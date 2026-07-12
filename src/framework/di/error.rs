/// Errors returned by `Container::resolve` / `resolve_tagged` and friends.
///
/// None of these are ever panics on the resolve/request/command path —
/// a missing registration, a failed factory, a missing scope, etc. are all
/// ordinary `Result`s the caller (or `Inject`'s `DiRejection`) decides how to
/// handle.
#[derive(Debug, thiserror::Error)]
pub enum DiError {
    #[error("no service registered for `{type_name}`{}", tag_suffix(tag))]
    NotRegistered {
        type_name: &'static str,
        tag: Option<String>,
    },

    #[error("circular dependency detected while resolving `{type_name}`: {cycle}")]
    CircularDependency {
        type_name: &'static str,
        cycle: String,
    },

    #[error("factory for `{type_name}` failed: {source}")]
    FactoryFailed {
        type_name: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Resolving a `Scoped` registration outside an active HTTP request —
    /// no `ScopeMiddleware` in the stack (console command, scheduler task,
    /// a test without a manually-entered scope), or resolving from code
    /// running inside a separate `tokio::spawn`ed task, where the task-local
    /// does not propagate (see `dev_docs/di/risks.md` R11).
    #[error(
        "cannot resolve `{type_name}` as a Scoped service outside an active \
         request scope — register `ScopeMiddleware` on the route, or \
         resolve this type only from within a request"
    )]
    ScopeNotActive { type_name: &'static str },

    #[error("DI container global singleton was already installed")]
    AlreadyInstalled,
}

impl DiError {
    /// Convenience for provider factories: wraps any downstream error (e.g.
    /// a `DatabaseError`) into `DiError::FactoryFailed`, filling in `T`'s
    /// type name automatically via `std::any::type_name::<T>()`.
    pub fn factory_failed<T: ?Sized + 'static>(
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::FactoryFailed {
            type_name: std::any::type_name::<T>(),
            source: Box::new(source),
        }
    }

    /// Convenience for `Container::resolve`'s internal handling of
    /// `Entry::Scoped` when there is no active `CURRENT_SCOPE`.
    pub(crate) fn scope_not_active<T: ?Sized + 'static>() -> Self {
        Self::ScopeNotActive {
            type_name: std::any::type_name::<T>(),
        }
    }
}

fn tag_suffix(tag: &Option<String>) -> String {
    match tag {
        Some(tag) => format!(" (tag: `{tag}`)"),
        None => String::new(),
    }
}
