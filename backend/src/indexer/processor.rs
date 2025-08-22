use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{Duration, Instant};
use rust_decimal::Decimal;
use crate::{Address, U256};

use super::events::{ProcessedEvent, EventData, V2EventData, V3EventData};
use crate::database::{queries, models::{PositionV2, PositionV3}};
use crate::cache::CacheManager;
use crate::{IndexerResult, IndexerError};

#[derive(Debug, Clone)]
pub struct ProcessorMetrics {
    pub events_processed: u64,
    pub events_failed: u64,
    pub processing_time_ms: u64,
    pub last_processed_block: u64,
}

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub retry_attempts: u32,
    pub enable_il_calculation: bool,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            batch_timeout_ms: 1000,
            retry_attempts: 3,
            enable_il_calculation: true,
        }
    }
}

#[derive(Debug)]
pub struct EventProcessor {
    event_receiver: mpsc::UnboundedReceiver<ProcessedEvent>,
    db_pool: sqlx::PgPool,
    cache_manager: Arc<CacheManager>,
    config: ProcessorConfig,
    metrics: Arc<RwLock<ProcessorMetrics>>,
    pending_v2_positions: Arc<RwLock<HashMap<String, PositionV2>>>,
    pending_v3_positions: Arc<RwLock<HashMap<String, PositionV3>>>,
}

impl EventProcessor {
    pub async fn new(
        event_receiver: mpsc::UnboundedReceiver<ProcessedEvent>,
        db_pool: sqlx::PgPool,
    ) -> IndexerResult<Self> {
        let cache_manager = Arc::new(CacheManager::new(crate::cache::CacheConfig::default()).await
            .map_err(|e| IndexerError::ProcessingError(format!("Failed to create cache manager: {}", e)))?);
        
        Ok(Self {
            event_receiver,
            db_pool,
            cache_manager,
            config: ProcessorConfig::default(),
            metrics: Arc::new(RwLock::new(ProcessorMetrics {
                events_processed: 0,
                events_failed: 0,
                processing_time_ms: 0,
                last_processed_block: 0,
            })),
            pending_v2_positions: Arc::new(RwLock::new(HashMap::new())),
            pending_v3_positions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn process_event(&self, event: super::events::ProcessedEvent) -> IndexerResult<()> {
        // TODO: Implement actual event processing
        tracing::info!("Processing event: {:?}", event);
        Ok(())
    }

    pub async fn start_processing(mut self) -> IndexerResult<()> {
        tracing::info!("Starting event processor with config: {:?}", self.config);

        let mut batch_events = Vec::new();
        let mut last_batch_time = Instant::now();

        while let Some(event) = self.event_receiver.recv().await {
            batch_events.push(event);

            // Process batch if we hit size limit or timeout
            let should_process_batch = batch_events.len() >= self.config.batch_size
                || last_batch_time.elapsed() > Duration::from_millis(self.config.batch_timeout_ms);

            if should_process_batch && !batch_events.is_empty() {
                if let Err(e) = self.process_event_batch(&batch_events).await {
                    tracing::error!("Failed to process event batch: {}", e);
                    self.increment_failed_events(batch_events.len() as u64).await;
                } else {
                    self.increment_processed_events(batch_events.len() as u64).await;
                }

                batch_events.clear();
                last_batch_time = Instant::now();
            }
        }

        // Process remaining events
        if !batch_events.is_empty() {
            if let Err(e) = self.process_event_batch(&batch_events).await {
                tracing::error!("Failed to process final event batch: {}", e);
            }
        }

        Ok(())
    }

    async fn process_event_batch(&self, events: &[ProcessedEvent]) -> IndexerResult<()> {
        let start_time = Instant::now();
        
        // Group events by type for efficient batch processing
        let mut v2_events = Vec::new();
        let mut v3_events = Vec::new();

        for event in events {
            match &event.data {
                super::events::EventData::V2(v2_data) => v2_events.push((event, v2_data)),
                super::events::EventData::V3(v3_data) => v3_events.push((event, v3_data)),
                _ => {
                    // Skip other event types for now
                    tracing::debug!("Skipping event type: {:?}", event.data);
                }
            }
        }

        // Process V2 events
        if !v2_events.is_empty() {
            self.process_v2_events(&v2_events).await?;
        }

        // Process V3 events
        if !v3_events.is_empty() {
            self.process_v3_events(&v3_events).await?;
        }

        // Update metrics
        let processing_time = start_time.elapsed().as_millis() as u64;
        self.update_processing_time(processing_time).await;

        // Update last processed block
        if let Some(last_event) = events.last() {
            self.update_last_processed_block(last_event.block_number).await;
        }

        tracing::debug!("Processed batch of {} events in {}ms", events.len(), processing_time);
        Ok(())
    }

    async fn process_v2_events(&self, events: &[(&ProcessedEvent, &V2EventData)]) -> IndexerResult<()> {
        let mut positions_to_upsert = Vec::new();

        for (event, v2_data) in events {
            match self.process_v2_event(event, v2_data).await {
                Ok(Some(position)) => positions_to_upsert.push(position),
                Ok(None) => {}, // Event processed but no position update needed
                Err(e) => {
                    tracing::error!("Failed to process V2 event: {}", e);
                    return Err(e);
                }
            }
        }

        // Batch upsert positions
        if !positions_to_upsert.is_empty() {
            queries::batch_upsert_v2_positions(&self.db_pool, &positions_to_upsert).await
                .map_err(|e| IndexerError::DatabaseError(crate::DatabaseError::QueryError(format!("Failed to batch upsert V2 positions: {}", e))))?;
            
            // Invalidate cache for affected users
            for position in &positions_to_upsert {
                self.cache_manager.invalidate_user_positions(&position.user_address).await;
            }
        }

        Ok(())
    }

    async fn process_v2_event(&self, event: &ProcessedEvent, v2_data: &V2EventData) -> IndexerResult<Option<PositionV2>> {
        match v2_data {
            V2EventData::Mint { sender, amount0, amount1 } => {
                self.handle_v2_mint(event, sender, *amount0, *amount1).await
            },
            V2EventData::Burn { sender, amount0, amount1, to } => {
                self.handle_v2_burn(event, sender, *amount0, *amount1, to).await
            },
            V2EventData::Swap { sender, amount0_in, amount1_in, amount0_out, amount1_out, to } => {
                self.handle_v2_swap(event, sender, *amount0_in, *amount1_in, *amount0_out, *amount1_out, to).await
            },
            V2EventData::Sync { reserve0, reserve1 } => {
                self.handle_v2_sync(event, *reserve0, *reserve1).await
            },
        }
    }

    async fn handle_v2_mint(&self, event: &ProcessedEvent, sender: &Address, amount0: U256, amount1: U256) -> IndexerResult<Option<PositionV2>> {
        let position_key = format!("{}:{}", sender, event.contract_address);
        
        // Get or create position
        let mut pending_positions = self.pending_v2_positions.write().await;
        let mut position = pending_positions.get(&position_key).cloned()
            .unwrap_or_else(|| PositionV2 {
                id: 0, // Will be set by database
                user_address: sender.to_string(),
                pair_address: event.contract_address.to_string(),
                token0: String::new(), // Will be fetched from pair info
                token1: String::new(), // Will be fetched from pair info
                liquidity: Decimal::ZERO,
                token0_amount: u256_to_decimal(amount0),
                token1_amount: u256_to_decimal(amount1),
                block_number: event.block_number as i64,
                transaction_hash: event.transaction_hash.to_string(),
                timestamp: event.timestamp,
                created_at: event.timestamp,
                updated_at: event.timestamp,
                current_il_percentage: None,
                fees_earned_usd: None,
            });

        // Update position with mint data
        position.token0_amount += u256_to_decimal(amount0);
        position.token1_amount += u256_to_decimal(amount1);
        position.updated_at = event.timestamp;
        position.block_number = event.block_number as i64;
        position.transaction_hash = event.transaction_hash.to_string();

        pending_positions.insert(position_key, position.clone());
        Ok(Some(position))
    }

    async fn handle_v2_burn(&self, event: &ProcessedEvent, sender: &Address, amount0: U256, amount1: U256, _to: &Address) -> IndexerResult<Option<PositionV2>> {
        let position_key = format!("{}:{}", sender, event.contract_address);
        
        let mut pending_positions = self.pending_v2_positions.write().await;
        if let Some(mut position) = pending_positions.get(&position_key).cloned() {
            // Update position with burn data (reduce liquidity)
            position.token0_amount = position.token0_amount.saturating_sub(u256_to_decimal(amount0));
            position.token1_amount = position.token1_amount.saturating_sub(u256_to_decimal(amount1));
            position.updated_at = event.timestamp;
            position.block_number = event.block_number as i64;
            position.transaction_hash = event.transaction_hash.to_string();

            pending_positions.insert(position_key, position.clone());
            Ok(Some(position))
        } else {
            // Position not found in pending, might need to fetch from DB
            tracing::warn!("Burn event for unknown position: {}", position_key);
            Ok(None)
        }
    }

    async fn handle_v2_swap(&self, event: &ProcessedEvent, _sender: &Address, _amount0_in: U256, _amount1_in: U256, _amount0_out: U256, _amount1_out: U256, _to: &Address) -> IndexerResult<Option<PositionV2>> {
        // Swap events don't directly affect position amounts but can be used for fee calculation
        // For now, we'll skip direct position updates from swaps
        tracing::debug!("Processing V2 swap event at block {}", event.block_number);
        Ok(None)
    }

    async fn handle_v2_sync(&self, event: &ProcessedEvent, _reserve0: U256, _reserve1: U256) -> IndexerResult<Option<PositionV2>> {
        // Sync events update pool reserves, useful for price calculations
        tracing::debug!("Processing V2 sync event at block {}", event.block_number);
        Ok(None)
    }

    async fn process_v3_events(&self, events: &[(&ProcessedEvent, &V3EventData)]) -> IndexerResult<()> {
        let mut positions_to_upsert = Vec::new();

        for (event, v3_data) in events {
            match self.process_v3_event(event, v3_data).await {
                Ok(Some(position)) => positions_to_upsert.push(position),
                Ok(None) => {},
                Err(e) => {
                    tracing::error!("Failed to process V3 event: {}", e);
                    return Err(e);
                }
            }
        }

        // Batch upsert positions
        if !positions_to_upsert.is_empty() {
            queries::batch_upsert_v3_positions(&self.db_pool, &positions_to_upsert).await
                .map_err(|e| IndexerError::DatabaseError(crate::DatabaseError::QueryError(format!("Failed to batch upsert V3 positions: {}", e))))?;
            
            // Invalidate cache for affected users
            for position in &positions_to_upsert {
                self.cache_manager.invalidate_user_positions(&position.user_address).await;
            }
        }

        Ok(())
    }

    async fn process_v3_event(&self, event: &ProcessedEvent, v3_data: &V3EventData) -> IndexerResult<Option<PositionV3>> {
        match v3_data {
            V3EventData::IncreaseLiquidity { token_id, liquidity, amount0, amount1 } => {
                self.handle_v3_increase_liquidity(event, *token_id, *liquidity, *amount0, *amount1).await
            },
            V3EventData::DecreaseLiquidity { token_id, liquidity, amount0, amount1 } => {
                self.handle_v3_decrease_liquidity(event, *token_id, *liquidity, *amount0, *amount1).await
            },
            V3EventData::Collect { token_id, recipient, amount0, amount1 } => {
                self.handle_v3_collect(event, *token_id, recipient, *amount0, *amount1).await
            },
            V3EventData::Transfer { from, to, token_id } => {
                self.handle_v3_transfer(event, from, to, *token_id).await
            },
        }
    }

    async fn handle_v3_increase_liquidity(&self, event: &ProcessedEvent, token_id: U256, liquidity: U256, amount0: U256, amount1: U256) -> IndexerResult<Option<PositionV3>> {
        let position_key = format!("{}:{}", token_id, event.contract_address);
        
        let mut pending_positions = self.pending_v3_positions.write().await;
        let mut position = pending_positions.get(&position_key).cloned()
            .unwrap_or_else(|| PositionV3 {
                id: 0,
                user_address: String::new(), // Will be updated when we have owner info
                pool_address: event.contract_address.to_string(),
                token_id: token_id.to::<u64>() as i64,
                token0: String::new(), // Will be fetched from pool info
                token1: String::new(), // Will be fetched from pool info
                fee_tier: 0, // Will be fetched from pool info
                tick_lower: 0, // Will be fetched from position manager
                tick_upper: 0,
                liquidity: Decimal::ZERO,
                token0_amount: Some(Decimal::ZERO),
                token1_amount: Some(Decimal::ZERO),
                fees_token0: Decimal::ZERO,
                fees_token1: Decimal::ZERO,
                block_number: event.block_number as i64,
                transaction_hash: event.transaction_hash.to_string(),
                timestamp: event.timestamp,
                created_at: event.timestamp,
                updated_at: event.timestamp,
                current_tick: None,
                in_range: None,
                current_il_percentage: None,
                fees_earned_usd: None,
            });

        // Update position with increased liquidity
        position.liquidity += Decimal::from(liquidity.to::<u128>());
        position.token0_amount = Some(position.token0_amount.unwrap_or(Decimal::ZERO) + u256_to_decimal(amount0));
        position.token1_amount = Some(position.token1_amount.unwrap_or(Decimal::ZERO) + u256_to_decimal(amount1));
        position.updated_at = event.timestamp;
        position.block_number = event.block_number as i64;
        position.transaction_hash = event.transaction_hash.to_string();

        pending_positions.insert(position_key, position.clone());
        Ok(Some(position))
    }

    async fn handle_v3_decrease_liquidity(&self, event: &ProcessedEvent, token_id: U256, liquidity: U256, amount0: U256, amount1: U256) -> IndexerResult<Option<PositionV3>> {
        let position_key = format!("{}:{}", token_id, event.contract_address);
        
        let mut pending_positions = self.pending_v3_positions.write().await;
        if let Some(mut position) = pending_positions.get(&position_key).cloned() {
            position.liquidity = position.liquidity.saturating_sub(Decimal::from(liquidity.to::<u128>()));
            position.token0_amount = Some(position.token0_amount.unwrap_or(Decimal::ZERO).saturating_sub(u256_to_decimal(amount0)));
            position.token1_amount = Some(position.token1_amount.unwrap_or(Decimal::ZERO).saturating_sub(u256_to_decimal(amount1)));
            position.updated_at = event.timestamp;
            position.block_number = event.block_number as i64;
            position.transaction_hash = event.transaction_hash.to_string();

            pending_positions.insert(position_key, position.clone());
            Ok(Some(position))
        } else {
            tracing::warn!("Decrease liquidity event for unknown position: {}", position_key);
            Ok(None)
        }
    }

    async fn handle_v3_collect(&self, event: &ProcessedEvent, token_id: U256, _recipient: &Address, amount0: U256, amount1: U256) -> IndexerResult<Option<PositionV3>> {
        let position_key = format!("{}:{}", token_id, event.contract_address);
        
        let mut pending_positions = self.pending_v3_positions.write().await;
        if let Some(mut position) = pending_positions.get(&position_key).cloned() {
            // Update fees earned
            position.fees_token0 += u256_to_decimal(amount0);
            position.fees_token1 += u256_to_decimal(amount1);
            position.updated_at = event.timestamp;
            position.block_number = event.block_number as i64;
            position.transaction_hash = event.transaction_hash.to_string();

            pending_positions.insert(position_key, position.clone());
            Ok(Some(position))
        } else {
            tracing::warn!("Collect event for unknown position: {}", position_key);
            Ok(None)
        }
    }

    async fn handle_v3_transfer(&self, event: &ProcessedEvent, _from: &Address, to: &Address, token_id: U256) -> IndexerResult<Option<PositionV3>> {
        let position_key = format!("{}:{}", token_id, event.contract_address);
        
        let mut pending_positions = self.pending_v3_positions.write().await;
        if let Some(mut position) = pending_positions.get(&position_key).cloned() {
            // Update position owner
            position.user_address = to.to_string();
            position.updated_at = event.timestamp;
            position.block_number = event.block_number as i64;
            position.transaction_hash = event.transaction_hash.to_string();

            pending_positions.insert(position_key, position.clone());
            Ok(Some(position))
        } else {
            tracing::warn!("Transfer event for unknown position: {}", position_key);
            Ok(None)
        }
    }

    pub async fn get_metrics(&self) -> ProcessorMetrics {
        self.metrics.read().await.clone()
    }

    async fn increment_processed_events(&self, count: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.events_processed += count;
    }

    async fn increment_failed_events(&self, count: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.events_failed += count;
    }

    async fn update_processing_time(&self, time_ms: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.processing_time_ms = time_ms;
    }

    async fn update_last_processed_block(&self, block_number: u64) {
        let mut metrics = self.metrics.write().await;
        metrics.last_processed_block = block_number;
    }
}

// Helper function to convert U256 to Decimal
fn u256_to_decimal(value: U256) -> Decimal {
    // Convert U256 to string and then to Decimal
    // This handles large numbers that might not fit in standard integer types
    let value_str = value.to_string();
    Decimal::from_str_exact(&value_str).unwrap_or(Decimal::ZERO)
}
