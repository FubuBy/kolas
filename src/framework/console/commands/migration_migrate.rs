use crate::framework::console::{Args, BoxFuture, Command};
use crate::framework::database::{Database, migrate_default};

pub struct MigrationMigrateCommand;

impl Command for MigrationMigrateCommand {
    fn name(&self) -> &str {
        "migration:migrate"
    }

    fn description(&self) -> &str {
        "Run all pending database migrations"
    }

    fn execute(&self, _args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let path = Database::global().config().migrations_path().to_string();
            migrate_default(&path).await.map_err(Into::into)
        })
    }
}
