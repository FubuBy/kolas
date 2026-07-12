use std::sync::Arc;

use crate::app;
use crate::app::console::commands as app_commands;
use crate::app::console::schedule as app_schedule;
use crate::bootstrap::server::HttpServer;
use crate::framework::config::Config;
use crate::framework::console::commands as framework_commands;
use crate::framework::console::{Args, BoxFuture, Command, ConsoleKernel};
use crate::framework::database::Database;
use crate::framework::di::{Container, ContainerBuilder};
use crate::framework::logging::Logging;
use crate::framework::schedule::commands::{ScheduleRunCommand, ScheduleWorkCommand};
use crate::framework::schedule::{Schedule, Scheduler};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    Config::load("config")?.install_global();
    Database::install_global()?;
    let container = app::providers::all(ContainerBuilder::new()).build();
    Container::install_global(Arc::clone(&container))?;
    let _log_guard = Logging::init()?;

    // Build the application schedule. The default timezone comes from config;
    // it must be set before registering tasks so each event inherits it.
    let mut schedule = Schedule::new();

    if let Ok(tz) = Config::get("schedule.timezone", "UTC".to_string()).parse() {
        schedule = schedule.with_timezone(tz);
    }

    app_schedule::schedule(&mut schedule);

    let schedule = Arc::new(schedule);
    let scheduler = Arc::new(
        Scheduler::new()
            .with_log_runs(Config::get("schedule.log_runs", true))
            .with_log_overlapping(Config::get("schedule.log_overlapping", true)),
    );

    // A separate kernel holding the commands the scheduler may dispatch.
    let dispatch_kernel = Arc::new(
        ConsoleKernel::new()
            .register_all(framework_commands::all())
            .register_all(app_commands::all(&container).await?),
    );

    // Fail fast on an unknown command name or invalid cron expression.
    schedule.validate(&dispatch_kernel)?;

    ConsoleKernel::new()
        .register(ServeCommand)
        .register(ScheduleRunCommand::new(
            Arc::clone(&schedule),
            Arc::clone(&scheduler),
            Arc::clone(&dispatch_kernel),
        ))
        .register(ScheduleWorkCommand::new(
            Arc::clone(&schedule),
            Arc::clone(&scheduler),
            Arc::clone(&dispatch_kernel),
        ))
        .register_all(framework_commands::all())
        .register_all(app_commands::all(&container).await?)
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
