use crate::framework::console::{Args, BoxFuture, Command};
use crate::framework::database::{Database, rollback_default};

pub struct MigrationRollbackCommand;

impl Command for MigrationRollbackCommand {
    fn name(&self) -> &str {
        "migration:rollback"
    }

    fn description(&self) -> &str {
        "Roll back the last applied database migration"
    }

    fn execute(&self, _args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let path = Database::global().config().migrations_path().to_string();
            rollback_default(&path).await.map_err(Into::into)
        })
    }
}
