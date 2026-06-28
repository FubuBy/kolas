use crate::framework::console::{Args, BoxFuture, Command};

pub struct TestCommand;

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
            println!("Hello, {name}!");
            Ok(())
        })
    }
}
