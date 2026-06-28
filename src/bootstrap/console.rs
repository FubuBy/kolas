use crate::app::console::commands as app_commands;
use crate::bootstrap::server::HttpServer;
use crate::bootstrap::telemetry::Telemetry;
use crate::framework::config::Config;
use crate::framework::console::commands as framework_commands;
use crate::framework::console::{Args, BoxFuture, Command, ConsoleKernel};
use crate::framework::database::Database;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    Telemetry::init();
    Config::load("config")?.install_global();
    Database::install_global()?;

    ConsoleKernel::new()
        .register(ServeCommand)
        .register_all(framework_commands::all())
        .register_all(app_commands::all())
        .default_command("serve")
        .run()
        .await
}

struct ServeCommand;

impl Command for ServeCommand {
    fn name(&self) -> &str {
        "serve"
    }

    fn description(&self) -> &str {
        "Start the HTTP server"
    }

    fn execute(&self, _args: Args) -> BoxFuture<'_> {
        Box::pin(HttpServer::run())
    }
}
