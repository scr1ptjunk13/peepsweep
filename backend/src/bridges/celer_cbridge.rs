use super::{BridgeIntegration, BridgeQuote, BridgeResponse, BridgeError, BridgeStatus, CrossChainParams, BridgeStep};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct CelerCBridge {
    client: Client,
    base_url: String,
    supported_chains: Vec<u64>,
    supported_tokens: HashMap<u64, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct CelerEstimateResponse {
    #[serde(rename = "eq_value_token_amt")]
    eq_value_token_amt: String,
    #[serde(rename = "bridge_rate")]
    bridge_rate: f64,
    #[serde(rename = "perc_fee")]
    perc_fee: String,
    #[serde(rename = "base_fee")]
    base_fee: String,
    #[serde(rename = "slippage_tolerance")]
    slippage_tolerance: u64,
    #[serde(rename = "max_slippage")]
    max_slippage: u64,
    #[serde(rename = "estimated_receive_amt")]
    estimated_receive_amt: String,
}

#[derive(Debug, Serialize)]
struct CelerEstimateRequest {
    src_chain_id: u64,
    dst_chain_id: u64,
    token_symbol: String,
    usr_addr: String,
    slippage_tolerance: u64,
    amt: String,
    is_pegged: bool,
}

#[derive(Debug, Deserialize)]
struct CelerTransferConfig {
    chains: Vec<CelerChain>,
    chain_token: HashMap<String, CelerChainTokenInfo>,
    farming_reward_contract_addr: String,
    pegged_pair_configs: Vec<CelerPeggedPairConfig>,
}

#[derive(Debug, Deserialize)]
struct CelerChain {
    id: u64,
    name: String,
    icon: String,
    block_delay: u64,
    gas_token_symbol: String,
    explore_url: String,
    contract_addr: String,
    drop_gas_amt: String,
    drop_gas_cost_amt: String,
    drop_gas_balance_alert: String,
}

#[derive(Debug, Deserialize)]
struct CelerChainTokenInfo {
    token: HashMap<String, CelerToken>,
}

#[derive(Debug, Deserialize)]
struct CelerToken {
    token: CelerTokenDetails,
    name: String,
    icon: String,
    max_amt: String,
    min_amt: String,
    balance_warn_threshold: String,
    delay_threshold: String,
    delay_period: u64,
}

#[derive(Debug, Deserialize)]
struct CelerTokenDetails {
    symbol: String,
    address: String,
    decimal: u8,
    xfer_disabled: bool,
}

#[derive(Debug, Deserialize)]
struct CelerPeggedPairConfig {
    org_chain_id: u64,
    org_token: CelerTokenDetails,
    pegged_chain_id: u64,
    pegged_token: CelerTokenDetails,
    pegged_deposit_contract_addr: String,
    pegged_burn_contract_addr: String,
    canonical_token_contract_addr: String,
    vault_version: u64,
    bridge_version: u64,
}

impl CelerCBridge {
    pub fn new() -> Self {
        let mut supported_tokens = HashMap::new();
        
        // Ethereum mainnet tokens
        supported_tokens.insert(1, vec![
            "ETH".to_string(), "USDC".to_string(), "USDT".to_string(), 
            "DAI".to_string(), "WBTC".to_string()
        ]);
        
        // BSC tokens
        supported_tokens.insert(56, vec![
            "BNB".to_string(), "USDC".to_string(), "USDT".to_string(), 
            "BUSD".to_string(), "ETH".to_string()
        ]);
        
        // Polygon tokens
        supported_tokens.insert(137, vec![
            "MATIC".to_string(), "USDC".to_string(), "USDT".to_string(), 
            "DAI".to_string(), "WETH".to_string()
        ]);
        
        // Arbitrum tokens
        supported_tokens.insert(42161, vec![
            "ETH".to_string(), "USDC".to_string(), "USDT".to_string(), 
            "DAI".to_string(), "WBTC".to_string()
        ]);
        
        // Optimism tokens
        supported_tokens.insert(10, vec![
            "ETH".to_string(), "USDC".to_string(), "USDT".to_string(), 
            "DAI".to_string(), "OP".to_string()
        ]);
        
        // Avalanche tokens
        supported_tokens.insert(43114, vec![
            "AVAX".to_string(), "USDC".to_string(), "USDT".to_string(), 
            "DAI".to_string(), "WETH".to_string()
        ]);

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://cbridge-prod2.celer.app".to_string(),
            supported_chains: vec![1, 56, 137, 42161, 10, 43114],
            supported_tokens,
        }
    }

    pub fn supported_chains(&self) -> &Vec<u64> {
        &self.supported_chains
    }

    pub fn is_route_supported(&self, from_chain: u64, to_chain: u64, token: &str) -> bool {
        self.supported_chains.contains(&from_chain) &&
        self.supported_chains.contains(&to_chain) &&
        from_chain != to_chain &&
        self.supported_tokens.get(&from_chain).map_or(false, |tokens| tokens.contains(&token.to_string()))
    }

    fn normalize_token_symbol(&self, token: &str) -> String {
        match token.to_uppercase().as_str() {
            "WETH" => "ETH".to_string(),
            "WMATIC" => "MATIC".to_string(),
            "WBNB" => "BNB".to_string(),
            _ => token.to_uppercase(),
        }
    }

    fn calculate_slippage_tolerance(&self, slippage_percent: f64) -> u64 {
        // Convert percentage to Celer's format: slippage_tolerance = slippage_percent * 1M / 100
        // For example: 0.5% becomes 0.5 * 1M / 100 = 5000
        (slippage_percent * 1_000_000.0 / 100.0) as u64
    }

    async fn fetch_transfer_configs(&self) -> Result<CelerTransferConfig, BridgeError> {
        let url = format!("{}/v2/getTransferConfigsForAll", self.base_url);
        
        tracing::info!("Fetching Celer cBridge transfer configs from: {}", url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| BridgeError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Celer cBridge config API error {}: {}", status, error_text);
            return Err(BridgeError::NetworkError(format!("HTTP {}: {}", status, error_text)));
        }

        let config: CelerTransferConfig = response
            .json()
            .await
            .map_err(|e| BridgeError::NetworkError(format!("JSON parsing error: {}", e)))?;

        tracing::info!("Celer cBridge configs received: {} chains, {} chain tokens", 
                   config.chains.len(), config.chain_token.len());

        Ok(config)
    }

    async fn fetch_quote(&self, params: &CrossChainParams) -> Result<CelerEstimateResponse, BridgeError> {
        let token_symbol = self.normalize_token_symbol(&params.token_in);
        let slippage_tolerance = self.calculate_slippage_tolerance(params.slippage);

        // Use POST request for EstimateAmt API
        let url = format!("{}/v2/estimateAmt", self.base_url);

        let request_body = CelerEstimateRequest {
            src_chain_id: params.from_chain_id,
            dst_chain_id: params.to_chain_id,
            token_symbol,
            usr_addr: params.user_address.clone(),
            slippage_tolerance,
            amt: params.amount_in.clone(),
            is_pegged: false, // Default to false, can be enhanced later
        };

        tracing::info!("Fetching Celer cBridge quote from: {}", url);

        let response = self.client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| BridgeError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Celer cBridge API error {}: {}", status, error_text);
            return Err(BridgeError::NetworkError(format!("HTTP {}: {}", status, error_text)));
        }

        let quote: CelerEstimateResponse = response
            .json()
            .await
            .map_err(|e| BridgeError::NetworkError(format!("JSON parsing error: {}", e)))?;

        tracing::info!("Celer cBridge quote received: estimated_receive_amt={}, bridge_rate={}", 
                   quote.estimated_receive_amt, quote.bridge_rate);

        Ok(quote)
    }
}

#[async_trait]
impl BridgeIntegration for CelerCBridge {
    fn name(&self) -> &str {
        "Celer cBridge"
    }


    async fn get_quote(&self, params: &CrossChainParams) -> Result<BridgeQuote, BridgeError> {
        // Check if route is supported
        if !self.supported_chains.contains(&params.from_chain_id) ||
           !self.supported_chains.contains(&params.to_chain_id) {
            return Err(BridgeError::UnsupportedRoute);
        }

        let token_symbol = self.normalize_token_symbol(&params.token_in);
        if let Some(tokens) = self.supported_tokens.get(&params.from_chain_id) {
            if !tokens.contains(&token_symbol) {
                return Err(BridgeError::UnsupportedRoute);
            }
        }

        // Try to fetch real quote from Celer cBridge API
        match self.fetch_quote(params).await {
            Ok(quote) => {
                let total_fee = quote.perc_fee.parse::<u64>().unwrap_or(0) + 
                               quote.base_fee.parse::<u64>().unwrap_or(0);

                let estimated_receive_amt = quote.estimated_receive_amt.clone();
                
                Ok(BridgeQuote {
                    bridge_name: self.name().to_string(),
                    amount_out: quote.estimated_receive_amt,
                    estimated_time: 300, // 5 minutes typical for Celer cBridge
                    fee: total_fee.to_string(),
                    gas_estimate: "120000".to_string(), // Typical gas for cBridge transfers
                    route: vec![BridgeStep {
                        bridge: self.name().to_string(),
                        from_chain: params.from_chain_id,
                        to_chain: params.to_chain_id,
                        token_in: params.token_in.clone(),
                        token_out: params.token_out.clone(),
                        amount_in: params.amount_in.clone(),
                        amount_out: estimated_receive_amt,
                        estimated_time: 300,
                    }],
                    confidence_score: 0.90, // High confidence for Celer cBridge
                    liquidity_available: quote.eq_value_token_amt,
                })
            }
            Err(e) => {
                tracing::warn!("Celer cBridge API failed, using fallback quote: {}", e);
                
                // Fallback quote calculation
                let amount_in_f64: f64 = params.amount_in.parse().unwrap_or(1000000.0);
                let amount_out = (amount_in_f64 * 0.998) as u64; // 0.2% fee
                let fee = (amount_in_f64 * 0.002) as u64;

                Ok(BridgeQuote {
                    bridge_name: self.name().to_string(),
                    amount_out: amount_out.to_string(),
                    estimated_time: 300,
                    fee: fee.to_string(),
                    gas_estimate: "120000".to_string(),
                    route: vec![BridgeStep {
                        bridge: self.name().to_string(),
                        from_chain: params.from_chain_id,
                        to_chain: params.to_chain_id,
                        token_in: params.token_in.clone(),
                        token_out: params.token_out.clone(),
                        amount_in: params.amount_in.clone(),
                        amount_out: amount_out.to_string(),
                        estimated_time: 300,
                    }],
                    confidence_score: 0.75, // Lower confidence for fallback
                    liquidity_available: (amount_in_f64 * 1000.0).to_string(),
                })
            }
        }
    }

    async fn execute_bridge(&self, params: &CrossChainParams) -> Result<BridgeResponse, BridgeError> {
        // This would implement the actual bridge execution
        // For now, return a mock response
        Ok(BridgeResponse {
            transaction_hash: format!("0xceler{:x}", rand::random::<u64>()),
            bridge_id: self.name().to_string(),
            status: BridgeStatus::Pending,
            estimated_completion: 300,
            tracking_url: Some("https://cbridge.celer.network/".to_string()),
        })
    }

    fn supports_route(&self, from_chain: u64, to_chain: u64) -> bool {
        self.supported_chains.contains(&from_chain) && self.supported_chains.contains(&to_chain)
    }

    fn get_supported_tokens(&self, chain_id: u64) -> Vec<String> {
        self.supported_tokens.get(&chain_id).cloned().unwrap_or_default()
    }

    async fn get_status(&self, _tx_hash: &str) -> Result<BridgeStatus, BridgeError> {
        // This would check the actual bridge status
        // For now, return a mock status
        Ok(BridgeStatus::Completed)
    }

    async fn health_check(&self) -> Result<bool, BridgeError> {
        // Try to fetch transfer configs to check if API is healthy
        match self.fetch_transfer_configs().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // Return false instead of error for health check
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_celer_cbridge_initialization() {
        let bridge = CelerCBridge::new();
        assert_eq!(bridge.name(), "Celer cBridge");
        assert!(bridge.supported_chains().contains(&1)); // Ethereum
        assert!(bridge.supported_chains().contains(&42161)); // Arbitrum
    }

    #[tokio::test]
    async fn test_route_support() {
        let bridge = CelerCBridge::new();
        
        // Test supported route
        assert!(bridge.is_route_supported(1, 42161, "USDC"));
        assert!(bridge.is_route_supported(1, 137, "ETH"));
        
        // Test unsupported route
        assert!(!bridge.is_route_supported(999, 42161, "USDC"));
        assert!(!bridge.is_route_supported(1, 42161, "UNKNOWN"));
    }

    #[test]
    fn test_slippage_calculation() {
        let bridge = CelerCBridge::new();
        
        // 0.5% should become 5000
        assert_eq!(bridge.calculate_slippage_tolerance(0.5), 5000);
        
        // 1.0% should become 10000
        assert_eq!(bridge.calculate_slippage_tolerance(1.0), 10000);
    }

    #[test]
    fn test_token_normalization() {
        let bridge = CelerCBridge::new();
        
        assert_eq!(bridge.normalize_token_symbol("WETH"), "ETH");
        assert_eq!(bridge.normalize_token_symbol("wmatic"), "MATIC");
        assert_eq!(bridge.normalize_token_symbol("USDC"), "USDC");
    }
}
