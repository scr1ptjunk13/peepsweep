use super::{BridgeIntegration, BridgeQuote, BridgeResponse, BridgeError, BridgeStatus, CrossChainParams, BridgeStep};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HopProtocol {
    client: Client,
    base_url: String,
    supported_chains: Vec<u64>,
    supported_tokens: HashMap<u64, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct HopQuoteResponse {
    #[serde(rename = "amountOut")]
    amount_out: String,
    #[serde(rename = "totalFee")]
    total_fee: String,
    #[serde(rename = "estimatedTime")]
    estimated_time: Option<u64>,
    #[serde(rename = "priceImpact")]
    price_impact: Option<f64>,
    #[serde(rename = "lpFees")]
    lp_fees: Option<String>,
    #[serde(rename = "adjustedAmountOut")]
    adjusted_amount_out: Option<String>,
}

#[derive(Debug, Serialize)]
struct HopQuoteRequest {
    amount: String,
    token: String,
    #[serde(rename = "fromChain")]
    from_chain: u64,
    #[serde(rename = "toChain")]
    to_chain: u64,
    slippage: f64,
}

impl HopProtocol {
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
            "WETH".to_string(),
            "HOP".to_string(),
        ]);
        
        // Arbitrum tokens
        supported_tokens.insert(42161, vec![
            "ETH".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "WETH".to_string(),
        ]);
        
        // Optimism tokens
        supported_tokens.insert(10, vec![
            "ETH".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "WETH".to_string(),
        ]);
        
        // Polygon tokens
        supported_tokens.insert(137, vec![
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
            "WETH".to_string(),
            "MATIC".to_string(),
        ]);

        Self {
            client,
            base_url: "https://api.hop.exchange".to_string(),
            supported_chains: vec![1, 10, 42161, 137, 100, 42220], // Ethereum, Optimism, Arbitrum, Polygon, Gnosis, Celo
            supported_tokens,
        }
    }

    fn get_hop_token_symbol(&self, token: &str) -> String {
        match token.to_uppercase().as_str() {
            "ETH" | "WETH" => "ETH".to_string(),
            "USDC" => "USDC".to_string(),
            "USDT" => "USDT".to_string(),
            "DAI" => "DAI".to_string(),
            "WBTC" => "WBTC".to_string(),
            "MATIC" | "WMATIC" => "MATIC".to_string(),
            _ => token.to_uppercase(),
        }
    }

    fn get_chain_slug(&self, chain_id: u64) -> Option<&str> {
        match chain_id {
            1 => Some("ethereum"),
            10 => Some("optimism"),
            42161 => Some("arbitrum"),
            137 => Some("polygon"),
            100 => Some("gnosis"),
            42220 => Some("celo"),
            _ => None,
        }
    }

    async fn fetch_quote(&self, params: &CrossChainParams) -> Result<HopQuoteResponse, BridgeError> {
        let token_symbol = self.get_hop_token_symbol(&params.token_in);
        
        let from_chain_slug = self.get_chain_slug(params.from_chain_id)
            .ok_or_else(|| BridgeError::UnsupportedRoute)?;
        let to_chain_slug = self.get_chain_slug(params.to_chain_id)
            .ok_or_else(|| BridgeError::UnsupportedRoute)?;

        let url = format!(
            "{}/v1/quote?amount={}&token={}&fromChain={}&toChain={}&slippage={}",
            self.base_url,
            params.amount_in,
            token_symbol,
            from_chain_slug,
            to_chain_slug,
            params.slippage
        );

        tracing::info!("Fetching Hop Protocol quote from: {}", url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| BridgeError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            tracing::error!("Hop Protocol API error {}: {}", status, error_text);
            return Err(BridgeError::NetworkError(format!("HTTP {}: {}", status, error_text)));
        }

        let quote: HopQuoteResponse = response
            .json()
            .await
            .map_err(|e| BridgeError::NetworkError(format!("JSON parsing error: {}", e)))?;

        tracing::info!("Hop Protocol quote received: amount_out={}, fee={}", 
                   quote.amount_out, quote.total_fee);

        Ok(quote)
    }
}

#[async_trait]
impl BridgeIntegration for HopProtocol {
    fn name(&self) -> &str {
        "Hop Protocol"
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
        let token_symbol = self.get_hop_token_symbol(&params.token_in);
        if !supported_tokens.contains(&token_symbol) {
            return Err(BridgeError::UnsupportedRoute);
        }

        let quote = self.fetch_quote(params).await?;

        // Calculate confidence score based on liquidity and price impact
        let confidence_score = match quote.price_impact {
            Some(impact) if impact < 0.01 => 0.95, // < 1% impact = high confidence
            Some(impact) if impact < 0.03 => 0.85, // < 3% impact = good confidence
            Some(impact) if impact < 0.05 => 0.70, // < 5% impact = medium confidence
            Some(_) => 0.50, // > 5% impact = low confidence
            None => 0.80, // No impact data = default confidence
        };

        // Estimate time based on chain combination
        let estimated_time = quote.estimated_time.unwrap_or_else(|| {
            match (params.from_chain_id, params.to_chain_id) {
                (1, _) | (_, 1) => 900,  // 15 minutes for Ethereum routes
                (10, 42161) | (42161, 10) => 300, // 5 minutes for L2-L2
                _ => 600, // 10 minutes default
            }
        });

        // Use adjusted amount out if available, otherwise use regular amount out
        let amount_out = quote.adjusted_amount_out.unwrap_or(quote.amount_out.clone());

        Ok(BridgeQuote {
            bridge_name: self.name().to_string(),
            amount_out: amount_out.clone(),
            estimated_time,
            fee: quote.total_fee,
            gas_estimate: "150000".to_string(), // Typical Hop gas usage
            route: vec![BridgeStep {
                bridge: self.name().to_string(),
                from_chain: params.from_chain_id,
                to_chain: params.to_chain_id,
                token_in: params.token_in.clone(),
                token_out: params.token_out.clone(),
                amount_in: params.amount_in.clone(),
                amount_out,
                estimated_time,
            }],
            confidence_score,
            liquidity_available: "50000000".to_string(), // $50M typical liquidity
        })
    }

    async fn execute_bridge(&self, params: &CrossChainParams) -> Result<BridgeResponse, BridgeError> {
        // In a real implementation, this would:
        // 1. Get fresh quote
        // 2. Build transaction data
        // 3. Submit transaction to Hop Protocol contracts
        // 4. Return transaction hash and tracking info
        
        tracing::info!("Executing Hop Protocol bridge for {} {} from chain {} to chain {}", 
                   params.amount_in, params.token_in, params.from_chain_id, params.to_chain_id);

        // Mock implementation for now
        Ok(BridgeResponse {
            transaction_hash: format!("0x{:064x}", rand::random::<u64>()),
            bridge_id: format!("hop_{}", chrono::Utc::now().timestamp()),
            status: BridgeStatus::Pending,
            estimated_completion: chrono::Utc::now().timestamp() as u64 + 900, // 15 minutes
            tracking_url: Some(format!("https://app.hop.exchange/#/send?token={}", params.token_in)),
        })
    }

    async fn get_status(&self, bridge_id: &str) -> Result<BridgeStatus, BridgeError> {
        // In a real implementation, this would query Hop's API or contracts
        // for the actual bridge status
        tracing::info!("Checking Hop Protocol bridge status for ID: {}", bridge_id);
        
        // Mock implementation
        Ok(BridgeStatus::InProgress)
    }

    async fn health_check(&self) -> Result<bool, BridgeError> {
        let health_url = format!("{}/v1/health", self.base_url);
        
        match self.client.get(&health_url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}
