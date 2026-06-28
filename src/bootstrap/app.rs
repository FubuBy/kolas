use crate::bootstrap::server::HttpServer;
use crate::bootstrap::telemetry::Telemetry;
use crate::framework::config::Config;
use crate::framework::database::{DEFAULT_MIGRATIONS_PATH, Database, migrate_default};

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    Telemetry::init();

    Config::load("config")?.install_global();
    Database::install_global()?;

    if Config::get::<bool>("database.auto_migrate", false) {
        let path: String = Config::get(
            "database.migrations_path",
            DEFAULT_MIGRATIONS_PATH.to_string(),
        );
        migrate_default(&path).await?;
    }

    HttpServer::run().await
}
