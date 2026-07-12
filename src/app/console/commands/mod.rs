pub mod test_command;

pub use test_command::TestCommand;

use crate::framework::console::Command;
use crate::framework::di::{Container, DiError};
use crate::framework::logging::Logger;

pub async fn all(container: &Container) -> Result<Vec<Box<dyn Command>>, DiError> {
    let logger = container.resolve_in::<dyn Logger>().await?;
    Ok(vec![Box::new(TestCommand::new(logger))])
}
