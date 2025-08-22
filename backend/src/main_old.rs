use axum::Router;
use tokio::signal;
use std::{net::SocketAddr, sync::Arc};
use tracing::{info, error};
use alloy::{
    providers::{ProviderBuilder, Provider},
    primitives::Address,
    transports::http::{Client, Http},
};
use tower_http::cors::CorsLayer;

// Simple modules we need
mod api;
mod utils;

#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<dyn Provider<alloy::transports::http::Http<Client>> + Send + Sync>,
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    // Load configuration from environment
    dotenv::dotenv().ok();
    let rpc_url = std::env::var("ETHEREUM_RPC_URL")
        .unwrap_or_else(|_| "https://eth-mainnet.g.alchemy.com/v2/demo".to_string());
    let bind_address = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    info!("Starting PeepSweep Backend on {}", bind_address);

    // Initialize blockchain provider
    let provider = ProviderBuilder::new()
        .on_http(rpc_url.parse()?)
        .boxed();
    let provider = Arc::new(provider);
    info!("✅ Blockchain provider connected");

    // Create application state
    let app_state = AppState {
        provider,
    };

    // Build application with middleware
    let app = create_app(app_state);

    // Start server
    let addr: SocketAddr = bind_address.parse()
        .map_err(|e| format!("Invalid bind address: {}", e))?;
    let listener = tokio::net::TcpListener::bind(&addr).await
        .map_err(|e| format!("Failed to bind to address: {}", e))?;
    
    info!("🚀 PeepSweep Backend running on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}

fn create_app(app_state: AppState) -> Router {
    Router::new()
        .nest("/api", api::positions::positions_router())
        .layer(CorsLayer::permissive())
        .with_state(app_state)
}


async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Received shutdown signal, starting graceful shutdown...");
}