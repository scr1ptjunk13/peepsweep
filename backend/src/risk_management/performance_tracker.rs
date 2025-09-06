use crate::risk_management::types::{UserId, RiskError};
use crate::risk_management::position_tracker::PositionTracker;
use crate::risk_management::redis_cache::RiskCache;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use num_traits::pow::Pow;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use std::collections::HashMap;

/// Performance metrics for a user's portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub user_id: UserId,
    pub total_value_usd: Decimal,
    pub total_pnl: Decimal,
    pub roi_percentage: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown_percentage: Decimal,
    pub win_rate_percentage: Decimal,
    pub average_return_percentage: Decimal,
    pub return_volatility_percentage: Decimal,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub average_winning_trade: Decimal,
    pub average_losing_trade: Decimal,
    pub last_updated: u64,
}

/// Historical return data for calculating performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnHistory {
    pub returns: Vec<Decimal>,
    pub timestamps: Vec<u64>,
    pub max_history_size: usize,
}

impl ReturnHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            returns: Vec::new(),
            timestamps: Vec::new(),
            max_history_size: max_size,
        }
    }

    pub fn add_return(&mut self, return_rate: Decimal, timestamp: u64) {
        self.returns.push(return_rate);
        self.timestamps.push(timestamp);

        // Keep only the most recent returns
        if self.returns.len() > self.max_history_size {
            self.returns.remove(0);
            self.timestamps.remove(0);
        }
    }

    pub fn calculate_mean(&self) -> Decimal {
        if self.returns.is_empty() {
            return Decimal::ZERO;
        }
        
        let sum: Decimal = self.returns.iter().sum();
        sum / Decimal::from(self.returns.len())
    }

    pub fn calculate_volatility(&self) -> Decimal {
        if self.returns.len() < 2 {
            return Decimal::ZERO;
        }

        let mean = self.calculate_mean();
        let variance: Decimal = self.returns
            .iter()
            .map(|r| (*r - mean) * (*r - mean))
            .sum::<Decimal>() / Decimal::from(self.returns.len() - 1);

        // Approximate square root using Newton's method
        if variance <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let mut x = variance / Decimal::from(2);
        for _ in 0..10 {
            let x_new = (x + variance / x) / Decimal::from(2);
            if (x_new - x).abs() < Decimal::new(1, 10) {
                break;
            }
            x = x_new;
        }
        x
    }
}

/// Portfolio value history for drawdown calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownHistory {
    pub values: Vec<Decimal>,
    pub timestamps: Vec<u64>,
    pub max_history_size: usize,
}

impl DrawdownHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            values: Vec::new(),
            timestamps: Vec::new(),
            max_history_size: max_size,
        }
    }

    pub fn add_value(&mut self, value: Decimal, timestamp: u64) {
        self.values.push(value);
        self.timestamps.push(timestamp);

        // Keep only the most recent values
        if self.values.len() > self.max_history_size {
            self.values.remove(0);
            self.timestamps.remove(0);
        }
    }

    pub fn calculate_max_drawdown(&self) -> Decimal {
        if self.values.len() < 2 {
            return Decimal::ZERO;
        }

        let mut max_drawdown = Decimal::ZERO;
        let mut peak = self.values[0];

        for &value in &self.values[1..] {
            if value > peak {
                peak = value;
            } else {
                let drawdown = (peak - value) / peak * Decimal::from(100);
                if drawdown > max_drawdown {
                    max_drawdown = drawdown;
                }
            }
        }

        max_drawdown
    }
}

/// Trade result tracking for win/loss analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeResults {
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub total_winning_pnl: Decimal,
    pub total_losing_pnl: Decimal,
}

impl TradeResults {
    pub fn new() -> Self {
        Self {
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            total_winning_pnl: Decimal::ZERO,
            total_losing_pnl: Decimal::ZERO,
        }
    }

    pub fn add_trade(&mut self, pnl: Decimal, is_win: bool) {
        self.total_trades += 1;
        
        if is_win {
            self.winning_trades += 1;
            self.total_winning_pnl += pnl;
        } else {
            self.losing_trades += 1;
            self.total_losing_pnl += pnl.abs();
        }
    }

    pub fn win_rate(&self) -> Decimal {
        if self.total_trades == 0 {
            return Decimal::ZERO;
        }
        Decimal::from(self.winning_trades) / Decimal::from(self.total_trades) * Decimal::from(100)
    }

    pub fn average_winning_trade(&self) -> Decimal {
        if self.winning_trades == 0 {
            return Decimal::ZERO;
        }
        self.total_winning_pnl / Decimal::from(self.winning_trades)
    }

    pub fn average_losing_trade(&self) -> Decimal {
        if self.losing_trades == 0 {
            return Decimal::ZERO;
        }
        self.total_losing_pnl / Decimal::from(self.losing_trades)
    }
}

/// Historical performance data storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalPerformanceData {
    pub return_history: HashMap<UserId, ReturnHistory>,
    pub drawdown_history: HashMap<UserId, DrawdownHistory>,
    pub trade_results: HashMap<UserId, TradeResults>,
}

impl HistoricalPerformanceData {
    pub fn new() -> Self {
        Self {
            return_history: HashMap::new(),
            drawdown_history: HashMap::new(),
            trade_results: HashMap::new(),
        }
    }
}

/// Performance calculator for computing various portfolio metrics
pub struct PerformanceCalculator {
    risk_free_rate: Decimal, // Annual risk-free rate for Sharpe ratio calculation
}

impl PerformanceCalculator {
    pub fn new(risk_free_rate: Decimal) -> Self {
        Self { risk_free_rate }
    }

    /// Calculate Sharpe ratio from return history
    pub fn calculate_sharpe_ratio(&self, return_history: &ReturnHistory) -> Decimal {
        if return_history.returns.len() < 2 {
            return Decimal::ZERO;
        }

        let mean_return = return_history.calculate_mean();
        let volatility = return_history.calculate_volatility();

        if volatility == Decimal::ZERO {
            return Decimal::ZERO;
        }

        // Annualize the returns (assuming daily returns)
        let annualized_return = mean_return * Decimal::from(365);
        let annualized_volatility = volatility * Decimal::from(365).sqrt();

        if annualized_volatility == Decimal::ZERO {
            return Decimal::ZERO;
        }

        (annualized_return - self.risk_free_rate) / annualized_volatility
    }

    /// Calculate comprehensive performance metrics
    pub fn calculate_metrics(
        &self,
        user_id: UserId,
        current_value: Decimal,
        current_pnl: Decimal,
        initial_value: Decimal,
        return_history: &ReturnHistory,
        drawdown_history: &DrawdownHistory,
        trade_results: &TradeResults,
    ) -> PerformanceMetrics {
        let roi_percentage = if initial_value > Decimal::ZERO {
            current_pnl / initial_value * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        let sharpe_ratio = self.calculate_sharpe_ratio(return_history);
        let max_drawdown_percentage = drawdown_history.calculate_max_drawdown();
        let win_rate_percentage = trade_results.win_rate();
        let average_return_percentage = return_history.calculate_mean() * Decimal::from(100);
        let return_volatility_percentage = return_history.calculate_volatility() * Decimal::from(100);

        PerformanceMetrics {
            user_id,
            total_value_usd: current_value,
            total_pnl: current_pnl,
            roi_percentage,
            sharpe_ratio,
            max_drawdown_percentage,
            win_rate_percentage,
            average_return_percentage,
            return_volatility_percentage,
            total_trades: trade_results.total_trades,
            winning_trades: trade_results.winning_trades,
            losing_trades: trade_results.losing_trades,
            average_winning_trade: trade_results.average_winning_trade(),
            average_losing_trade: trade_results.average_losing_trade(),
            last_updated: chrono::Utc::now().timestamp() as u64,
        }
    }
}

/// Portfolio performance tracker integrating with position tracker and Redis cache
pub struct PortfolioPerformanceTracker {
    position_tracker: Arc<PositionTracker>,
    redis_cache: Arc<RwLock<RiskCache>>,
    performance_calculator: PerformanceCalculator,
    historical_data: Arc<RwLock<HistoricalPerformanceData>>,
    initial_values: Arc<RwLock<HashMap<UserId, Decimal>>>, // Track initial portfolio values
    max_return_history_size: usize, // Maximum size of return history
    max_drawdown_history_size: usize, // Maximum size of drawdown history
}

impl PortfolioPerformanceTracker {
    /// Create a new portfolio performance tracker
    pub async fn new(
        position_tracker: Arc<PositionTracker>,
        redis_cache: Arc<RwLock<RiskCache>>,
    ) -> Result<Self, RiskError> {
        let performance_calculator = PerformanceCalculator::new(Decimal::new(2, 2)); // 2% risk-free rate
        let historical_data = Arc::new(RwLock::new(HistoricalPerformanceData::new()));
        let initial_values = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            position_tracker,
            redis_cache,
            performance_calculator,
            historical_data,
            initial_values,
            max_return_history_size: 1000,
            max_drawdown_history_size: 1000,
        })
    }

    /// Calculate current performance metrics for a user
    pub async fn calculate_performance_metrics(&self, user_id: UserId) -> Result<PerformanceMetrics, RiskError> {
        // Get current positions from position tracker
        let positions = self.position_tracker.get_user_position(&user_id)
            .ok_or_else(|| RiskError::UserNotFound(user_id))?;
        
        // Calculate current portfolio value
        let current_value: Decimal = positions.balances.values()
            .map(|balance| balance.value_usd)
            .sum();
        
        let current_pnl = positions.pnl;

        // Get or set initial portfolio value
        let initial_value = {
            let mut initial_values = self.initial_values.write().await;
            *initial_values.entry(user_id).or_insert(current_value)
        };

        // Get historical data
        let historical_data = self.historical_data.read().await;
        
        let return_history = historical_data.return_history
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| ReturnHistory::new(1000)); // Keep 1000 data points
        
        let drawdown_history = historical_data.drawdown_history
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| DrawdownHistory::new(1000));
        
        let trade_results = historical_data.trade_results
            .get(&user_id)
            .cloned()
            .unwrap_or_else(|| TradeResults::new());

        // Calculate comprehensive metrics
        let metrics = self.performance_calculator.calculate_metrics(
            user_id,
            current_value,
            current_pnl,
            initial_value,
            &return_history,
            &drawdown_history,
            &trade_results,
        );

        // Cache performance metrics
        let cache_key = format!("performance_metrics:{}", user_id);
        let metrics_json = serde_json::to_string(&metrics)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;
        {
            let mut cache = self.redis_cache.write().await;
            cache.set(&cache_key, &metrics_json, Some(300)).await?; // 5 minutes TTL
        }

        // Cache historical data
        let return_cache_key = format!("return_history:{}", user_id);
        if let Some(return_history) = historical_data.return_history.get(&user_id) {
            let return_json = serde_json::to_string(return_history)
                .map_err(|e| RiskError::SerializationError(e.to_string()))?;
            {
                let mut cache = self.redis_cache.write().await;
                cache.set(&return_cache_key, &return_json, Some(86400)).await?; // 24 hours TTL
            }
        }

        let drawdown_cache_key = format!("drawdown_history:{}", user_id);
        if let Some(drawdown_history) = historical_data.drawdown_history.get(&user_id) {
            let drawdown_json = serde_json::to_string(drawdown_history)
                .map_err(|e| RiskError::SerializationError(e.to_string()))?;
            {
                let mut cache = self.redis_cache.write().await;
                cache.set(&drawdown_cache_key, &drawdown_json, Some(86400)).await?; // 24 hours TTL
            }
        }

        let trade_cache_key = format!("trade_results:{}", user_id);
        if let Some(trade_results) = historical_data.trade_results.get(&user_id) {
            let trade_json = serde_json::to_string(trade_results)
                .map_err(|e| RiskError::SerializationError(e.to_string()))?;
            {
                let mut cache = self.redis_cache.write().await;
                cache.set(&trade_cache_key, &trade_json, Some(86400)).await?; // 24 hours TTL
            }
        }

        Ok(metrics)
    }

    /// Add return data to history for Sharpe ratio calculation
    pub async fn add_return_to_history(&self, user_id: UserId, return_rate: Decimal) -> Result<(), RiskError> {
        let mut historical_data = self.historical_data.write().await;
        
        let return_history = historical_data.return_history
            .entry(user_id)
            .or_insert_with(|| ReturnHistory::new(1000));
        
        let timestamp = chrono::Utc::now().timestamp() as u64;
        return_history.add_return(return_rate, timestamp);

        // Persist to Redis
        let cache_key = format!("return_history:{}", user_id);
        let history_json = serde_json::to_string(return_history)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;
        
        {
            let mut cache = self.redis_cache.write().await;
            cache.set(&cache_key, &history_json, Some(86400)).await?; // Cache for 24 hours
        }

        Ok(())
    }

    /// Add portfolio value to drawdown history
    pub async fn add_value_to_drawdown_history(&self, user_id: UserId, value: Decimal) -> Result<(), RiskError> {
        let mut historical_data = self.historical_data.write().await;
        
        let drawdown_history = historical_data.drawdown_history
            .entry(user_id)
            .or_insert_with(|| DrawdownHistory::new(1000));
        
        let timestamp = chrono::Utc::now().timestamp() as u64;
        drawdown_history.add_value(value, timestamp);

        // Persist to Redis
        let cache_key = format!("drawdown_history:{}", user_id);
        let history_json = serde_json::to_string(drawdown_history)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;
        
        {
            let mut cache = self.redis_cache.write().await;
            cache.set(&cache_key, &history_json, Some(86400)).await?; // Cache for 24 hours
        }

        Ok(())
    }

    /// Record trade result for win/loss analysis
    pub async fn record_trade_result(&self, user_id: UserId, pnl: Decimal, is_win: bool) -> Result<(), RiskError> {
        let mut historical_data = self.historical_data.write().await;
        
        let trade_results = historical_data.trade_results
            .entry(user_id)
            .or_insert_with(|| TradeResults::new());
        
        trade_results.add_trade(pnl, is_win);

        // Persist to Redis
        let cache_key = format!("trade_results:{}", user_id);
        let results_json = serde_json::to_string(trade_results)
            .map_err(|e| RiskError::SerializationError(e.to_string()))?;
        
        {
            let mut cache = self.redis_cache.write().await;
            cache.set(&cache_key, &results_json, Some(86400)).await?; // Cache for 24 hours
        }

        Ok(())
    }

    /// Load historical data from Redis cache
    pub async fn load_historical_data(&self, user_id: UserId) -> Result<(), RiskError> {
        let mut historical_data = self.historical_data.write().await;

        // Load return history
        let return_cache_key = format!("return_history:{}", user_id);
        {
            let mut cache = self.redis_cache.write().await;
            if let Ok(Some(return_json)) = cache.get(&return_cache_key).await {
                if let Ok(return_history) = serde_json::from_str::<ReturnHistory>(&return_json) {
                    historical_data.return_history.insert(user_id, return_history);
                }
            }
        }

        // Load drawdown history
        let drawdown_cache_key = format!("drawdown_history:{}", user_id);
        {
            let mut cache = self.redis_cache.write().await;
            if let Ok(Some(drawdown_json)) = cache.get(&drawdown_cache_key).await {
                if let Ok(drawdown_history) = serde_json::from_str::<DrawdownHistory>(&drawdown_json) {
                    historical_data.drawdown_history.insert(user_id, drawdown_history);
                }
            }
        }

        // Load trade results
        let trade_cache_key = format!("trade_results:{}", user_id);
        {
            let mut cache = self.redis_cache.write().await;
            if let Ok(Some(trade_json)) = cache.get(&trade_cache_key).await {
                if let Ok(trade_results) = serde_json::from_str::<TradeResults>(&trade_json) {
                    historical_data.trade_results.insert(user_id, trade_results);
                }
            }
        }

        Ok(())
    }

    /// Get cached performance metrics if available
    pub async fn get_cached_metrics(&self, user_id: UserId) -> Result<Option<PerformanceMetrics>, RiskError> {
        let cache_key = format!("performance_metrics:{}", user_id);
        
        let mut cache = self.redis_cache.write().await;
        if let Ok(Some(metrics_json)) = cache.get(&cache_key).await {
            let metrics = serde_json::from_str::<PerformanceMetrics>(&metrics_json)
                .map_err(|e| RiskError::SerializationError(e.to_string()))?;
            Ok(Some(metrics))
        } else {
            Ok(None)
        }
    }
}

// Helper trait for Decimal square root approximation
trait DecimalExt {
    fn sqrt(self) -> Self;
}

impl DecimalExt for Decimal {
    fn sqrt(self) -> Self {
        if self <= Decimal::ZERO {
            return Decimal::ZERO;
        }

        let mut x = self / Decimal::from(2);
        for _ in 0..10 {
            let x_new = (x + self / x) / Decimal::from(2);
            if (x_new - x).abs() < Decimal::new(1, 10) {
                break;
            }
            x = x_new;
        }
        x
    }
}
