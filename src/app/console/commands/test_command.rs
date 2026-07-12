use std::sync::Arc;

use crate::framework::console::{Args, BoxFuture, Command};
use crate::framework::logging::Logger;

pub struct TestCommand {
    logger: Arc<dyn Logger>,
}

impl TestCommand {
    pub fn new(logger: Arc<dyn Logger>) -> Self {
        Self { logger }
    }
}

impl Command for TestCommand {
    fn name(&self) -> &str {
        "test"
    }

    fn description(&self) -> &str {
        "Print a greeting. Usage: test [name]"
    }

    fn execute(&self, args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let name = args.positional(0).unwrap_or("World");
            self.logger.info(&format!("Hello, {name}!"));
            Ok(())
        })
    }
}
