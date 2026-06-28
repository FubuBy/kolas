use crate::bootstrap::server::HttpServer;
use crate::framework::config::Config;
use crate::framework::database::{Database, migrate};
use crate::framework::logging::Logging;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    Config::load("config")?.install_global();
    Database::install_global()?;
    let _log_guard = Logging::init()?;

    if Config::get::<bool>("database.auto_migrate", false) {
        for (connection, path) in Database::global().config().auto_migrate_targets() {
            migrate(&connection, &path).await?;
        }
    }

    HttpServer::run().await
}
