use tokio::net::TcpListener;

use crate::framework::config::Config;

/// HTTP listen + `axum::serve` using the global [`Config`] facade (must be installed before `run`).
pub struct HttpServer;

impl HttpServer {
    pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
        let host: String = Config::get("app.host", "127.0.0.1".to_string());
        let port: u16 = Config::get("app.port", 3000);

        let addr = format!("{host}:{port}");
        let app = crate::routes::api::routes();
        let listener = TcpListener::bind(&addr).await?;

        println!("Listening on http://{}", listener.local_addr()?);

        axum::serve(listener, app).await?;
        Ok(())
    }
}
