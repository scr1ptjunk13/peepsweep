use super::{BridgeIntegration, BridgeQuote, BridgeResponse, BridgeError, BridgeStatus, CrossChainParams, BridgeStep};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AcrossProtocol {
    client: Client,
    base_url: String,
    supported_chains: Vec<u64>,
    supported_tokens: HashMap<u64, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AcrossQuoteResponse {
    #[serde(rename = "totalRelayFee")]
    total_relay_fee: AcrossFee,
    #[serde(rename = "capitalFee")]
    capital_fee: AcrossFee,
    #[serde(rename = "relayGasFee")]
    relay_gas_fee: AcrossFee,
    #[serde(rename = "lpFee")]
    lp_fee: AcrossFee,
    #[serde(rename = "timestamp")]
    timestamp: String,
    #[serde(rename = "isAmountTooLow")]
    is_amount_too_low: bool,
    #[serde(rename = "quoteBlock")]
    quote_block: String,
}

#[derive(Debug, Deserialize)]
struct AcrossFee {
    #[serde(rename = "pct")]
    pct: String,
    #[serde(rename = "total")]
    total: String,
}

#[derive(Debug, Serialize)]
struct AcrossQuoteRequest {
    token: String,
    #[serde(rename = "inputAmount")]
    input_amount: String,
    #[serde(rename = "originChainId")]
    origin_chain_id: u64,
    #[serde(rename = "destinationChainId")]
    destination_chain_id: u64,
    recipient: String,
    #[serde(rename = "skipAmountLimit")]
    skip_amount_limit: bool,
}

impl AcrossProtocol {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        let mut supported_tokens = HashMap::new();
        
        // Ethereum mainnet tokens
        supported_tokens.insert(1, vec![
            "ETH".to_string(),
            "WETH".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "UMA".to_string(),
            "ACX".to_string(),
        ]);
        
        // Arbitrum tokens
        supported_tokens.insert(42161, vec![
            "ETH".to_string(),
            "WETH".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "ARB".to_string(),
        ]);
        
        // Optimism tokens
        supported_tokens.insert(10, vec![
            "ETH".to_string(),
            "WETH".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "OP".to_string(),
        ]);
        
        // Polygon tokens
        supported_tokens.insert(137, vec![
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "WETH".to_string(),
            "WMATIC".to_string(),
        ]);

        // Base tokens
        supported_tokens.insert(8453, vec![
            "ETH".to_string(),
            "WETH".to_string(),
            "USDC".to_string(),
            "DAI".to_string(),
        ]);

        Self {
            client,
            base_url: "https://app.across.to".to_string(),
            supported_chains: vec![1, 10, 42161, 137, 8453, 324], // Ethereum, Optimism, Arbitrum, Polygon, Base, zkSync
            supported_tokens,
        }
    }

    fn get_across_token_address(&self, chain_id: u64, symbol: &str) -> Option<String> {
        match (chain_id, symbol.to_uppercase().as_str()) {
            // Ethereum mainnet
            (1, "ETH") => Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()), // WETH
            (1, "WETH") => Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
            (1, "USDC") => Some("0xA0b86a33E6441E6C5a6F6c7e2C0d3C8C8a2B0e8B".to_string()),
            (1, "USDT") => Some("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()),
            (1, "DAI") => Some("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()),
            (1, "WBTC") => Some("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string()),
            
            // Arbitrum
            (42161, "ETH") => Some("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string()), // WETH
            (42161, "WETH") => Some("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string()),
            (42161, "USDC") => Some("0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".to_string()),
            (42161, "USDT") => Some("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string()),
            
            // Optimism
            (10, "ETH") => Some("0x4200000000000000000000000000000000000006".to_string()), // WETH
            (10, "WETH") => Some("0x4200000000000000000000000000000000000006".to_string()),
            (10, "USDC") => Some("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string()),
            (10, "USDT") => Some("0x94b008aA00579c1307B0EF2c499aD98a8ce58e58".to_string()),
            
            // Polygon
            (137, "USDC") => Some("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()),
            (137, "USDT") => Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string()),
            (137, "WMATIC") => Some("0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270".to_string()),
            
            _ => None,
        }
    }

    async fn fetch_quote(&self, params: &CrossChainParams) -> Result<AcrossQuoteResponse, BridgeError> {
        let input_token = self.get_across_token_address(params.from_chain_id, &params.token_in)
            .ok_or_else(|| BridgeError::UnsupportedRoute)?;
        let output_token = self.get_across_token_address(params.to_chain_id, &params.token_out)
            .ok_or_else(|| BridgeError::UnsupportedRoute)?;

        // Use GET request with query parameters as per official API docs
        let url = format!(
            "{}/api/suggested-fees?inputToken={}&outputToken={}&originChainId={}&destinationChainId={}&amount={}&recipient={}",
            self.base_url,
            input_token,
            output_token,
            params.from_chain_id,
            params.to_chain_id,
            params.amount_in,
            params.user_address
        );

        tracing::info!("Fetching Across Protocol quote from: {}", url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| BridgeError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Across Protocol API error {}: {}", status, error_text);
            return Err(BridgeError::NetworkError(format!("HTTP {}: {}", status, error_text)));
        }

        let quote: AcrossQuoteResponse = response
            .json()
            .await
            .map_err(|e| BridgeError::NetworkError(format!("JSON parsing error: {}", e)))?;

        tracing::info!("Across Protocol quote received: total_fee={}", quote.total_relay_fee.total);

        if quote.is_amount_too_low {
            return Err(BridgeError::InvalidParameters("Amount too low for Across Protocol".to_string()));
        }

        Ok(quote)
    }
}

#[async_trait]
impl BridgeIntegration for AcrossProtocol {
    fn name(&self) -> &str {
        "Across Protocol"
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
        if !supported_tokens.contains(&params.token_in.to_uppercase()) {
            return Err(BridgeError::UnsupportedRoute);
        }

        let quote = self.fetch_quote(params).await?;

        // Calculate amount out (input amount minus total fees)
        let amount_in: f64 = params.amount_in.parse()
            .map_err(|_| BridgeError::InvalidParameters("Invalid amount_in".to_string()))?;
        let total_fee: f64 = quote.total_relay_fee.total.parse()
            .map_err(|_| BridgeError::NetworkError("Invalid fee format".to_string()))?;
        
        let amount_out = amount_in - total_fee;
        if amount_out <= 0.0 {
            return Err(BridgeError::InsufficientLiquidity);
        }

        // Calculate confidence score based on fee percentage
        let fee_pct: f64 = quote.total_relay_fee.pct.parse().unwrap_or(0.01);
        let confidence_score = match fee_pct {
            pct if pct < 0.005 => 0.95, // < 0.5% fee = high confidence
            pct if pct < 0.01 => 0.85,  // < 1% fee = good confidence
            pct if pct < 0.02 => 0.70,  // < 2% fee = medium confidence
            _ => 0.50, // > 2% fee = low confidence
        };

        // Across is known for fast transfers (1-4 minutes typically)
        let estimated_time = match (params.from_chain_id, params.to_chain_id) {
            (1, _) | (_, 1) => 240,  // 4 minutes for Ethereum routes
            _ => 120, // 2 minutes for L2-L2
        };

        Ok(BridgeQuote {
            bridge_name: self.name().to_string(),
            amount_out: amount_out.to_string(),
            estimated_time,
            fee: quote.total_relay_fee.total,
            gas_estimate: "120000".to_string(), // Typical Across gas usage
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
            liquidity_available: "75000000".to_string(), // $75M typical liquidity
        })
    }

    async fn execute_bridge(&self, params: &CrossChainParams) -> Result<BridgeResponse, BridgeError> {
        // In a real implementation, this would:
        // 1. Get fresh quote
        // 2. Build transaction data for Across SpokePool contract
        // 3. Submit transaction with proper relay parameters
        // 4. Return transaction hash and tracking info
        
        tracing::info!("Executing Across Protocol bridge for {} {} from chain {} to chain {}", 
                   params.amount_in, params.token_in, params.from_chain_id, params.to_chain_id);

        // Mock implementation for now
        Ok(BridgeResponse {
            transaction_hash: format!("0x{:064x}", rand::random::<u64>()),
            bridge_id: format!("across_{}", chrono::Utc::now().timestamp()),
            status: BridgeStatus::Pending,
            estimated_completion: chrono::Utc::now().timestamp() as u64 + 240, // 4 minutes
            tracking_url: Some("https://app.across.to/transactions".to_string()),
        })
    }

    async fn get_status(&self, bridge_id: &str) -> Result<BridgeStatus, BridgeError> {
        // In a real implementation, this would query Across's API or contracts
        // for the actual bridge status
        tracing::info!("Checking Across Protocol bridge status for ID: {}", bridge_id);
        
        // Mock implementation
        Ok(BridgeStatus::InProgress)
    }

    async fn health_check(&self) -> Result<bool, BridgeError> {
        let health_url = format!("{}/api/health", self.base_url);
        
        match self.client.get(&health_url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}
