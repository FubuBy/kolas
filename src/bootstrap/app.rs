use crate::bootstrap::server::HttpServer;
use crate::bootstrap::telemetry::Telemetry;
use crate::framework::config::Config;
use crate::framework::database::{Database, migrate};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    Telemetry::init();

    Config::load("config")?.install_global();
    Database::install_global()?;

    if Config::get::<bool>("database.auto_migrate", false) {
        for (connection, path) in Database::global().config().auto_migrate_targets() {
            migrate(&connection, &path).await?;
        }
    }

    HttpServer::run().await
}
