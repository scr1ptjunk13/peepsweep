use crate::risk_management::types::{RiskError, UserId};
use crate::analytics::data_models::{PnLData, PositionPnL};
use rust_decimal::Decimal;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

/// Price oracle for real-time price data
#[async_trait::async_trait]
pub trait PriceOracle: Send + Sync {
    async fn get_current_price(&self, token_address: &str) -> Result<Decimal, RiskError>;
    async fn get_historical_price(&self, token_address: &str, timestamp: DateTime<Utc>) -> Result<Decimal, RiskError>;
    async fn is_price_stale(&self, token_address: &str) -> Result<bool, RiskError>;
}

/// Position tracker for user holdings
#[async_trait::async_trait]
pub trait PositionTracker: Send + Sync {
    async fn get_user_positions(&self, user_id: &UserId) -> Result<HashMap<String, Position>, RiskError>;
    async fn get_cost_basis(&self, user_id: &UserId, token_address: &str) -> Result<Decimal, RiskError>;
}

/// Trade history for realized P&L calculations
#[async_trait::async_trait]
pub trait TradeHistory: Send + Sync {
    async fn get_realized_pnl(&self, user_id: &UserId, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<Decimal, RiskError>;
    async fn get_trade_count(&self, user_id: &UserId) -> Result<u64, RiskError>;
}

/// Position data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub token_address: String,
    pub token_symbol: String,
    pub quantity: Decimal,
    pub average_entry_price: Decimal,
    pub last_updated: DateTime<Utc>,
}

/// P&L calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLResult {
    pub user_id: UserId,
    pub timestamp: DateTime<Utc>,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub total_pnl: Decimal,
    pub portfolio_value: Decimal,
    pub daily_change: Decimal,
    pub daily_change_percent: Decimal,
    pub positions: Vec<PositionPnL>,
    pub calculation_duration_ms: u64,
}

/// Core P&L Calculator Engine
pub struct PnLCalculator {
    price_oracle: Arc<dyn PriceOracle>,
    position_tracker: Arc<dyn PositionTracker>,
    trade_history: Arc<dyn TradeHistory>,
    calculation_stats: Arc<RwLock<PnLCalculationStats>>,
}

#[derive(Debug, Default, Clone)]
pub struct PnLCalculationStats {
    pub total_calculations: u64,
    pub successful_calculations: u64,
    pub failed_calculations: u64,
    pub average_calculation_time_ms: f64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl PnLCalculator {
    pub fn new(
        price_oracle: Arc<dyn PriceOracle>,
        position_tracker: Arc<dyn PositionTracker>,
        trade_history: Arc<dyn TradeHistory>,
    ) -> Self {
        Self {
            price_oracle,
            position_tracker,
            trade_history,
            calculation_stats: Arc::new(RwLock::new(PnLCalculationStats::default())),
        }
    }

    /// Calculate current P&L for a user
    pub async fn calculate_current_pnl(&self, user_id: &UserId) -> Result<PnLResult, RiskError> {
        let start_time = std::time::Instant::now();
        
        // Update stats
        {
            let mut stats = self.calculation_stats.write().await;
            stats.total_calculations += 1;
        }

        let result = self.calculate_pnl_internal(user_id).await;
        
        let calculation_duration = start_time.elapsed().as_millis() as u64;
        
        // Update stats based on result
        {
            let mut stats = self.calculation_stats.write().await;
            match &result {
                Ok(_) => {
                    stats.successful_calculations += 1;
                    let total_time = stats.average_calculation_time_ms * (stats.successful_calculations - 1) as f64;
                    stats.average_calculation_time_ms = (total_time + calculation_duration as f64) / stats.successful_calculations as f64;
                }
                Err(_) => stats.failed_calculations += 1,
            }
        }

        result
    }

    async fn calculate_pnl_internal(&self, user_id: &UserId) -> Result<PnLResult, RiskError> {
        let calculation_start = std::time::Instant::now();
        
        // Get user positions
        let positions = self.position_tracker.get_user_positions(user_id).await?;
        
        let mut total_unrealized_pnl = Decimal::ZERO;
        let mut total_portfolio_value = Decimal::ZERO;
        let mut position_pnls = Vec::new();

        // Calculate P&L for each position
        for (token_address, position) in positions {
            let current_price = self.price_oracle.get_current_price(&token_address).await?;
            
            // Check if price is stale
            if self.price_oracle.is_price_stale(&token_address).await? {
                tracing::warn!("Stale price detected for token: {}", token_address);
            }

            let market_value = position.quantity * current_price;
            let cost_basis = position.quantity * position.average_entry_price;
            let unrealized_pnl = market_value - cost_basis;
            let return_percentage = if cost_basis > Decimal::ZERO {
                (unrealized_pnl / cost_basis) * Decimal::from(100)
            } else {
                Decimal::ZERO
            };

            total_unrealized_pnl += unrealized_pnl;
            total_portfolio_value += market_value;

            position_pnls.push(PositionPnL {
                token: position.token_symbol.clone(),
                amount: position.quantity,
                value_usd: market_value,
                pnl: unrealized_pnl,
                pnl_percent: return_percentage,
                token_address: token_address.clone(),
                token_symbol: position.token_symbol,
                quantity: position.quantity,
                average_entry_price_usd: position.average_entry_price,
                current_price_usd: current_price,
                unrealized_pnl_usd: unrealized_pnl,
                realized_pnl_usd: Decimal::ZERO, // Will be calculated separately
                cost_basis_usd: cost_basis,
                market_value_usd: market_value,
                return_percentage,
                last_updated: Utc::now(),
            });
        }

        // Get realized P&L for today
        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let total_realized_pnl = self.trade_history.get_realized_pnl(user_id, today_start, Utc::now()).await?;

        // Calculate daily change (simplified - would need historical data)
        let daily_change_usd = total_unrealized_pnl; // Simplified
        let daily_change_percent = if total_portfolio_value > Decimal::ZERO {
            (daily_change_usd / total_portfolio_value) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        let total_pnl = total_unrealized_pnl + total_realized_pnl;
        let calculation_duration = calculation_start.elapsed().as_millis() as u64;

        Ok(PnLResult {
            user_id: *user_id,
            timestamp: Utc::now(),
            unrealized_pnl: total_unrealized_pnl,
            realized_pnl: total_realized_pnl,
            total_pnl: total_pnl,
            portfolio_value: total_portfolio_value,
            daily_change: daily_change_usd,
            daily_change_percent,
            positions: position_pnls,
            calculation_duration_ms: calculation_duration,
        })
    }

    /// Get calculation statistics
    pub async fn get_stats(&self) -> PnLCalculationStats {
        (*self.calculation_stats.read().await).clone()
    }
}

/// Mock implementations for testing
pub struct MockPriceOracle {
    prices: Arc<RwLock<HashMap<String, Decimal>>>,
}

impl MockPriceOracle {
    pub fn new() -> Self {
        let mut prices = HashMap::new();
        prices.insert("ETH".to_string(), Decimal::from(3200));
        prices.insert("BTC".to_string(), Decimal::from(65000));
        prices.insert("USDC".to_string(), Decimal::from(1));
        
        Self {
            prices: Arc::new(RwLock::new(prices)),
        }
    }

    pub async fn set_price(&self, token: &str, price: Decimal) {
        let mut prices = self.prices.write().await;
        prices.insert(token.to_string(), price);
    }
}

#[async_trait::async_trait]
impl PriceOracle for MockPriceOracle {
    async fn get_current_price(&self, token_address: &str) -> Result<Decimal, RiskError> {
        let prices = self.prices.read().await;
        prices.get(token_address)
            .copied()
            .ok_or_else(|| RiskError::PriceNotFound(token_address.to_string()))
    }

    async fn get_historical_price(&self, token_address: &str, _timestamp: DateTime<Utc>) -> Result<Decimal, RiskError> {
        self.get_current_price(token_address).await
    }

    async fn is_price_stale(&self, _token_address: &str) -> Result<bool, RiskError> {
        Ok(false) // Mock never has stale prices
    }
}

pub struct MockPositionTracker {
    positions: Arc<RwLock<HashMap<UserId, HashMap<String, Position>>>>,
}

impl MockPositionTracker {
    pub fn new() -> Self {
        Self {
            positions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_position(&self, user_id: UserId, position: Position) {
        let mut positions = self.positions.write().await;
        positions.entry(user_id)
            .or_insert_with(HashMap::new)
            .insert(position.token_address.clone(), position);
    }
}

#[async_trait::async_trait]
impl PositionTracker for MockPositionTracker {
    async fn get_user_positions(&self, user_id: &UserId) -> Result<HashMap<String, Position>, RiskError> {
        let positions = self.positions.read().await;
        Ok(positions.get(user_id).cloned().unwrap_or_default())
    }

    async fn get_cost_basis(&self, user_id: &UserId, token_address: &str) -> Result<Decimal, RiskError> {
        let positions = self.positions.read().await;
        if let Some(user_positions) = positions.get(user_id) {
            if let Some(position) = user_positions.get(token_address) {
                return Ok(position.quantity * position.average_entry_price);
            }
        }
        Ok(Decimal::ZERO)
    }
}

pub struct MockTradeHistory;

impl MockTradeHistory {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl TradeHistory for MockTradeHistory {
    async fn get_realized_pnl(&self, _user_id: &UserId, _start_time: DateTime<Utc>, _end_time: DateTime<Utc>) -> Result<Decimal, RiskError> {
        Ok(Decimal::from(150)) // Mock realized P&L
    }

    async fn get_trade_count(&self, _user_id: &UserId) -> Result<u64, RiskError> {
        Ok(25) // Mock trade count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pnl_calculation() {
        let price_oracle = Arc::new(MockPriceOracle::new());
        let position_tracker = Arc::new(MockPositionTracker::new());
        let trade_history = Arc::new(MockTradeHistory::new());

        let calculator = PnLCalculator::new(price_oracle.clone(), position_tracker.clone(), trade_history);

        let user_id = Uuid::new_v4();
        
        // Add a test position
        let position = Position {
            token_address: "ETH".to_string(),
            token_symbol: "ETH".to_string(),
            quantity: Decimal::from(10),
            average_entry_price: Decimal::from(3000),
            last_updated: Utc::now(),
        };
        
        position_tracker.add_position(user_id, position).await;

        let result = calculator.calculate_current_pnl(&user_id).await;
        assert!(result.is_ok());

        let pnl = result.unwrap();
        assert_eq!(pnl.user_id, user_id);
        assert!(pnl.portfolio_value > Decimal::ZERO);
        assert_eq!(pnl.positions.len(), 1);
    }

    #[tokio::test]
    async fn test_calculation_stats() {
        let price_oracle = Arc::new(MockPriceOracle::new());
        let position_tracker = Arc::new(MockPositionTracker::new());
        let trade_history = Arc::new(MockTradeHistory::new());

        let calculator = PnLCalculator::new(price_oracle, position_tracker, trade_history);

        let user_id = Uuid::new_v4();
        let _ = calculator.calculate_current_pnl(&user_id).await;

        let stats = calculator.get_stats().await;
        assert_eq!(stats.total_calculations, 1);
        assert_eq!(stats.successful_calculations, 1);
    }
}
