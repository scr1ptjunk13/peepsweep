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
use crate::risk_management::RiskError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub id: Uuid,
    pub token_pair: TokenPair,
    pub source_dex: String,
    pub target_dex: String,
    pub source_price: Decimal,
    pub target_price: Decimal,
    pub price_difference: Decimal,
    pub profit_percentage: Decimal,
    pub estimated_profit_usd: Decimal,
    pub estimated_gas_cost: Decimal,
    pub net_profit_usd: Decimal,
    pub liquidity_available: Decimal,
    pub execution_time_estimate: u64, // milliseconds
    pub confidence_score: f64, // 0.0 to 1.0
    pub detected_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub chain_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub base_token: String,
    pub quote_token: String,
    pub base_token_address: String,
    pub quote_token_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageConfig {
    pub min_profit_threshold: Decimal, // Minimum profit percentage (e.g., 0.02 for 2%)
    pub max_gas_cost_percentage: Decimal, // Max gas cost as % of profit
    pub min_liquidity_usd: Decimal,
    pub max_execution_time_ms: u64,
    pub min_confidence_score: f64,
    pub enabled_chains: Vec<u64>,
    pub enabled_dexes: Vec<String>,
    pub monitored_tokens: Vec<String>,
    pub update_interval_ms: u64,
}

impl Default for ArbitrageConfig {
    fn default() -> Self {
        Self {
            min_profit_threshold: Decimal::from_str("0.02").unwrap(), // 2%
            max_gas_cost_percentage: Decimal::from_str("0.30").unwrap(), // 30% of profit
            min_liquidity_usd: Decimal::from_str("10000").unwrap(), // $10k
            max_execution_time_ms: 30000, // 30 seconds
            min_confidence_score: 0.7,
            enabled_chains: vec![1, 137, 42161, 10], // Ethereum, Polygon, Arbitrum, Optimism
            enabled_dexes: vec![
                "Uniswap".to_string(),
                "Curve".to_string(),
                "Balancer".to_string(),
                "Paraswap".to_string(),
            ],
            monitored_tokens: vec![
                "ETH".to_string(),
                "WETH".to_string(),
                "USDC".to_string(),
                "USDT".to_string(),
                "DAI".to_string(),
                "WBTC".to_string(),
            ],
            update_interval_ms: 5000, // 5 seconds
        }
    }
}

pub struct ArbitrageDetector {
    config: Arc<RwLock<ArbitrageConfig>>,
    dex_aggregator: Arc<DEXAggregator>,
    price_cache: Arc<RwLock<HashMap<String, PriceData>>>,
    opportunities: Arc<RwLock<Vec<ArbitrageOpportunity>>>,
    is_running: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
struct PriceData {
    pub dex: String,
    pub token_pair: TokenPair,
    pub price: Decimal,
    pub liquidity: Decimal,
    pub timestamp: DateTime<Utc>,
    pub chain_id: u64,
}

impl ArbitrageDetector {
    pub fn new(dex_aggregator: Arc<DEXAggregator>, config: ArbitrageConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            dex_aggregator,
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            opportunities: Arc::new(RwLock::new(Vec::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn start_monitoring(&self) -> Result<(), RiskError> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(RiskError::ConfigurationError("Detector is already running".to_string()));
        }
        *is_running = true;
        drop(is_running);

        let detector = Arc::new(self.clone());
        tokio::spawn(async move {
            detector.monitoring_loop().await;
        });

        Ok(())
    }

    pub async fn stop_monitoring(&self) {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
    }

    pub async fn update_config(&self, new_config: ArbitrageConfig) {
        let mut config = self.config.write().await;
        *config = new_config;
    }

    pub async fn get_opportunities(&self) -> Vec<ArbitrageOpportunity> {
        let opportunities = self.opportunities.read().await;
        opportunities.clone()
    }

    pub async fn get_opportunity_by_id(&self, id: Uuid) -> Option<ArbitrageOpportunity> {
        let opportunities = self.opportunities.read().await;
        opportunities.iter().find(|op| op.id == id).cloned()
    }

    // Test helper method to manually add opportunities
    pub async fn add_test_opportunity(&self, opportunity: ArbitrageOpportunity) {
        let mut opportunities = self.opportunities.write().await;
        opportunities.push(opportunity);
    }

    async fn monitoring_loop(&self) {
        while *self.is_running.read().await {
            if let Err(e) = self.scan_for_opportunities().await {
                tracing::error!("Error scanning for arbitrage opportunities: {:?}", e);
            }

            self.cleanup_expired_opportunities().await;

            let config = self.config.read().await;
            let interval = config.update_interval_ms;
            drop(config);

            tokio::time::sleep(tokio::time::Duration::from_millis(interval)).await;
        }
    }

    async fn scan_for_opportunities(&self) -> Result<(), RiskError> {
        let config = self.config.read().await;
        let monitored_tokens = config.monitored_tokens.clone();
        let enabled_dexes = config.enabled_dexes.clone();
        let enabled_chains = config.enabled_chains.clone();
        drop(config);

        // Update price cache for all monitored tokens
        for chain_id in enabled_chains.iter() {
            for token in monitored_tokens.iter() {
                for quote_token in monitored_tokens.iter() {
                    if token == quote_token {
                        continue;
                    }

                    let token_pair = TokenPair {
                        base_token: token.clone(),
                        quote_token: quote_token.clone(),
                        base_token_address: self.get_token_address(token, *chain_id).await,
                        quote_token_address: self.get_token_address(quote_token, *chain_id).await,
                    };

                    for dex in enabled_dexes.iter() {
                        if let Ok(price_data) = self.fetch_price_data(dex, &token_pair, *chain_id).await {
                            let cache_key = format!("{}:{}:{}:{}", dex, token, quote_token, chain_id);
                            let mut cache = self.price_cache.write().await;
                            cache.insert(cache_key, price_data);
                        }
                    }
                }
            }
        }

        // Analyze price differences for arbitrage opportunities
        self.analyze_price_differences().await?;

        Ok(())
    }

    async fn fetch_price_data(&self, dex: &str, token_pair: &TokenPair, chain_id: u64) -> Result<PriceData, RiskError> {
        // Simulate fetching price data from DEX
        // In real implementation, this would call the actual DEX APIs
        let token_in = token_pair.base_token.clone();
        let token_out = token_pair.quote_token.clone();
        let chain = chain_id.to_string();

        let quote_params = QuoteParams {
            token_in: token_in.clone(),
            token_in_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
            token_in_decimals: Some(18),
            token_out: "USDC".to_string(),
            token_out_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
            token_out_decimals: Some(18),
            amount_in: "1000".to_string(),
            slippage: Some(0.5),
            chain: Some(chain.clone()),
        };

        // Mock price data for demonstration
        let base_price = match token_pair.base_token.as_str() {
            "ETH" | "WETH" => Decimal::from_str("3400").unwrap(),
            "USDC" | "USDT" | "DAI" => Decimal::from_str("1").unwrap(),
            "WBTC" => Decimal::from_str("68000").unwrap(),
            _ => Decimal::from_str("100").unwrap(),
        };

        // Add some variance based on DEX
        let price_variance = match dex {
            "Uniswap" => Decimal::from_str("1.0").unwrap(),
            "Curve" => Decimal::from_str("0.998").unwrap(),
            "Balancer" => Decimal::from_str("1.002").unwrap(),
            "Paraswap" => Decimal::from_str("0.999").unwrap(),
            _ => Decimal::from_str("1.0").unwrap(),
        };

        Ok(PriceData {
            dex: dex.to_string(),
            token_pair: token_pair.clone(),
            price: base_price * price_variance,
            liquidity: Decimal::from_str("50000").unwrap(), // $50k liquidity
            timestamp: Utc::now(),
            chain_id,
        })
    }

    async fn analyze_price_differences(&self) -> Result<(), RiskError> {
        let config = self.config.read().await;
        let min_profit = config.min_profit_threshold;
        let max_gas_percentage = config.max_gas_cost_percentage;
        let min_liquidity = config.min_liquidity_usd;
        let min_confidence = config.min_confidence_score;
        drop(config);

        let cache = self.price_cache.read().await;
        let mut price_by_token: HashMap<String, Vec<&PriceData>> = HashMap::new();

        // Group prices by token pair
        for price_data in cache.values() {
            let key = format!("{}:{}", price_data.token_pair.base_token, price_data.token_pair.quote_token);
            price_by_token.entry(key).or_insert_with(Vec::new).push(price_data);
        }

        let mut new_opportunities = Vec::new();

        // Find arbitrage opportunities
        for (token_pair_key, prices) in price_by_token.iter() {
            if prices.len() < 2 {
                continue;
            }

            for i in 0..prices.len() {
                for j in (i + 1)..prices.len() {
                    let price1 = prices[i];
                    let price2 = prices[j];

                    if price1.dex == price2.dex {
                        continue;
                    }

                    let (lower_price, higher_price) = if price1.price < price2.price {
                        (price1, price2)
                    } else {
                        (price2, price1)
                    };

                    let price_diff = higher_price.price - lower_price.price;
                    let profit_percentage = price_diff / lower_price.price;

                    if profit_percentage >= min_profit {
                        // Estimate gas cost (simplified)
                        let estimated_gas_cost = Decimal::from_str("50").unwrap(); // $50 gas cost
                        let trade_amount = std::cmp::min(lower_price.liquidity, higher_price.liquidity);
                        let gross_profit = trade_amount * profit_percentage;
                        let net_profit = gross_profit - estimated_gas_cost;

                        let gas_percentage = estimated_gas_cost / gross_profit;
                        if gas_percentage <= max_gas_percentage && trade_amount >= min_liquidity {
                            let confidence_score = self.calculate_confidence_score(lower_price, higher_price).await;
                            
                            if confidence_score >= min_confidence {
                                let opportunity = ArbitrageOpportunity {
                                    id: Uuid::new_v4(),
                                    token_pair: lower_price.token_pair.clone(),
                                    source_dex: lower_price.dex.clone(),
                                    target_dex: higher_price.dex.clone(),
                                    source_price: lower_price.price,
                                    target_price: higher_price.price,
                                    price_difference: price_diff,
                                    profit_percentage,
                                    estimated_profit_usd: gross_profit,
                                    estimated_gas_cost,
                                    net_profit_usd: net_profit,
                                    liquidity_available: trade_amount,
                                    execution_time_estimate: 15000, // 15 seconds
                                    confidence_score,
                                    detected_at: Utc::now(),
                                    expires_at: Utc::now() + chrono::Duration::minutes(5),
                                    chain_id: lower_price.chain_id,
                                };

                                new_opportunities.push(opportunity);
                            }
                        }
                    }
                }
            }
        }

        // Update opportunities list
        let mut opportunities = self.opportunities.write().await;
        opportunities.extend(new_opportunities);

        // Sort by net profit (highest first)
        opportunities.sort_by(|a, b| b.net_profit_usd.cmp(&a.net_profit_usd));

        // Keep only top 50 opportunities
        opportunities.truncate(50);

        tracing::info!("Found {} arbitrage opportunities", opportunities.len());

        Ok(())
    }

    async fn calculate_confidence_score(&self, price1: &PriceData, price2: &PriceData) -> f64 {
        let mut score = 0.8; // Base confidence

        // Adjust based on liquidity
        let min_liquidity = std::cmp::min(price1.liquidity, price2.liquidity);
        if min_liquidity > Decimal::from_str("100000").unwrap() {
            score += 0.1;
        }

        // Adjust based on price data freshness
        let now = Utc::now();
        let age1 = (now - price1.timestamp).num_seconds();
        let age2 = (now - price2.timestamp).num_seconds();
        
        if age1 < 10 && age2 < 10 {
            score += 0.1;
        } else if age1 > 60 || age2 > 60 {
            score -= 0.2;
        }

        // Ensure score is between 0.0 and 1.0
        f64::max(score, 0.0).min(1.0)
    }

    async fn cleanup_expired_opportunities(&self) {
        let mut opportunities = self.opportunities.write().await;
        let now = Utc::now();
        opportunities.retain(|op| op.expires_at > now);
    }

    async fn get_token_address(&self, token: &str, chain_id: u64) -> String {
        // Mock token addresses - in real implementation, this would be a proper mapping
        match (token, chain_id) {
            ("ETH", _) => "0x0000000000000000000000000000000000000000".to_string(),
            ("WETH", 1) => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
            ("USDC", 1) => "0xA0b86a33E6441E8C8C7014C8C3C8C0C8C0C8C0C8".to_string(),
            ("USDT", 1) => "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
            ("DAI", 1) => "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
            ("WBTC", 1) => "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string(),
            _ => format!("0x{:040x}", token.len() * chain_id as usize),
        }
    }
}

impl Clone for ArbitrageDetector {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            dex_aggregator: Arc::clone(&self.dex_aggregator),
            price_cache: Arc::clone(&self.price_cache),
            opportunities: Arc::clone(&self.opportunities),
            is_running: Arc::clone(&self.is_running),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_retention::ArbitrageConfig;

    #[tokio::test]
    async fn test_arbitrage_detector_creation() {
        // Mock DEXAggregator for testing
        // In real implementation, you'd use the actual DEXAggregator
        let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
        let mock_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
        let detector = ArbitrageDetector::new(mock_aggregator, ArbitrageConfig::default());
        
        let opportunities = detector.get_opportunities().await;
        assert_eq!(opportunities.len(), 0);
    }

    #[tokio::test]
    async fn test_config_update() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
        let mock_aggregator = Arc::new(DEXAggregator::new(redis_client).await.unwrap());
        let detector = ArbitrageDetector::new(mock_aggregator, ArbitrageConfig::default());
        
        let mut new_config = ArbitrageConfig::default();
        new_config.min_profit_threshold = Decimal::from_str("0.05").unwrap(); // 5%
        
        detector.update_config(new_config).await;
        
        let config = detector.config.read().await;
        assert_eq!(config.min_profit_threshold, Decimal::from_str("0.05").unwrap());
    }
}
