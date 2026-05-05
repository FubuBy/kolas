pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let app = crate::routes::api::routes();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    println!("Listening on http://{}", listener.local_addr()?);

    axum::serve(listener, app).await?;
    Ok(())
}
