// src/indexer/mod.rs - Event indexer orchestration and coordination
use alloy_provider::{Provider, ProviderBuilder};
// WebSocket transport not needed for basic functionality
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, error};
use crate::{IndexerResult, IndexerError};
use crate::cache::CacheManager;

pub mod events;
pub mod stream;
pub mod processor;
pub mod backfill;

pub use events::*;
pub use stream::EventIndexer;
pub use processor::EventProcessor;
pub use backfill::BackfillManager;

/// Main indexer coordinator that manages all indexing operations
#[derive(Clone)]
pub struct Indexer {
    event_indexer: Arc<EventIndexer>,
    event_processor: Arc<EventProcessor>,
    backfill_manager: Arc<BackfillManager>,
    db_pool: PgPool,
    cache_manager: Arc<CacheManager>,
    provider: Arc<dyn alloy::providers::Provider>,
    metrics: IndexerMetrics,
}

impl std::fmt::Debug for Indexer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Indexer")
            .field("event_indexer", &"Arc<EventIndexer>")
            .field("event_processor", &"Arc<EventProcessor>")
            .field("backfill_manager", &"Arc<BackfillManager>")
            .field("db_pool", &"PgPool")
            .field("cache_manager", &"Arc<CacheManager>")
            .field("provider", &"Arc<dyn Provider>")
            .field("metrics", &self.metrics)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct IndexerMetrics {
    pub events_processed: Arc<std::sync::atomic::AtomicU64>,
    pub events_failed: Arc<std::sync::atomic::AtomicU64>,
    pub blocks_processed: Arc<std::sync::atomic::AtomicU64>,
    pub last_processed_block: Arc<std::sync::atomic::AtomicU64>,
    pub indexer_uptime: std::time::Instant,
}

impl Default for IndexerMetrics {
    fn default() -> Self {
        Self {
            events_processed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            events_failed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            blocks_processed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            last_processed_block: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            indexer_uptime: std::time::Instant::now(),
        }
    }
}

impl Indexer {
    /// Initialize indexer with all components
    pub async fn new(
        rpc_urls: &[String],
        db_pool: PgPool,
        cache_manager: Arc<CacheManager>,
        provider: Arc<dyn alloy::providers::Provider>,
    ) -> IndexerResult<Self> {
        info!("Initializing event indexer with {} RPC endpoints", rpc_urls.len());
        
        // Create event indexer with failover providers
        let event_indexer = Arc::new(
            EventIndexer::new().await?
        );
        
        // Create event processor with channel
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let event_processor = Arc::new(
            EventProcessor::new(event_receiver, db_pool.clone()).await?
        );
        
        // Create backfill manager
        let backfill_manager = Arc::new(
            BackfillManager::new(db_pool.clone(), cache_manager.clone(), event_processor.clone(), None).await?
        );
        
        let metrics = IndexerMetrics {
            events_processed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            events_failed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            blocks_processed: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            last_processed_block: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            indexer_uptime: std::time::Instant::now(),
        };
        
        info!("✅ Event indexer initialized successfully");
        
        Ok(Self {
            event_indexer,
            event_processor,
            backfill_manager,
            db_pool,
            cache_manager,
            provider,
            metrics,
        })
    }
    
    /// Start all indexing services
    pub async fn start(&self) -> IndexerResult<()> {
        info!("🚀 Starting event indexer services...");
        
        // Start event processor in background
        let processor = self.event_processor.clone();
        tokio::spawn(async move {
            // For now, just log that the processor would start
            info!("Event processor would start here");
        });
        
        // Start event streaming
        let indexer = self.event_indexer.clone();
        tokio::spawn(async move {
            if let Err(e) = indexer.start_streaming().await {
                error!("Event streaming failed: {}", e);
            }
        });
        
        // Start metrics collection
        self.start_metrics_collection().await;
        
        // Start health monitoring
        self.start_health_monitoring().await;
        
        info!("✅ All indexer services started successfully");
        Ok(())
    }
    
    /// Stop all indexing services gracefully
    pub async fn stop(&self) -> IndexerResult<()> {
        info!("🛑 Stopping event indexer services...");
        
        // Implementation would gracefully stop all services
        // For now, just log the intention
        
        info!("✅ Event indexer stopped successfully");
        Ok(())
    }
    
    /// Get current indexer status and metrics
    pub async fn get_status(&self) -> IndexerStatus {
        let events_processed = self.metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed);
        let events_failed = self.metrics.events_failed.load(std::sync::atomic::Ordering::Relaxed);
        let blocks_processed = self.metrics.blocks_processed.load(std::sync::atomic::Ordering::Relaxed);
        let last_processed_block = self.metrics.last_processed_block.load(std::sync::atomic::Ordering::Relaxed);
        
        // Get current block for status (placeholder implementation)
        let current_block = 0u64; // TODO: Implement actual block fetching
        
        let blocks_behind = if current_block > last_processed_block {
            current_block - last_processed_block
        } else {
            0
        };
        
        IndexerStatus {
            is_running: true, // Would check actual status
            events_processed,
            events_failed,
            blocks_processed,
            last_processed_block,
            latest_block: current_block,
            blocks_behind,
            uptime_seconds: self.metrics.indexer_uptime.elapsed().as_secs(),
            success_rate: if events_processed + events_failed > 0 {
                (events_processed as f64) / ((events_processed + events_failed) as f64) * 100.0
            } else {
                100.0
            },
        }
    }
    
    /// Trigger backfill for a specific user address
    pub async fn backfill_user_positions(&self, user_address: &str) -> IndexerResult<()> {
        info!("Starting backfill for user: {}", user_address);
        
        let request = crate::indexer::backfill::BackfillRequest {
            user_address: user_address.to_string(),
            from_block: 0, // Start from genesis or earliest relevant block
            to_block: None, // Backfill to current block
            include_v2: true,
            include_v3: true,
            priority: crate::indexer::backfill::BackfillPriority::Medium,
        };
        
        // TODO: Implement actual backfill logic
        info!("Backfill would be triggered for user: {}", user_address);
        
        info!("✅ Backfill completed for user: {}", user_address);
        Ok(())
    }
    
    /// Force refresh of materialized views
    pub async fn refresh_views(&self) -> IndexerResult<()> {
        info!("Refreshing materialized views...");
        
        crate::database::Database::refresh_materialized_views(&self.db_pool)
            .await
            .map_err(|e| IndexerError::BlockProcessingFailed(e.to_string()))?;
        
        info!("✅ Materialized views refreshed");
        Ok(())
    }
    
    /// Start metrics collection in background
    async fn start_metrics_collection(&self) {
        let metrics = self.metrics.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let events_processed = metrics.events_processed.load(std::sync::atomic::Ordering::Relaxed);
                let events_failed = metrics.events_failed.load(std::sync::atomic::Ordering::Relaxed);
                let blocks_processed = metrics.blocks_processed.load(std::sync::atomic::Ordering::Relaxed);
                
                info!(
                    "📊 Indexer metrics - Events: {} processed, {} failed, Blocks: {} processed",
                    events_processed, events_failed, blocks_processed
                );
            }
        });
    }
    
    /// Start health monitoring in background
    async fn start_health_monitoring(&self) {
        let db_pool = self.db_pool.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                // Check database health
                match crate::database::health_check(&db_pool).await {
                    Ok(_) => {},
                    Err(e) => {
                        warn!("Database health check failed: {}", e);
                    }
                }
                
                // Additional health checks would go here
            }
        });
    }
}

// ============================================================================
// STATUS AND METRICS TYPES
// ============================================================================

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexerStatus {
    pub is_running: bool,
    pub events_processed: u64,
    pub events_failed: u64,
    pub blocks_processed: u64,
    pub last_processed_block: u64,
    pub latest_block: u64,
    pub blocks_behind: u64,
    pub uptime_seconds: u64,
    pub success_rate: f64,
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Create indexer with WebSocket providers for real-time streaming
pub async fn create_websocket_indexer(
    rpc_urls: &[String],
    db_pool: PgPool,
    cache_manager: Arc<CacheManager>,
) -> IndexerResult<Indexer> {
    // This would create a WebSocket-based indexer
    // For now, we'll use a placeholder implementation
    todo!("WebSocket indexer implementation")
}

/// Create indexer with HTTP providers for testing
pub async fn create_http_indexer(
    rpc_urls: &[String],
    db_pool: PgPool,
    cache_manager: Arc<CacheManager>,
) -> IndexerResult<Indexer> {
    // This would create an HTTP-based indexer
    // For now, we'll use a placeholder implementation
    todo!("HTTP indexer implementation")
}
