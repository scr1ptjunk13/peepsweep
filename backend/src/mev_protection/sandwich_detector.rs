use crate::types::SwapParams;
use super::MevProtectionError;
use tracing::{info, warn};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde_json::Value;
use reqwest::Client;

/// Sandwich attack detection system
pub struct SandwichDetector {
    enabled: bool,
    detection_threshold: f64,
    client: Client,
    mempool_cache: HashMap<String, MempoolTransaction>,
    suspicious_patterns: Vec<SandwichPattern>,
}

#[derive(Debug, Clone)]
struct MempoolTransaction {
    hash: String,
    from: String,
    to: String,
    value: String,
    gas_price: u64,
    timestamp: Instant,
    token_in: Option<String>,
    token_out: Option<String>,
    amount_in: Option<String>,
}

#[derive(Debug, Clone)]
struct SandwichPattern {
    front_run_tx: String,
    target_tx: String,
    back_run_tx: String,
    detected_at: Instant,
    confidence: f64,
}

impl SandwichDetector {
    pub async fn new() -> Self {
        Self {
            enabled: true,
            detection_threshold: 0.20, // 20% threshold for sandwich detection (temporarily relaxed for testing)
            client: Client::new(),
            mempool_cache: HashMap::new(),
            suspicious_patterns: Vec::new(),
        }
    }

    pub async fn analyze_transaction(&mut self, params: &SwapParams) -> Result<(), MevProtectionError> {
        if !self.enabled {
            return Ok(());
        }

        info!("🔍 Analyzing transaction for sandwich attacks");
        
        // Step 1: Fetch current mempool transactions
        let mempool_txs = self.fetch_mempool_transactions().await?;
        
        // Step 2: Analyze for sandwich patterns
        let risk_score = self.calculate_sandwich_risk(params, &mempool_txs).await?;
        
        // Step 3: Check for suspicious gas price patterns
        let gas_anomaly = self.detect_gas_price_anomalies(&mempool_txs).await?;
        
        // Step 4: Analyze token pair activity
        let pair_activity = self.analyze_token_pair_activity(params, &mempool_txs).await?;
        
        // Step 5: Calculate overall threat level
        let threat_level = (risk_score + gas_anomaly + pair_activity) / 3.0;
        
        info!("🔍 Sandwich risk analysis: risk_score={:.3}, gas_anomaly={:.3}, pair_activity={:.3}, threat_level={:.3}", 
              risk_score, gas_anomaly, pair_activity, threat_level);
        
        if threat_level > self.detection_threshold {
            warn!("⚠️ HIGH SANDWICH RISK DETECTED: {:.1}% threat level", threat_level * 100.0);
            return Err(MevProtectionError::SandwichDetected(
                format!("Sandwich attack risk: {:.1}% (threshold: {:.1}%)", 
                       threat_level * 100.0, self.detection_threshold * 100.0)
            ));
        }
        
        info!("✅ Sandwich attack analysis passed");
        Ok(())
    }

    async fn fetch_mempool_transactions(&self) -> Result<Vec<MempoolTransaction>, MevProtectionError> {
        // In production, this would connect to Ethereum mempool via WebSocket
        // For now, simulate mempool analysis with realistic patterns
        
        let mut transactions = Vec::new();
        
        // Simulate some mempool transactions with varying gas prices
        for i in 0..10 {
            transactions.push(MempoolTransaction {
                hash: format!("0x{:064x}", i),
                from: format!("0x{:040x}", i * 123),
                to: "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string(), // Uniswap V3 Router
                value: "0".to_string(),
                gas_price: 20_000_000_000 + (i * 1_000_000_000), // Varying gas prices
                timestamp: Instant::now(),
                token_in: Some("ETH".to_string()),
                token_out: Some("USDC".to_string()),
                amount_in: Some(format!("{}", 1000000000000000000u64 + i * 100000000000000000u64)),
            });
        }
        
        Ok(transactions)
    }
    
    async fn calculate_sandwich_risk(&self, params: &SwapParams, mempool_txs: &[MempoolTransaction]) -> Result<f64, MevProtectionError> {
        let mut risk_score: f64 = 0.0;
        
        // Check for transactions targeting the same token pair
        let same_pair_count = mempool_txs.iter()
            .filter(|tx| {
                tx.token_in.as_ref() == Some(&params.token_in) && 
                tx.token_out.as_ref() == Some(&params.token_out)
            })
            .count();
        
        // Higher risk if many transactions target same pair
        if same_pair_count > 3 {
            risk_score += 0.3;
        } else if same_pair_count > 1 {
            risk_score += 0.1;
        }
        
        // Check for large transactions that could impact price
        let user_amount: u64 = params.amount_in.parse().unwrap_or(0);
        let large_tx_count = mempool_txs.iter()
            .filter(|tx| {
                if let Some(amount_str) = &tx.amount_in {
                    if let Ok(amount) = amount_str.parse::<u64>() {
                        return amount > user_amount / 2; // Transactions > 50% of user's size
                    }
                }
                false
            })
            .count();
        
        if large_tx_count > 2 {
            risk_score += 0.4;
        } else if large_tx_count > 0 {
            risk_score += 0.2;
        }
        
        Ok(risk_score.min(1.0))
    }
    
    async fn detect_gas_price_anomalies(&self, mempool_txs: &[MempoolTransaction]) -> Result<f64, MevProtectionError> {
        if mempool_txs.is_empty() {
            return Ok(0.0);
        }
        
        // Calculate average gas price
        let avg_gas_price: f64 = mempool_txs.iter()
            .map(|tx| tx.gas_price as f64)
            .sum::<f64>() / mempool_txs.len() as f64;
        
        // Check for unusually high gas prices (potential front-runners)
        let high_gas_count = mempool_txs.iter()
            .filter(|tx| tx.gas_price as f64 > avg_gas_price * 1.5)
            .count();
        
        let anomaly_score = if high_gas_count > mempool_txs.len() / 3 {
            0.5 // High anomaly if >33% of transactions have high gas
        } else if high_gas_count > 0 {
            0.2 // Moderate anomaly
        } else {
            0.0
        };
        
        Ok(anomaly_score)
    }
    
    async fn analyze_token_pair_activity(&self, params: &SwapParams, mempool_txs: &[MempoolTransaction]) -> Result<f64, MevProtectionError> {
        // Analyze activity patterns for the specific token pair
        let pair_transactions: Vec<_> = mempool_txs.iter()
            .filter(|tx| {
                (tx.token_in.as_ref() == Some(&params.token_in) && tx.token_out.as_ref() == Some(&params.token_out)) ||
                (tx.token_in.as_ref() == Some(&params.token_out) && tx.token_out.as_ref() == Some(&params.token_in))
            })
            .collect();
        
        if pair_transactions.is_empty() {
            return Ok(0.0);
        }
        
        // Check for rapid succession of transactions (potential sandwich setup)
        let mut activity_score: f64 = 0.0;
        
        // Sort by timestamp and check intervals
        let mut sorted_txs = pair_transactions.clone();
        sorted_txs.sort_by_key(|tx| tx.timestamp);
        
        for window in sorted_txs.windows(2) {
            let time_diff = window[1].timestamp.duration_since(window[0].timestamp);
            if time_diff < Duration::from_secs(30) { // Transactions within 30 seconds
                activity_score += 0.2;
            }
        }
        
        // Check for alternating buy/sell patterns
        let buy_count = pair_transactions.iter()
            .filter(|tx| tx.token_in.as_ref() == Some(&params.token_in))
            .count();
        let sell_count = pair_transactions.len() - buy_count;
        
        if buy_count > 0 && sell_count > 0 && (buy_count as f64 - sell_count as f64).abs() <= 1.0 {
            activity_score += 0.3; // Suspicious if equal buy/sell activity
        }
        
        Ok(activity_score.min(1.0))
    }
    
    pub async fn enable(&mut self) {
        self.enabled = true;
        info!("Sandwich attack detection enabled");
    }

    pub async fn disable(&mut self) {
        self.enabled = false;
        warn!("Sandwich attack detection disabled");
    }
    
    pub fn get_detection_stats(&self) -> (usize, usize) {
        (self.mempool_cache.len(), self.suspicious_patterns.len())
    }
}
