use crate::framework::console::{Args, BoxFuture, Command};
use crate::framework::database::{Database, migrate};

pub struct MigrationMigrateCommand;

impl Command for MigrationMigrateCommand {
    fn name(&self) -> &str {
        "migration:migrate"
    }

    fn description(&self) -> &str {
        "Run pending migrations. Usage: migration:migrate [--connection=<name>]"
    }

    fn execute(&self, args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let cfg = Database::global().config();
            let connection = cfg.resolve_connection(args.get("connection"))?;
            let path = cfg.migrations_path_for(&connection);
            migrate(&connection, &path).await.map_err(Into::into)
        })
    }
}
