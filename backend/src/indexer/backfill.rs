use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{sleep, Duration, Instant};
use alloy::{
    providers::Provider,
    rpc::types::{Filter, BlockNumberOrTag, Log},
    primitives::{Address, U256},
};
use sqlx::PgPool;
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

use crate::{IndexerResult, IndexerError};
use crate::database::{queries, models::{PositionV2, PositionV3}};
use crate::indexer::{events::{ProcessedEvent, decode_v2_log, decode_v3_log}, processor::EventProcessor};
use crate::cache::CacheManager;

/// Backfill manager for historical data processing
#[derive(Debug)]
pub struct BackfillManager {
    db_pool: PgPool,
    cache_manager: Arc<CacheManager>,
    event_processor: Arc<EventProcessor>,
    config: BackfillConfig,
    metrics: BackfillMetrics,
    rate_limiter: Arc<Semaphore>,
}

#[derive(Debug, Clone)]
pub struct BackfillConfig {
    pub batch_size: u64,
    pub max_concurrent_batches: usize,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
    pub rate_limit_per_second: u32,
    pub checkpoint_interval: u64,
}

impl Default for BackfillConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            max_concurrent_batches: 5,
            retry_attempts: 3,
            retry_delay_ms: 1000,
            rate_limit_per_second: 10,
            checkpoint_interval: 10000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackfillMetrics {
    pub total_blocks_processed: Arc<RwLock<u64>>,
    pub total_events_processed: Arc<RwLock<u64>>,
    pub failed_batches: Arc<RwLock<u64>>,
    pub start_time: Arc<RwLock<Option<Instant>>>,
    pub last_processed_block: Arc<RwLock<u64>>,
    pub processing_rate: Arc<RwLock<f64>>, // blocks per second
}

impl Default for BackfillMetrics {
    fn default() -> Self {
        Self {
            total_blocks_processed: Arc::new(RwLock::new(0)),
            total_events_processed: Arc::new(RwLock::new(0)),
            failed_batches: Arc::new(RwLock::new(0)),
            start_time: Arc::new(RwLock::new(None)),
            last_processed_block: Arc::new(RwLock::new(0)),
            processing_rate: Arc::new(RwLock::new(0.0)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackfillProgress {
    pub total_blocks: u64,
    pub processed_blocks: u64,
    pub remaining_blocks: u64,
    pub progress_percentage: f64,
    pub estimated_completion: Option<DateTime<Utc>>,
    pub processing_rate: f64,
    pub total_events: u64,
}

#[derive(Debug, Clone)]
pub struct BackfillRequest {
    pub user_address: String,
    pub from_block: u64,
    pub to_block: Option<u64>,
    pub include_v2: bool,
    pub include_v3: bool,
    pub priority: BackfillPriority,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackfillPriority {
    Low,
    Medium,
    High,
}

impl BackfillManager {
    pub async fn new(
        db_pool: PgPool,
        cache_manager: Arc<CacheManager>,
        event_processor: Arc<EventProcessor>,
        config: Option<BackfillConfig>,
    ) -> IndexerResult<Self> {
        let config = config.unwrap_or_default();
        let rate_limiter = Arc::new(Semaphore::new(config.rate_limit_per_second as usize));

        Ok(Self {
            db_pool,
            cache_manager,
            event_processor,
            config,
            metrics: BackfillMetrics::default(),
            rate_limiter,
        })
    }

    /// Start backfill for a specific user
    pub async fn backfill_user_positions<P>(
        &self,
        provider: &P,
        request: BackfillRequest,
    ) -> IndexerResult<BackfillProgress>
    where
        P: Provider + Clone + 'static,
    {
        let start_time = Instant::now();
        *self.metrics.start_time.write().await = Some(start_time);

        let current_block = provider.get_block_number().await
            .map_err(|e| IndexerError::ProviderError(format!("Failed to get current block: {}", e)))?;

        let to_block = request.to_block.unwrap_or(current_block);
        let total_blocks = to_block - request.from_block + 1;

        tracing::info!(
            "Starting backfill for user {} from block {} to {} ({} blocks)",
            request.user_address,
            request.from_block,
            to_block,
            total_blocks
        );

        // Process in batches
        let mut current_block = request.from_block;
        let mut total_events = 0u64;

        while current_block <= to_block {
            let end_block = std::cmp::min(current_block + self.config.batch_size - 1, to_block);
            
            match self.process_batch(provider, &request, current_block, end_block).await {
                Ok(events_count) => {
                    total_events += events_count;
                    *self.metrics.total_blocks_processed.write().await += end_block - current_block + 1;
                    *self.metrics.total_events_processed.write().await += events_count;
                    *self.metrics.last_processed_block.write().await = end_block;

                    // Update processing rate
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let blocks_processed = *self.metrics.total_blocks_processed.read().await;
                    *self.metrics.processing_rate.write().await = blocks_processed as f64 / elapsed;

                    // Checkpoint progress
                    if current_block % self.config.checkpoint_interval == 0 {
                        self.save_checkpoint(&request.user_address, end_block).await?;
                    }
                }
                Err(e) => {
                    *self.metrics.failed_batches.write().await += 1;
                    tracing::error!("Failed to process batch {}-{}: {}", current_block, end_block, e);
                    
                    // Retry with exponential backoff
                    for attempt in 1..=self.config.retry_attempts {
                        sleep(Duration::from_millis(self.config.retry_delay_ms * attempt as u64)).await;
                        
                        if let Ok(events_count) = self.process_batch(provider, &request, current_block, end_block).await {
                            total_events += events_count;
                            break;
                        }
                    }
                }
            }

            current_block = end_block + 1;

            // Rate limiting
            let _permit = self.rate_limiter.acquire().await.unwrap();
            sleep(Duration::from_millis(100)).await; // Small delay between batches
        }

        // Final checkpoint
        self.save_checkpoint(&request.user_address, to_block).await?;

        let progress = self.get_progress(request.from_block, to_block).await;
        tracing::info!("Backfill completed for user {}: {} events processed", request.user_address, total_events);

        Ok(progress)
    }

    /// Process a batch of blocks
    async fn process_batch<P>(
        &self,
        provider: &P,
        request: &BackfillRequest,
        from_block: u64,
        to_block: u64,
    ) -> IndexerResult<u64>
    where
        P: Provider + Clone + 'static,
    {
        let mut total_events = 0u64;

        // Process V2 events if requested
        if request.include_v2 {
            let v2_events = self.fetch_v2_events(provider, &request.user_address, from_block, to_block).await?;
            total_events += v2_events.len() as u64;
            
            for event in v2_events {
                if let Err(e) = self.event_processor.process_event(event).await {
                    tracing::warn!("Failed to process V2 event: {}", e);
                }
            }
        }

        // Process V3 events if requested
        if request.include_v3 {
            let v3_events = self.fetch_v3_events(provider, &request.user_address, from_block, to_block).await?;
            total_events += v3_events.len() as u64;
            
            for event in v3_events {
                if let Err(e) = self.event_processor.process_event(event).await {
                    tracing::warn!("Failed to process V3 event: {}", e);
                }
            }
        }

        tracing::debug!("Processed batch {}-{}: {} events", from_block, to_block, total_events);
        Ok(total_events)
    }

    /// Fetch Uniswap V2 events for a user
    async fn fetch_v2_events<P>(
        &self,
        provider: &P,
        user_address: &str,
        from_block: u64,
        to_block: u64,
    ) -> IndexerResult<Vec<ProcessedEvent>>
    where
        P: Provider + Clone + 'static,
    {
        let user_addr: Address = user_address.parse()
            .map_err(|e| IndexerError::InvalidAddress(format!("Invalid user address: {}", e)))?;

        let filter = Filter::new()
            .from_block(BlockNumberOrTag::Number(from_block))
            .to_block(BlockNumberOrTag::Number(to_block));
            // TODO: Add proper topic filtering for user address
            // .topic1(user_addr); // Filter by user address in indexed parameters

        let logs = provider.get_logs(&filter).await
            .map_err(|e| IndexerError::ProviderError(format!("Failed to fetch V2 logs: {}", e)))?;

        let mut events = Vec::new();
        for log in logs {
            if let Ok(event) = decode_v2_log(&log) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Fetch Uniswap V3 events for a user
    async fn fetch_v3_events<P>(
        &self,
        provider: &P,
        user_address: &str,
        from_block: u64,
        to_block: u64,
    ) -> IndexerResult<Vec<ProcessedEvent>>
    where
        P: Provider + Clone + 'static,
    {
        let user_addr: Address = user_address.parse()
            .map_err(|e| IndexerError::InvalidAddress(format!("Invalid user address: {}", e)))?;

        let filter = Filter::new()
            .from_block(BlockNumberOrTag::Number(from_block))
            .to_block(BlockNumberOrTag::Number(to_block));
            // TODO: Add proper topic filtering for user address
            // .topic1(user_addr); // Filter by user address

        let logs = provider.get_logs(&filter).await
            .map_err(|e| IndexerError::ProviderError(format!("Failed to fetch V3 logs: {}", e)))?;

        let mut events = Vec::new();
        for log in logs {
            if let Ok(event) = decode_v3_log(&log) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Get current backfill progress
    pub async fn get_progress(&self, from_block: u64, to_block: u64) -> BackfillProgress {
        let processed_blocks = *self.metrics.total_blocks_processed.read().await;
        let total_blocks = to_block - from_block + 1;
        let remaining_blocks = total_blocks.saturating_sub(processed_blocks);
        let progress_percentage = (processed_blocks as f64 / total_blocks as f64) * 100.0;
        let processing_rate = *self.metrics.processing_rate.read().await;

        let estimated_completion = if processing_rate > 0.0 && remaining_blocks > 0 {
            let remaining_seconds = remaining_blocks as f64 / processing_rate;
            Some(Utc::now() + chrono::Duration::seconds(remaining_seconds as i64))
        } else {
            None
        };

        BackfillProgress {
            total_blocks,
            processed_blocks,
            remaining_blocks,
            progress_percentage,
            estimated_completion,
            processing_rate,
            total_events: *self.metrics.total_events_processed.read().await,
        }
    }

    /// Save backfill checkpoint
    async fn save_checkpoint(&self, user_address: &str, block_number: u64) -> IndexerResult<()> {
        sqlx::query!(
            "INSERT INTO backfill_checkpoints (user_address, last_processed_block, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (user_address) 
             DO UPDATE SET last_processed_block = $2, updated_at = NOW()",
            user_address,
            block_number as i64
        )
        .execute(&self.db_pool)
        .await
            .map_err(|e| IndexerError::DatabaseError(crate::DatabaseError::QueryError(format!("Failed to save checkpoint: {}", e))))?;

        Ok(())
    }

    /// Load backfill checkpoint
    pub async fn load_checkpoint(&self, user_address: &str) -> IndexerResult<Option<u64>> {
        let result = sqlx::query!(
            "SELECT last_processed_block FROM backfill_checkpoints WHERE user_address = $1",
            user_address
        )
        .fetch_optional(&self.db_pool)
        .await
            .map_err(|e| IndexerError::DatabaseError(crate::DatabaseError::QueryError(format!("Failed to load checkpoint: {}", e))))?;

        Ok(result.map(|row| row.last_processed_block as u64))
    }

    /// Get backfill metrics
    pub async fn get_metrics(&self) -> BackfillMetrics {
        self.metrics.clone()
    }

    /// Reset backfill progress
    pub async fn reset_progress(&self) {
        *self.metrics.total_blocks_processed.write().await = 0;
        *self.metrics.total_events_processed.write().await = 0;
        *self.metrics.failed_batches.write().await = 0;
        *self.metrics.start_time.write().await = None;
        *self.metrics.last_processed_block.write().await = 0;
        *self.metrics.processing_rate.write().await = 0.0;
    }

    /// Cancel ongoing backfill (placeholder for future implementation)
    pub async fn cancel_backfill(&self, user_address: &str) -> IndexerResult<()> {
        tracing::info!("Cancelling backfill for user: {}", user_address);
        // Implementation would involve setting a cancellation flag
        // and gracefully stopping the backfill process
        Ok(())
    }

    /// Estimate backfill duration
    pub async fn estimate_duration(&self, from_block: u64, to_block: u64) -> Duration {
        let total_blocks = to_block - from_block + 1;
        let processing_rate = *self.metrics.processing_rate.read().await;
        
        if processing_rate > 0.0 {
            Duration::from_secs((total_blocks as f64 / processing_rate) as u64)
        } else {
            // Default estimate based on batch size and rate limit
            let batches = (total_blocks + self.config.batch_size - 1) / self.config.batch_size;
            let seconds_per_batch = 1.0 / self.config.rate_limit_per_second as f64;
            Duration::from_secs((batches as f64 * seconds_per_batch) as u64)
        }
    }
}
