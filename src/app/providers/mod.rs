pub mod logger_provider;

use crate::framework::di::ContainerBuilder;

pub fn all(builder: ContainerBuilder) -> ContainerBuilder {
    logger_provider::register(builder)
    // Additional providers fold in here as the application grows.
}
