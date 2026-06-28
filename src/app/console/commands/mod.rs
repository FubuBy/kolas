pub mod test_command;

pub use test_command::TestCommand;

use crate::framework::console::Command;

pub fn all() -> Vec<Box<dyn Command>> {
    vec![Box::new(TestCommand)]
}
