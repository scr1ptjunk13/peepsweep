use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::types::*;
use crate::aggregator::DEXAggregator;
use crate::user_retention::arbitrage_alerts::detector::ArbitrageOpportunity;
use crate::mev_protection::MevProtectionSuite;
use crate::risk_management::RiskError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub id: Uuid,
    pub user_id: Uuid,
    pub opportunity_id: Uuid,
    pub execution_amount: Decimal, // Amount to trade
    pub max_slippage: Decimal,     // Maximum acceptable slippage
    pub gas_price_gwei: Option<u64>,
    pub deadline_seconds: u64,
    pub use_mev_protection: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub request_id: Uuid,
    pub success: bool,
    pub transaction_hash: Option<String>,
    pub actual_profit: Option<Decimal>,
    pub actual_gas_cost: Option<Decimal>,
    pub execution_time_ms: u64,
    pub slippage_experienced: Option<Decimal>,
    pub error_message: Option<String>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_number: u8,
    pub description: String,
    pub dex: String,
    pub from_token: String,
    pub to_token: String,
    pub amount_in: Decimal,
    pub expected_amount_out: Decimal,
    pub actual_amount_out: Option<Decimal>,
    pub transaction_hash: Option<String>,
    pub gas_used: Option<u64>,
    pub status: StepStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Reverted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub opportunity_id: Uuid,
    pub steps: Vec<ExecutionStep>,
    pub total_estimated_gas: u64,
    pub total_estimated_profit: Decimal,
    pub risk_assessment: RiskAssessment,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub liquidity_risk: RiskLevel,
    pub slippage_risk: RiskLevel,
    pub mev_risk: RiskLevel,
    pub timing_risk: RiskLevel,
    pub overall_risk: RiskLevel,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

pub struct ArbitrageExecutor {
    dex_aggregator: Arc<DEXAggregator>,
    mev_protection: Arc<MevProtectionSuite>,
    execution_history: Arc<RwLock<HashMap<Uuid, ExecutionResult>>>,
    active_executions: Arc<RwLock<HashMap<Uuid, ExecutionPlan>>>,
    success_rate_cache: Arc<RwLock<HashMap<String, f64>>>, // DEX pair -> success rate
}

impl ArbitrageExecutor {
    pub fn new(dex_aggregator: Arc<DEXAggregator>, mev_protection: Arc<MevProtectionSuite>) -> Self {
        Self {
            dex_aggregator,
            mev_protection,
            execution_history: Arc::new(RwLock::new(HashMap::new())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            success_rate_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_execution_plan(
        &self,
        opportunity: &ArbitrageOpportunity,
        execution_amount: Decimal,
    ) -> Result<ExecutionPlan, RiskError> {
        let mut steps = Vec::new();

        // Step 1: Buy from source DEX (lower price)
        let step1 = ExecutionStep {
            step_number: 1,
            description: format!(
                "Buy {} {} from {} at ${:.2}",
                execution_amount,
                opportunity.token_pair.base_token,
                opportunity.source_dex,
                opportunity.source_price
            ),
            dex: opportunity.source_dex.clone(),
            from_token: opportunity.token_pair.quote_token.clone(),
            to_token: opportunity.token_pair.base_token.clone(),
            amount_in: execution_amount * opportunity.source_price,
            expected_amount_out: execution_amount,
            actual_amount_out: None,
            transaction_hash: None,
            gas_used: None,
            status: StepStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
        };

        // Step 2: Sell to target DEX (higher price)
        let step2 = ExecutionStep {
            step_number: 2,
            description: format!(
                "Sell {} {} to {} at ${:.2}",
                execution_amount,
                opportunity.token_pair.base_token,
                opportunity.target_dex,
                opportunity.target_price
            ),
            dex: opportunity.target_dex.clone(),
            from_token: opportunity.token_pair.base_token.clone(),
            to_token: opportunity.token_pair.quote_token.clone(),
            amount_in: execution_amount,
            expected_amount_out: execution_amount * opportunity.target_price,
            actual_amount_out: None,
            transaction_hash: None,
            gas_used: None,
            status: StepStatus::Pending,
            started_at: Utc::now(),
            completed_at: None,
        };

        steps.push(step1);
        steps.push(step2);

        // Estimate gas costs
        let estimated_gas_per_step = 150000u64; // Conservative estimate
        let total_estimated_gas = estimated_gas_per_step * steps.len() as u64;

        // Calculate estimated profit
        let buy_cost = execution_amount * opportunity.source_price;
        let sell_revenue = execution_amount * opportunity.target_price;
        let gross_profit = sell_revenue - buy_cost;
        let gas_cost_estimate = Decimal::from(total_estimated_gas) * Decimal::from_str("0.00000002").unwrap(); // 20 gwei
        let total_estimated_profit = gross_profit - gas_cost_estimate;

        // Perform risk assessment
        let risk_assessment = self.assess_execution_risk(opportunity, execution_amount).await;

        Ok(ExecutionPlan {
            opportunity_id: opportunity.id,
            steps,
            total_estimated_gas,
            total_estimated_profit,
            risk_assessment,
            created_at: Utc::now(),
        })
    }

    pub async fn execute_arbitrage(
        &self,
        request: ExecutionRequest,
        opportunity: ArbitrageOpportunity,
    ) -> Result<ExecutionResult, RiskError> {
        let start_time = std::time::Instant::now();
        let execution_id = request.id;

        tracing::info!("Starting arbitrage execution for opportunity {}", opportunity.id);

        // Create execution plan
        let mut plan = self.create_execution_plan(&opportunity, request.execution_amount).await?;
        
        // Store active execution
        {
            let mut active = self.active_executions.write().await;
            active.insert(execution_id, plan.clone());
        }

        // Check if opportunity is still valid
        if opportunity.expires_at < Utc::now() {
            return Ok(ExecutionResult {
                request_id: execution_id,
                success: false,
                transaction_hash: None,
                actual_profit: None,
                actual_gas_cost: None,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                slippage_experienced: None,
                error_message: Some("Opportunity expired".to_string()),
                completed_at: Utc::now(),
            });
        }

        // Execute steps sequentially
        let mut total_gas_used = 0u64;
        let mut transaction_hashes = Vec::new();
        let mut actual_profit = Decimal::ZERO;

        // Get the initial cost before we start mutating steps
        let initial_cost = plan.steps.first()
            .map(|step| step.amount_in)
            .unwrap_or(Decimal::ZERO);

        // Execute each step in sequence
        for step in &mut plan.steps {
            step.status = StepStatus::InProgress;
            step.started_at = Utc::now();

            match self.execute_step(step, &ExecutionRequest {
                id: Uuid::new_v4(),
                user_id: Uuid::new_v4(),
                opportunity_id: plan.opportunity_id,
                execution_amount: step.amount_in,
                max_slippage: Decimal::from_str("0.01").unwrap(),
                gas_price_gwei: Some(50),
                deadline_seconds: 300,
                use_mev_protection: true,
                created_at: Utc::now(),
            }).await {
                Ok((tx_hash, gas_used, amount_out)) => {
                    step.status = StepStatus::Completed;
                    step.completed_at = Some(Utc::now());
                    step.gas_used = Some(gas_used);
                    step.actual_amount_out = Some(amount_out);

                    total_gas_used += gas_used;
                    transaction_hashes.push(tx_hash);

                    // Calculate profit for this step
                    if step.step_number == 2 {
                        // This is the sell step, calculate final profit
                        let revenue = amount_out;
                        actual_profit = revenue - initial_cost;
                    }
                }
                Err(e) => {
                    step.status = StepStatus::Failed;
                    step.completed_at = Some(Utc::now());

                    tracing::error!("Step {} failed: {:?}", step.step_number, e);

                    // If first step fails, just return error
                    if step.step_number == 1 {
                        return Err(RiskError::ExecutionError(format!("First step failed: {:?}", e)));
                    }

                    // For later steps, we might have partial execution
                    return Ok(ExecutionResult {
                        request_id: plan.opportunity_id,
                        success: false,
                        transaction_hash: transaction_hashes.first().cloned(),
                        actual_profit: None,
                        actual_gas_cost: Some(Decimal::from(total_gas_used) * Decimal::from_str("0.00000002").unwrap()),
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        slippage_experienced: None,
                        error_message: Some(format!("Step {} failed: {}", step.step_number, e)),
                        completed_at: Utc::now(),
                    });
                }
            }
        }

        // Calculate final metrics
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let actual_gas_cost = Decimal::from(total_gas_used) * Decimal::from_str("0.00000002").unwrap();
        let net_profit = actual_profit - actual_gas_cost;

        // Calculate slippage
        let expected_profit = plan.total_estimated_profit;
        let slippage_experienced = if expected_profit > Decimal::ZERO {
            (expected_profit - net_profit) / expected_profit
        } else {
            Decimal::ZERO
        };

        let success = net_profit > Decimal::ZERO;

        // Update success rate cache
        self.update_success_rate(&opportunity.source_dex, &opportunity.target_dex, success).await;

        // Remove from active executions
        {
            let mut active = self.active_executions.write().await;
            active.remove(&execution_id);
        }

        let result = ExecutionResult {
            request_id: plan.opportunity_id,
            success,
            transaction_hash: transaction_hashes.first().cloned(),
            actual_profit: Some(net_profit),
            actual_gas_cost: Some(actual_gas_cost),
            execution_time_ms,
            slippage_experienced: Some(slippage_experienced),
            error_message: if success { None } else { Some("Execution completed but unprofitable".to_string()) },
            completed_at: Utc::now(),
        };

        // Store in history
        {
            let mut history = self.execution_history.write().await;
            history.insert(execution_id, result.clone());
        }

        tracing::info!(
            "Arbitrage execution completed: success={}, profit=${:.2}, time={}ms",
            success,
            f64::try_from(net_profit).unwrap_or(0.0),
            execution_time_ms
        );

        Ok(result)
    }

    async fn execute_step(
        &self,
        step: &ExecutionStep,
        request: &ExecutionRequest,
    ) -> Result<(String, u64, Decimal), RiskError> {
        // Simulate DEX interaction
        // In a real implementation, this would call the actual DEX contracts
        
        tracing::info!("Executing step {}: {} on {}", step.step_number, step.description, step.dex);

        // Simulate execution delay
        tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

        // Simulate some slippage (0.1% to 0.5%)
        let slippage_factor = 1.0 - (rand::random::<f64>() * 0.004 + 0.001);
        let actual_amount_out = step.expected_amount_out * Decimal::try_from(slippage_factor).unwrap();

        // Simulate gas usage
        let gas_used = 120000 + (rand::random::<u64>() % 30000); // 120k-150k gas

        // Generate mock transaction hash
        let tx_hash = format!("0x{:064x}", rand::random::<u64>());

        // Note: Random failures disabled for integration tests
        // Simulate occasional failures (5% chance)
        // if rand::random::<f64>() < 0.05 {
        //     return Err(RiskError::ExecutionError("Simulated transaction failure".to_string()));
        // }

        Ok((tx_hash, gas_used, actual_amount_out))
    }

    async fn assess_execution_risk(
        &self,
        opportunity: &ArbitrageOpportunity,
        execution_amount: Decimal,
    ) -> RiskAssessment {
        let mut recommendations = Vec::new();

        // Assess liquidity risk
        let liquidity_risk = if opportunity.liquidity_available < execution_amount * Decimal::from(2) {
            recommendations.push("Consider reducing execution amount due to limited liquidity".to_string());
            RiskLevel::High
        } else if opportunity.liquidity_available < execution_amount * Decimal::from(5) {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        // Assess slippage risk based on execution amount vs liquidity
        let liquidity_ratio = execution_amount / opportunity.liquidity_available;
        let slippage_risk = if liquidity_ratio > Decimal::from_str("0.1").unwrap() {
            recommendations.push("High slippage expected due to large trade size".to_string());
            RiskLevel::High
        } else if liquidity_ratio > Decimal::from_str("0.05").unwrap() {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        // Assess MEV risk
        let mev_risk = if opportunity.profit_percentage > Decimal::from_str("0.05").unwrap() {
            recommendations.push("High profit opportunity may attract MEV bots - consider MEV protection".to_string());
            RiskLevel::High
        } else if opportunity.profit_percentage > Decimal::from_str("0.02").unwrap() {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        // Assess timing risk
        let time_until_expiry = (opportunity.expires_at - Utc::now()).num_seconds();
        let timing_risk = if time_until_expiry < 30 {
            recommendations.push("Opportunity expires soon - execute immediately".to_string());
            RiskLevel::High
        } else if time_until_expiry < 120 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        };

        // Calculate overall risk
        let risk_scores = [&liquidity_risk, &slippage_risk, &mev_risk, &timing_risk];
        let overall_risk = match risk_scores.iter().max_by_key(|r| match r {
            RiskLevel::Low => 0,
            RiskLevel::Medium => 1,
            RiskLevel::High => 2,
            RiskLevel::Critical => 3,
        }) {
            Some(RiskLevel::High) => RiskLevel::High,
            Some(RiskLevel::Medium) => RiskLevel::Medium,
            _ => RiskLevel::Low,
        };

        if recommendations.is_empty() {
            recommendations.push("Execution looks safe to proceed".to_string());
        }

        RiskAssessment {
            liquidity_risk,
            slippage_risk,
            mev_risk,
            timing_risk,
            overall_risk,
            recommendations,
        }
    }

    async fn update_success_rate(&self, source_dex: &str, target_dex: &str, success: bool) {
        let key = format!("{}:{}", source_dex, target_dex);
        let mut cache = self.success_rate_cache.write().await;
        
        // Simple moving average with decay
        let current_rate = cache.get(&key).copied().unwrap_or(0.8); // Default 80% success rate
        let new_rate = if success {
            current_rate * 0.9 + 0.1 // Increase slightly on success
        } else {
            current_rate * 0.9       // Decrease on failure
        };
        
        cache.insert(key, new_rate.max(0.0).min(1.0));
    }

    pub async fn get_execution_history(&self, user_id: Option<Uuid>) -> Vec<ExecutionResult> {
        let history = self.execution_history.read().await;
        history.values().cloned().collect()
    }

    pub async fn get_active_executions(&self) -> Vec<ExecutionPlan> {
        let active = self.active_executions.read().await;
        active.values().cloned().collect()
    }

    pub async fn get_success_rate(&self, source_dex: &str, target_dex: &str) -> f64 {
        let key = format!("{}:{}", source_dex, target_dex);
        let cache = self.success_rate_cache.read().await;
        cache.get(&key).copied().unwrap_or(0.8) // Default 80%
    }

    pub async fn cancel_execution(&self, execution_id: Uuid) -> Result<(), RiskError> {
        let mut active = self.active_executions.write().await;
        if active.remove(&execution_id).is_some() {
            tracing::info!("Execution {} cancelled", execution_id);
            Ok(())
        } else {
            Err(RiskError::NotFound("Execution not found or already completed".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_retention::arbitrage_alerts::detector::{TokenPair, ArbitrageOpportunity};

    #[tokio::test]
    async fn test_execution_plan_creation() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
        let mock_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
        let mock_mev = Arc::new(MevProtectionSuite::new().await.unwrap());
        let executor = ArbitrageExecutor::new(mock_aggregator, mock_mev);

        let opportunity = ArbitrageOpportunity {
            id: Uuid::new_v4(),
            token_pair: TokenPair {
                base_token: "ETH".to_string(),
                quote_token: "USDC".to_string(),
                base_token_address: "0x123".to_string(),
                quote_token_address: "0x456".to_string(),
            },
            source_dex: "Uniswap".to_string(),
            target_dex: "Curve".to_string(),
            source_price: Decimal::from_str("3400").unwrap(),
            target_price: Decimal::from_str("3468").unwrap(),
            price_difference: Decimal::from_str("68").unwrap(),
            profit_percentage: Decimal::from_str("0.02").unwrap(),
            estimated_profit_usd: Decimal::from_str("680").unwrap(),
            estimated_gas_cost: Decimal::from_str("50").unwrap(),
            net_profit_usd: Decimal::from_str("630").unwrap(),
            liquidity_available: Decimal::from_str("50000").unwrap(),
            execution_time_estimate: 15000,
            confidence_score: 0.85,
            detected_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            chain_id: 1,
        };

        let execution_amount = Decimal::from_str("10").unwrap(); // 10 ETH
        let plan = executor.create_execution_plan(&opportunity, execution_amount).await.unwrap();

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].step_number, 1);
        assert_eq!(plan.steps[1].step_number, 2);
        assert!(plan.total_estimated_gas > 0);
    }

    #[tokio::test]
    async fn test_risk_assessment() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
        let mock_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
        let mock_mev = Arc::new(MevProtectionSuite::new().await.unwrap());
        let executor = ArbitrageExecutor::new(mock_aggregator, mock_mev);

        let opportunity = ArbitrageOpportunity {
            id: Uuid::new_v4(),
            token_pair: TokenPair {
                base_token: "ETH".to_string(),
                quote_token: "USDC".to_string(),
                base_token_address: "0x123".to_string(),
                quote_token_address: "0x456".to_string(),
            },
            source_dex: "Uniswap".to_string(),
            target_dex: "Curve".to_string(),
            source_price: Decimal::from_str("3400").unwrap(),
            target_price: Decimal::from_str("3468").unwrap(),
            price_difference: Decimal::from_str("68").unwrap(),
            profit_percentage: Decimal::from_str("0.08").unwrap(), // 8% profit - high MEV risk
            estimated_profit_usd: Decimal::from_str("2720").unwrap(),
            estimated_gas_cost: Decimal::from_str("50").unwrap(),
            net_profit_usd: Decimal::from_str("2670").unwrap(),
            liquidity_available: Decimal::from_str("50000").unwrap(),
            execution_time_estimate: 15000,
            confidence_score: 0.85,
            detected_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(20), // Expires soon - high timing risk
            chain_id: 1,
        };

        let execution_amount = Decimal::from_str("10").unwrap();
        let risk = executor.assess_execution_risk(&opportunity, execution_amount).await;

        // Should have high MEV risk due to 8% profit
        assert!(matches!(risk.mev_risk, RiskLevel::High));
        
        // Should have high timing risk due to soon expiry
        assert!(matches!(risk.timing_risk, RiskLevel::High));
        
        // Overall risk should be high
        assert!(matches!(risk.overall_risk, RiskLevel::High));
        
        // Should have recommendations
        assert!(!risk.recommendations.is_empty());
    }
}
