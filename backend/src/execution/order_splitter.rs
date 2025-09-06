use crate::aggregator::DEXAggregator;
use crate::execution::slippage_predictor::{SlippagePredictor, SlippagePrediction};
use crate::types::{SwapParams, QuoteParams};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, Instant};
use uuid::Uuid;
use num_traits::ToPrimitive;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
#[derive(Error, Debug)]
pub enum OrderSplittingError {
    #[error("Invalid order parameters: {0}")]
    InvalidParameters(String),
    #[error("Insufficient liquidity for order splitting")]
    InsufficientLiquidity,
    #[error("Order splitting calculation failed: {0}")]
    CalculationError(String),
    #[error("DEX aggregator error: {0}")]
    DexAggregatorError(String),
    #[error("Execution failed: {0}")]
    ExecutionError(String),
    #[error("Slippage predictor error: {0}")]
    SlippagePredictorError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSplitParams {
    pub from_token: String,
    pub to_token: String,
    pub total_amount: Decimal,
    pub strategy: SplittingStrategy,
    pub max_slippage_bps: Decimal,
    pub time_window_seconds: u64,
    pub min_chunk_size: Option<Decimal>,
    pub max_chunks: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SplittingStrategy {
    TWAP { intervals: u32 },
    VWAP { volume_target: Decimal },
    Iceberg { visible_size_percent: Decimal },
    Adaptive { aggressiveness: Decimal },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderChunk {
    pub chunk_id: Uuid,
    pub from_token: String,
    pub to_token: String,
    pub amount: Decimal,
    pub execution_time: u64,
    pub target_dexs: Vec<String>,
    pub max_slippage_bps: Decimal,
    pub priority: u32,
    pub status: ChunkStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChunkStatus {
    Pending,
    Executing,
    Completed { actual_output: Decimal, slippage_bps: Decimal },
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitOrderExecution {
    pub order_id: Uuid,
    pub chunks: Vec<OrderChunk>,
    pub total_executed: Decimal,
    pub total_received: Decimal,
    pub average_slippage_bps: Decimal,
    pub execution_start: u64,
    pub execution_end: Option<u64>,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Planning,
    Executing,
    Completed,
    PartiallyCompleted,
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TWAPExecution {
    pub interval_duration: Duration,
    pub chunks_per_interval: u32,
    pub total_intervals: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VWAPExecution {
    pub volume_profile: Vec<VolumeWindow>,
    pub chunk_sizes: Vec<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeWindow {
    pub start_time: u64,
    pub end_time: u64,
    pub expected_volume: Decimal,
    pub weight: Decimal,
}

pub struct OrderSplitter {
    dex_aggregator: Arc<DEXAggregator>,
    slippage_predictor: Arc<SlippagePredictor>,
    active_orders: Arc<RwLock<HashMap<Uuid, SplitOrderExecution>>>,
    volume_profiles: Arc<RwLock<HashMap<String, Vec<VolumeWindow>>>>,
}

impl OrderSplitter {
    pub fn new(
        dex_aggregator: Arc<DEXAggregator>,
        slippage_predictor: Arc<SlippagePredictor>,
    ) -> Self {
        Self {
            dex_aggregator,
            slippage_predictor,
            active_orders: Arc::new(RwLock::new(HashMap::new())),
            volume_profiles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Split a large order into smaller chunks based on strategy
    pub async fn split_order(&self, params: OrderSplitParams) -> Result<SplitOrderExecution, OrderSplittingError> {
        self.validate_order_params(&params)?;

        let order_id = Uuid::new_v4();
        let token_pair = format!("{}/{}", params.from_token, params.to_token);

        // Get slippage prediction for the full order
        let full_order_prediction = self.slippage_predictor
            .predict_slippage(&params.from_token, &params.to_token, params.total_amount)
            .await
            .map_err(|e| OrderSplittingError::SlippagePredictorError(e.to_string()))?;

        // Generate chunks based on strategy
        let chunks = match params.strategy {
            SplittingStrategy::TWAP { intervals } => {
                self.generate_twap_chunks(&params, intervals, &full_order_prediction).await?
            }
            SplittingStrategy::VWAP { volume_target } => {
                self.generate_vwap_chunks(&params, volume_target, &full_order_prediction).await?
            }
            SplittingStrategy::Iceberg { visible_size_percent } => {
                self.generate_iceberg_chunks(&params, visible_size_percent, &full_order_prediction).await?
            }
            SplittingStrategy::Adaptive { aggressiveness } => {
                self.generate_adaptive_chunks(&params, aggressiveness, &full_order_prediction).await?
            }
        };

        let execution = SplitOrderExecution {
            order_id,
            chunks,
            total_executed: Decimal::ZERO,
            total_received: Decimal::ZERO,
            average_slippage_bps: Decimal::ZERO,
            execution_start: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            execution_end: None,
            status: ExecutionStatus::Planning,
        };

        // Store the order execution
        let mut active_orders = self.active_orders.write().await;
        active_orders.insert(order_id, execution.clone());

        info!(
            "Created split order {} with {} chunks for {} {} -> {}",
            order_id, execution.chunks.len(), params.total_amount, params.from_token, params.to_token
        );

        Ok(execution)
    }

    /// Execute a split order
    pub async fn execute_split_order(&self, order_id: Uuid) -> Result<SplitOrderExecution, OrderSplittingError> {
        let mut active_orders = self.active_orders.write().await;
        let mut execution = active_orders.get(&order_id)
            .ok_or_else(|| OrderSplittingError::InvalidParameters("Order not found".to_string()))?
            .clone();

        execution.status = ExecutionStatus::Executing;
        active_orders.insert(order_id, execution.clone());
        drop(active_orders);

        // Execute chunks according to their timing
        for (chunk_index, chunk) in execution.chunks.iter().enumerate() {
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            
            // Wait until it's time to execute this chunk
            if chunk.execution_time > current_time {
                let wait_duration = Duration::from_secs(chunk.execution_time - current_time);
                tokio::time::sleep(wait_duration).await;
            }

            // Execute the chunk
            match self.execute_chunk(chunk, &execution).await {
                Ok(result) => {
                    let mut active_orders = self.active_orders.write().await;
                    if let Some(mut exec) = active_orders.get_mut(&order_id) {
                        exec.chunks[chunk_index].status = ChunkStatus::Completed {
                            actual_output: result.amount_out,
                            slippage_bps: result.slippage_bps,
                        };
                        exec.total_executed += chunk.amount;
                        exec.total_received += result.amount_out;
                    }
                }
                Err(e) => {
                    let mut active_orders = self.active_orders.write().await;
                    if let Some(mut exec) = active_orders.get_mut(&order_id) {
                        exec.chunks[chunk_index].status = ChunkStatus::Failed {
                            error: e.to_string(),
                        };
                    }
                    warn!("Chunk execution failed: {}", e);
                }
            }
        }

        // Update final execution status
        let mut active_orders = self.active_orders.write().await;
        if let Some(mut execution) = active_orders.get_mut(&order_id) {
            execution.execution_end = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
            
            let completed_chunks = execution.chunks.iter()
                .filter(|c| matches!(c.status, ChunkStatus::Completed { .. }))
                .count();
            
            if completed_chunks == execution.chunks.len() {
                execution.status = ExecutionStatus::Completed;
            } else if completed_chunks > 0 {
                execution.status = ExecutionStatus::PartiallyCompleted;
            } else {
                execution.status = ExecutionStatus::Failed {
                    error: "All chunks failed".to_string(),
                };
            }

            // Calculate average slippage
            let total_slippage: Decimal = execution.chunks.iter()
                .filter_map(|c| match &c.status {
                    ChunkStatus::Completed { slippage_bps, .. } => Some(*slippage_bps),
                    _ => None,
                })
                .sum();
            
            if completed_chunks > 0 {
                execution.average_slippage_bps = total_slippage / Decimal::from(completed_chunks);
            }

            Ok(execution.clone())
        } else {
            Err(OrderSplittingError::ExecutionError("Order not found during execution".to_string()))
        }
    }

    /// Generate TWAP chunks
    async fn generate_twap_chunks(
        &self,
        params: &OrderSplitParams,
        intervals: u32,
        prediction: &SlippagePrediction,
    ) -> Result<Vec<OrderChunk>, OrderSplittingError> {
        let chunk_size = params.total_amount / Decimal::from(intervals);
        let interval_duration = Duration::from_secs(params.time_window_seconds / intervals as u64);
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let mut chunks = Vec::new();
        
        for i in 0..intervals {
            let execution_time = current_time + (i as u64 * interval_duration.as_secs());
            
            // Adjust chunk size based on slippage prediction
            let adjusted_size = if chunk_size > prediction.recommended_max_trade_size {
                prediction.recommended_max_trade_size
            } else {
                chunk_size
            };

            chunks.push(OrderChunk {
                chunk_id: Uuid::new_v4(),
                from_token: params.from_token.clone(),
                to_token: params.to_token.clone(),
                amount: adjusted_size,
                execution_time,
                target_dexs: self.select_optimal_dexs(&params.from_token, &params.to_token, adjusted_size).await?,
                max_slippage_bps: params.max_slippage_bps,
                priority: i + 1,
                status: ChunkStatus::Pending,
            });
        }

        Ok(chunks)
    }

    /// Generate VWAP chunks
    async fn generate_vwap_chunks(
        &self,
        params: &OrderSplitParams,
        volume_target: Decimal,
        prediction: &SlippagePrediction,
    ) -> Result<Vec<OrderChunk>, OrderSplittingError> {
        let token_pair = format!("{}/{}", params.from_token, params.to_token);
        let volume_profile = self.get_volume_profile(&token_pair, params.time_window_seconds).await?;

        let mut chunks = Vec::new();
        let mut remaining_amount = params.total_amount;
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        for (i, window) in volume_profile.iter().enumerate() {
            if remaining_amount <= Decimal::ZERO {
                break;
            }

            // Calculate chunk size based on volume weight
            let chunk_size = (params.total_amount * window.weight).min(remaining_amount);
            
        }

        Ok(chunks)
    }

    /// Generate Iceberg chunks
    async fn generate_iceberg_chunks(
        &self,
        params: &OrderSplitParams,
        visible_size_percent: Decimal,
        prediction: &SlippagePrediction,
    ) -> Result<Vec<OrderChunk>, OrderSplittingError> {
        let visible_size = params.total_amount * visible_size_percent / Decimal::from(100);
        let chunk_size = visible_size.min(prediction.recommended_max_trade_size);
        let num_chunks = (params.total_amount / chunk_size).ceil().to_u32().unwrap_or(1);

        let mut chunks = Vec::new();
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let interval_seconds = params.time_window_seconds / num_chunks as u64;

        for i in 0..num_chunks {
            let remaining = params.total_amount - (Decimal::from(i) * chunk_size);
            let actual_chunk_size = chunk_size.min(remaining);

            if actual_chunk_size <= Decimal::ZERO {
                break;
            }

            chunks.push(OrderChunk {
                chunk_id: Uuid::new_v4(),
                from_token: params.from_token.clone(),
                to_token: params.to_token.clone(),
                amount: actual_chunk_size,
                execution_time: current_time + (i as u64 * interval_seconds),
                target_dexs: vec!["Uniswap".to_string()], // Simplified
                max_slippage_bps: params.max_slippage_bps,
                priority: i as u32,
                status: ChunkStatus::Pending,
            });
        }

        Ok(chunks)
    }

    /// Generate Adaptive chunks
    async fn generate_adaptive_chunks(
        &self,
        params: &OrderSplitParams,
        aggressiveness: Decimal,
        prediction: &SlippagePrediction,
    ) -> Result<Vec<OrderChunk>, OrderSplittingError> {
        // Aggressiveness: 0.0 = very conservative, 1.0 = very aggressive
        let base_chunk_size = prediction.recommended_max_trade_size;
        let aggressive_multiplier = Decimal::ONE + aggressiveness;
        let chunk_size = (base_chunk_size * aggressive_multiplier).min(params.total_amount / Decimal::from(2));

        let num_chunks = (params.total_amount / chunk_size).ceil().to_u32().unwrap_or(1);
        let mut chunks = Vec::new();
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        // More aggressive = shorter intervals
        let base_interval = params.time_window_seconds / num_chunks as u64;
        let interval_seconds = if aggressiveness > Decimal::from_str("0.5").unwrap() {
            base_interval / 2
        } else {
            base_interval
        };

        for i in 0..num_chunks {
            let remaining = params.total_amount - (Decimal::from(i) * chunk_size);
            let actual_chunk_size = chunk_size.min(remaining);

            if actual_chunk_size <= Decimal::ZERO {
                break;
            }

            chunks.push(OrderChunk {
                chunk_id: Uuid::new_v4(),
                from_token: params.from_token.clone(),
                to_token: params.to_token.clone(),
                amount: actual_chunk_size,
                execution_time: current_time + (i as u64 * interval_seconds),
                target_dexs: self.select_optimal_dexs(&params.from_token, &params.to_token, actual_chunk_size).await?,
                max_slippage_bps: params.max_slippage_bps * (Decimal::ONE + aggressiveness),
                priority: i + 1,
                status: ChunkStatus::Pending,
            });
        }

        Ok(chunks)
    }

    /// Execute a single chunk
    async fn execute_chunk(&self, chunk: &OrderChunk, execution: &SplitOrderExecution) -> Result<ChunkExecutionResult, OrderSplittingError> {
        let swap_params = SwapParams {
            token_in: chunk.from_token.clone(),
            token_out: chunk.to_token.clone(),
            amount_in: chunk.amount.to_string(),
            amount_out_min: "0".to_string(),
            routes: vec![],
            user_address: "0x0000000000000000000000000000000000000000".to_string(),
            slippage: (chunk.max_slippage_bps / Decimal::from(100)).to_f64().unwrap_or(1.0),
        };

        // Execute the swap through DEX aggregator
        match self.dex_aggregator.execute_swap(swap_params).await {
            Ok(swap_response) => {
                let amount_out = Decimal::from_str(&swap_response.amount_out)
                    .unwrap_or(Decimal::ZERO);
                
                // Calculate actual slippage
                let expected_output = chunk.amount; // Simplified calculation
                let slippage_bps = if expected_output > Decimal::ZERO {
                    ((expected_output - amount_out) / expected_output) * Decimal::from(10000)
                } else {
                    Decimal::ZERO
                };

                Ok(ChunkExecutionResult {
                    amount_out,
                    slippage_bps,
                    gas_used: swap_response.gas_used.parse().unwrap_or(0),
                    transaction_hash: swap_response.tx_hash,
                })
            }
            Err(e) => Err(OrderSplittingError::ExecutionError(e.to_string()))
        }
    }

    /// Select optimal DEXs for a chunk
    async fn select_optimal_dexs(&self, from_token: &str, to_token: &str, amount: Decimal) -> Result<Vec<String>, OrderSplittingError> {
        let token_in = from_token.clone();
        let token_out = to_token.clone();
        let split_amount = amount;
        let quote_params = QuoteParams {
            token_in: token_in.to_string(),
            token_in_address: None,
            token_in_decimals: None,
            token_out: token_out.to_string(),
            token_out_address: None,
            token_out_decimals: None,
            amount_in: split_amount.to_string(),
            chain: Some("ethereum".to_string()),
            slippage: Some(1.0),
        };

        match self.dex_aggregator.get_quote_with_guaranteed_routes(&quote_params).await {
            Ok(quote) => {
                // Extract DEX names from route breakdown
                let dex_names: Vec<String> = quote.routes.iter()
                    .map(|route| route.dex.clone())
                    .collect();
                Ok(dex_names)
            }
            Err(e) => {
                warn!("Failed to get DEX recommendations: {}", e);
                Ok(vec!["Uniswap".to_string()]) // Fallback
            }
        }
    }

    /// Get volume profile for VWAP execution
    async fn get_volume_profile(&self, token_pair: &str, time_window_seconds: u64) -> Result<Vec<VolumeWindow>, OrderSplittingError> {
        let profiles = self.volume_profiles.read().await;
        
        if let Some(profile) = profiles.get(token_pair) {
            return Ok(profile.clone());
        }
        
        drop(profiles);

        // Generate default volume profile (simplified)
        let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let window_duration = time_window_seconds / 10; // 10 windows
        let mut windows = Vec::new();

        for i in 0..10 {
            let start_time = current_time + (i * window_duration);
            let end_time = start_time + window_duration;
            
            // Simulate volume distribution (higher during market hours)
            let weight = match i {
                0..=2 => Decimal::from_str("0.05").unwrap(), // Low volume
                3..=7 => Decimal::from_str("0.15").unwrap(), // High volume
                _ => Decimal::from_str("0.10").unwrap(),     // Medium volume
            };

            windows.push(VolumeWindow {
                start_time,
                end_time,
                expected_volume: Decimal::from(100_000) * weight,
                weight,
            });
        }

        // Cache the profile
        let mut profiles = self.volume_profiles.write().await;
        profiles.insert(token_pair.to_string(), windows.clone());

        Ok(windows)
    }

    /// Get order execution status
    pub async fn get_order_status(&self, order_id: Uuid) -> Option<SplitOrderExecution> {
        let active_orders = self.active_orders.read().await;
        active_orders.get(&order_id).cloned()
    }

    /// Cancel an active order
    pub async fn cancel_order(&self, order_id: Uuid) -> Result<(), OrderSplittingError> {
        let mut active_orders = self.active_orders.write().await;
        
        if let Some(mut execution) = active_orders.get_mut(&order_id) {
            execution.status = ExecutionStatus::Failed {
                error: "Cancelled by user".to_string(),
            };
            
            // Mark pending chunks as failed
            for chunk in &mut execution.chunks {
                if matches!(chunk.status, ChunkStatus::Pending) {
                    chunk.status = ChunkStatus::Failed {
                        error: "Order cancelled".to_string(),
                    };
                }
            }
            
            info!("Cancelled order {}", order_id);
            Ok(())
        } else {
            Err(OrderSplittingError::InvalidParameters("Order not found".to_string()))
        }
    }

    fn validate_order_params(&self, params: &OrderSplitParams) -> Result<(), OrderSplittingError> {
        if params.total_amount <= Decimal::ZERO {
            return Err(OrderSplittingError::InvalidParameters("Amount must be positive".to_string()));
        }

        if params.max_slippage_bps <= Decimal::ZERO || params.max_slippage_bps > Decimal::from(10000) {
            return Err(OrderSplittingError::InvalidParameters("Invalid slippage tolerance".to_string()));
        }

        if params.time_window_seconds == 0 {
            return Err(OrderSplittingError::InvalidParameters("Time window must be positive".to_string()));
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ChunkExecutionResult {
    amount_out: Decimal,
    slippage_bps: Decimal,
    gas_used: u64,
    transaction_hash: String,
}
