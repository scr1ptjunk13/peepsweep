use axum::Router;
use tokio::signal;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tracing::{info, warn, error};
use alloy::providers::{ProviderBuilder, WsConnect};

// Import all modules
// Remove mod lib; - we use it as a library crate
mod database;
mod indexer;
mod api;
mod calculations;
mod cache;
mod utils;

use peepsweep_backend::*;
use database::Database;
use indexer::Indexer;
use api::AppState;
use cache::CacheManager;
use calculations::{CalculationEngine, pricing::PricingEngine};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .compact()
        .init();

    // Load configuration from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/peepsweep".to_string());
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "wss://eth-mainnet.g.alchemy.com/v2/your-api-key".to_string());
    let bind_address = std::env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    info!("Starting PeepSweep Backend on {}", bind_address);

    // Initialize database
    let db_pool = Database::connect(&database_url).await
        .map_err(|e| format!("Failed to connect to database: {}", e))?;
    Database::migrate(&db_pool).await
        .map_err(|e| format!("Failed to run migrations: {}", e))?;
    info!("✅ Database connected and migrated");

    // Initialize cache manager
    let cache_config = cache::CacheConfig {
        redis_url: redis_url.clone(),
        ..Default::default()
    };
    let cache_manager = Arc::new(CacheManager::new(cache_config).await
        .map_err(|e| format!("Failed to initialize cache: {}", e))?);
    info!("✅ Cache manager initialized");

    // Initialize pricing engine
    let pricing_engine = Arc::new(PricingEngine::new(
        db_pool.clone(),
        cache_manager.clone(),
        None
    ));
    info!("✅ Pricing engine initialized");

    // Initialize calculation engine
    let calculation_engine = Arc::new(CalculationEngine::new(
        db_pool.clone(),
        cache_manager.clone(),
        pricing_engine.clone(),
    ).await.map_err(|e| format!("Failed to initialize calculation engine: {}", e))?);
    info!("✅ Calculation engine initialized");

    // Initialize blockchain provider
    let ws = WsConnect::new(&rpc_url);
    let provider = ProviderBuilder::new().on_ws(ws).await
        .map_err(|e| format!("Failed to connect to RPC: {}", e))?;
    let provider: Arc<dyn alloy::providers::Provider> = Arc::new(provider.boxed());
    info!("✅ Blockchain provider connected");

    // Initialize indexer
    let rpc_urls = vec![rpc_url.clone()];
    let indexer = Arc::new(Indexer::new(
        &rpc_urls,
        db_pool.clone(),
        cache_manager.clone(),
        provider.clone(),
    ).await.map_err(|e| format!("Failed to initialize indexer: {}", e))?);
    info!("✅ Indexer initialized");

    // Create application state
    let app_state = AppState {
        db_pool: db_pool.clone(),
        cache_manager: cache_manager.clone(),
        pricing_engine: pricing_engine.clone(),
        calculation_engine: calculation_engine.clone(),
        indexer: indexer.clone(),
        provider: provider.clone(),
    };

    // Start background services
    start_background_services(app_state.clone()).await?;

    // Build application with middleware
    let app = api::create_app(app_state);

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

async fn start_background_services(app_state: AppState) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("Starting background services...");

    // Start event indexer
    let indexer = app_state.indexer.clone();
    tokio::spawn(async move {
        if let Err(e) = indexer.start().await {
            error!("Event indexer error: {}", e);
        }
    });

    // Start pricing engine background updates
    let pricing_engine = app_state.pricing_engine.clone();
    tokio::spawn(async move {
        if let Err(e) = pricing_engine.start_price_updates().await {
            error!("Pricing engine error: {}", e);
        }
    });

    // Start cache maintenance
    let cache_manager = app_state.cache_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // Every 5 minutes
        loop {
            interval.tick().await;
            // Run cache maintenance tasks
            // Cache cleanup is handled internally by the cache manager
        }
    });

    // Start materialized view refresh
    let db_pool = app_state.db_pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // Every minute
        loop {
            interval.tick().await;
            if let Err(e) = refresh_materialized_views(&db_pool).await {
                warn!("Materialized view refresh error: {}", e);
            }
        }
    });

    // Start health monitoring
    let app_state_clone = app_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30)); // Every 30 seconds
        loop {
            interval.tick().await;
            monitor_system_health(&app_state_clone).await;
        }
    });

    info!("✅ All background services started");
    Ok(())
}

async fn refresh_materialized_views(
    db_pool: &sqlx::PgPool,
) -> std::result::Result<(), sqlx::Error> {
    sqlx::query!("REFRESH MATERIALIZED VIEW CONCURRENTLY user_positions_summary")
        .execute(db_pool)
        .await?;
    
    Ok(())
}

async fn monitor_system_health(app_state: &AppState) {
    // Check database health
    if let Err(e) = Database::health_check(&app_state.db_pool).await {
        error!("Database health check failed: {}", e);
    }

    // Check cache health
    if let Err(e) = app_state.cache_manager.health_check().await {
        error!("Cache health check failed: {}", e);
    }

    // Check indexer status
    let status = app_state.indexer.get_status().await;
    if !status.is_running {
        warn!("Indexer is not running. Events processed: {}", status.events_processed);
    }
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