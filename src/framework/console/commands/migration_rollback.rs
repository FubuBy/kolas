use crate::framework::console::{Args, BoxFuture, Command};
use crate::framework::database::{Database, rollback};

pub struct MigrationRollbackCommand;

impl Command for MigrationRollbackCommand {
    fn name(&self) -> &str {
        "migration:rollback"
    }

    fn description(&self) -> &str {
        "Roll back the last applied migration. Usage: migration:rollback [--connection=<name>]"
    }

    fn execute(&self, args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let cfg = Database::global().config();
            let connection = cfg.resolve_connection(args.get("connection"))?;
            let path = cfg.migrations_path_for(&connection);
            rollback(&connection, &path).await.map_err(Into::into)
        })
    }
}
