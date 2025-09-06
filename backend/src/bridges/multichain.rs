use super::{BridgeIntegration, BridgeQuote, BridgeResponse, BridgeError, BridgeStatus, CrossChainParams, BridgeStep};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Multichain {
    client: Client,
    base_url: String,
    supported_chains: Vec<u64>,
    supported_tokens: HashMap<u64, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct MultichainServerInfo {
    #[serde(rename = "chainID")]
    chain_id: String,
    #[serde(rename = "version")]
    version: String,
    #[serde(rename = "routerContract")]
    router_contract: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MultichainTokenInfo {
    #[serde(rename = "chainId")]
    chain_id: String,
    #[serde(rename = "address")]
    address: String,
    #[serde(rename = "decimals")]
    decimals: u8,
    #[serde(rename = "symbol")]
    symbol: String,
    #[serde(rename = "name")]
    name: String,
}

impl Multichain {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        let mut supported_tokens = HashMap::new();
        
        // Ethereum mainnet tokens
        supported_tokens.insert(1, vec![
            "ETH".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "LINK".to_string(),
            "UNI".to_string(),
            "AAVE".to_string(),
            "MULTI".to_string(),
        ]);
        
        // BSC tokens
        supported_tokens.insert(56, vec![
            "BNB".to_string(),
            "USDT".to_string(),
            "BUSD".to_string(),
            "CAKE".to_string(),
            "MULTI".to_string(),
        ]);
        
        // Polygon tokens
        supported_tokens.insert(137, vec![
            "MATIC".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WETH".to_string(),
            "MULTI".to_string(),
        ]);
        
        // Avalanche tokens
        supported_tokens.insert(43114, vec![
            "AVAX".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WETH".to_string(),
            "MULTI".to_string(),
        ]);
        
        // Fantom tokens
        supported_tokens.insert(250, vec![
            "FTM".to_string(),
            "USDC".to_string(),
            "DAI".to_string(),
            "WETH".to_string(),
            "MULTI".to_string(),
        ]);

        Self {
            client,
            base_url: "https://bridgeapi.anyswap.exchange".to_string(),
            supported_chains: vec![1, 56, 137, 43114, 250, 42161, 10, 128, 66, 321], // 50+ chains supported
            supported_tokens,
        }
    }

    fn get_multichain_token_id(&self, chain_id: u64, symbol: &str) -> Option<String> {
        // Multichain uses specific token IDs for cross-chain transfers
        match (chain_id, symbol.to_uppercase().as_str()) {
            // Ethereum mainnet
            (1, "ETH") => Some("ETH".to_string()),
            (1, "USDC") => Some("0xA0b86a33E6441E6C5a6F6c7e2C0d3C8C8a2B0e8B".to_string()),
            (1, "USDT") => Some("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()),
            (1, "DAI") => Some("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()),
            (1, "WBTC") => Some("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string()),
            
            // BSC
            (56, "BNB") => Some("BNB".to_string()),
            (56, "USDT") => Some("0x55d398326f99059fF775485246999027B3197955".to_string()),
            (56, "BUSD") => Some("0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56".to_string()),
            
            // Polygon
            (137, "MATIC") => Some("MATIC".to_string()),
            (137, "USDC") => Some("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()),
            (137, "USDT") => Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string()),
            
            // Avalanche
            (43114, "AVAX") => Some("AVAX".to_string()),
            (43114, "USDC") => Some("0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E".to_string()),
            (43114, "USDT") => Some("0x9702230A8Ea53601f5cD2dc00fDBc13d4dF4A8c7".to_string()),
            
            // Fantom
            (250, "FTM") => Some("FTM".to_string()),
            (250, "USDC") => Some("0x04068DA6C83AFCFA0e13ba15A6696662335D5B75".to_string()),
            
            _ => None,
        }
    }

    async fn fetch_server_info(&self) -> Result<MultichainServerInfo, BridgeError> {
        let url = format!("{}/v2/serverInfo", self.base_url);

        tracing::info!("Fetching Multichain server info from: {}", url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| BridgeError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Multichain API error {}: {}", status, error_text);
            return Err(BridgeError::NetworkError(format!("HTTP {}: {}", status, error_text)));
        }

        let server_info: MultichainServerInfo = response
            .json()
            .await
            .map_err(|e| BridgeError::NetworkError(format!("JSON parsing error: {}", e)))?;

        tracing::info!("Multichain server info received: version={}", server_info.version);

        Ok(server_info)
    }

    fn calculate_multichain_fee(&self, amount: f64, from_chain: u64, to_chain: u64) -> f64 {
        // Multichain fees vary by route and token
        let base_fee = match (from_chain, to_chain) {
            (1, _) | (_, 1) => amount * 0.001, // 0.1% for Ethereum routes
            (56, 137) | (137, 56) => amount * 0.0005, // 0.05% for BSC-Polygon
            _ => amount * 0.0008, // 0.08% default
        };
        
        // Minimum fee of $1 equivalent
        base_fee.max(1.0)
    }
}

#[async_trait]
impl BridgeIntegration for Multichain {
    fn name(&self) -> &str {
        "Multichain"
    }

    fn supports_route(&self, from_chain: u64, to_chain: u64) -> bool {
        self.supported_chains.contains(&from_chain) && 
        self.supported_chains.contains(&to_chain) &&
        from_chain != to_chain
    }

    fn get_supported_tokens(&self, chain_id: u64) -> Vec<String> {
        self.supported_tokens.get(&chain_id).cloned().unwrap_or_default()
    }

    async fn get_quote(&self, params: &CrossChainParams) -> Result<BridgeQuote, BridgeError> {
        // Validate route support
        if !self.supports_route(params.from_chain_id, params.to_chain_id) {
            return Err(BridgeError::UnsupportedRoute);
        }

        // Validate token support
        let supported_tokens = self.get_supported_tokens(params.from_chain_id);
        if !supported_tokens.iter().any(|t| t.eq_ignore_ascii_case(&params.token_in)) {
            return Err(BridgeError::UnsupportedRoute);
        }

        // Check if token is supported on destination chain
        let dest_tokens = self.get_supported_tokens(params.to_chain_id);
        if !dest_tokens.iter().any(|t| t.eq_ignore_ascii_case(&params.token_out)) {
            return Err(BridgeError::UnsupportedRoute);
        }

        // Parse amount
        let amount_in: f64 = params.amount_in.parse()
            .map_err(|_| BridgeError::InvalidParameters("Invalid amount_in".to_string()))?;

        // Calculate fee
        let fee = self.calculate_multichain_fee(amount_in, params.from_chain_id, params.to_chain_id);
        let amount_out = amount_in - fee;

        if amount_out <= 0.0 {
            return Err(BridgeError::InsufficientLiquidity);
        }

        // Calculate confidence score based on fee percentage and chain support
        let fee_pct = fee / amount_in;
        let confidence_score = match fee_pct {
            pct if pct < 0.001 => 0.90, // < 0.1% fee = high confidence
            pct if pct < 0.002 => 0.80, // < 0.2% fee = good confidence
            pct if pct < 0.005 => 0.65, // < 0.5% fee = medium confidence
            _ => 0.45, // > 0.5% fee = lower confidence (due to security concerns)
        };

        // Multichain uses validator network, can be slower but supports many chains
        let estimated_time = match (params.from_chain_id, params.to_chain_id) {
            (1, _) | (_, 1) => 1200, // 20 minutes for Ethereum routes
            (56, 137) | (137, 56) => 600, // 10 minutes for BSC-Polygon
            _ => 900, // 15 minutes default
        };

        Ok(BridgeQuote {
            bridge_name: self.name().to_string(),
            amount_out: amount_out.to_string(),
            estimated_time,
            fee: fee.to_string(),
            gas_estimate: "250000".to_string(), // Higher gas due to validator network
            route: vec![BridgeStep {
                bridge: self.name().to_string(),
                from_chain: params.from_chain_id,
                to_chain: params.to_chain_id,
                token_in: params.token_in.clone(),
                token_out: params.token_out.clone(),
                amount_in: params.amount_in.clone(),
                amount_out: amount_out.to_string(),
                estimated_time,
            }],
            confidence_score,
            liquidity_available: "200000000".to_string(), // $200M+ liquidity across 50+ chains
        })
    }

    async fn execute_bridge(&self, params: &CrossChainParams) -> Result<BridgeResponse, BridgeError> {
        // In a real implementation, this would:
        // 1. Get fresh quote
        // 2. Build transaction data for Multichain Router contract
        // 3. Submit transaction with proper validator network parameters
        // 4. Return transaction hash and tracking info
        
        tracing::info!("Executing Multichain bridge for {} {} from chain {} to chain {}", 
                   params.amount_in, params.token_in, params.from_chain_id, params.to_chain_id);

        // Mock implementation for now
        Ok(BridgeResponse {
            transaction_hash: format!("0x{:064x}", rand::random::<u64>()),
            bridge_id: format!("multichain_{}", chrono::Utc::now().timestamp()),
            status: BridgeStatus::Pending,
            estimated_completion: chrono::Utc::now().timestamp() as u64 + 1200, // 20 minutes
            tracking_url: Some("https://anyswap.exchange/#/explorer".to_string()),
        })
    }

    async fn get_status(&self, bridge_id: &str) -> Result<BridgeStatus, BridgeError> {
        // In a real implementation, this would query Multichain's API or contracts
        // for the actual bridge status
        tracing::info!("Checking Multichain bridge status for ID: {}", bridge_id);
        
        // Mock implementation
        Ok(BridgeStatus::InProgress)
    }

    async fn health_check(&self) -> Result<bool, BridgeError> {
        match self.fetch_server_info().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}
