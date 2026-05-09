use crate::bootstrap::server::HttpServer;
use crate::bootstrap::telemetry::Telemetry;
use crate::framework::config::Config;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    Telemetry::init();

    Config::load("config")?.install_global();

    HttpServer::run().await
}
