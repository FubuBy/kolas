mod migration_create;
mod migration_migrate;
mod migration_rollback;

use migration_create::MigrationCreateCommand;
use migration_migrate::MigrationMigrateCommand;
use migration_rollback::MigrationRollbackCommand;

use crate::framework::console::Command;

pub fn all() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(MigrationCreateCommand),
        Box::new(MigrationMigrateCommand),
        Box::new(MigrationRollbackCommand),
    ]
}
