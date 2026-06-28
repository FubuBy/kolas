use super::{Args, Command};

pub struct ConsoleKernel {
    commands: Vec<Box<dyn Command>>,
    default_command: Option<String>,
}

impl Default for ConsoleKernel {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleKernel {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            default_command: None,
        }
    }

    pub fn register(mut self, command: impl Command + 'static) -> Self {
        self.commands.push(Box::new(command));
        self
    }

    pub fn register_all(mut self, commands: Vec<Box<dyn Command>>) -> Self {
        self.commands.extend(commands);
        self
    }

    /// Sets the command to run when none is given on the command line.
    /// The name must match a registered command; the framework core does not
    /// assume any particular default.
    pub fn default_command(mut self, name: impl Into<String>) -> Self {
        self.default_command = Some(name.into());
        self
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut raw = std::env::args().skip(1);
        let name = raw
            .next()
            .or_else(|| self.default_command.clone())
            .unwrap_or_else(|| "help".to_string());
        let rest: Vec<String> = raw.collect();

        if name == "list" || name == "help" || name == "--help" || name == "-h" {
            self.print_help();
            return Ok(());
        }

        for cmd in &self.commands {
            if cmd.name() == name {
                return cmd.execute(Args::parse(rest)).await;
            }
        }

        eprintln!("Unknown command: {name}\n");
        self.print_help();
        Err(format!("unknown command: {name}").into())
    }

    fn print_help(&self) {
        println!("Available commands:");
        for cmd in &self.commands {
            println!("  {:<20} {}", cmd.name(), cmd.description());
        }
    }
}
