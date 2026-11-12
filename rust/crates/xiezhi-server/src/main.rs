//! Xiezhi webhook API server。

use anyhow::Result;
use std::net::SocketAddr;
use tracing::info;
use xiezhi_api::{app_with_dashboard, spawn_daily_report_scheduler, DashboardState};
use xiezhi_core::VERSION;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_filename("conf/.env");
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let port: u16 = std::env::var("SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5001);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("xiezhi-server {VERSION} listening on {addr}");

    let dashboard = DashboardState::from_env();
    spawn_daily_report_scheduler(dashboard.store.clone());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app_with_dashboard(dashboard)).await?;
    Ok(())
}
