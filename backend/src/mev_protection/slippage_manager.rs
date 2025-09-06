use crate::types::SwapParams;
use crate::mev_protection::MevProtectionError;
use tracing::{info, warn};
use reqwest::Client;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use rand;
use chrono::{Utc, Timelike};

#[derive(Debug, Clone)]
struct MarketCondition {
    volatility: f64,
    liquidity_depth: f64,
    gas_price: u64,
    mev_risk_score: f64,
    timestamp: Instant,
}

#[derive(Debug, Clone)]
struct SlippageAdjustment {
    original_slippage: f64,
    adjusted_slippage: f64,
    adjustment_reason: String,
    confidence: f64,
}

/// Dynamic slippage adjustment based on market conditions
pub struct DynamicSlippageManager {
    enabled: bool,
    base_slippage: f64,
    max_slippage: f64,
    min_slippage: f64,
    client: Client,
    market_cache: HashMap<String, MarketCondition>,
    cache_duration: Duration,
}

impl DynamicSlippageManager {
    pub async fn new() -> Self {
        Self {
            enabled: true,
            base_slippage: 0.005, // 0.5% base slippage
            max_slippage: 0.05,   // 5% maximum slippage
            min_slippage: 0.001,  // 0.1% minimum slippage
            client: Client::new(),
            market_cache: HashMap::new(),
            cache_duration: Duration::from_secs(30), // 30 second cache
        }
    }

    pub async fn adjust_slippage(&self, params: &SwapParams) -> Result<SwapParams, MevProtectionError> {
        if !self.enabled {
            return Ok(params.clone());
        }

        info!("⚙️ Adjusting slippage dynamically for {}->{} swap", params.token_in, params.token_out);
        
        // Step 1: Analyze current market conditions
        let market_condition = self.analyze_market_conditions(params).await?;
        
        // Step 2: Calculate dynamic slippage adjustment
        let adjustment = self.calculate_slippage_adjustment(params, &market_condition).await?;
        
        // Apply the adjusted slippage by modifying amount_out_min
        let mut protected_params = params.clone();
        let adjusted_amount_out_min = self.apply_slippage_to_amount(&protected_params, adjustment.adjusted_slippage)?;
        protected_params.amount_out_min = adjusted_amount_out_min;
        
        info!("✅ Slippage adjusted: {:.3}% -> {:.3}% ({})", 
              adjustment.original_slippage * 100.0, 
              adjustment.adjusted_slippage * 100.0, 
              adjustment.adjustment_reason);
        
        Ok(protected_params)
    }

    pub async fn enable(&mut self) {
        self.enabled = true;
        info!("Dynamic slippage management enabled");
    }

    pub async fn disable(&mut self) {
        self.enabled = false;
        warn!("Dynamic slippage management disabled");
    }
    
    /// Analyze current market conditions for the token pair
    async fn analyze_market_conditions(&self, params: &SwapParams) -> Result<MarketCondition, MevProtectionError> {
        let pair_key = format!("{}-{}", params.token_in, params.token_out);
        
        // Check cache first
        if let Some(cached) = self.market_cache.get(&pair_key) {
            if cached.timestamp.elapsed() < self.cache_duration {
                info!("📊 Using cached market conditions for {}", pair_key);
                return Ok(cached.clone());
            }
        }
        
        info!("🔍 Analyzing market conditions for {}", pair_key);
        
        // Analyze multiple market factors
        let volatility = self.calculate_volatility(params).await?;
        let liquidity_depth = self.assess_liquidity_depth(params).await?;
        let gas_price = self.get_current_gas_price().await?;
        let mev_risk_score = self.calculate_mev_risk(params).await?;
        
        let condition = MarketCondition {
            volatility,
            liquidity_depth,
            gas_price,
            mev_risk_score,
            timestamp: Instant::now(),
        };
        
        info!("📊 Market analysis: volatility={:.3}, liquidity={:.0}K, gas={} gwei, mev_risk={:.3}", 
              volatility, liquidity_depth / 1000.0, gas_price / 1_000_000_000, mev_risk_score);
        
        Ok(condition)
    }
    
    /// Calculate dynamic slippage adjustment based on market conditions
    async fn calculate_slippage_adjustment(&self, params: &SwapParams, market: &MarketCondition) -> Result<SlippageAdjustment, MevProtectionError> {
        // Calculate slippage from amount_out_min if not directly available
        let original_slippage = self.calculate_current_slippage(params);
        let mut adjusted_slippage = original_slippage;
        let mut reasons = Vec::new();
        let mut confidence: f64 = 1.0;
        
        // Factor 1: Volatility adjustment
        if market.volatility > 0.05 { // High volatility (>5%)
            let volatility_multiplier = 1.0 + (market.volatility - 0.05) * 2.0;
            adjusted_slippage *= volatility_multiplier;
            reasons.push(format!("high volatility ({:.1}%)", market.volatility * 100.0));
            confidence *= 0.9;
        } else if market.volatility < 0.01 { // Low volatility (<1%)
            adjusted_slippage *= 0.8; // Reduce slippage in stable conditions
            reasons.push("low volatility".to_string());
            confidence *= 1.1;
        }
        
        // Factor 2: Liquidity depth adjustment
        if market.liquidity_depth < 100_000.0 { // Low liquidity (<$100K)
            let liquidity_multiplier = 1.0 + (100_000.0 - market.liquidity_depth) / 200_000.0;
            adjusted_slippage *= liquidity_multiplier;
            reasons.push(format!("low liquidity (${:.0}K)", market.liquidity_depth / 1000.0));
            confidence *= 0.8;
        }
        
        // Factor 3: Gas price adjustment (high gas = more MEV competition)
        if market.gas_price > 50_000_000_000 { // >50 gwei
            let gas_multiplier = 1.0 + ((market.gas_price as f64 - 50_000_000_000.0) / 100_000_000_000.0) * 0.5;
            adjusted_slippage *= gas_multiplier;
            reasons.push(format!("high gas ({} gwei)", market.gas_price / 1_000_000_000));
            confidence *= 0.85;
        }
        
        // Factor 4: MEV risk adjustment
        if market.mev_risk_score > 0.7 { // High MEV risk
            adjusted_slippage *= 1.0 + market.mev_risk_score * 0.3;
            reasons.push(format!("high MEV risk ({:.1}%)", market.mev_risk_score * 100.0));
            confidence *= 0.75;
        }
        
        // Factor 5: Trade size adjustment
        let trade_size_usd = self.estimate_trade_size_usd(params).await.unwrap_or(1000.0);
        if trade_size_usd > 50_000.0 { // Large trades need more slippage
            let size_multiplier = 1.0 + (trade_size_usd - 50_000.0) / 500_000.0 * 0.2;
            adjusted_slippage *= size_multiplier;
            reasons.push(format!("large trade (${:.0}K)", trade_size_usd / 1000.0));
        }
        
        // Apply safety bounds
        adjusted_slippage = adjusted_slippage.max(self.min_slippage).min(self.max_slippage);
        
        let adjustment_reason = if reasons.is_empty() {
            "optimal conditions".to_string()
        } else {
            reasons.join(", ")
        };
        
        Ok(SlippageAdjustment {
            original_slippage,
            adjusted_slippage,
            adjustment_reason,
            confidence: confidence.min(1.0).max(0.1),
        })
    }
    
    /// Calculate token pair volatility
    async fn calculate_volatility(&self, params: &SwapParams) -> Result<f64, MevProtectionError> {
        // In production, this would fetch real price data from multiple sources
        // For now, simulate volatility based on token types
        let volatility = match (params.token_in.as_str(), params.token_out.as_str()) {
            ("ETH", "USDC") | ("USDC", "ETH") => 0.03, // 3% - major pair
            ("ETH", "WBTC") | ("WBTC", "ETH") => 0.04, // 4% - crypto-crypto
            ("USDC", "USDT") | ("USDT", "USDC") => 0.001, // 0.1% - stablecoin pair
            ("USDC", "DAI") | ("DAI", "USDC") => 0.002, // 0.2% - stablecoin pair
            _ => 0.06, // 6% - unknown/exotic pairs
        };
        
        // Add some randomness to simulate real market conditions
        let noise = (rand::random::<f64>() - 0.5) * 0.01; // ±0.5% noise
        Ok((volatility + noise).max(0.001))
    }
    
    /// Assess liquidity depth for the token pair
    async fn assess_liquidity_depth(&self, params: &SwapParams) -> Result<f64, MevProtectionError> {
        // Simulate liquidity depth based on token pair popularity
        let base_liquidity = match (params.token_in.as_str(), params.token_out.as_str()) {
            ("ETH", "USDC") | ("USDC", "ETH") => 10_000_000.0, // $10M - major pair
            ("ETH", "WBTC") | ("WBTC", "ETH") => 5_000_000.0,  // $5M
            ("USDC", "USDT") | ("USDT", "USDC") => 8_000_000.0, // $8M - stables
            ("USDC", "DAI") | ("DAI", "USDC") => 3_000_000.0,   // $3M
            _ => 500_000.0, // $500K - smaller pairs
        };
        
        // Add market condition variance
        let variance = (rand::random::<f64>() - 0.5) * 0.3; // ±15% variance
        Ok(base_liquidity * (1.0 + variance))
    }
    
    /// Get current gas price from network
    async fn get_current_gas_price(&self) -> Result<u64, MevProtectionError> {
        // In production, this would query real gas price APIs
        // Simulate current gas conditions
        let base_gas = 20_000_000_000u64; // 20 gwei base
        let congestion_multiplier = 1.0 + rand::random::<f64>() * 3.0; // 1x to 4x multiplier
        Ok((base_gas as f64 * congestion_multiplier) as u64)
    }
    
    /// Calculate MEV risk score for the transaction
    async fn calculate_mev_risk(&self, params: &SwapParams) -> Result<f64, MevProtectionError> {
        let mut risk_score: f64 = 0.0;
        
        // Higher risk for popular pairs (more MEV bots watching)
        match (params.token_in.as_str(), params.token_out.as_str()) {
            ("ETH", "USDC") | ("USDC", "ETH") => risk_score += 0.8, // High MEV activity
            ("ETH", "WBTC") | ("WBTC", "ETH") => risk_score += 0.6,
            ("USDC", "USDT") | ("USDT", "USDC") => risk_score += 0.3, // Lower MEV on stables
            _ => risk_score += 0.4, // Medium risk for other pairs
        }
        
        // Trade size impact on MEV risk
        let trade_size = params.amount_in.parse::<f64>().unwrap_or(0.0);
        if trade_size > 1_000_000_000_000_000_000.0 { // > 1 ETH equivalent
            risk_score += 0.2;
        }
        
        // Time-based risk (higher during active trading hours)
        let hour = Utc::now().hour();
        if (13..=21).contains(&hour) { // 1 PM to 9 PM UTC (active trading)
            risk_score += 0.1;
        }
        
        Ok(risk_score.min(1.0))
    }
    
    /// Estimate trade size in USD
    async fn estimate_trade_size_usd(&self, params: &SwapParams) -> Result<f64, MevProtectionError> {
        let amount = params.amount_in.parse::<f64>().unwrap_or(0.0);
        
        // Rough USD conversion (in production, use real price feeds)
        let usd_value = match params.token_in.as_str() {
            "ETH" | "WETH" => amount / 1_000_000_000_000_000_000.0 * 2500.0, // ~$2500/ETH
            "WBTC" => amount / 100_000_000.0 * 45000.0, // ~$45K/BTC
            "USDC" | "USDT" | "DAI" => amount / 1_000_000.0, // $1 per token
            _ => amount / 1_000_000_000_000_000_000.0 * 100.0, // Default $100 per token
        };
        
        Ok(usd_value)
    }
    
    /// Calculate current slippage from swap parameters
    fn calculate_current_slippage(&self, params: &SwapParams) -> f64 {
        // Estimate slippage based on amount_in vs amount_out_min
        let amount_in = params.amount_in.parse::<f64>().unwrap_or(0.0);
        let amount_out_min = params.amount_out_min.parse::<f64>().unwrap_or(0.0);
        
        if amount_in > 0.0 && amount_out_min > 0.0 {
            // Calculate implied slippage from the ratio
            let implied_rate = amount_out_min / amount_in;
            let expected_rate = 1.0; // Assume 1:1 for simplicity
            let slippage = (expected_rate - implied_rate) / expected_rate;
            slippage.max(0.001).min(0.1) // Bound between 0.1% and 10%
        } else {
            self.base_slippage // Fallback to base slippage
        }
    }
    
    /// Apply slippage adjustment to amount_out_min
    fn apply_slippage_to_amount(&self, params: &SwapParams, new_slippage: f64) -> Result<String, MevProtectionError> {
        let amount_in = params.amount_in.parse::<f64>().unwrap_or(0.0);
        
        if amount_in <= 0.0 {
            return Ok(params.amount_out_min.clone());
        }
        
        // Calculate new amount_out_min with adjusted slippage
        let expected_output = amount_in; // Simplified 1:1 assumption
        let adjusted_output = expected_output * (1.0 - new_slippage);
        
        Ok(adjusted_output.to_string())
    }

    /// Get slippage statistics for monitoring
    pub async fn get_slippage_stats(&self) -> HashMap<String, f64> {
        let mut stats = HashMap::new();
        stats.insert("base_slippage".to_string(), self.base_slippage);
        stats.insert("min_slippage".to_string(), self.min_slippage);
        stats.insert("max_slippage".to_string(), self.max_slippage);
        stats.insert("cache_entries".to_string(), self.market_cache.len() as f64);
        stats
    }
}
