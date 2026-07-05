# Add a console command

Console commands live in `src/app/console/commands/`. To add a new command, two steps are needed: create the command file and register it in `src/app/console/commands/mod.rs`. Nothing else needs to be touched.

## 1. Create the command file

Create `src/app/console/commands/<name>_command.rs`. A command is a unit struct that implements the `Command` trait from `kolas::framework::console`. Arguments arrive parsed as an `Args` value — read positional arguments with `args.positional(i)` (zero-based), named arguments with `args.get("key")`, and boolean flags with `args.has("flag")`.

Example: `src/app/console/commands/report_command.rs`

```rust
use kolas::framework::console::{Args, BoxFuture, Command};

pub struct ReportCommand;

impl Command for ReportCommand {
    fn name(&self) -> &str {
        "report"
    }

    fn description(&self) -> &str {
        "Generate a report. Usage: report [period] [user]"
    }

    fn execute(&self, args: Args) -> BoxFuture<'_> {
        Box::pin(async move {
            let period = args.positional(0).unwrap_or("daily");
            let user   = args.positional(1).unwrap_or("all");
            println!("Generating {period} report for {user}…");
            Ok(())
        })
    }
}
```

Run it:

```bash
cargo run -- report              # Generating daily report for all…
cargo run -- report weekly alice # Generating weekly report for alice…
```

## 2. Register the command in `src/app/console/commands/mod.rs`

This is the only file that needs to change. Declare the module, re-export the struct, and add it to the `all()` vector — `bootstrap/console.rs` picks it up automatically from there:

```rust
pub mod test_command;
pub mod report_command;          // <-- add

pub use test_command::TestCommand;
pub use report_command::ReportCommand;   // <-- add

use crate::framework::console::Command;

pub fn all() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(TestCommand),
        Box::new(ReportCommand),  // <-- add
    ]
}
```

## 3. Passing arguments

Everything after the command name is parsed into an `Args` value. The kernel recognizes three shapes:

- `--key=value` — named argument, read with `args.get("key") -> Option<&str>` (use the `=` form; there is no space-separated `--key value`)
- `--flag` — boolean flag, test with `args.has("flag") -> bool`
- bare tokens — positional arguments (in order), read with `args.positional(i) -> Option<&str>`

```rust
fn execute(&self, args: Args) -> BoxFuture<'_> {
    Box::pin(async move {
        let period = args.positional(0).unwrap_or("daily"); // report weekly
        let format = args.get("format").unwrap_or("text");  // --format=json
        let dry_run = args.has("dry-run");                  // --dry-run
        // ...
        Ok(())
    })
}
```

## 4. Built-in commands

| Command | Invocation | Description |
|---|---|---|
| `serve` | `cargo run` or `cargo run -- serve` | Start the HTTP server (default when no command is given) |
| `migration:create` | `cargo run -- migration:create <name> [--connection=<name>]` | Create a new `up`/`down` migration file pair |
| `migration:migrate` | `cargo run -- migration:migrate [--connection=<name>]` | Run all pending migrations for a connection |
| `migration:rollback` | `cargo run -- migration:rollback [--connection=<name>]` | Roll back the last applied migration |
| `help` | `cargo run -- help` | List all registered commands with descriptions |

[← Back to readme](../readme.md)
