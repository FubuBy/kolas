use std::sync::Arc;

use crate::framework::di::ContainerBuilder;
use crate::framework::logging::{Logger, TracingLogger};

/// Wiring only — see `crate::framework::logging` for the `Logger` trait and
/// its default `TracingLogger` implementation.
///
/// Registered as a plain `singleton` (not `singleton_factory`): `TracingLogger`
/// has no dependencies and no construction cost, so there's nothing to defer.
pub fn register(builder: ContainerBuilder) -> ContainerBuilder {
    builder.singleton::<dyn Logger>(Arc::new(TracingLogger))
}
