use axum::{routing::get, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use std::net::SocketAddr;

pub async fn run(addr: SocketAddr, handle: PrometheusHandle) -> anyhow::Result<()> {
    axum::serve(
        tokio::net::TcpListener::bind(addr).await?,
        Router::new()
            .route("/metrics", get(move || async move { handle.render() }))
            .route("/health", get(|| async { "ok" })),
    )
    .await?;
    Ok(())
}