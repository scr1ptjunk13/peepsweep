use crate::risk_management::types::{UserId, TradeId, RiskError};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Gas usage data for a single transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GasUsageRecord {
    pub transaction_hash: String,
    pub user_id: UserId,
    pub trade_id: TradeId,
    pub gas_used: u64,
    pub gas_price: Decimal, // in Gwei
    pub gas_cost_eth: Decimal,
    pub gas_cost_usd: Decimal,
    pub trade_value_usd: Decimal,
    pub gas_efficiency: Decimal, // gas cost / trade value
    pub dex_name: String,
    pub route_type: String, // "direct", "multi-hop", "aggregated"
    pub token_pair: String,
    pub timestamp: DateTime<Utc>,
    pub block_number: u64,
    pub transaction_status: TransactionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Reverted,
}

/// Gas price data from oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasPriceData {
    pub slow: Decimal,    // Gwei
    pub standard: Decimal, // Gwei
    pub fast: Decimal,    // Gwei
    pub instant: Decimal, // Gwei
    pub timestamp: DateTime<Utc>,
    pub source: String,
}

/// Gas efficiency metrics for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasEfficiencyMetrics {
    pub average_gas_used: u64,
    pub average_gas_price: Decimal,
    pub average_gas_cost_usd: Decimal,
    pub average_efficiency_ratio: Decimal,
    pub total_gas_spent_usd: Decimal,
    pub transaction_count: u64,
    pub failed_transaction_count: u64,
    pub gas_wasted_on_failures: Decimal,
    pub most_efficient_dex: Option<String>,
    pub least_efficient_dex: Option<String>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

/// Transaction monitoring interface
#[async_trait::async_trait]
pub trait TransactionMonitor: Send + Sync {
    async fn get_transaction_receipt(&self, tx_hash: &str) -> Result<Option<TransactionReceipt>, RiskError>;
    async fn get_current_block_number(&self) -> Result<u64, RiskError>;
    async fn monitor_pending_transactions(&self) -> Result<Vec<String>, RiskError>;
}

/// Gas price oracle interface
#[async_trait::async_trait]
pub trait GasPriceOracle: Send + Sync {
    async fn get_current_gas_prices(&self) -> Result<GasPriceData, RiskError>;
    async fn get_historical_gas_prices(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<GasPriceData>, RiskError>;
    async fn predict_optimal_gas_price(&self, target_confirmation_time: u64) -> Result<Decimal, RiskError>;
}

/// Gas efficiency calculator
pub trait GasEfficiencyCalculator: Send + Sync {
    fn calculate_efficiency_ratio(&self, gas_cost_usd: Decimal, trade_value_usd: Decimal) -> Decimal;
    fn calculate_gas_per_dollar(&self, gas_used: u64, trade_value_usd: Decimal) -> Decimal;
    fn compare_route_efficiency(&self, routes: &[GasUsageRecord]) -> Vec<RouteEfficiencyComparison>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceipt {
    pub transaction_hash: String,
    pub block_number: u64,
    pub gas_used: u64,
    pub gas_price: Decimal,
    pub status: TransactionStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEfficiencyComparison {
    pub route_identifier: String,
    pub average_gas_used: u64,
    pub average_efficiency_ratio: Decimal,
    pub transaction_count: u64,
    pub success_rate: Decimal,
}

/// Main gas usage tracker implementation
pub struct GasUsageTracker {
    transaction_monitor: Arc<MockTransactionMonitor>,
    gas_price_oracle: Arc<MockGasPriceOracle>,
    efficiency_calculator: Arc<MockGasEfficiencyCalculator>,
    gas_records: Arc<RwLock<HashMap<String, GasUsageRecord>>>,
    pending_transactions: Arc<RwLock<HashMap<String, PendingTransaction>>>,
}

#[derive(Debug, Clone)]
struct PendingTransaction {
    user_id: UserId,
    trade_id: TradeId,
    expected_gas_limit: u64,
    gas_price: Decimal,
    trade_value_usd: Decimal,
    dex_name: String,
    route_type: String,
    token_pair: String,
    submitted_at: DateTime<Utc>,
}

impl GasUsageTracker {
    pub fn new(
        transaction_monitor: Arc<MockTransactionMonitor>,
        gas_price_oracle: Arc<MockGasPriceOracle>,
        efficiency_calculator: Arc<MockGasEfficiencyCalculator>,
    ) -> Self {
        Self {
            transaction_monitor,
            gas_price_oracle,
            efficiency_calculator,
            gas_records: Arc::new(RwLock::new(HashMap::new())),
            pending_transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a new transaction for gas tracking
    pub async fn track_transaction(
        &self,
        tx_hash: String,
        user_id: UserId,
        trade_id: TradeId,
        expected_gas_limit: u64,
        gas_price: Decimal,
        trade_value_usd: Decimal,
        dex_name: String,
        route_type: String,
        token_pair: String,
    ) -> Result<(), RiskError> {
        let pending_tx = PendingTransaction {
            user_id,
            trade_id,
            expected_gas_limit,
            gas_price,
            trade_value_usd,
            dex_name,
            route_type,
            token_pair,
            submitted_at: Utc::now(),
        };

        let mut pending = self.pending_transactions.write().await;
        pending.insert(tx_hash, pending_tx);
        Ok(())
    }

    /// Update transaction status when confirmed
    pub async fn update_transaction_status(&self, tx_hash: &str) -> Result<(), RiskError> {
        let receipt_opt = self.transaction_monitor.get_transaction_receipt(tx_hash).await?;
        
        if let Some(receipt) = receipt_opt {
            let mut pending = self.pending_transactions.write().await;
            if let Some(pending_tx) = pending.remove(tx_hash) {
                let gas_cost_eth = Decimal::from(receipt.gas_used) * receipt.gas_price / Decimal::from(1_000_000_000u64); // Convert from Gwei
                
                // Get ETH price for USD conversion (simplified - in real implementation, use price oracle)
                let eth_price_usd = Decimal::from(3200); // Placeholder
                let gas_cost_usd = gas_cost_eth * eth_price_usd;
                
                let efficiency_ratio = self.efficiency_calculator.calculate_efficiency_ratio(
                    gas_cost_usd,
                    pending_tx.trade_value_usd,
                );

                let gas_record = GasUsageRecord {
                    transaction_hash: tx_hash.to_string(),
                    user_id: pending_tx.user_id,
                    trade_id: pending_tx.trade_id,
                    gas_used: receipt.gas_used,
                    gas_price: receipt.gas_price,
                    gas_cost_eth,
                    gas_cost_usd,
                    trade_value_usd: pending_tx.trade_value_usd,
                    gas_efficiency: efficiency_ratio,
                    dex_name: pending_tx.dex_name,
                    route_type: pending_tx.route_type,
                    token_pair: pending_tx.token_pair,
                    timestamp: receipt.timestamp,
                    block_number: receipt.block_number,
                    transaction_status: receipt.status,
                };

                let mut records = self.gas_records.write().await;
                records.insert(tx_hash.to_string(), gas_record);
            }
        }

        Ok(())
    }

    /// Get gas usage records for a user within a time range
    pub async fn get_user_gas_usage(
        &self,
        user_id: UserId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<GasUsageRecord>, RiskError> {
        let records = self.gas_records.read().await;
        let filtered_records: Vec<GasUsageRecord> = records
            .values()
            .filter(|record| {
                record.user_id == user_id
                    && record.timestamp >= from
                    && record.timestamp <= to
            })
            .cloned()
            .collect();

        Ok(filtered_records)
    }

    /// Calculate gas efficiency metrics for a user
    pub async fn calculate_gas_efficiency_metrics(
        &self,
        user_id: UserId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<GasEfficiencyMetrics, RiskError> {
        let records = self.get_user_gas_usage(user_id, from, to).await?;
        
        if records.is_empty() {
            return Err(RiskError::InsufficientData("No gas usage data found for the specified period".to_string()));
        }

        let total_gas_used: u64 = records.iter().map(|r| r.gas_used).sum();
        let total_gas_cost_usd: Decimal = records.iter().map(|r| r.gas_cost_usd).sum();
        let total_efficiency_ratio: Decimal = records.iter().map(|r| r.gas_efficiency).sum();
        let failed_transactions: Vec<&GasUsageRecord> = records
            .iter()
            .filter(|r| matches!(r.transaction_status, TransactionStatus::Failed | TransactionStatus::Reverted))
            .collect();

        let gas_wasted_on_failures: Decimal = failed_transactions
            .iter()
            .map(|r| r.gas_cost_usd)
            .sum();

        // Find most and least efficient DEXs
        let mut dex_efficiency: HashMap<String, (Decimal, u64)> = HashMap::new();
        for record in &records {
            let entry = dex_efficiency.entry(record.dex_name.clone()).or_insert((Decimal::ZERO, 0));
            entry.0 += record.gas_efficiency;
            entry.1 += 1;
        }

        let mut dex_avg_efficiency: Vec<(String, Decimal)> = dex_efficiency
            .into_iter()
            .map(|(dex, (total_eff, count))| (dex, total_eff / Decimal::from(count)))
            .collect();
        
        dex_avg_efficiency.sort_by(|a, b| a.1.cmp(&b.1));

        let most_efficient_dex = dex_avg_efficiency.first().map(|(dex, _)| dex.clone());
        let least_efficient_dex = dex_avg_efficiency.last().map(|(dex, _)| dex.clone());

        Ok(GasEfficiencyMetrics {
            average_gas_used: total_gas_used / records.len() as u64,
            average_gas_price: records.iter().map(|r| r.gas_price).sum::<Decimal>() / Decimal::from(records.len()),
            average_gas_cost_usd: total_gas_cost_usd / Decimal::from(records.len()),
            average_efficiency_ratio: total_efficiency_ratio / Decimal::from(records.len()),
            total_gas_spent_usd: total_gas_cost_usd,
            transaction_count: records.len() as u64,
            failed_transaction_count: failed_transactions.len() as u64,
            gas_wasted_on_failures,
            most_efficient_dex,
            least_efficient_dex,
            period_start: from,
            period_end: to,
        })
    }

    /// Get gas usage comparison by DEX
    pub async fn get_dex_gas_comparison(
        &self,
        user_id: UserId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<RouteEfficiencyComparison>, RiskError> {
        let records = self.get_user_gas_usage(user_id, from, to).await?;
        Ok(self.efficiency_calculator.compare_route_efficiency(&records))
    }

    /// Get current gas price recommendations
    pub async fn get_gas_price_recommendations(&self) -> Result<GasPriceData, RiskError> {
        self.gas_price_oracle.get_current_gas_prices().await
    }

    /// Predict optimal gas price for target confirmation time
    pub async fn predict_optimal_gas_price(&self, target_confirmation_time: u64) -> Result<Decimal, RiskError> {
        self.gas_price_oracle.predict_optimal_gas_price(target_confirmation_time).await
    }

    /// Process pending transactions and update their status
    pub async fn process_pending_transactions(&self) -> Result<u64, RiskError> {
        let pending_hashes: Vec<String> = {
            let pending = self.pending_transactions.read().await;
            pending.keys().cloned().collect()
        };

        let mut processed_count = 0;
        for tx_hash in pending_hashes {
            if let Ok(_) = self.update_transaction_status(&tx_hash).await {
                processed_count += 1;
            }
        }

        Ok(processed_count)
    }

    /// Get health status of gas tracking system
    pub async fn get_health_status(&self) -> GasTrackerHealth {
        let pending_count = self.pending_transactions.read().await.len();
        let total_records = self.gas_records.read().await.len();
        
        GasTrackerHealth {
            is_operational: true,
            pending_transactions: pending_count as u64,
            total_tracked_transactions: total_records as u64,
            last_update: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasTrackerHealth {
    pub is_operational: bool,
    pub pending_transactions: u64,
    pub total_tracked_transactions: u64,
    pub last_update: DateTime<Utc>,
}

// Mock implementations for testing
pub struct MockTransactionMonitor {
    pub receipts: Arc<RwLock<HashMap<String, TransactionReceipt>>>,
}

impl MockTransactionMonitor {
    pub fn new() -> Self {
        Self {
            receipts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_receipt(&self, tx_hash: String, receipt: TransactionReceipt) {
        let mut receipts = self.receipts.write().await;
        receipts.insert(tx_hash, receipt);
    }
}

#[async_trait::async_trait]
impl TransactionMonitor for MockTransactionMonitor {
    async fn get_transaction_receipt(&self, tx_hash: &str) -> Result<Option<TransactionReceipt>, RiskError> {
        let receipts = self.receipts.read().await;
        Ok(receipts.get(tx_hash).cloned())
    }

    async fn get_current_block_number(&self) -> Result<u64, RiskError> {
        Ok(18_500_000)
    }

    async fn monitor_pending_transactions(&self) -> Result<Vec<String>, RiskError> {
        Ok(vec![])
    }
}

pub struct MockGasPriceOracle {
    pub current_prices: GasPriceData,
}

impl MockGasPriceOracle {
    pub fn new() -> Self {
        Self {
            current_prices: GasPriceData {
                slow: Decimal::from(20),
                standard: Decimal::from(25),
                fast: Decimal::from(30),
                instant: Decimal::from(35),
                timestamp: Utc::now(),
                source: "mock".to_string(),
            },
        }
    }
}

#[async_trait::async_trait]
impl GasPriceOracle for MockGasPriceOracle {
    async fn get_current_gas_prices(&self) -> Result<GasPriceData, RiskError> {
        Ok(self.current_prices.clone())
    }

    async fn get_historical_gas_prices(&self, _from: DateTime<Utc>, _to: DateTime<Utc>) -> Result<Vec<GasPriceData>, RiskError> {
        Ok(vec![self.current_prices.clone()])
    }

    async fn predict_optimal_gas_price(&self, _target_confirmation_time: u64) -> Result<Decimal, RiskError> {
        Ok(self.current_prices.standard)
    }
}

pub struct DefaultGasEfficiencyCalculator;

impl DefaultGasEfficiencyCalculator {
    pub fn new() -> Self {
        Self
    }
}

impl GasEfficiencyCalculator for DefaultGasEfficiencyCalculator {
    fn calculate_efficiency_ratio(&self, gas_cost_usd: Decimal, trade_value_usd: Decimal) -> Decimal {
        if trade_value_usd.is_zero() {
            return Decimal::MAX;
        }
        gas_cost_usd / trade_value_usd
    }

    fn calculate_gas_per_dollar(&self, gas_used: u64, trade_value_usd: Decimal) -> Decimal {
        if trade_value_usd.is_zero() {
            return Decimal::MAX;
        }
        Decimal::from(gas_used) / trade_value_usd
    }

    fn compare_route_efficiency(&self, records: &[GasUsageRecord]) -> Vec<RouteEfficiencyComparison> {
        let mut route_stats: HashMap<String, (Decimal, u64, u64, u64)> = HashMap::new();
        
        for record in records {
            let route_key = format!("{}_{}", record.dex_name, record.route_type);
            let entry = route_stats.entry(route_key).or_insert((Decimal::ZERO, 0, 0, 0));
            entry.0 += record.gas_efficiency;
            entry.1 += record.gas_used;
            entry.2 += 1; // total transactions
            if matches!(record.transaction_status, TransactionStatus::Confirmed) {
                entry.3 += 1; // successful transactions
            }
        }

        route_stats
            .into_iter()
            .map(|(route, (total_eff, total_gas, total_tx, successful_tx))| {
                RouteEfficiencyComparison {
                    route_identifier: route,
                    average_gas_used: total_gas / total_tx,
                    average_efficiency_ratio: total_eff / Decimal::from(total_tx),
                    transaction_count: total_tx,
                    success_rate: if total_tx > 0 {
                        Decimal::from(successful_tx) / Decimal::from(total_tx)
                    } else {
                        Decimal::ZERO
                    },
                }
            })
            .collect()
    }
}

/// Mock implementation for testing
pub struct MockGasEfficiencyCalculator;

impl MockGasEfficiencyCalculator {
    pub fn new() -> Self {
        Self
    }
}

impl GasEfficiencyCalculator for MockGasEfficiencyCalculator {
    fn calculate_efficiency_ratio(&self, gas_cost_usd: Decimal, trade_value_usd: Decimal) -> Decimal {
        if trade_value_usd.is_zero() {
            return Decimal::MAX;
        }
        gas_cost_usd / trade_value_usd
    }

    fn calculate_gas_per_dollar(&self, gas_used: u64, trade_value_usd: Decimal) -> Decimal {
        if trade_value_usd.is_zero() {
            return Decimal::MAX;
        }
        Decimal::from(gas_used) / trade_value_usd
    }

    fn compare_route_efficiency(&self, records: &[GasUsageRecord]) -> Vec<RouteEfficiencyComparison> {
        // Mock implementation - return empty for simplicity
        Vec::new()
    }
}
