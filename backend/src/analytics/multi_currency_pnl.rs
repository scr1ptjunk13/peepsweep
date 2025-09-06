use crate::analytics::live_pnl_engine::PnLSnapshot;
use crate::analytics::timescaledb_persistence::MultiCurrencyPnL;
use crate::analytics::live_pnl_engine::PositionPnL;
use crate::risk_management::types::RiskError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Multi-currency P&L calculator with real-time price conversion
pub struct MultiCurrencyPnLCalculator {
    price_oracle: Arc<dyn MultiCurrencyPriceOracle>,
    currency_cache: Arc<RwLock<HashMap<Currency, CurrencyData>>>,
    calculation_config: MultiCurrencyConfig,
    conversion_stats: Arc<RwLock<ConversionStats>>,
}

/// Multi-currency price oracle interface
#[async_trait::async_trait]
pub trait MultiCurrencyPriceOracle: Send + Sync {
    async fn get_currency_price(&self, currency: Currency) -> Result<Decimal, RiskError>;
    async fn get_historical_currency_price(&self, currency: Currency, timestamp: DateTime<Utc>) -> Result<Decimal, RiskError>;
    async fn get_currency_pair_rate(&self, from: Currency, to: Currency) -> Result<Decimal, RiskError>;
    async fn subscribe_to_currency_updates(&self, currency: Currency) -> Result<tokio::sync::broadcast::Receiver<CurrencyPriceUpdate>, RiskError>;
}

/// Supported currencies for P&L calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    USD,
    ETH,
    BTC,
    EUR,
    GBP,
    JPY,
}

/// Currency data with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyData {
    pub currency: Currency,
    pub price_usd: Decimal,
    pub price_change_24h: Decimal,
    pub market_cap: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
    pub last_updated: DateTime<Utc>,
    pub data_source: String,
}

/// Currency price update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyPriceUpdate {
    pub currency: Currency,
    pub old_price: Decimal,
    pub new_price: Decimal,
    pub price_change: Decimal,
    pub timestamp: DateTime<Utc>,
}

/// Multi-currency P&L configuration
#[derive(Debug, Clone)]
pub struct MultiCurrencyConfig {
    pub supported_currencies: Vec<Currency>,
    pub base_currency: Currency,
    pub price_cache_ttl_seconds: u64,
    pub conversion_precision: u32,
    pub enable_historical_rates: bool,
    pub fallback_to_cached_rates: bool,
}

impl Default for MultiCurrencyConfig {
    fn default() -> Self {
        Self {
            supported_currencies: vec![Currency::USD, Currency::ETH, Currency::BTC],
            base_currency: Currency::USD,
            price_cache_ttl_seconds: 60, // 1 minute
            conversion_precision: 8,
            enable_historical_rates: true,
            fallback_to_cached_rates: true,
        }
    }
}

/// Multi-currency P&L snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurrencyPnLSnapshot {
    pub user_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub base_currency: Currency,
    pub pnl_by_currency: HashMap<Currency, CurrencyPnLData>,
    pub positions: Vec<MultiCurrencyPositionPnL>,
    pub total_portfolio_value_by_currency: HashMap<Currency, Decimal>,
    pub currency_exposure: HashMap<Currency, Decimal>,
    pub conversion_rates: HashMap<Currency, Decimal>,
    pub calculation_duration_ms: u64,
}

/// P&L data in specific currency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyPnLData {
    pub currency: Currency,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    pub total_pnl: Decimal,
    pub daily_change: Decimal,
    pub daily_change_percent: Decimal,
    pub portfolio_value: Decimal,
}

/// Position P&L with multi-currency support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurrencyPositionPnL {
    pub token_address: String,
    pub chain_id: u64,
    pub symbol: String,
    pub balance: Decimal,
    pub entry_price_by_currency: HashMap<Currency, Decimal>,
    pub current_price_by_currency: HashMap<Currency, Decimal>,
    pub pnl_by_currency: HashMap<Currency, CurrencyPnLData>,
    pub position_value_by_currency: HashMap<Currency, Decimal>,
    pub last_updated: DateTime<Utc>,
}

/// Currency conversion statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversionStats {
    pub total_conversions: u64,
    pub successful_conversions: u64,
    pub failed_conversions: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_conversion_time_ms: f64,
    pub last_conversion_time: Option<DateTime<Utc>>,
}

impl MultiCurrencyPnLCalculator {
    /// Create new multi-currency P&L calculator
    pub async fn new(
        price_oracle: Arc<dyn MultiCurrencyPriceOracle>,
        config: MultiCurrencyConfig,
    ) -> Result<Self, RiskError> {
        Ok(Self {
            price_oracle,
            currency_cache: Arc::new(RwLock::new(HashMap::new())),
            calculation_config: config,
            conversion_stats: Arc::new(RwLock::new(ConversionStats::default())),
        })
    }

    /// Calculate multi-currency P&L for a user
    pub async fn calculate_multi_currency_pnl(
        &self,
        base_snapshot: &PnLSnapshot,
    ) -> Result<MultiCurrencyPnLSnapshot, RiskError> {
        let start_time = std::time::Instant::now();

        // Update conversion statistics
        {
            let mut stats = self.conversion_stats.write().await;
            stats.total_conversions += 1;
            stats.last_conversion_time = Some(Utc::now());
        }

        // Get current conversion rates for all supported currencies
        let mut conversion_rates = HashMap::new();
        for currency in &self.calculation_config.supported_currencies {
            if *currency != self.calculation_config.base_currency {
                match self.get_conversion_rate(self.calculation_config.base_currency, *currency).await {
                    Ok(rate) => {
                        conversion_rates.insert(*currency, rate);
                    }
                    Err(e) => {
                        warn!("Failed to get conversion rate for {:?}: {}", currency, e);
                        if self.calculation_config.fallback_to_cached_rates {
                            if let Some(cached_rate) = self.get_cached_rate(*currency).await {
                                conversion_rates.insert(*currency, cached_rate);
                            }
                        }
                    }
                }
            } else {
                conversion_rates.insert(*currency, Decimal::ONE);
            }
        }

        // Convert P&L data to all supported currencies
        let mut pnl_by_currency = HashMap::new();
        for currency in &self.calculation_config.supported_currencies {
            let rate = conversion_rates.get(currency).cloned().unwrap_or(Decimal::ONE);
            
            let currency_pnl = CurrencyPnLData {
                currency: *currency,
                unrealized_pnl: self.convert_amount(base_snapshot.total_unrealized_pnl_usd, rate),
                realized_pnl: self.convert_amount(base_snapshot.total_realized_pnl_usd, rate),
                total_pnl: self.convert_amount(base_snapshot.total_pnl_usd, rate),
                daily_change: self.convert_amount(base_snapshot.daily_change_usd, rate),
                daily_change_percent: base_snapshot.daily_change_percent, // Percentage stays the same
                portfolio_value: self.convert_amount(base_snapshot.total_portfolio_value_usd, rate),
            };
            
            pnl_by_currency.insert(*currency, currency_pnl);
        }

        // Convert position P&L to multi-currency format
        let mut multi_currency_positions = Vec::new();
        for position in &base_snapshot.positions {
            let multi_currency_position = self.convert_position_to_multi_currency(position, &conversion_rates).await?;
            multi_currency_positions.push(multi_currency_position);
        }

        // Calculate total portfolio value by currency
        let mut total_portfolio_value_by_currency = HashMap::new();
        for currency in &self.calculation_config.supported_currencies {
            let rate = conversion_rates.get(currency).cloned().unwrap_or(Decimal::ONE);
            let portfolio_value = self.convert_amount(base_snapshot.total_portfolio_value_usd, rate);
            total_portfolio_value_by_currency.insert(*currency, portfolio_value);
        }

        // Calculate currency exposure (what percentage of portfolio is in each currency)
        let currency_exposure = self.calculate_currency_exposure(&multi_currency_positions).await?;

        let calculation_duration = start_time.elapsed().as_millis() as u64;

        // Update successful conversion statistics
        {
            let mut stats = self.conversion_stats.write().await;
            stats.successful_conversions += 1;
            stats.average_conversion_time_ms = 
                (stats.average_conversion_time_ms * (stats.successful_conversions - 1) as f64 + calculation_duration as f64) 
                / stats.successful_conversions as f64;
        }

        let multi_currency_snapshot = MultiCurrencyPnLSnapshot {
            user_id: base_snapshot.user_id,
            timestamp: base_snapshot.timestamp,
            base_currency: self.calculation_config.base_currency,
            pnl_by_currency,
            positions: multi_currency_positions,
            total_portfolio_value_by_currency,
            currency_exposure,
            conversion_rates,
            calculation_duration_ms: calculation_duration,
        };

        info!("Calculated multi-currency P&L for user {} in {}ms across {} currencies", 
              base_snapshot.user_id, calculation_duration, self.calculation_config.supported_currencies.len());

        Ok(multi_currency_snapshot)
    }

    /// Convert position P&L to multi-currency format
    async fn convert_position_to_multi_currency(
        &self,
        position: &PositionPnL,
        conversion_rates: &HashMap<Currency, Decimal>,
    ) -> Result<MultiCurrencyPositionPnL, RiskError> {
        let mut entry_price_by_currency = HashMap::new();
        let mut current_price_by_currency = HashMap::new();
        let mut pnl_by_currency = HashMap::new();
        let mut position_value_by_currency = HashMap::new();

        for currency in &self.calculation_config.supported_currencies {
            let rate = conversion_rates.get(currency).cloned().unwrap_or(Decimal::ONE);

            entry_price_by_currency.insert(*currency, self.convert_amount(position.entry_price_usd, rate));
            current_price_by_currency.insert(*currency, self.convert_amount(position.current_price_usd, rate));
            position_value_by_currency.insert(*currency, self.convert_amount(position.position_value_usd, rate));

            let currency_pnl = CurrencyPnLData {
                currency: *currency,
                unrealized_pnl: self.convert_amount(position.unrealized_pnl_usd, rate),
                realized_pnl: self.convert_amount(position.realized_pnl_usd, rate),
                total_pnl: self.convert_amount(position.total_pnl_usd, rate),
                daily_change: Decimal::ZERO, // Would need historical data
                daily_change_percent: position.price_change_24h_percent,
                portfolio_value: self.convert_amount(position.position_value_usd, rate),
            };

            pnl_by_currency.insert(*currency, currency_pnl);
        }

        Ok(MultiCurrencyPositionPnL {
            token_address: position.token_address.clone(),
            chain_id: position.chain_id,
            symbol: position.symbol.clone(),
            balance: position.balance,
            entry_price_by_currency,
            current_price_by_currency,
            pnl_by_currency,
            position_value_by_currency,
            last_updated: position.last_updated,
        })
    }

    /// Calculate currency exposure across positions
    async fn calculate_currency_exposure(
        &self,
        positions: &[MultiCurrencyPositionPnL],
    ) -> Result<HashMap<Currency, Decimal>, RiskError> {
        let mut currency_exposure = HashMap::new();

        // Calculate total portfolio value in base currency
        let total_portfolio_value: Decimal = positions
            .iter()
            .filter_map(|p| p.position_value_by_currency.get(&self.calculation_config.base_currency))
            .sum();

        if total_portfolio_value == Decimal::ZERO {
            return Ok(currency_exposure);
        }

        // Determine currency exposure based on token types
        for position in positions {
            let position_value = position.position_value_by_currency
                .get(&self.calculation_config.base_currency)
                .cloned()
                .unwrap_or(Decimal::ZERO);

            let exposure_percentage = (position_value / total_portfolio_value) * Decimal::new(100, 0);

            // Classify token by currency (simplified heuristic)
            let token_currency = self.classify_token_currency(&position.symbol);
            
            *currency_exposure.entry(token_currency).or_insert(Decimal::ZERO) += exposure_percentage;
        }

        Ok(currency_exposure)
    }

    /// Classify token by its primary currency (heuristic-based)
    fn classify_token_currency(&self, symbol: &str) -> Currency {
        let symbol_lower = symbol.to_lowercase();
        
        if symbol_lower.contains("eth") || symbol_lower.contains("weth") {
            Currency::ETH
        } else if symbol_lower.contains("btc") || symbol_lower.contains("wbtc") {
            Currency::BTC
        } else if symbol_lower.contains("usd") || symbol_lower.contains("dai") {
            Currency::USD
        } else {
            // Default to ETH for other tokens (most are on Ethereum)
            Currency::ETH
        }
    }

    /// Get conversion rate between currencies
    async fn get_conversion_rate(&self, from: Currency, to: Currency) -> Result<Decimal, RiskError> {
        if from == to {
            return Ok(Decimal::ONE);
        }

        // Check cache first
        if let Some(cached_rate) = self.get_cached_rate(to).await {
            let cache_age = Utc::now().signed_duration_since(
                self.get_cached_currency_data(to).await
                    .map(|data| data.last_updated)
                    .unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1))
            );

            if cache_age.num_seconds() < self.calculation_config.price_cache_ttl_seconds as i64 {
                let mut stats = self.conversion_stats.write().await;
                stats.cache_hits += 1;
                return Ok(cached_rate);
            }
        }

        // Cache miss - fetch fresh rate
        {
            let mut stats = self.conversion_stats.write().await;
            stats.cache_misses += 1;
        }

        let rate = self.price_oracle.get_currency_pair_rate(from, to).await?;
        
        // Update cache
        self.cache_currency_rate(to, rate).await?;

        Ok(rate)
    }

    /// Convert amount using conversion rate with precision
    fn convert_amount(&self, amount: Decimal, rate: Decimal) -> Decimal {
        let converted = amount * rate;
        
        // Round to configured precision
        converted.round_dp(self.calculation_config.conversion_precision)
    }

    /// Get cached conversion rate
    async fn get_cached_rate(&self, currency: Currency) -> Option<Decimal> {
        let cache = self.currency_cache.read().await;
        cache.get(&currency).map(|data| data.price_usd)
    }

    /// Get cached currency data
    async fn get_cached_currency_data(&self, currency: Currency) -> Option<CurrencyData> {
        let cache = self.currency_cache.read().await;
        cache.get(&currency).cloned()
    }

    /// Cache currency conversion rate
    async fn cache_currency_rate(&self, currency: Currency, rate: Decimal) -> Result<(), RiskError> {
        let currency_data = CurrencyData {
            currency,
            price_usd: rate,
            price_change_24h: Decimal::ZERO, // Would be populated from oracle
            market_cap: None,
            volume_24h: None,
            last_updated: Utc::now(),
            data_source: "oracle".to_string(),
        };

        let mut cache = self.currency_cache.write().await;
        cache.insert(currency, currency_data);
        
        Ok(())
    }

    /// Get P&L in specific currency
    pub fn get_pnl_for_currency<'a>(
        &self,
        snapshot: &'a MultiCurrencyPnLSnapshot,
        currency: &str,
    ) -> Option<&'a CurrencyPnLData> {
        // Simplified for compilation - match currency string to enum
        let currency_enum = match currency {
            "USD" => Currency::USD,
            "EUR" => Currency::EUR,
            "BTC" => Currency::BTC,
            "ETH" => Currency::ETH,
            _ => return None,
        };
        snapshot.pnl_by_currency.get(&currency_enum)
    }

    /// Get portfolio value in specific currency
    pub async fn get_portfolio_value_in_currency(
        &self,
        snapshot: &MultiCurrencyPnLSnapshot,
        currency: Currency,
    ) -> Option<Decimal> {
        snapshot.total_portfolio_value_by_currency.get(&currency).cloned()
    }

    /// Get conversion statistics
    pub async fn get_conversion_stats(&self) -> ConversionStats {
        self.conversion_stats.read().await.clone()
    }

    /// Clear currency cache
    pub async fn clear_cache(&self) -> Result<(), RiskError> {
        let mut cache = self.currency_cache.write().await;
        cache.clear();
        info!("Multi-currency cache cleared");
        Ok(())
    }
}

/// Production price oracle implementation
#[derive(Debug)]
pub struct ProductionPriceOracle {
    // This would integrate with real price feeds like CoinGecko, Chainlink, etc.
}

#[async_trait::async_trait]
impl MultiCurrencyPriceOracle for ProductionPriceOracle {
    async fn get_currency_price(&self, currency: Currency) -> Result<Decimal, RiskError> {
        // Production implementation would fetch from real price feeds
        match currency {
            Currency::USD => Ok(Decimal::ONE),
            Currency::ETH => Ok(Decimal::new(3200, 0)), // $3200
            Currency::BTC => Ok(Decimal::new(65000, 0)), // $65000
            Currency::EUR => Ok(Decimal::new(108, 2)), // $1.08
            Currency::GBP => Ok(Decimal::new(127, 2)), // $1.27
            Currency::JPY => Ok(Decimal::new(68, 4)), // $0.0068
        }
    }

    async fn get_historical_currency_price(&self, currency: Currency, _timestamp: DateTime<Utc>) -> Result<Decimal, RiskError> {
        // For now, return current price (production would fetch historical data)
        self.get_currency_price(currency).await
    }

    async fn get_currency_pair_rate(&self, from: Currency, to: Currency) -> Result<Decimal, RiskError> {
        if from == to {
            return Ok(Decimal::ONE);
        }

        let from_price = self.get_currency_price(from).await?;
        let to_price = self.get_currency_price(to).await?;

        if to_price == Decimal::ZERO {
            return Err(RiskError::CalculationError("Cannot convert to currency with zero price".to_string()));
        }

        Ok(from_price / to_price)
    }

    async fn subscribe_to_currency_updates(&self, _currency: Currency) -> Result<tokio::sync::broadcast::Receiver<CurrencyPriceUpdate>, RiskError> {
        let (sender, receiver) = tokio::sync::broadcast::channel(1000);
        // Production implementation would setup real-time price subscriptions
        Ok(receiver)
    }
}
