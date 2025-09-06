use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// User identifier type
pub type UserId = Uuid;

/// Token address type
pub type TokenAddress = String;

/// DEX identifier type
pub type DexId = String;

/// Trade identifier type
pub type TradeId = Uuid;

/// P&L data structure for real-time tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PnLData {
    pub user_id: UserId,
    pub timestamp: DateTime<Utc>,
    pub unrealized_pnl_usd: Decimal,
    pub realized_pnl_usd: Decimal,
    pub total_pnl_usd: Decimal,
    pub unrealized_pnl_eth: Decimal,
    pub realized_pnl_eth: Decimal,
    pub total_pnl_eth: Decimal,
    pub unrealized_pnl_btc: Decimal,
    pub realized_pnl_btc: Decimal,
    pub total_pnl_btc: Decimal,
    pub portfolio_value_usd: Decimal,
    pub cost_basis_usd: Decimal,
    pub total_return_percentage: Decimal,
    pub daily_pnl_usd: Decimal,
    pub positions: HashMap<TokenAddress, PositionPnL>,
}

/// Position-specific P&L data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionPnL {
    pub token: String,
    pub amount: Decimal,
    pub value_usd: Decimal,
    pub pnl: Decimal,
    pub pnl_percent: Decimal,
    pub token_address: TokenAddress,
    pub token_symbol: String,
    pub quantity: Decimal,
    pub average_entry_price_usd: Decimal,
    pub current_price_usd: Decimal,
    pub unrealized_pnl_usd: Decimal,
    pub realized_pnl_usd: Decimal,
    pub cost_basis_usd: Decimal,
    pub market_value_usd: Decimal,
    pub return_percentage: Decimal,
    pub last_updated: DateTime<Utc>,
}

/// Performance metrics data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceMetrics {
    pub user_id: UserId,
    pub calculation_date: DateTime<Utc>,
    pub total_return_percentage: Decimal,
    pub annualized_return_percentage: Decimal,
    pub sharpe_ratio: Decimal,
    pub sortino_ratio: Decimal,
    pub maximum_drawdown_percentage: Decimal,
    pub current_drawdown_percentage: Decimal,
    pub win_rate_percentage: Decimal,
    pub profit_factor: Decimal,
    pub average_win_percentage: Decimal,
    pub average_loss_percentage: Decimal,
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub largest_win_percentage: Decimal,
    pub largest_loss_percentage: Decimal,
    pub average_trade_size_usd: Decimal,
    pub total_volume_usd: Decimal,
    pub total_fees_paid_usd: Decimal,
    pub benchmark_comparison: BenchmarkComparison,
    pub risk_metrics: RiskMetrics,
}

/// Benchmark comparison data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkComparison {
    pub vs_eth_return_percentage: Decimal,
    pub vs_btc_return_percentage: Decimal,
    pub vs_sp500_return_percentage: Decimal,
    pub alpha_vs_eth: Decimal,
    pub alpha_vs_btc: Decimal,
    pub beta_vs_eth: Decimal,
    pub beta_vs_btc: Decimal,
    pub correlation_with_eth: Decimal,
    pub correlation_with_btc: Decimal,
}

/// Risk metrics data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskMetrics {
    pub value_at_risk_95_percentage: Decimal,
    pub value_at_risk_99_percentage: Decimal,
    pub conditional_var_95_percentage: Decimal,
    pub volatility_percentage: Decimal,
    pub downside_volatility_percentage: Decimal,
    pub skewness: Decimal,
    pub kurtosis: Decimal,
    pub calmar_ratio: Decimal,
    pub sterling_ratio: Decimal,
}

/// Gas usage data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasUsageData {
    pub user_id: UserId,
    pub trade_id: TradeId,
    pub transaction_hash: String,
    pub timestamp: DateTime<Utc>,
    pub gas_used: u64,
    pub gas_price_gwei: Decimal,
    pub gas_cost_eth: Decimal,
    pub gas_cost_usd: Decimal,
    pub trade_value_usd: Decimal,
    pub gas_efficiency_bps: Decimal, // Gas cost as basis points of trade value
    pub dex_id: DexId,
    pub token_pair: (TokenAddress, TokenAddress),
    pub route_hops: u8,
    pub transaction_type: TransactionType,
    pub execution_status: ExecutionStatus,
    pub block_number: u64,
    pub gas_optimization_score: Decimal, // 0-100 score
}

/// Transaction type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    Swap,
    AddLiquidity,
    RemoveLiquidity,
    Bridge,
    Approval,
    MultiHop,
    Batch,
}

/// Execution status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Confirmed,
    Failed,
    Reverted,
}

/// Gas optimization report data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasOptimizationReport {
    pub user_id: UserId,
    pub report_date: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_gas_used: u64,
    pub total_gas_cost_eth: Decimal,
    pub total_gas_cost_usd: Decimal,
    pub average_gas_price_gwei: Decimal,
    pub gas_efficiency_score: Decimal, // 0-100
    pub potential_savings_usd: Decimal,
    pub optimization_recommendations: Vec<GasOptimizationRecommendation>,
    pub gas_usage_by_dex: HashMap<DexId, GasUsageByDex>,
    pub gas_usage_trends: GasUsageTrends,
}

/// Gas optimization recommendation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasOptimizationRecommendation {
    pub recommendation_type: RecommendationType,
    pub title: String,
    pub description: String,
    pub potential_savings_usd: Decimal,
    pub confidence_score: Decimal, // 0-1
    pub implementation_difficulty: DifficultyLevel,
    pub affected_routes: Vec<String>,
}

/// Recommendation type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecommendationType {
    RouteOptimization,
    TimingOptimization,
    BatchingOpportunity,
    DEXSelection,
    GasPriceStrategy,
    TransactionBundling,
}

/// Implementation difficulty level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DifficultyLevel {
    Easy,
    Medium,
    Hard,
}

/// Gas usage by DEX data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasUsageByDex {
    pub dex_id: DexId,
    pub total_gas_used: u64,
    pub total_gas_cost_usd: Decimal,
    pub average_gas_per_trade: u64,
    pub efficiency_score: Decimal,
    pub trade_count: u64,
}

/// Gas usage trends data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasUsageTrends {
    pub daily_gas_usage: Vec<(DateTime<Utc>, u64)>,
    pub gas_price_trend: Vec<(DateTime<Utc>, Decimal)>,
    pub efficiency_trend: Vec<(DateTime<Utc>, Decimal)>,
    pub peak_usage_hours: Vec<u8>, // Hours of day (0-23)
    pub optimal_trading_hours: Vec<u8>, // Hours with lowest gas prices
}

/// Trade record data structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeRecord {
    pub trade_id: TradeId,
    pub user_id: UserId,
    pub timestamp: DateTime<Utc>,
    pub token_in: TokenAddress,
    pub token_out: TokenAddress,
    pub token_in_symbol: String,
    pub token_out_symbol: String,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub expected_amount_out: Decimal,
    pub slippage_percentage: Decimal,
    pub price_impact_percentage: Decimal,
    pub dex_id: DexId,
    pub route: Vec<RouteHop>,
    pub gas_data: GasUsageData,
    pub fees: TradeFees,
    pub execution_time_ms: u64,
    pub trade_status: TradeStatus,
    pub pnl_impact: TradePnLImpact,
    pub market_conditions: MarketConditions,
}

/// Route hop information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteHop {
    pub dex_id: DexId,
    pub token_in: TokenAddress,
    pub token_out: TokenAddress,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub pool_address: String,
    pub fee_tier: Decimal,
}

/// Trade fees breakdown
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeFees {
    pub protocol_fee_usd: Decimal,
    pub dex_fee_usd: Decimal,
    pub gas_fee_usd: Decimal,
    pub total_fee_usd: Decimal,
    pub fee_percentage_of_trade: Decimal,
}

/// Trade status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TradeStatus {
    Pending,
    Executed,
    PartiallyFilled,
    Failed,
    Cancelled,
    Expired,
}

/// Trade P&L impact
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradePnLImpact {
    pub realized_pnl_usd: Decimal,
    pub position_change: PositionChange,
    pub portfolio_weight_change: Decimal,
    pub risk_contribution_change: Decimal,
}

/// Position change data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionChange {
    pub token_address: TokenAddress,
    pub quantity_change: Decimal,
    pub average_price_change: Decimal,
    pub cost_basis_change: Decimal,
}

/// Market conditions at trade time
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MarketConditions {
    pub volatility_24h: Decimal,
    pub volume_24h_usd: Decimal,
    pub liquidity_depth_usd: Decimal,
    pub spread_bps: Decimal,
    pub market_trend: MarketTrend,
    pub gas_price_percentile: Decimal, // 0-100
}

/// Market trend enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MarketTrend {
    Bullish,
    Bearish,
    Sideways,
    Volatile,
}

/// Analytics cache key structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    pub key_type: CacheKeyType,
    pub user_id: Option<UserId>,
    pub time_range: Option<TimeRange>,
    pub additional_params: HashMap<String, String>,
}

/// Cache key type enumeration
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CacheKeyType {
    PnLData,
    PerformanceMetrics,
    GasOptimizationReport,
    GasUsageData,
    TradeHistory,
    PriceData,
    BenchmarkData,
}

/// Time range for analytics queries
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Analytics job data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsJob {
    pub job_id: Uuid,
    pub job_type: JobType,
    pub user_id: Option<UserId>,
    pub parameters: HashMap<String, String>,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub priority: JobPriority,
}

/// Job type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobType {
    CalculatePnL,
    UpdatePerformanceMetrics,
    GenerateGasReport,
    UpdateTradeHistory,
    RecalculateBenchmarks,
    CleanupOldData,
}

/// Job status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

/// Job priority enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for PnLData {
    fn default() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            unrealized_pnl_usd: Decimal::ZERO,
            realized_pnl_usd: Decimal::ZERO,
            total_pnl_usd: Decimal::ZERO,
            unrealized_pnl_eth: Decimal::ZERO,
            realized_pnl_eth: Decimal::ZERO,
            total_pnl_eth: Decimal::ZERO,
            unrealized_pnl_btc: Decimal::ZERO,
            realized_pnl_btc: Decimal::ZERO,
            total_pnl_btc: Decimal::ZERO,
            portfolio_value_usd: Decimal::ZERO,
            cost_basis_usd: Decimal::ZERO,
            total_return_percentage: Decimal::ZERO,
            daily_pnl_usd: Decimal::ZERO,
            positions: HashMap::new(),
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            calculation_date: Utc::now(),
            total_return_percentage: Decimal::ZERO,
            annualized_return_percentage: Decimal::ZERO,
            sharpe_ratio: Decimal::ZERO,
            sortino_ratio: Decimal::ZERO,
            maximum_drawdown_percentage: Decimal::ZERO,
            current_drawdown_percentage: Decimal::ZERO,
            win_rate_percentage: Decimal::ZERO,
            profit_factor: Decimal::ZERO,
            average_win_percentage: Decimal::ZERO,
            average_loss_percentage: Decimal::ZERO,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            largest_win_percentage: Decimal::ZERO,
            largest_loss_percentage: Decimal::ZERO,
            average_trade_size_usd: Decimal::ZERO,
            total_volume_usd: Decimal::ZERO,
            total_fees_paid_usd: Decimal::ZERO,
            benchmark_comparison: BenchmarkComparison::default(),
            risk_metrics: RiskMetrics::default(),
        }
    }
}

impl Default for BenchmarkComparison {
    fn default() -> Self {
        Self {
            vs_eth_return_percentage: Decimal::ZERO,
            vs_btc_return_percentage: Decimal::ZERO,
            vs_sp500_return_percentage: Decimal::ZERO,
            alpha_vs_eth: Decimal::ZERO,
            alpha_vs_btc: Decimal::ZERO,
            beta_vs_eth: Decimal::ZERO,
            beta_vs_btc: Decimal::ZERO,
            correlation_with_eth: Decimal::ZERO,
            correlation_with_btc: Decimal::ZERO,
        }
    }
}

impl Default for RiskMetrics {
    fn default() -> Self {
        Self {
            value_at_risk_95_percentage: Decimal::ZERO,
            value_at_risk_99_percentage: Decimal::ZERO,
            conditional_var_95_percentage: Decimal::ZERO,
            volatility_percentage: Decimal::ZERO,
            downside_volatility_percentage: Decimal::ZERO,
            skewness: Decimal::ZERO,
            kurtosis: Decimal::ZERO,
            calmar_ratio: Decimal::ZERO,
            sterling_ratio: Decimal::ZERO,
        }
    }
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }
    
    pub fn last_24h() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::hours(24);
        Self::new(start, end)
    }
    
    pub fn last_7d() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(7);
        Self::new(start, end)
    }
    
    pub fn last_30d() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(30);
        Self::new(start, end)
    }
    
    pub fn last_90d() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(90);
        Self::new(start, end)
    }
    
    pub fn last_1y() -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::days(365);
        Self::new(start, end)
    }
}

impl CacheKey {
    pub fn new(key_type: CacheKeyType) -> Self {
        Self {
            key_type,
            user_id: None,
            time_range: None,
            additional_params: HashMap::new(),
        }
    }
    
    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }
    
    pub fn with_time_range(mut self, time_range: TimeRange) -> Self {
        self.time_range = Some(time_range);
        self
    }
    
    pub fn with_param(mut self, key: String, value: String) -> Self {
        self.additional_params.insert(key, value);
        self
    }
    
    pub fn to_string(&self) -> String {
        let mut parts = vec![format!("{:?}", self.key_type)];
        
        if let Some(user_id) = &self.user_id {
            parts.push(format!("user:{}", user_id));
        }
        
        if let Some(time_range) = &self.time_range {
            parts.push(format!("range:{}:{}", 
                time_range.start.timestamp(), 
                time_range.end.timestamp()
            ));
        }
        
        for (key, value) in &self.additional_params {
            parts.push(format!("{}:{}", key, value));
        }
        
        parts.join(":")
    }
}
