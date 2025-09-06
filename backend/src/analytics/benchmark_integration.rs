use crate::analytics::performance_metrics::{BenchmarkData, BenchmarkReturn, PerformanceComparison, BenchmarkComparison};
use crate::risk_management::types::RiskError;
use chrono::{DateTime, Utc, Duration};
use reqwest::Client;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Benchmark data manager for performance comparisons
#[derive(Debug)]
pub struct BenchmarkDataManager {
    client: Client,
    benchmark_cache: Arc<RwLock<HashMap<String, BenchmarkData>>>,
    price_cache: Arc<RwLock<HashMap<String, PriceCacheEntry>>>,
    cache_ttl_minutes: u64,
    supported_benchmarks: Vec<BenchmarkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub name: String,
    pub symbol: String,
    pub coingecko_id: String,
    pub description: String,
    pub category: BenchmarkCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BenchmarkCategory {
    Cryptocurrency,
    DeFiToken,
    StableCoin,
    Index,
    Traditional,
}

#[derive(Debug, Clone)]
struct PriceCacheEntry {
    price: Decimal,
    timestamp: DateTime<Utc>,
}

/// CoinGecko API response structures
#[derive(Debug, Deserialize)]
struct CoinGeckoHistoricalResponse {
    prices: Vec<[f64; 2]>, // [timestamp_ms, price]
}

#[derive(Debug, Deserialize)]
struct CoinGeckoPriceResponse {
    #[serde(flatten)]
    prices: HashMap<String, HashMap<String, f64>>,
}

impl BenchmarkDataManager {
    pub fn new(cache_ttl_minutes: u64) -> Self {
        let supported_benchmarks = vec![
            BenchmarkConfig {
                name: "Ethereum".to_string(),
                symbol: "ETH".to_string(),
                coingecko_id: "ethereum".to_string(),
                description: "Ethereum native token".to_string(),
                category: BenchmarkCategory::Cryptocurrency,
            },
            BenchmarkConfig {
                name: "Bitcoin".to_string(),
                symbol: "BTC".to_string(),
                coingecko_id: "bitcoin".to_string(),
                description: "Bitcoin cryptocurrency".to_string(),
                category: BenchmarkCategory::Cryptocurrency,
            },
            BenchmarkConfig {
                name: "Uniswap".to_string(),
                symbol: "UNI".to_string(),
                coingecko_id: "uniswap".to_string(),
                description: "Uniswap governance token".to_string(),
                category: BenchmarkCategory::DeFiToken,
            },
            BenchmarkConfig {
                name: "Chainlink".to_string(),
                symbol: "LINK".to_string(),
                coingecko_id: "chainlink".to_string(),
                description: "Chainlink oracle token".to_string(),
                category: BenchmarkCategory::DeFiToken,
            },
            BenchmarkConfig {
                name: "Aave".to_string(),
                symbol: "AAVE".to_string(),
                coingecko_id: "aave".to_string(),
                description: "Aave lending protocol token".to_string(),
                category: BenchmarkCategory::DeFiToken,
            },
            BenchmarkConfig {
                name: "Compound".to_string(),
                symbol: "COMP".to_string(),
                coingecko_id: "compound-governance-token".to_string(),
                description: "Compound governance token".to_string(),
                category: BenchmarkCategory::DeFiToken,
            },
            BenchmarkConfig {
                name: "USD Coin".to_string(),
                symbol: "USDC".to_string(),
                coingecko_id: "usd-coin".to_string(),
                description: "USD Coin stablecoin".to_string(),
                category: BenchmarkCategory::StableCoin,
            },
            BenchmarkConfig {
                name: "DeFi Pulse Index".to_string(),
                symbol: "DPI".to_string(),
                coingecko_id: "defipulse-index".to_string(),
                description: "DeFi Pulse Index token".to_string(),
                category: BenchmarkCategory::Index,
            },
        ];

        Self {
            client: Client::new(),
            benchmark_cache: Arc::new(RwLock::new(HashMap::new())),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl_minutes,
            supported_benchmarks,
        }
    }

    /// Get list of supported benchmarks
    pub fn get_supported_benchmarks(&self) -> Vec<BenchmarkConfig> {
        self.supported_benchmarks.clone()
    }

    /// Get benchmark data for a specific symbol
    pub async fn get_benchmark_data(
        &self,
        symbol: &str,
        days: u32,
    ) -> Result<BenchmarkData, RiskError> {
        let cache_key = format!("{}_{}", symbol, days);
        
        // Check cache first
        if let Some(cached_data) = self.get_cached_benchmark(&cache_key).await {
            return Ok(cached_data);
        }

        // Find benchmark config
        let benchmark_config = self.supported_benchmarks
            .iter()
            .find(|b| b.symbol.eq_ignore_ascii_case(symbol))
            .ok_or_else(|| RiskError::ExternalApiError(format!("Unsupported benchmark: {}", symbol)))?;

        // Fetch historical data from CoinGecko
        let historical_data = self.fetch_historical_data(&benchmark_config.coingecko_id, days).await?;
        
        // Convert to benchmark returns
        let returns = self.calculate_benchmark_returns(&historical_data);
        
        let benchmark_data = BenchmarkData {
            name: benchmark_config.name.clone(),
            symbol: benchmark_config.symbol.clone(),
            returns,
            last_updated: Utc::now(),
        };

        // Cache the result
        self.cache_benchmark_data(cache_key, benchmark_data.clone()).await;

        Ok(benchmark_data)
    }

    /// Get current price for a benchmark
    pub async fn get_current_benchmark_price(&self, symbol: &str) -> Result<Decimal, RiskError> {
        // Check price cache first
        if let Some(cached_price) = self.get_cached_price(symbol).await {
            return Ok(cached_price);
        }

        // Find benchmark config
        let benchmark_config = self.supported_benchmarks
            .iter()
            .find(|b| b.symbol.eq_ignore_ascii_case(symbol))
            .ok_or_else(|| RiskError::ExternalApiError(format!("Unsupported benchmark: {}", symbol)))?;

        // Fetch current price from CoinGecko
        let price = self.fetch_current_price(&benchmark_config.coingecko_id).await?;
        
        // Cache the price
        self.cache_price(symbol, price).await;

        Ok(price)
    }

    /// Update all benchmark data
    pub async fn update_all_benchmarks(&self, days: u32) -> Result<(), RiskError> {
        let mut update_results = Vec::new();
        
        for benchmark in &self.supported_benchmarks {
            match self.get_benchmark_data(&benchmark.symbol, days).await {
                Ok(_) => {
                    debug!("Updated benchmark data for {}", benchmark.symbol);
                }
                Err(e) => {
                    error!("Failed to update benchmark {}: {}", benchmark.symbol, e);
                    update_results.push(Err::<(), RiskError>(e));
                }
            }
        }

        // Return error if any updates failed
        if let Some(first_error) = update_results.into_iter().find_map(|r| r.err()) {
            return Err(first_error);
        }

        info!("Successfully updated all benchmark data");
        Ok(())
    }

    /// Get benchmark data for multiple symbols
    pub async fn get_multiple_benchmarks(
        &self,
        symbols: &[String],
        days: u32,
    ) -> Result<HashMap<String, BenchmarkData>, RiskError> {
        let mut results = HashMap::new();
        
        for symbol in symbols {
            match self.get_benchmark_data(symbol, days).await {
                Ok(data) => {
                    results.insert(symbol.clone(), data);
                }
                Err(e) => {
                    warn!("Failed to get benchmark data for {}: {}", symbol, e);
                    // Continue with other benchmarks even if one fails
                }
            }
        }

        if results.is_empty() {
            return Err(RiskError::ExternalApiError("No benchmark data available".to_string()));
        }

        Ok(results)
    }

    /// Calculate correlation between user returns and benchmark
    pub fn calculate_correlation(
        &self,
        user_returns: &[Decimal],
        benchmark_returns: &[Decimal],
    ) -> Result<Decimal, RiskError> {
        if user_returns.len() != benchmark_returns.len() || user_returns.len() < 2 {
            return Err(RiskError::InsufficientData("Insufficient data for correlation calculation".to_string()));
        }

        let n = user_returns.len() as f64;
        
        // Calculate means
        let user_mean = user_returns.iter().sum::<Decimal>() / Decimal::from(user_returns.len());
        let benchmark_mean = benchmark_returns.iter().sum::<Decimal>() / Decimal::from(benchmark_returns.len());
        
        // Calculate correlation coefficient
        let mut numerator = Decimal::ZERO;
        let mut user_variance = Decimal::ZERO;
        let mut benchmark_variance = Decimal::ZERO;
        
        for i in 0..user_returns.len() {
            let user_diff = user_returns[i] - user_mean;
            let benchmark_diff = benchmark_returns[i] - benchmark_mean;
            
            numerator += user_diff * benchmark_diff;
            user_variance += user_diff * user_diff;
            benchmark_variance += benchmark_diff * benchmark_diff;
        }
        
        let denominator = (user_variance * benchmark_variance).sqrt().unwrap_or(Decimal::ZERO);
        
        if denominator == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }
        
        Ok(numerator / denominator)
    }

    /// Calculate beta (systematic risk) relative to benchmark
    pub fn calculate_beta(
        &self,
        user_returns: &[Decimal],
        benchmark_returns: &[Decimal],
    ) -> Result<Decimal, RiskError> {
        if user_returns.len() != benchmark_returns.len() || user_returns.len() < 2 {
            return Err(RiskError::InsufficientData("Insufficient data for beta calculation".to_string()));
        }

        // Calculate means
        let benchmark_mean = benchmark_returns.iter().sum::<Decimal>() / Decimal::from(benchmark_returns.len());
        let user_mean = user_returns.iter().sum::<Decimal>() / Decimal::from(user_returns.len());
        
        // Calculate covariance and benchmark variance
        let mut covariance = Decimal::ZERO;
        let mut benchmark_variance = Decimal::ZERO;
        
        for i in 0..user_returns.len() {
            let user_diff = user_returns[i] - user_mean;
            let benchmark_diff = benchmark_returns[i] - benchmark_mean;
            
            covariance += user_diff * benchmark_diff;
            benchmark_variance += benchmark_diff * benchmark_diff;
        }
        
        covariance /= Decimal::from(user_returns.len() - 1);
        benchmark_variance /= Decimal::from(benchmark_returns.len() - 1);
        
        if benchmark_variance == Decimal::ZERO {
            return Ok(Decimal::ONE); // Default beta of 1.0
        }
        
        Ok(covariance / benchmark_variance)
    }

    /// Calculate alpha (excess return over benchmark)
    pub fn calculate_alpha(
        &self,
        user_return: Decimal,
        benchmark_return: Decimal,
        beta: Decimal,
        risk_free_rate: Decimal,
    ) -> Decimal {
        // Alpha = User Return - (Risk Free Rate + Beta * (Benchmark Return - Risk Free Rate))
        user_return - (risk_free_rate + beta * (benchmark_return - risk_free_rate))
    }

    /// Calculate tracking error
    pub fn calculate_tracking_error(
        &self,
        user_returns: &[Decimal],
        benchmark_returns: &[Decimal],
    ) -> Result<Decimal, RiskError> {
        if user_returns.len() != benchmark_returns.len() || user_returns.len() < 2 {
            return Err(RiskError::InsufficientData("Insufficient data for tracking error calculation".to_string()));
        }

        // Calculate excess returns
        let excess_returns: Vec<Decimal> = user_returns
            .iter()
            .zip(benchmark_returns.iter())
            .map(|(user, benchmark)| user - benchmark)
            .collect();

        // Calculate standard deviation of excess returns
        let mean_excess = excess_returns.iter().sum::<Decimal>() / Decimal::from(excess_returns.len());
        
        let variance: Decimal = excess_returns
            .iter()
            .map(|r| (*r - mean_excess).powi(2))
            .sum::<Decimal>() / Decimal::from(excess_returns.len() - 1);

        Ok(variance.sqrt().unwrap_or(Decimal::ZERO))
    }

    // Private helper methods
    async fn fetch_historical_data(&self, coingecko_id: &str, days: u32) -> Result<Vec<(DateTime<Utc>, Decimal)>, RiskError> {
        let url = format!(
            "https://api.coingecko.com/api/v3/coins/{}/market_chart?vs_currency=usd&days={}",
            coingecko_id, days
        );

        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| RiskError::ExternalApiError(format!("CoinGecko API error: {}", e)))?;

        let data: CoinGeckoHistoricalResponse = response
            .json()
            .await
            .map_err(|e| RiskError::ExternalApiError(format!("JSON parsing error: {}", e)))?;

        let mut historical_data = Vec::new();
        
        for price_point in data.prices {
            let timestamp_ms = price_point[0] as i64;
            let price = price_point[1];
            
            let datetime = DateTime::from_timestamp_millis(timestamp_ms)
                .ok_or_else(|| RiskError::ExternalApiError("Invalid timestamp".to_string()))?;
            
            let decimal_price = Decimal::try_from(price)
                .map_err(|e| RiskError::ExternalApiError(format!("Price conversion error: {}", e)))?;
            
            historical_data.push((datetime, decimal_price));
        }

        Ok(historical_data)
    }

    async fn fetch_current_price(&self, coingecko_id: &str) -> Result<Decimal, RiskError> {
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd",
            coingecko_id
        );

        let response = self.client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| RiskError::ExternalApiError(format!("CoinGecko API error: {}", e)))?;

        let data: serde_json::Value = response
            .json()
            .await
            .map_err(|e| RiskError::ExternalApiError(format!("JSON parsing error: {}", e)))?;

        let price_value = data
            .get(coingecko_id)
            .and_then(|token| token.get("usd"))
            .and_then(|price| price.as_f64())
            .ok_or_else(|| RiskError::ExternalApiError("Price not found in response".to_string()))?;

        Decimal::try_from(price_value)
            .map_err(|e| RiskError::ExternalApiError(format!("Price conversion error: {}", e)))
    }

    fn calculate_benchmark_returns(&self, historical_data: &[(DateTime<Utc>, Decimal)]) -> Vec<BenchmarkReturn> {
        let mut returns = Vec::new();
        
        for i in 1..historical_data.len() {
            let (current_date, current_price) = &historical_data[i];
            let (_, previous_price) = &historical_data[i - 1];
            
            let return_percentage = if *previous_price != Decimal::ZERO {
                ((current_price - previous_price) / previous_price) * Decimal::from(100)
            } else {
                Decimal::ZERO
            };
            
            returns.push(BenchmarkReturn {
                timestamp: *current_date,
                price: *current_price,
                return_percentage,
            });
        }
        
        returns
    }

    async fn get_cached_benchmark(&self, cache_key: &str) -> Option<BenchmarkData> {
        let cache = self.benchmark_cache.read().await;
        if let Some(data) = cache.get(cache_key) {
            let age = Utc::now().signed_duration_since(data.last_updated);
            if age.num_minutes() < self.cache_ttl_minutes as i64 {
                return Some(data.clone());
            }
        }
        None
    }

    async fn cache_benchmark_data(&self, cache_key: String, data: BenchmarkData) {
        let mut cache = self.benchmark_cache.write().await;
        cache.insert(cache_key, data);
    }

    async fn get_cached_price(&self, symbol: &str) -> Option<Decimal> {
        let cache = self.price_cache.read().await;
        if let Some(entry) = cache.get(symbol) {
            let age = Utc::now().signed_duration_since(entry.timestamp);
            if age.num_minutes() < self.cache_ttl_minutes as i64 {
                return Some(entry.price);
            }
        }
        None
    }

    async fn cache_price(&self, symbol: &str, price: Decimal) {
        let mut cache = self.price_cache.write().await;
        cache.insert(symbol.to_string(), PriceCacheEntry {
            price,
            timestamp: Utc::now(),
        });
    }
}

/// Benchmark data aggregator for collecting data from multiple sources
#[derive(Debug)]
pub struct BenchmarkDataAggregator {
    data_manager: Arc<BenchmarkDataManager>,
    update_interval_hours: u64,
    is_running: Arc<RwLock<bool>>,
}

impl BenchmarkDataAggregator {
    pub fn new(data_manager: Arc<BenchmarkDataManager>, update_interval_hours: u64) -> Self {
        Self {
            data_manager,
            update_interval_hours,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start automatic benchmark data updates
    pub async fn start_automatic_updates(&self) -> Result<(), RiskError> {
        let mut running = self.is_running.write().await;
        if *running {
            return Err(RiskError::ServiceAlreadyRunning("BenchmarkDataAggregator".to_string()));
        }
        *running = true;
        drop(running);

        let data_manager = self.data_manager.clone();
        let is_running = self.is_running.clone();
        let update_interval = self.update_interval_hours;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(update_interval * 3600));
            
            while *is_running.read().await {
                interval.tick().await;
                
                match data_manager.update_all_benchmarks(365).await {
                    Ok(_) => {
                        info!("Benchmark data update completed successfully");
                    }
                    Err(e) => {
                        error!("Benchmark data update failed: {}", e);
                    }
                }
            }
        });

        info!("Benchmark data aggregator started with {}h update interval", update_interval);
        Ok(())
    }

    /// Stop automatic updates
    pub async fn stop_automatic_updates(&self) {
        let mut running = self.is_running.write().await;
        *running = false;
        info!("Benchmark data aggregator stopped");
    }

    /// Force immediate update of all benchmarks
    pub async fn force_update(&self) -> Result<(), RiskError> {
        info!("Forcing benchmark data update");
        self.data_manager.update_all_benchmarks(365).await
    }
}

