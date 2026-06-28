use std::path::Path;

use crate::framework::console::{Args, BoxFuture, Command};
use crate::framework::database::Database;

pub struct MigrationCreateCommand;

impl Command for MigrationCreateCommand {
    fn name(&self) -> &str {
        "migration:create"
    }

    fn description(&self) -> &str {
        "Create a new migration file pair. Usage: migration:create <name>"
    }

    fn execute(&self, args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let name = match args.positional(0) {
                Some(n) => n.to_string(),
                None => return Err("Usage: migration:create <name>".into()),
            };

            let dir = Database::global().config().migrations_path().to_string();

            let path = Path::new(&dir);
            if !path.exists() {
                std::fs::create_dir_all(path)?;
            }

            let version = next_version(path)?;
            let prefix = format!("{version:04}");

            let up = format!("{dir}/{prefix}_{name}.up.sql");
            let down = format!("{dir}/{prefix}_{name}.down.sql");

            std::fs::write(&up, "-- Write your UP migration here\n")?;
            std::fs::write(&down, "-- Write your DOWN migration here\n")?;

            println!("Created {up}");
            println!("Created {down}");

            Ok(())
        })
    }
}

fn next_version(dir: &Path) -> Result<u32, Box<dyn std::error::Error>> {
    let mut max = 0;
    for entry in std::fs::read_dir(dir)? {
        // Propagate per-entry IO errors rather than silently skipping them:
        // a dropped entry could hide the highest version and yield a
        // duplicate prefix that sqlx then rejects at migrate time.
        let entry = entry?;
        if let Some(version) = entry
            .file_name()
            .to_string_lossy()
            .split('_')
            .next()
            .and_then(|s| s.parse::<u32>().ok())
        {
            max = max.max(version);
        }
    }
    Ok(max + 1)
}
