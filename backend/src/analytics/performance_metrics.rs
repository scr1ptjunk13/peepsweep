use crate::analytics::data_models::*;
use crate::risk_management::types::{RiskError, UserId};
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use rust_decimal::MathematicalOps;
use rust_decimal_macros::dec;
use uuid::Uuid;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use tracing::{debug, info, warn, error};
use tokio::sync::RwLock;
use std::sync::Arc;

/// Core performance metrics for a trading portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub user_id: Uuid,
    pub calculation_time: DateTime<Utc>,
    pub time_period: TimePeriod,
    
    // Return metrics
    pub total_return_usd: Decimal,
    pub total_return_percentage: Decimal,
    pub annualized_return: Decimal,
    pub daily_return_average: Decimal,
    pub daily_return_volatility: Decimal,
    
    // Risk metrics
    pub sharpe_ratio: Decimal,
    pub sortino_ratio: Decimal,
    pub max_drawdown: Decimal,
    pub max_drawdown_duration_days: i64,
    pub value_at_risk_95: Decimal,
    
    // Trading metrics
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,
    pub win_rate: Decimal,
    pub average_win: Decimal,
    pub average_loss: Decimal,
    pub profit_factor: Decimal,
    
    // Portfolio metrics
    pub starting_portfolio_value: Decimal,
    pub current_portfolio_value: Decimal,
    pub peak_portfolio_value: Decimal,
    pub average_trade_size: Decimal,
    pub total_fees_paid: Decimal,
    pub total_gas_spent: Decimal,
}

/// Time period for performance calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimePeriod {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
    AllTime,
    Custom { start: DateTime<Utc>, end: DateTime<Utc> },
}

/// Benchmark data for performance comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkData {
    pub name: String,
    pub symbol: String,
    pub returns: Vec<BenchmarkReturn>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReturn {
    pub timestamp: DateTime<Utc>,
    pub price: Decimal,
    pub return_percentage: Decimal,
}

/// Performance comparison against benchmarks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceComparison {
    pub user_metrics: PerformanceMetrics,
    pub benchmark_comparisons: Vec<BenchmarkComparison>,
    pub relative_performance_score: Decimal,
    pub percentile_rank: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkComparison {
    pub benchmark_name: String,
    pub user_return: Decimal,
    pub benchmark_return: Decimal,
    pub alpha: Decimal,
    pub beta: Decimal,
    pub correlation: Decimal,
    pub tracking_error: Decimal,
}

/// Historical return data for calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnHistory {
    pub user_id: Uuid,
    pub returns: Vec<DailyReturn>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyReturn {
    pub date: DateTime<Utc>,
    pub portfolio_value: Decimal,
    pub daily_return: Decimal,
    pub cumulative_return: Decimal,
}

/// Drawdown analysis data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownAnalysis {
    pub max_drawdown: Decimal,
    pub max_drawdown_start: DateTime<Utc>,
    pub max_drawdown_end: DateTime<Utc>,
    pub max_drawdown_duration: Duration,
    pub current_drawdown: Decimal,
    pub drawdown_periods: Vec<DrawdownPeriod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownPeriod {
    pub start_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
    pub peak_value: Decimal,
    pub trough_value: Decimal,
    pub drawdown_percentage: Decimal,
    pub recovery_date: Option<DateTime<Utc>>,
}

/// Performance metrics calculator engine
#[derive(Debug)]
pub struct PerformanceMetricsCalculator {
    trade_history: Arc<RwLock<HashMap<Uuid, Vec<TradeRecord>>>>,
    return_history: Arc<RwLock<HashMap<Uuid, ReturnHistory>>>,
    benchmark_data: Arc<RwLock<HashMap<String, BenchmarkData>>>,
    risk_free_rate: Decimal,
    calculation_cache: Arc<RwLock<HashMap<String, CachedCalculation>>>,
}

#[derive(Debug, Clone)]
struct CachedCalculation {
    result: PerformanceMetrics,
    calculated_at: DateTime<Utc>,
    cache_ttl: Duration,
}

impl PerformanceMetricsCalculator {
    pub fn new(risk_free_rate: Decimal) -> Self {
        Self {
            trade_history: Arc::new(RwLock::new(HashMap::new())),
            return_history: Arc::new(RwLock::new(HashMap::new())),
            benchmark_data: Arc::new(RwLock::new(HashMap::new())),
            risk_free_rate,
            calculation_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Calculate comprehensive performance metrics for a user
    pub async fn calculate_performance_metrics(
        &self,
        user_id: &Uuid,
        time_period: TimePeriod,
    ) -> Result<PerformanceMetrics, RiskError> {
        let cache_key = format!("{}_{:?}", user_id, time_period);
        
        // Check cache first
        if let Some(cached) = self.get_cached_calculation(&cache_key).await {
            return Ok(cached.result);
        }

        let start_time = std::time::Instant::now();
        
        // Get trade history and return data
        let trades = self.get_user_trades(user_id).await?;
        let returns = self.get_user_returns(user_id).await?;
        
        if trades.is_empty() || returns.returns.is_empty() {
            return Err(RiskError::InsufficientData("No trade or return data available".to_string()));
        }

        // Filter data by time period
        let (filtered_trades, filtered_returns) = self.filter_data_by_period(&trades, &returns, &time_period);
        
        if filtered_returns.is_empty() {
            return Err(RiskError::InsufficientData("No data available for specified time period".to_string()));
        }

        // Calculate basic return metrics
        let total_return_usd = self.calculate_total_return(&filtered_returns);
        let total_return_percentage = self.calculate_total_return_percentage(&filtered_returns);
        let annualized_return = self.calculate_annualized_return(&filtered_returns, &time_period);
        let (daily_return_average, daily_return_volatility) = self.calculate_return_statistics(&filtered_returns);
        
        // Calculate risk metrics
        let sharpe_ratio = self.calculate_sharpe_ratio(daily_return_average, daily_return_volatility);
        let sortino_ratio = self.calculate_sortino_ratio(&filtered_returns);
        let drawdown_analysis = self.calculate_drawdown_analysis(&filtered_returns);
        let value_at_risk_95 = self.calculate_value_at_risk(&filtered_returns, 0.05);
        
        // Calculate trading metrics
        let trading_metrics = self.calculate_trading_metrics(&filtered_trades);
        
        // Calculate portfolio metrics
        let portfolio_metrics = self.calculate_portfolio_metrics(&filtered_returns, &filtered_trades);
        
        let metrics = PerformanceMetrics {
            user_id: *user_id,
            calculation_time: Utc::now(),
            time_period,
            total_return_usd,
            total_return_percentage,
            annualized_return,
            daily_return_average,
            daily_return_volatility,
            sharpe_ratio,
            sortino_ratio,
            max_drawdown: drawdown_analysis.max_drawdown,
            max_drawdown_duration_days: drawdown_analysis.max_drawdown_duration.num_days(),
            value_at_risk_95,
            total_trades: trading_metrics.0,
            winning_trades: trading_metrics.1,
            losing_trades: trading_metrics.2,
            win_rate: trading_metrics.3,
            average_win: trading_metrics.4,
            average_loss: trading_metrics.5,
            profit_factor: trading_metrics.6,
            starting_portfolio_value: portfolio_metrics.0,
            current_portfolio_value: portfolio_metrics.1,
            peak_portfolio_value: portfolio_metrics.2,
            average_trade_size: portfolio_metrics.3,
            total_fees_paid: portfolio_metrics.4,
            total_gas_spent: portfolio_metrics.5,
        };

        // Cache the result
        self.cache_calculation(cache_key, metrics.clone(), Duration::minutes(15)).await;
        
        let calculation_time = start_time.elapsed();
        debug!("Performance metrics calculated for user {} in {:?}", user_id, calculation_time);
        
        Ok(metrics)
    }

    /// Calculate total return in USD
    fn calculate_total_return(&self, returns: &[DailyReturn]) -> Decimal {
        if returns.is_empty() {
            return Decimal::ZERO;
        }
        
        let first_value = returns.first().unwrap().portfolio_value;
        let last_value = returns.last().unwrap().portfolio_value;
        
        last_value - first_value
    }

    /// Calculate total return percentage
    fn calculate_total_return_percentage(&self, returns: &[DailyReturn]) -> Decimal {
        if returns.is_empty() {
            return Decimal::ZERO;
        }
        
        let first_value = returns.first().unwrap().portfolio_value;
        let last_value = returns.last().unwrap().portfolio_value;
        
        if first_value == Decimal::ZERO {
            return Decimal::ZERO;
        }
        
        ((last_value - first_value) / first_value) * Decimal::from(100)
    }

    /// Calculate annualized return
    fn calculate_annualized_return(&self, returns: &[DailyReturn], time_period: &TimePeriod) -> Decimal {
        if returns.len() < 2 {
            return Decimal::ZERO;
        }
        
        let total_return_pct = self.calculate_total_return_percentage(returns) / Decimal::from(100);
        let days = self.get_period_days(time_period, returns);
        
        if days <= 0 {
            return Decimal::ZERO;
        }
        
        let years = Decimal::from(days) / Decimal::from(365);
        
        if years == Decimal::ZERO {
            return Decimal::ZERO;
        }
        
        // Annualized return = (1 + total_return)^(1/years) - 1
        let one_plus_return = Decimal::ONE + total_return_pct;
        let exponent = Decimal::ONE / years;
        
        // Simplified calculation for now (in production, use proper power function)
        let annualized = if years <= Decimal::ONE {
            total_return_pct / years
        } else {
            total_return_pct * Decimal::from(365) / Decimal::from(days)
        };
        
        annualized * Decimal::from(100)
    }

    /// Calculate daily return statistics
    fn calculate_return_statistics(&self, returns: &[DailyReturn]) -> (Decimal, Decimal) {
        if returns.len() < 2 {
            return (Decimal::ZERO, Decimal::ZERO);
        }
        
        let daily_returns: Vec<Decimal> = returns.iter().map(|r| r.daily_return).collect();
        
        // Calculate average
        let sum: Decimal = daily_returns.iter().sum();
        let average = sum / Decimal::from(daily_returns.len());
        
        // Calculate volatility (standard deviation)
        let variance_sum: Decimal = daily_returns
            .iter()
            .map(|r| (*r - average).powi(2))
            .sum();
        
        let variance = variance_sum / Decimal::from(daily_returns.len() - 1);
        let volatility = variance.sqrt().unwrap_or(Decimal::ZERO);
        
        (average, volatility)
    }

    /// Calculate Sharpe ratio
    fn calculate_sharpe_ratio(&self, average_return: Decimal, volatility: Decimal) -> Decimal {
        if volatility == Decimal::ZERO {
            return Decimal::ZERO;
        }
        
        let daily_risk_free_rate = self.risk_free_rate / Decimal::from(365);
        let excess_return = average_return - daily_risk_free_rate;
        
        excess_return / volatility
    }

    /// Calculate Sortino ratio (downside deviation)
    fn calculate_sortino_ratio(&self, returns: &[DailyReturn]) -> Decimal {
        if returns.len() < 2 {
            return Decimal::ZERO;
        }
        
        let daily_returns: Vec<Decimal> = returns.iter().map(|r| r.daily_return).collect();
        let average: Decimal = daily_returns.iter().sum::<Decimal>() / Decimal::from(daily_returns.len());
        
        // Calculate downside deviation (only negative returns)
        let downside_returns: Vec<Decimal> = daily_returns
            .iter()
            .filter(|&&r| r < Decimal::ZERO)
            .cloned()
            .collect();
        
        if downside_returns.is_empty() {
            return Decimal::MAX; // Perfect Sortino ratio
        }
        
        let downside_variance: Decimal = downside_returns
            .iter()
            .map(|r| r.powi(2))
            .sum::<Decimal>() / Decimal::from(downside_returns.len());
        
        let downside_deviation = downside_variance.sqrt().unwrap_or(Decimal::ZERO);
        
        if downside_deviation == Decimal::ZERO {
            return Decimal::ZERO;
        }
        
        let daily_risk_free_rate = self.risk_free_rate / Decimal::from(365);
        (average - daily_risk_free_rate) / downside_deviation
    }

    /// Calculate drawdown analysis
    fn calculate_drawdown_analysis(&self, returns: &[DailyReturn]) -> DrawdownAnalysis {
        if returns.is_empty() {
            return DrawdownAnalysis {
                max_drawdown: Decimal::ZERO,
                max_drawdown_start: Utc::now(),
                max_drawdown_end: Utc::now(),
                max_drawdown_duration: Duration::zero(),
                current_drawdown: Decimal::ZERO,
                drawdown_periods: vec![],
            };
        }
        
        let mut max_drawdown = Decimal::ZERO;
        let mut max_drawdown_start = returns[0].date;
        let mut max_drawdown_end = returns[0].date;
        let mut peak_value = returns[0].portfolio_value;
        let mut peak_date = returns[0].date;
        let mut drawdown_periods = Vec::new();
        let mut current_drawdown_start: Option<DateTime<Utc>> = None;
        
        for return_data in returns.iter() {
            let current_value = return_data.portfolio_value;
            
            // Update peak
            if current_value > peak_value {
                // End current drawdown period if any
                if let Some(start_date) = current_drawdown_start {
                    let drawdown_pct = ((peak_value - current_value) / peak_value) * Decimal::from(100);
                    if drawdown_pct > Decimal::ZERO {
                        drawdown_periods.push(DrawdownPeriod {
                            start_date,
                            end_date: Some(return_data.date),
                            peak_value,
                            trough_value: current_value,
                            drawdown_percentage: drawdown_pct,
                            recovery_date: Some(return_data.date),
                        });
                    }
                    current_drawdown_start = None;
                }
                
                peak_value = current_value;
                peak_date = return_data.date;
            } else if current_value < peak_value {
                // Start new drawdown period if not already in one
                if current_drawdown_start.is_none() {
                    current_drawdown_start = Some(peak_date);
                }
                
                let drawdown_pct = ((peak_value - current_value) / peak_value) * Decimal::from(100);
                
                // Update max drawdown
                if drawdown_pct > max_drawdown {
                    max_drawdown = drawdown_pct;
                    max_drawdown_start = peak_date;
                    max_drawdown_end = return_data.date;
                }
            }
        }
        
        // Calculate current drawdown
        let last_value = returns.last().unwrap().portfolio_value;
        let current_drawdown = if last_value < peak_value {
            ((peak_value - last_value) / peak_value) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        DrawdownAnalysis {
            max_drawdown,
            max_drawdown_start,
            max_drawdown_end,
            max_drawdown_duration: max_drawdown_end.signed_duration_since(max_drawdown_start),
            current_drawdown,
            drawdown_periods,
        }
    }

    /// Calculate Value at Risk (VaR)
    fn calculate_value_at_risk(&self, returns: &[DailyReturn], confidence_level: f64) -> Decimal {
        if returns.len() < 10 {
            return Decimal::ZERO;
        }
        
        let mut daily_returns: Vec<Decimal> = returns.iter().map(|r| r.daily_return).collect();
        daily_returns.sort();
        
        let index = ((1.0 - confidence_level) * daily_returns.len() as f64) as usize;
        let index = index.min(daily_returns.len() - 1);
        
        -daily_returns[index] // VaR is typically expressed as a positive number
    }

    /// Calculate trading metrics
    fn calculate_trading_metrics(&self, trades: &[TradeRecord]) -> (u64, u64, u64, Decimal, Decimal, Decimal, Decimal) {
        if trades.is_empty() {
            return (0, 0, 0, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
        }
        
        let total_trades = trades.len() as u64;
        let mut winning_trades = 0u64;
        let mut losing_trades = 0u64;
        let mut total_wins = Decimal::ZERO;
        let mut total_losses = Decimal::ZERO;
        
        for trade in trades {
            if trade.pnl_impact.realized_pnl_usd > Decimal::ZERO {
                winning_trades += 1;
                total_wins += trade.pnl_impact.realized_pnl_usd;
            } else if trade.pnl_impact.realized_pnl_usd < Decimal::ZERO {
                losing_trades += 1;
                total_losses += trade.pnl_impact.realized_pnl_usd.abs();
            }
        }
        
        let win_rate = if total_trades > 0 {
            Decimal::from(winning_trades) / Decimal::from(total_trades) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        let average_win = if winning_trades > 0 {
            total_wins / Decimal::from(winning_trades)
        } else {
            Decimal::ZERO
        };
        
        let average_loss = if losing_trades > 0 {
            total_losses / Decimal::from(losing_trades)
        } else {
            Decimal::ZERO
        };
        
        let profit_factor = if total_losses > Decimal::ZERO {
            total_wins / total_losses
        } else if total_wins > Decimal::ZERO {
            Decimal::MAX
        } else {
            Decimal::ZERO
        };
        
        (total_trades, winning_trades, losing_trades, win_rate, average_win, average_loss, profit_factor)
    }

    /// Calculate portfolio metrics
    fn calculate_portfolio_metrics(&self, returns: &[DailyReturn], trades: &[TradeRecord]) -> (Decimal, Decimal, Decimal, Decimal, Decimal, Decimal) {
        if returns.is_empty() {
            return (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
        }
        
        let starting_value = returns.first().unwrap().portfolio_value;
        let current_value = returns.last().unwrap().portfolio_value;
        let peak_value = returns.iter().map(|r| r.portfolio_value).max().unwrap_or(Decimal::ZERO);
        
        let average_trade_size = if !trades.is_empty() {
            trades.iter().map(|t| t.amount_in).sum::<Decimal>() / Decimal::from(trades.len())
        } else {
            Decimal::ZERO
        };
        
        let total_fees = trades.iter().map(|t| t.fees.total_fee_usd).sum();
        let total_gas = trades.iter().map(|t| t.gas_data.gas_cost_usd).sum();
        
        (starting_value, current_value, peak_value, average_trade_size, total_fees, total_gas)
    }

    // Helper methods
    async fn get_user_trades(&self, user_id: &Uuid) -> Result<Vec<TradeRecord>, RiskError> {
        let history = self.trade_history.read().await;
        Ok(history.get(user_id).cloned().unwrap_or_default())
    }

    async fn get_user_returns(&self, user_id: &Uuid) -> Result<ReturnHistory, RiskError> {
        let history = self.return_history.read().await;
        history.get(user_id).cloned().ok_or_else(|| RiskError::UserNotFound(*user_id))
    }

    fn filter_data_by_period(&self, trades: &[TradeRecord], returns: &ReturnHistory, period: &TimePeriod) -> (Vec<TradeRecord>, Vec<DailyReturn>) {
        let (start_time, end_time) = self.get_period_bounds(period, returns);
        
        let filtered_trades: Vec<TradeRecord> = trades
            .iter()
            .filter(|t| t.timestamp >= start_time && t.timestamp <= end_time)
            .cloned()
            .collect();
        
        let filtered_returns: Vec<DailyReturn> = returns.returns
            .iter()
            .filter(|r| r.date >= start_time && r.date <= end_time)
            .cloned()
            .collect();
        
        (filtered_trades, filtered_returns)
    }

    fn get_period_bounds(&self, period: &TimePeriod, returns: &ReturnHistory) -> (DateTime<Utc>, DateTime<Utc>) {
        let now = Utc::now();
        let end_time = now;
        
        let start_time = match period {
            TimePeriod::Daily => now - Duration::days(1),
            TimePeriod::Weekly => now - Duration::weeks(1),
            TimePeriod::Monthly => now - Duration::days(30),
            TimePeriod::Quarterly => now - Duration::days(90),
            TimePeriod::Yearly => now - Duration::days(365),
            TimePeriod::AllTime => returns.returns.first().map(|r| r.date).unwrap_or(now),
            TimePeriod::Custom { start, end: _ } => *start,
        };
        
        let end_time = match period {
            TimePeriod::Custom { start: _, end } => *end,
            _ => end_time,
        };
        
        (start_time, end_time)
    }

    fn get_period_days(&self, period: &TimePeriod, returns: &[DailyReturn]) -> i64 {
        match period {
            TimePeriod::Daily => 1,
            TimePeriod::Weekly => 7,
            TimePeriod::Monthly => 30,
            TimePeriod::Quarterly => 90,
            TimePeriod::Yearly => 365,
            TimePeriod::AllTime => {
                if returns.len() >= 2 {
                    let first_date = returns.first().unwrap().date;
                    let last_date = returns.last().unwrap().date;
                    last_date.signed_duration_since(first_date).num_days()
                } else {
                    1
                }
            },
            TimePeriod::Custom { start, end } => {
                end.signed_duration_since(*start).num_days()
            }
        }
    }

    async fn get_cached_calculation(&self, cache_key: &str) -> Option<CachedCalculation> {
        let cache = self.calculation_cache.read().await;
        if let Some(cached) = cache.get(cache_key) {
            if Utc::now().signed_duration_since(cached.calculated_at) < cached.cache_ttl {
                return Some(cached.clone());
            }
        }
        None
    }

    async fn cache_calculation(&self, cache_key: String, result: PerformanceMetrics, ttl: Duration) {
        let mut cache = self.calculation_cache.write().await;
        cache.insert(cache_key, CachedCalculation {
            result,
            calculated_at: Utc::now(),
            cache_ttl: ttl,
        });
    }

    /// Add trade data for a user
    pub async fn add_user_trades(&self, user_id: Uuid, trades: Vec<TradeRecord>) {
        let mut history = self.trade_history.write().await;
        history.insert(user_id, trades);
    }

    /// Add return history for a user
    pub async fn add_user_returns(&self, user_id: Uuid, returns: ReturnHistory) {
        let mut history = self.return_history.write().await;
        history.insert(user_id, returns);
    }

    /// Update benchmark data
    pub async fn update_benchmark_data(&self, name: String, data: BenchmarkData) {
        let mut benchmarks = self.benchmark_data.write().await;
        benchmarks.insert(name, data);
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            calculation_time: Utc::now(),
            time_period: TimePeriod::AllTime,
            total_return_usd: Decimal::ZERO,
            total_return_percentage: Decimal::ZERO,
            annualized_return: Decimal::ZERO,
            daily_return_average: Decimal::ZERO,
            daily_return_volatility: Decimal::ZERO,
            sharpe_ratio: Decimal::ZERO,
            sortino_ratio: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            max_drawdown_duration_days: 0,
            value_at_risk_95: Decimal::ZERO,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: Decimal::ZERO,
            average_win: Decimal::ZERO,
            average_loss: Decimal::ZERO,
            profit_factor: Decimal::ZERO,
            starting_portfolio_value: Decimal::ZERO,
            current_portfolio_value: Decimal::ZERO,
            peak_portfolio_value: Decimal::ZERO,
            average_trade_size: Decimal::ZERO,
            total_fees_paid: Decimal::ZERO,
            total_gas_spent: Decimal::ZERO,
        }
    }
}
