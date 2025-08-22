use std::{sync::{Arc, RwLock}, time::Instant, collections::HashMap};
use crate::Address;
use crate::{
    cache::CacheManager,
    database::{models::TokenPrice, queries},
    utils::math::DecimalMath,
    CalculationError,
    CalculationResult,
    COINGECKO_API_BASE,
    CHAINLINK_PRICE_FEEDS,
};
use rust_decimal::{Decimal, prelude::*};
use sqlx::PgPool;
use chrono::{DateTime, Utc, Duration};
use tracing::{debug, info, warn};
use alloy_transport_http::reqwest::Client;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoingeckoPrice {
    pub usd: Decimal,
    pub usd_24h_change: Option<Decimal>,
    pub last_updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub token_address: String,
    pub price_usd: Decimal,
    pub timestamp: DateTime<Utc>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoingeckoResponse {
    #[serde(flatten)]
    pub prices: HashMap<String, CoingeckoPrice>,
}

#[derive(Debug, Clone)]
pub struct PriceSource {
    pub source: String,
    pub price: Decimal,
    pub confidence: f32,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PricingConfig {
    pub update_interval_seconds: u64,
    pub max_price_age_minutes: i64,
    pub min_confidence_threshold: f32,
    pub enable_chainlink: bool,
    pub enable_coingecko: bool,
    pub enable_uniswap_twap: bool,
    pub twap_period_hours: i64,
}

impl Default for PricingConfig {
    fn default() -> Self {
        Self {
            update_interval_seconds: 300, // 5 minutes
            max_price_age_minutes: 15,
            min_confidence_threshold: 0.8,
            enable_chainlink: true,
            enable_coingecko: true,
            enable_uniswap_twap: true,
            twap_period_hours: 24,
        }
    }
}

#[derive(Debug)]
pub struct PricingEngine {
    db_pool: sqlx::PgPool,
    cache_manager: Arc<CacheManager>,
    http_client: Client,
    config: PricingConfig,
    price_cache: Arc<RwLock<HashMap<Address, Vec<PriceSource>>>>,
    last_update: Arc<RwLock<Instant>>,
}

impl PricingEngine {
    pub fn new(
        db_pool: sqlx::PgPool,
        cache_manager: Arc<CacheManager>,
        config: Option<PricingConfig>,
    ) -> Self {
        Self {
            db_pool,
            cache_manager,
            http_client: Client::new(),
            config: config.unwrap_or_default(),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            last_update: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub async fn start_price_updates(&self) -> CalculationResult<()> {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(self.config.update_interval_seconds));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.update_all_prices().await {
                tracing::error!("Failed to update prices: {}", e);
            }
        }
    }

    pub async fn update_all_prices(&self) -> CalculationResult<()> {
        tracing::info!("Starting price update cycle");
        
        // Get all tokens that need price updates
        let tokens = self.get_tracked_tokens().await?;
        
        for token_address in tokens {
            if let Err(e) = self.update_token_price(&token_address).await {
                tracing::error!("Failed to update price for token {}: {}", token_address, e);
            }
        }

        // Update last update time
        *self.last_update.write().unwrap() = Instant::now();
        
        tracing::info!("Completed price update cycle");
        Ok(())
    }

    async fn get_tracked_tokens(&self) -> CalculationResult<Vec<Address>> {
        // Get unique token addresses from positions
        let token_addresses = sqlx::query!(
            "SELECT DISTINCT unnest(array[token0, token1]) as token_address 
             FROM (
                 SELECT token0, token1 FROM positions_v2
                 UNION ALL
                 SELECT token0, token1 FROM positions_v3
             ) t"
        )
        .fetch_all(&self.db_pool)
        .await
        .map_err(|e| CalculationError::DatabaseError(format!("Failed to get tracked tokens: {}", e)))?;

        let addresses: Vec<Address> = token_addresses
            .into_iter()
            .filter_map(|row| row.token_address.and_then(|addr| addr.parse().ok()))
            .collect();

        Ok(addresses)
    }

    pub async fn update_token_price(&self, token_address: &Address) -> CalculationResult<()> {
        let mut price_sources = Vec::new();

        // Try Chainlink first (highest confidence)
        if self.config.enable_chainlink {
            if let Ok(price) = self.get_chainlink_price(token_address).await {
                price_sources.push(price);
            }
        }

        // Try CoinGecko
        if self.config.enable_coingecko {
            if let Ok(price) = self.get_coingecko_price(token_address).await {
                price_sources.push(price);
            }
        }

        // Try Uniswap TWAP
        if self.config.enable_uniswap_twap {
            if let Ok(price) = self.get_uniswap_twap_price(token_address).await {
                price_sources.push(price);
            }
        }

        if price_sources.is_empty() {
            return Err(CalculationError::PriceNotFound(format!("No price sources available for token {}", token_address)));
        }

        // Calculate weighted average price
        let final_price = self.calculate_weighted_price(&price_sources)?;
        
        // Store in database
        self.store_price(token_address, &final_price, &price_sources).await?;
        
        // Update cache
        self.price_cache.write().unwrap().insert(*token_address, price_sources);
        
        // Cache the final price
        self.cache_manager.set_token_price(token_address, final_price.price).await;

        Ok(())
    }

    async fn get_chainlink_price(&self, token_address: &Address) -> CalculationResult<PriceSource> {
        // Check if we have a Chainlink price feed for this token
        // Temporarily comment out Chainlink price feeds to fix compilation
        // if let Some(feed_address) = CHAINLINK_PRICE_FEEDS.get(token_address) {
            // In a real implementation, you would call the Chainlink price feed contract
            // For now, we'll return a placeholder
            // Ok(PriceSource {
            //     source: "chainlink".to_string(),
            //     price: Decimal::ZERO, // Would be fetched from contract
            //     confidence: 0.95,
            //     timestamp: Utc::now(),
            // })
        // } else {
            Err(CalculationError::PriceNotFound("No Chainlink feed available".to_string()))
        // }
    }

    async fn get_coingecko_price(&self, token_address: &Address) -> CalculationResult<PriceSource> {
        let url = format!("{}/simple/token_price/ethereum", COINGECKO_API_BASE);
        
        let response = self.http_client
            .get(&url)
            .query(&[
                ("contract_addresses", token_address.to_string()),
                ("vs_currencies", "usd".to_string()),
                ("include_24hr_change", "true".to_string()),
            ])
            .send()
            .await
            .map_err(|e| CalculationError::PriceFeedError(format!("CoinGecko API error: {}", e)))?;

        let price_data: CoingeckoResponse = response
            .json()
            .await
            .map_err(|e| CalculationError::PriceFeedError(format!("Failed to parse CoinGecko response: {}", e)))?;

        let token_key = token_address.to_string().to_lowercase();
        if let Some(price_info) = price_data.prices.get(&token_key) {
            Ok(PriceSource {
                source: "coingecko".to_string(),
                price: price_info.usd,
                confidence: 0.85,
                timestamp: Utc::now(),
            })
        } else {
            Err(CalculationError::PriceNotFound("Token not found in CoinGecko".to_string()))
        }
    }

    async fn get_uniswap_twap_price(&self, token_address: &Address) -> CalculationResult<PriceSource> {
        // Get recent price snapshots from database
        let snapshots = queries::get_token_price_history(&self.db_pool, &token_address.to_string(), self.config.twap_period_hours).await
            .map_err(|e| CalculationError::DatabaseError(format!("Failed to get price history: {}", e)))?;

        if snapshots.is_empty() {
            return Err(CalculationError::PriceNotFound("No historical price data available".to_string()));
        }

        let pricing_snapshots: Vec<crate::calculations::pricing::PriceSnapshot> = snapshots.into_iter().map(|s| crate::calculations::pricing::PriceSnapshot {
            token_address: s.token_address,
            price_usd: s.price_usd,
            timestamp: s.timestamp,
            source: s.source,
        }).collect();
        let twap = Self::calculate_twap(pricing_snapshots.as_slice(), self.config.twap_period_hours)?;
        
        Ok(PriceSource {
            source: "uniswap_twap".to_string(),
            price: twap,
            confidence: 0.75,
            timestamp: Utc::now(),
        })
    }

    fn calculate_weighted_price(&self, sources: &[PriceSource]) -> CalculationResult<PriceSource> {
        if sources.is_empty() {
            return Err(CalculationError::PriceNotFound("No price sources provided".to_string()));
        }

        // Filter sources by confidence threshold
        let valid_sources: Vec<&PriceSource> = sources
            .iter()
            .filter(|s| s.confidence >= self.config.min_confidence_threshold)
            .collect();

        if valid_sources.is_empty() {
            return Err(CalculationError::PriceNotFound("No sources meet confidence threshold".to_string()));
        }

        // Calculate weighted average
        let total_weight: f32 = valid_sources.iter().map(|s| s.confidence).sum();
        let weighted_sum: Decimal = valid_sources
            .iter()
            .map(|s| s.price * Decimal::from_f32_retain(s.confidence).unwrap_or(Decimal::ZERO))
            .sum();

        let weighted_price = weighted_sum / Decimal::from_f32_retain(total_weight).unwrap_or(Decimal::ONE);

        // Use the source with highest confidence as the primary source
        let best_source = valid_sources
            .iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .unwrap();

        Ok(PriceSource {
            source: format!("weighted_{}", best_source.source),
            price: weighted_price,
            confidence: total_weight / valid_sources.len() as f32,
            timestamp: Utc::now(),
        })
    }

    async fn store_price(&self, token_address: &Address, price_source: &PriceSource, all_sources: &[PriceSource]) -> CalculationResult<()> {
        // Store main price
        let price_usd = price_source.price;
        let token_price = TokenPrice {
            token_address: token_address.to_string(),
            price_usd: Some(price_usd),
            price_eth: None,
            block_number: 0, // PriceSource doesn't have block_number field
            timestamp: Utc::now(),
            updated_at: Utc::now(),
        };

        queries::upsert_token_price(
            &self.db_pool, 
            &token_price.token_address, 
            token_price.price_usd.unwrap_or_default(), 
            token_price.price_eth, 
            token_price.block_number
        ).await
            .map_err(|e| CalculationError::DatabaseError(format!("Failed to store token price: {}", e)))?;

        // Store price snapshots for all sources
        for source in all_sources {
            let snapshot = PriceSnapshot {
                token_address: token_address.to_string(),
                price_usd: source.price,
                source: source.source.clone(),
                timestamp: source.timestamp,
            };

            let db_snapshot = crate::database::models::PriceSnapshot {
                token_address: snapshot.token_address.clone(),
                price_usd: snapshot.price_usd,
                source: snapshot.source.clone(),
                timestamp: snapshot.timestamp,
            };
            if let Err(e) = queries::insert_price_snapshot(&self.db_pool, &db_snapshot).await {
                tracing::warn!("Failed to store price snapshot: {}", e);
            }
        }

        Ok(())
    }

    pub async fn get_token_price(&self, token_address: &Address) -> CalculationResult<Decimal> {
        // Try cache first
        if let Some(cached_price) = self.cache_manager.get_token_price(token_address).await {
            return Ok(cached_price);
        }

        // Try in-memory cache
        if let Some(sources) = self.price_cache.read().unwrap().get(token_address) {
            if let Some(latest) = sources.first() {
                let age = Utc::now().signed_duration_since(latest.timestamp);
                if age < Duration::minutes(self.config.max_price_age_minutes) {
                    return Ok(latest.price);
                }
            }
        }

        // Fetch from database
        match queries::get_latest_token_price(&self.db_pool, token_address).await {
            Ok(Some(token_price)) => {
                let age = Utc::now().signed_duration_since(token_price.timestamp);
                if age < Duration::minutes(self.config.max_price_age_minutes) {
                    // Cache the result
                    if let Some(price) = token_price.price_usd {
                        self.cache_manager.set_token_price(token_address, price).await;
                        Ok(price)
                    } else {
                        Err(CalculationError::PriceNotFound(token_address.to_string()))
                    }
                } else {
                    // Price is too old, trigger update
                    self.update_token_price(token_address).await?;
                    Box::pin(self.get_token_price(token_address)).await
                }
            }
            Ok(None) => {
                // No price found, trigger update
                self.update_token_price(token_address).await?;
                Box::pin(self.get_token_price(token_address)).await
            }
            Err(e) => Err(CalculationError::DatabaseError(format!("Failed to fetch token price: {}", e)))
        }
    }

    pub fn calculate_time_weighted_average_price(
        &self,
        price_snapshots: &[PriceSnapshot],
        duration_hours: i64,
    ) -> CalculationResult<Decimal> {
        if price_snapshots.is_empty() {
            return Err(CalculationError::InsufficientData("No price snapshots provided".to_string()));
        }

        let cutoff_time = Utc::now() - Duration::hours(duration_hours);
        let relevant_prices: Vec<&PriceSnapshot> = price_snapshots
            .iter()
            .filter(|p| p.timestamp >= cutoff_time)
            .collect();

        if relevant_prices.is_empty() {
            return Ok(price_snapshots[0].price_usd);
        }

        // Time-weighted average calculation
        let mut total_weighted_price = Decimal::ZERO;
        let mut total_time_weight = 0i64;

        for window in relevant_prices.windows(2) {
            let current = &window[0];
            let next = &window[1];
            
            let time_diff = next.timestamp.signed_duration_since(current.timestamp).num_seconds();
            total_weighted_price += current.price_usd * Decimal::from(time_diff);
            total_time_weight += time_diff;
        }

        if total_time_weight == 0 {
            // Fallback to simple average
            let sum: Decimal = relevant_prices.iter().map(|p| p.price_usd).sum();
            Ok(sum / Decimal::from(relevant_prices.len()))
        } else {
            Ok(total_weighted_price / Decimal::from(total_time_weight))
        }
    }

    pub fn calculate_price_impact(
        old_price: Decimal,
        new_price: Decimal,
    ) -> Decimal {
        if old_price.is_zero() {
            return Decimal::ZERO;
        }
        ((new_price - old_price) / old_price) * Decimal::from(100)
    }

    pub fn tick_to_price(tick: i32) -> CalculationResult<Decimal> {
        // Price = 1.0001^tick for Uniswap V3
        let base = Decimal::from_str_exact("1.0001")
            .map_err(|e| CalculationError::InvalidInput(format!("Invalid base value: {}", e)))?;
        
        // Handle large tick values that might cause overflow
        if tick.abs() > 887272 {
            return Err(CalculationError::InvalidInput("Tick value out of valid range".to_string()));
        }
        
        Ok(base.powd(Decimal::from(tick)).unwrap_or(Decimal::ZERO))
    }

    pub fn price_to_tick(price: Decimal) -> CalculationResult<i32> {
        if price <= Decimal::ZERO {
            return Err(CalculationError::InvalidInput("Price must be positive".to_string()));
        }
        
        // tick = log(price) / log(1.0001)
        let log_price = price.ln().ok_or_else(|| CalculationError::InvalidInput("Cannot calculate ln of price".to_string()))?;
        let log_base = Decimal::from_str_exact("1.0001")
            .map_err(|e| CalculationError::InvalidInput(format!("Invalid base value: {}", e)))?
            .ln().ok_or_else(|| CalculationError::InvalidInput("Cannot calculate ln of base".to_string()))?;
        
        let tick_decimal = log_price / log_base;
        Ok(tick_decimal.to_i32().unwrap_or(0))
    }

    pub fn calculate_liquidity_value(
        liquidity: Decimal,
        tick_current: i32,
        tick_lower: i32,
        tick_upper: i32,
        token0_price: Decimal,
        token1_price: Decimal,
    ) -> CalculationResult<(Decimal, Decimal)> {
        // Calculate token amounts from liquidity position
        let sqrt_price_current = Self::tick_to_sqrt_price(tick_current)?;
        let sqrt_price_lower = Self::tick_to_sqrt_price(tick_lower)?;
        let sqrt_price_upper = Self::tick_to_sqrt_price(tick_upper)?;

        let (amount0, amount1) = if tick_current < tick_lower {
            // Position is entirely in token0
            let amount0 = liquidity * (sqrt_price_upper - sqrt_price_lower) / (sqrt_price_lower * sqrt_price_upper);
            (amount0, Decimal::ZERO)
        } else if tick_current >= tick_upper {
            // Position is entirely in token1
            let amount1 = liquidity * (sqrt_price_upper - sqrt_price_lower);
            (Decimal::ZERO, amount1)
        } else {
            // Position is active, contains both tokens
            let amount0 = liquidity * (sqrt_price_upper - sqrt_price_current) / (sqrt_price_current * sqrt_price_upper);
            let amount1 = liquidity * (sqrt_price_current - sqrt_price_lower);
            (amount0, amount1)
        };

        Ok((amount0, amount1))
    }

    fn tick_to_sqrt_price(tick: i32) -> CalculationResult<Decimal> {
        let price = Self::tick_to_price(tick)?;
        Ok(price.sqrt().ok_or_else(|| CalculationError::InvalidInput("Cannot calculate sqrt".to_string()))?)
    }

    pub async fn get_price_history(&self, token_address: &Address, hours: i64) -> CalculationResult<Vec<PriceSnapshot>> {
        let snapshots = queries::get_token_price_history(&self.db_pool, &token_address.to_string(), hours).await
            .map_err(|e| CalculationError::DatabaseError(format!("Failed to get price history: {}", e)))?;
        Ok(snapshots.into_iter().map(|s| crate::calculations::pricing::PriceSnapshot {
            token_address: s.token_address,
            price_usd: s.price_usd,
            timestamp: s.timestamp,
            source: s.source,
        }).collect())
    }

    fn calculate_twap(snapshots: &[PriceSnapshot], period_hours: i64) -> CalculationResult<Decimal> {
        if snapshots.is_empty() {
            return Err(CalculationError::PriceNotFound("No price snapshots available".to_string()));
        }

        let now = Utc::now();
        let cutoff_time = now - chrono::Duration::hours(period_hours);
        
        let mut weighted_sum = Decimal::ZERO;
        let mut total_weight = Decimal::ZERO;
        
        for snapshot in snapshots {
            if snapshot.timestamp >= cutoff_time {
                let time_weight = Decimal::from((now - snapshot.timestamp).num_seconds().max(1));
                weighted_sum += snapshot.price_usd * time_weight;
                total_weight += time_weight;
            }
        }
        
        if total_weight == Decimal::ZERO {
            return Err(CalculationError::PriceNotFound("No recent price data available".to_string()));
        }
        
        Ok(weighted_sum / total_weight)
    }

    pub async fn get_metrics(&self) -> HashMap<String, serde_json::Value> {
        let mut metrics = HashMap::new();
        
        let cache_size = self.price_cache.read().unwrap().len();
        let last_update = self.last_update.read().unwrap().elapsed().as_secs();
        
        metrics.insert("cached_tokens".to_string(), serde_json::Value::from(cache_size));
        metrics.insert("seconds_since_last_update".to_string(), serde_json::Value::from(last_update));
        metrics.insert("update_interval_seconds".to_string(), serde_json::Value::from(self.config.update_interval_seconds));
        
        metrics
    }
}
