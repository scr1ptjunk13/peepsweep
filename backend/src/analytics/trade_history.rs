use crate::risk_management::types::{RiskError, UserId};
use chrono::{DateTime, Utc, Timelike};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Trade execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TradeStatus {
    Pending,
    Executed,
    Failed,
    Cancelled,
    PartiallyFilled,
}

/// Trade type classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TradeType {
    Swap,
    LimitOrder,
    MarketOrder,
    Bridge,
    Arbitrage,
}

/// Comprehensive trade record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub trade_id: Uuid,
    pub user_id: UserId,
    pub trade_type: TradeType,
    pub status: TradeStatus,
    pub timestamp: DateTime<Utc>,
    pub execution_timestamp: Option<DateTime<Utc>>,
    
    // Trade details
    pub input_token: String,
    pub output_token: String,
    pub input_amount: Decimal,
    pub output_amount: Option<Decimal>,
    pub expected_output: Decimal,
    
    // Execution details
    pub dex_used: String,
    pub route_path: Vec<String>,
    pub slippage_tolerance: Decimal,
    pub actual_slippage: Option<Decimal>,
    
    // Cost analysis
    pub gas_used: Option<u64>,
    pub gas_price: Option<Decimal>,
    pub gas_cost_usd: Option<Decimal>,
    pub protocol_fees: Decimal,
    pub network_fees: Decimal,
    
    // Performance metrics
    pub price_impact: Option<Decimal>,
    pub execution_time_ms: Option<u64>,
    pub pnl_usd: Option<Decimal>,
    
    // Transaction details
    pub transaction_hash: Option<String>,
    pub block_number: Option<u64>,
    pub nonce: Option<u64>,
    
    // Metadata
    pub metadata: HashMap<String, String>,
    pub error_message: Option<String>,
}

/// Trade search and filter criteria
#[derive(Debug, Clone, Deserialize)]
pub struct TradeFilter {
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub token_pairs: Option<Vec<(String, String)>>,
    pub dexes: Option<Vec<String>>,
    pub status: Option<Vec<TradeStatus>>,
    pub trade_types: Option<Vec<TradeType>>,
    pub min_amount_usd: Option<Decimal>,
    pub max_amount_usd: Option<Decimal>,
    pub min_pnl: Option<Decimal>,
    pub max_pnl: Option<Decimal>,
    pub search_text: Option<String>,
}

/// Trade sorting options
#[derive(Debug, Clone, Deserialize)]
pub enum TradeSortBy {
    Timestamp,
    Amount,
    PnL,
    GasCost,
    Slippage,
    ExecutionTime,
}

/// Trade query parameters
#[derive(Debug, Clone, Deserialize)]
pub struct TradeQuery {
    pub filter: Option<TradeFilter>,
    pub sort_by: Option<TradeSortBy>,
    pub sort_desc: Option<bool>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// Trade analytics summary
#[derive(Debug, Clone, Serialize)]
pub struct TradeAnalytics {
    pub total_trades: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub success_rate: Decimal,
    pub total_volume_usd: Decimal,
    pub total_pnl_usd: Decimal,
    pub total_fees_usd: Decimal,
    pub average_trade_size_usd: Decimal,
    pub average_execution_time_ms: Decimal,
    pub average_slippage: Decimal,
    pub most_used_dexes: Vec<(String, u64)>,
    pub most_traded_pairs: Vec<(String, String, u64)>,
    pub best_performing_pairs: Vec<(String, String, Decimal)>,
    pub worst_performing_pairs: Vec<(String, String, Decimal)>,
    pub hourly_trade_distribution: HashMap<u32, u64>,
    pub daily_pnl_history: Vec<(DateTime<Utc>, Decimal)>,
}

/// Trade data storage interface
#[async_trait::async_trait]
pub trait TradeDataStore: Send + Sync {
    async fn store_trade(&self, trade: &TradeRecord) -> Result<(), RiskError>;
    async fn update_trade(&self, trade_id: &Uuid, trade: &TradeRecord) -> Result<(), RiskError>;
    async fn get_trade(&self, trade_id: &Uuid) -> Result<Option<TradeRecord>, RiskError>;
    async fn query_trades(&self, user_id: &UserId, query: &TradeQuery) -> Result<Vec<TradeRecord>, RiskError>;
    async fn count_trades(&self, user_id: &UserId, filter: &Option<TradeFilter>) -> Result<u64, RiskError>;
    async fn delete_trade(&self, trade_id: &Uuid) -> Result<(), RiskError>;
}

/// Trade search indexing
#[async_trait::async_trait]
pub trait TradeSearchIndex: Send + Sync {
    async fn index_trade(&self, trade: &TradeRecord) -> Result<(), RiskError>;
    async fn search_trades(&self, user_id: &UserId, query: &str) -> Result<Vec<Uuid>, RiskError>;
    async fn remove_from_index(&self, trade_id: &Uuid) -> Result<(), RiskError>;
}

/// Trade data validation
pub trait TradeDataValidator: Send + Sync {
    fn validate_trade(&self, trade: &TradeRecord) -> Result<(), RiskError>;
    fn sanitize_metadata(&self, metadata: &mut HashMap<String, String>);
}

/// Trade history manager
pub struct TradeHistoryManager {
    data_store: Arc<dyn TradeDataStore>,
    search_index: Arc<dyn TradeSearchIndex>,
    validator: Arc<dyn TradeDataValidator>,
    analytics_cache: Arc<RwLock<HashMap<UserId, (TradeAnalytics, DateTime<Utc>)>>>,
}

impl TradeHistoryManager {
    pub fn new(
        data_store: Arc<dyn TradeDataStore>,
        search_index: Arc<dyn TradeSearchIndex>,
        validator: Arc<dyn TradeDataValidator>,
    ) -> Self {
        Self {
            data_store,
            search_index,
            validator,
            analytics_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a new trade
    pub async fn record_trade(&self, mut trade: TradeRecord) -> Result<Uuid, RiskError> {
        // Validate trade data
        self.validator.validate_trade(&trade)?;
        self.validator.sanitize_metadata(&mut trade.metadata);

        // Store trade
        self.data_store.store_trade(&trade).await?;

        // Index for search
        self.search_index.index_trade(&trade).await?;

        // Invalidate analytics cache
        self.invalidate_analytics_cache(&trade.user_id).await;

        Ok(trade.trade_id)
    }

    /// Update existing trade
    pub async fn update_trade(&self, trade_id: &Uuid, mut trade: TradeRecord) -> Result<(), RiskError> {
        // Validate updated trade data
        self.validator.validate_trade(&trade)?;
        self.validator.sanitize_metadata(&mut trade.metadata);

        // Update trade
        self.data_store.update_trade(trade_id, &trade).await?;

        // Re-index for search
        self.search_index.index_trade(&trade).await?;

        // Invalidate analytics cache
        self.invalidate_analytics_cache(&trade.user_id).await;

        Ok(())
    }

    /// Get trade by ID
    pub async fn get_trade(&self, trade_id: &Uuid) -> Result<Option<TradeRecord>, RiskError> {
        self.data_store.get_trade(trade_id).await
    }

    /// Query trades with filters and pagination
    pub async fn query_trades(&self, user_id: &UserId, query: &TradeQuery) -> Result<Vec<TradeRecord>, RiskError> {
        self.data_store.query_trades(user_id, query).await
    }

    /// Get trade count for pagination
    pub async fn count_trades(&self, user_id: &UserId, filter: &Option<TradeFilter>) -> Result<u64, RiskError> {
        self.data_store.count_trades(user_id, filter).await
    }

    /// Search trades by text
    pub async fn search_trades(&self, user_id: &UserId, search_query: &str) -> Result<Vec<TradeRecord>, RiskError> {
        let trade_ids = self.search_index.search_trades(user_id, search_query).await?;
        let mut trades = Vec::new();
        
        for trade_id in trade_ids {
            if let Some(trade) = self.data_store.get_trade(&trade_id).await? {
                trades.push(trade);
            }
        }
        
        Ok(trades)
    }

    /// Calculate comprehensive trade analytics
    pub async fn calculate_analytics(&self, user_id: &UserId) -> Result<TradeAnalytics, RiskError> {
        // Check cache first
        {
            let cache = self.analytics_cache.read().await;
            if let Some((analytics, timestamp)) = cache.get(user_id) {
                // Cache valid for 5 minutes
                if Utc::now().signed_duration_since(*timestamp).num_minutes() < 5 {
                    return Ok(analytics.clone());
                }
            }
        }

        // Calculate fresh analytics
        let query = TradeQuery {
            filter: None,
            sort_by: Some(TradeSortBy::Timestamp),
            sort_desc: Some(false),
            page: None,
            page_size: None,
        };

        let trades = self.query_trades(user_id, &query).await?;
        let analytics = self.compute_analytics(&trades);

        // Cache the result
        {
            let mut cache = self.analytics_cache.write().await;
            cache.insert(*user_id, (analytics.clone(), Utc::now()));
        }

        Ok(analytics)
    }

    /// Delete trade
    pub async fn delete_trade(&self, trade_id: &Uuid) -> Result<(), RiskError> {
        // Get trade first to invalidate cache
        if let Some(trade) = self.data_store.get_trade(trade_id).await? {
            self.invalidate_analytics_cache(&trade.user_id).await;
        }

        // Remove from search index
        self.search_index.remove_from_index(trade_id).await?;

        // Delete from storage
        self.data_store.delete_trade(trade_id).await?;

        Ok(())
    }

    /// Invalidate analytics cache for user
    async fn invalidate_analytics_cache(&self, user_id: &UserId) {
        let mut cache = self.analytics_cache.write().await;
        cache.remove(user_id);
    }

    /// Compute analytics from trade data
    fn compute_analytics(&self, trades: &[TradeRecord]) -> TradeAnalytics {
        let total_trades = trades.len() as u64;
        let successful_trades = trades.iter()
            .filter(|t| t.status == TradeStatus::Executed)
            .count() as u64;
        let failed_trades = trades.iter()
            .filter(|t| t.status == TradeStatus::Failed)
            .count() as u64;

        let success_rate = if total_trades > 0 {
            Decimal::from(successful_trades) / Decimal::from(total_trades) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        let total_volume_usd = trades.iter()
            .filter_map(|t| t.output_amount)
            .fold(Decimal::ZERO, |acc, amount| acc + amount);

        let total_pnl_usd = trades.iter()
            .filter_map(|t| t.pnl_usd)
            .fold(Decimal::ZERO, |acc, pnl| acc + pnl);

        let total_fees_usd = trades.iter()
            .map(|t| t.protocol_fees + t.network_fees + t.gas_cost_usd.unwrap_or(Decimal::ZERO))
            .fold(Decimal::ZERO, |acc, fees| acc + fees);

        let average_trade_size_usd = if total_trades > 0 {
            total_volume_usd / Decimal::from(total_trades)
        } else {
            Decimal::ZERO
        };

        let average_execution_time_ms = if total_trades > 0 {
            let total_time: u64 = trades.iter()
                .filter_map(|t| t.execution_time_ms)
                .sum();
            Decimal::from(total_time) / Decimal::from(total_trades)
        } else {
            Decimal::ZERO
        };

        let average_slippage = if total_trades > 0 {
            let total_slippage = trades.iter()
                .filter_map(|t| t.actual_slippage)
                .fold(Decimal::ZERO, |acc, slippage| acc + slippage);
            total_slippage / Decimal::from(total_trades)
        } else {
            Decimal::ZERO
        };

        // Most used DEXes
        let mut dex_usage: HashMap<String, u64> = HashMap::new();
        for trade in trades {
            *dex_usage.entry(trade.dex_used.clone()).or_insert(0) += 1;
        }
        let mut most_used_dexes: Vec<(String, u64)> = dex_usage.into_iter().collect();
        most_used_dexes.sort_by(|a, b| b.1.cmp(&a.1));
        most_used_dexes.truncate(10);

        // Most traded pairs
        let mut pair_usage: HashMap<(String, String), u64> = HashMap::new();
        for trade in trades {
            let pair = (trade.input_token.clone(), trade.output_token.clone());
            *pair_usage.entry(pair).or_insert(0) += 1;
        }
        let mut most_traded_pairs: Vec<(String, String, u64)> = pair_usage.into_iter()
            .map(|((input, output), count)| (input, output, count))
            .collect();
        most_traded_pairs.sort_by(|a, b| b.2.cmp(&a.2));
        most_traded_pairs.truncate(10);

        // Best/worst performing pairs
        let mut pair_pnl: HashMap<(String, String), Vec<Decimal>> = HashMap::new();
        for trade in trades {
            if let Some(pnl) = trade.pnl_usd {
                let pair = (trade.input_token.clone(), trade.output_token.clone());
                pair_pnl.entry(pair).or_insert_with(Vec::new).push(pnl);
            }
        }

        let mut best_performing_pairs: Vec<(String, String, Decimal)> = pair_pnl.iter()
            .map(|((input, output), pnls)| {
                let avg_pnl = pnls.iter().fold(Decimal::ZERO, |acc, pnl| acc + *pnl) / Decimal::from(pnls.len());
                (input.clone(), output.clone(), avg_pnl)
            })
            .collect();
        best_performing_pairs.sort_by(|a, b| b.2.cmp(&a.2));
        best_performing_pairs.truncate(5);

        let mut worst_performing_pairs = best_performing_pairs.clone();
        worst_performing_pairs.sort_by(|a, b| a.2.cmp(&b.2));
        worst_performing_pairs.truncate(5);

        // Hourly distribution
        let mut hourly_distribution: HashMap<u32, u64> = HashMap::new();
        for trade in trades {
            let hour = trade.timestamp.hour();
            *hourly_distribution.entry(hour).or_insert(0) += 1;
        }

        // Daily P&L history (last 30 days)
        let mut daily_pnl: HashMap<DateTime<Utc>, Decimal> = HashMap::new();
        for trade in trades {
            if let Some(pnl) = trade.pnl_usd {
                let date = trade.timestamp.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
                *daily_pnl.entry(date).or_insert(Decimal::ZERO) += pnl;
            }
        }
        let mut daily_pnl_history: Vec<(DateTime<Utc>, Decimal)> = daily_pnl.into_iter().collect();
        daily_pnl_history.sort_by(|a, b| a.0.cmp(&b.0));

        TradeAnalytics {
            total_trades,
            successful_trades,
            failed_trades,
            success_rate,
            total_volume_usd,
            total_pnl_usd,
            total_fees_usd,
            average_trade_size_usd,
            average_execution_time_ms,
            average_slippage,
            most_used_dexes,
            most_traded_pairs,
            best_performing_pairs,
            worst_performing_pairs,
            hourly_trade_distribution: hourly_distribution,
            daily_pnl_history,
        }
    }
}

// Mock implementations for testing
pub struct MockTradeDataStore {
    trades: Arc<RwLock<HashMap<Uuid, TradeRecord>>>,
}

impl MockTradeDataStore {
    pub fn new() -> Self {
        Self {
            trades: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl TradeDataStore for MockTradeDataStore {
    async fn store_trade(&self, trade: &TradeRecord) -> Result<(), RiskError> {
        let mut trades = self.trades.write().await;
        trades.insert(trade.trade_id, trade.clone());
        Ok(())
    }

    async fn update_trade(&self, trade_id: &Uuid, trade: &TradeRecord) -> Result<(), RiskError> {
        let mut trades = self.trades.write().await;
        if trades.contains_key(trade_id) {
            trades.insert(*trade_id, trade.clone());
            Ok(())
        } else {
            Err(RiskError::UserNotFound(Uuid::new_v4()))
        }
    }

    async fn get_trade(&self, trade_id: &Uuid) -> Result<Option<TradeRecord>, RiskError> {
        let trades = self.trades.read().await;
        Ok(trades.get(trade_id).cloned())
    }

    async fn query_trades(&self, user_id: &UserId, query: &TradeQuery) -> Result<Vec<TradeRecord>, RiskError> {
        let trades = self.trades.read().await;
        let mut user_trades: Vec<TradeRecord> = trades.values()
            .filter(|t| t.user_id == *user_id)
            .cloned()
            .collect();

        // Apply filters
        if let Some(filter) = &query.filter {
            user_trades.retain(|trade| {
                if let Some(start_date) = filter.start_date {
                    if trade.timestamp < start_date {
                        return false;
                    }
                }
                if let Some(end_date) = filter.end_date {
                    if trade.timestamp > end_date {
                        return false;
                    }
                }
                if let Some(statuses) = &filter.status {
                    if !statuses.contains(&trade.status) {
                        return false;
                    }
                }
                true
            });
        }

        // Apply sorting
        if let Some(sort_by) = &query.sort_by {
            let desc = query.sort_desc.unwrap_or(false);
            match sort_by {
                TradeSortBy::Timestamp => {
                    user_trades.sort_by(|a, b| if desc { b.timestamp.cmp(&a.timestamp) } else { a.timestamp.cmp(&b.timestamp) });
                }
                TradeSortBy::Amount => {
                    user_trades.sort_by(|a, b| {
                        let a_amount = a.output_amount.unwrap_or(Decimal::ZERO);
                        let b_amount = b.output_amount.unwrap_or(Decimal::ZERO);
                        if desc { b_amount.cmp(&a_amount) } else { a_amount.cmp(&b_amount) }
                    });
                }
                TradeSortBy::PnL => {
                    user_trades.sort_by(|a, b| {
                        let a_pnl = a.pnl_usd.unwrap_or(Decimal::ZERO);
                        let b_pnl = b.pnl_usd.unwrap_or(Decimal::ZERO);
                        if desc { b_pnl.cmp(&a_pnl) } else { a_pnl.cmp(&b_pnl) }
                    });
                }
                _ => {}
            }
        }

        // Apply pagination
        if let (Some(page), Some(page_size)) = (query.page, query.page_size) {
            let start = (page * page_size) as usize;
            let end = start + page_size as usize;
            user_trades = user_trades.into_iter().skip(start).take(page_size as usize).collect();
        }

        Ok(user_trades)
    }

    async fn count_trades(&self, user_id: &UserId, _filter: &Option<TradeFilter>) -> Result<u64, RiskError> {
        let trades = self.trades.read().await;
        let count = trades.values()
            .filter(|t| t.user_id == *user_id)
            .count() as u64;
        Ok(count)
    }

    async fn delete_trade(&self, trade_id: &Uuid) -> Result<(), RiskError> {
        let mut trades = self.trades.write().await;
        trades.remove(trade_id);
        Ok(())
    }
}

pub struct MockTradeSearchIndex;

impl MockTradeSearchIndex {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl TradeSearchIndex for MockTradeSearchIndex {
    async fn index_trade(&self, _trade: &TradeRecord) -> Result<(), RiskError> {
        Ok(())
    }

    async fn search_trades(&self, _user_id: &UserId, _query: &str) -> Result<Vec<Uuid>, RiskError> {
        Ok(Vec::new())
    }

    async fn remove_from_index(&self, _trade_id: &Uuid) -> Result<(), RiskError> {
        Ok(())
    }
}

pub struct MockTradeDataValidator;

impl MockTradeDataValidator {
    pub fn new() -> Self {
        Self
    }
}

impl TradeDataValidator for MockTradeDataValidator {
    fn validate_trade(&self, _trade: &TradeRecord) -> Result<(), RiskError> {
        Ok(())
    }

    fn sanitize_metadata(&self, _metadata: &mut HashMap<String, String>) {
        // Remove any potentially sensitive data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_trade_history_manager() {
        let data_store = Arc::new(MockTradeDataStore::new());
        let search_index = Arc::new(MockTradeSearchIndex::new());
        let validator = Arc::new(MockTradeDataValidator::new());
        
        let manager = TradeHistoryManager::new(data_store, search_index, validator);
        
        let user_id = Uuid::new_v4();
        let trade = TradeRecord {
            trade_id: Uuid::new_v4(),
            user_id,
            trade_type: TradeType::Swap,
            status: TradeStatus::Executed,
            timestamp: Utc::now(),
            execution_timestamp: Some(Utc::now()),
            input_token: "USDC".to_string(),
            output_token: "ETH".to_string(),
            input_amount: Decimal::from(1000),
            output_amount: Some(Decimal::from_f64_retain(0.5).unwrap()),
            expected_output: Decimal::from_f64_retain(0.5).unwrap(),
            dex_used: "Uniswap".to_string(),
            route_path: vec!["USDC".to_string(), "ETH".to_string()],
            slippage_tolerance: Decimal::from_f64_retain(0.5).unwrap(),
            actual_slippage: Some(Decimal::from_f64_retain(0.2).unwrap()),
            gas_used: Some(150000),
            gas_price: Some(Decimal::from(20)),
            gas_cost_usd: Some(Decimal::from(15)),
            protocol_fees: Decimal::from(3),
            network_fees: Decimal::from(15),
            price_impact: Some(Decimal::from_f64_retain(0.1).unwrap()),
            execution_time_ms: Some(2500),
            pnl_usd: Some(Decimal::from(50)),
            transaction_hash: Some("0x123".to_string()),
            block_number: Some(18000000),
            nonce: Some(42),
            metadata: HashMap::new(),
            error_message: None,
        };

        // Test recording trade
        let trade_id = manager.record_trade(trade.clone()).await.unwrap();
        assert_eq!(trade_id, trade.trade_id);

        // Test getting trade
        let retrieved_trade = manager.get_trade(&trade_id).await.unwrap().unwrap();
        assert_eq!(retrieved_trade.trade_id, trade.trade_id);

        // Test analytics calculation
        let analytics = manager.calculate_analytics(&user_id).await.unwrap();
        assert_eq!(analytics.total_trades, 1);
        assert_eq!(analytics.successful_trades, 1);
    }

    #[tokio::test]
    async fn test_trade_query_filtering() {
        let data_store = Arc::new(MockTradeDataStore::new());
        let search_index = Arc::new(MockTradeSearchIndex::new());
        let validator = Arc::new(MockTradeDataValidator::new());
        
        let manager = TradeHistoryManager::new(data_store, search_index, validator);
        
        let user_id = Uuid::new_v4();
        
        // Create multiple trades
        for i in 0..5 {
            let trade = TradeRecord {
                trade_id: Uuid::new_v4(),
                user_id,
                trade_type: TradeType::Swap,
                status: if i % 2 == 0 { TradeStatus::Executed } else { TradeStatus::Failed },
                timestamp: Utc::now(),
                execution_timestamp: Some(Utc::now()),
                input_token: "USDC".to_string(),
                output_token: "ETH".to_string(),
                input_amount: Decimal::from(1000 + i * 100),
                output_amount: Some(Decimal::from_f64_retain(0.5).unwrap()),
                expected_output: Decimal::from_f64_retain(0.5).unwrap(),
                dex_used: "Uniswap".to_string(),
                route_path: vec!["USDC".to_string(), "ETH".to_string()],
                slippage_tolerance: Decimal::from_f64_retain(0.5).unwrap(),
                actual_slippage: Some(Decimal::from_f64_retain(0.2).unwrap()),
                gas_used: Some(150000),
                gas_price: Some(Decimal::from(20)),
                gas_cost_usd: Some(Decimal::from(15)),
                protocol_fees: Decimal::from(3),
                network_fees: Decimal::from(15),
                price_impact: Some(Decimal::from_f64_retain(0.1).unwrap()),
                execution_time_ms: Some(2500),
                pnl_usd: Some(Decimal::from(50)),
                transaction_hash: Some(format!("0x{}", i)),
                block_number: Some(18000000 + i as u64),
                nonce: Some(42 + i as u64),
                metadata: HashMap::new(),
                error_message: None,
            };
            manager.record_trade(trade).await.unwrap();
        }

        // Test query with status filter
        let query = TradeQuery {
            filter: Some(TradeFilter {
                status: Some(vec![TradeStatus::Executed]),
                start_date: None,
                end_date: None,
                token_pairs: None,
                dexes: None,
                trade_types: None,
                min_amount_usd: None,
                max_amount_usd: None,
                min_pnl: None,
                max_pnl: None,
                search_text: None,
            }),
            sort_by: None,
            sort_desc: None,
            page: None,
            page_size: None,
        };

        let filtered_trades = manager.query_trades(&user_id, &query).await.unwrap();
        assert_eq!(filtered_trades.len(), 3); // 3 executed trades (indices 0, 2, 4)

        // Test analytics
        let analytics = manager.calculate_analytics(&user_id).await.unwrap();
        assert_eq!(analytics.total_trades, 5);
        assert_eq!(analytics.successful_trades, 3);
        assert_eq!(analytics.failed_trades, 2);
    }
}
