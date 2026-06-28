use std::path::Path;

use chrono::Utc;

use crate::framework::console::{Args, BoxFuture, Command};
use crate::framework::database::Database;

pub struct MigrationCreateCommand;

impl Command for MigrationCreateCommand {
    fn name(&self) -> &str {
        "migration:create"
    }

    fn description(&self) -> &str {
        "Create a new migration file pair. Usage: migration:create <name> [--connection=<name>]"
    }

    fn execute(&self, args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let name = match args.positional(0) {
                Some(n) => n.to_string(),
                None => return Err("Usage: migration:create <name>".into()),
            };

            // Scaffolding files needs only a directory, not a live connection,
            // so fall back to the global path rather than requiring a default
            // connection when `--connection` is omitted.
            let cfg = Database::global().config();
            let dir = match args.get("connection") {
                Some(connection) => cfg.migrations_path_for(connection),
                None => cfg.migrations_path().to_string(),
            };

            let path = Path::new(&dir);
            if !path.exists() {
                std::fs::create_dir_all(path)?;
            }

            // UTC timestamp `YYYYMMDDHHMMSSmmm` (down to the millisecond). It is
            // a contiguous digit string with no `_`, so sqlx parses the whole
            // prefix as the i64 version, and the chronological ordering matches
            // numeric ordering — no directory scan or shared counter, so two
            // developers branching in parallel don't collide on `0001`.
            let version = Utc::now().format("%Y%m%d%H%M%S%3f").to_string();

            let up = format!("{dir}/{version}_{name}.up.sql");
            let down = format!("{dir}/{version}_{name}.down.sql");

            std::fs::write(&up, "-- Write your UP migration here\n")?;
            std::fs::write(&down, "-- Write your DOWN migration here\n")?;

            println!("Created {up}");
            println!("Created {down}");

            Ok(())
        })
    }
}
