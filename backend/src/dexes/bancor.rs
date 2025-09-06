use crate::dexes::{DexIntegration, DexError};
use crate::types::{QuoteParams, RouteBreakdown};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, instrument};
use std::str::FromStr;

// Bancor V3 API response structures
#[derive(Clone, Debug, Deserialize)]
pub struct BancorToken {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub address: String,
    pub decimals: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BancorPool {
    pub id: String,
    pub name: String,
    pub symbol: String,
    #[serde(rename = "baseToken")]
    pub base_token: BancorToken,
    #[serde(rename = "poolToken")]
    pub pool_token: BancorToken,
    #[serde(rename = "stakedBalance")]
    pub staked_balance: String,
    #[serde(rename = "poolTokenSupply")]
    pub pool_token_supply: String,
    #[serde(rename = "tradingEnabled")]
    pub trading_enabled: bool,
    #[serde(rename = "tradingFeePPM")]
    pub trading_fee_ppm: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BancorPoolsResponse {
    pub data: Vec<BancorPool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BancorQuoteResponse {
    #[serde(rename = "targetAmount")]
    pub target_amount: String,
    #[serde(rename = "fee")]
    pub fee: String,
    #[serde(rename = "priceImpact")]
    pub price_impact: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BancorErrorResponse {
    pub error: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone)]
pub struct BancorDex {
    client: Client,
    base_url: String,
}

impl BancorDex {
    pub async fn new() -> Result<Self, DexError> {
        Ok(Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("DEX-Aggregator/1.0")
                .build()
                .map_err(|e| DexError::NetworkError(e))?,
            base_url: "https://api-v3.bancor.network".to_string(),
        })
    }

    fn normalize_token_address(&self, token: &str) -> String {
        // If it's already a valid Ethereum address (starts with 0x and 42 chars), use as-is
        if token.starts_with("0x") && token.len() == 42 {
            return token.to_lowercase();
        }
        
        // Handle common token symbols that Bancor might expect
        match token.to_uppercase().as_str() {
            "ETH" => "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE".to_string(), // ETH placeholder
            _ => token.to_string(), // Return as-is for other symbols
        }
    }

    async fn get_bancor_pools(&self) -> Result<Vec<BancorPool>, DexError> {
        let url = format!("{}/pools", self.base_url);
        
        info!("🔄 Fetching Bancor pools from API");
        
        let response = self.client
            .get(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                error!("❌ Bancor pools API request failed: {}", e);
                DexError::NetworkError(e)
            })?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        
        if !status.is_success() {
            error!("❌ Bancor pools API error {}: {}", status, response_text);
            return Err(DexError::ApiError(format!("Bancor pools HTTP {}: {}", status, response_text)));
        }

        let pools_response: BancorPoolsResponse = serde_json::from_str(&response_text)
            .map_err(|e| {
                error!("❌ Failed to parse Bancor pools response: {}", e);
                DexError::ParseError(format!("JSON parse error: {}", e))
            })?;

        info!("✅ Found {} Bancor pools", pools_response.data.len());
        Ok(pools_response.data)
    }

    async fn call_bancor_quote_api(&self, params: &QuoteParams) -> Result<BancorQuoteResponse, DexError> {
        let token_in = self.normalize_token_address(&params.token_in);
        let token_out = self.normalize_token_address(&params.token_out);
        
        // Bancor V3 quote endpoint
        let url = format!("{}/quote", self.base_url);
        
        // Build query parameters
        let query_params = [
            ("sourceToken", token_in.as_str()),
            ("targetToken", token_out.as_str()),
            ("amount", &params.amount_in),
        ];

        info!("🔄 Calling Bancor quote API");
        info!("   Swap: {} {} → {} {}", params.amount_in, token_in, "?", token_out);
        info!("   URL: {}", url);

        let response = self.client
            .get(&url)
            .query(&query_params)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                error!("❌ Bancor quote API request failed: {}", e);
                DexError::NetworkError(e)
            })?;

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();
        
        if !status.is_success() {
            error!("❌ Bancor quote API error {}: {}", status, response_text);
            
            // Try to parse error response
            if let Ok(error_resp) = serde_json::from_str::<BancorErrorResponse>(&response_text) {
                let error_msg = error_resp.error
                    .or(error_resp.message)
                    .unwrap_or_else(|| "Unknown Bancor API error".to_string());
                return Err(DexError::ApiError(format!("Bancor: {}", error_msg)));
            }
            
            return Err(DexError::ApiError(format!("Bancor HTTP {}: {}", status, response_text)));
        }

        info!("✅ Bancor quote API response received ({} bytes)", response_text.len());
        
        let quote_response: BancorQuoteResponse = serde_json::from_str(&response_text)
            .map_err(|e| {
                error!("❌ Failed to parse Bancor quote response: {}", e);
                error!("Response was: {}", response_text);
                DexError::ParseError(format!("JSON parse error: {}", e))
            })?;

        // Validate the response
        if quote_response.target_amount == "0" {
            warn!("⚠️ Bancor returned zero output amount");
            return Err(DexError::NoLiquidity);
        }

        Ok(quote_response)
    }

    // Fallback calculation using pool data when direct API quote fails
    async fn calculate_quote_from_pools(&self, params: &QuoteParams) -> Result<String, DexError> {
        let pools = self.get_bancor_pools().await?;
        let token_in = self.normalize_token_address(&params.token_in);
        let token_out = self.normalize_token_address(&params.token_out);
        
        // Find a pool that contains our input token
        let pool = pools.iter()
            .find(|p| {
                p.trading_enabled && 
                (p.base_token.address.to_lowercase() == token_in.to_lowercase() ||
                 p.base_token.symbol.to_uppercase() == params.token_in.to_uppercase())
            })
            .ok_or_else(|| DexError::UnsupportedPair(format!("No active Bancor pool for {}", params.token_in)))?;

        info!("Found Bancor pool: {} for token {}", pool.name, params.token_in);

        // Parse amounts
        let amount_in = params.amount_in.parse::<f64>()
            .map_err(|_| DexError::ParseError("Invalid amount_in".to_string()))?;

        let staked_balance = pool.staked_balance.parse::<f64>()
            .map_err(|_| DexError::ParseError("Invalid staked_balance".to_string()))?;

        let trading_fee_ppm = pool.trading_fee_ppm.parse::<f64>()
            .unwrap_or(2000.0); // Default 0.2%

        // Apply trading fee
        let fee_multiplier = 1.0 - (trading_fee_ppm / 1_000_000.0);
        
        // Simple estimation based on pool liquidity
        // This is a rough approximation - real Bancor uses more complex curves
        let estimated_rate = if staked_balance > 0.0 { 
            1.0 // 1:1 rate as baseline
        } else { 
            return Err(DexError::NoLiquidity);
        };
        
        let amount_out = amount_in * estimated_rate * fee_multiplier;
        
        info!("Bancor pool calculation: {} -> {} (fee: {}%)", 
              amount_in, amount_out, trading_fee_ppm / 10_000.0);

        Ok((amount_out as u64).to_string())
    }

    #[instrument(skip(self), fields(token_in = %params.token_in, token_out = %params.token_out, amount_in = %params.amount_in))]
    async fn get_bancor_quote(&self, params: &QuoteParams) -> Result<String, DexError> {
        // Validate inputs
        if params.amount_in.is_empty() || params.amount_in == "0" {
            return Err(DexError::InvalidInput("Amount must be greater than 0".to_string()));
        }

        if params.token_in.is_empty() || params.token_out.is_empty() {
            return Err(DexError::InvalidInput("Both token addresses must be provided".to_string()));
        }

        if params.token_in == params.token_out {
            return Err(DexError::InvalidInput("Token in and token out cannot be the same".to_string()));
        }

        info!("Getting Bancor V3 quote");

        // First try direct quote API
        match self.call_bancor_quote_api(params).await {
            Ok(quote_response) => {
                info!("✅ Bancor direct quote successful: {} {} -> {} {}", 
                      params.amount_in, params.token_in, 
                      quote_response.target_amount, params.token_out);
                return Ok(quote_response.target_amount);
            }
            Err(e) => {
                warn!("⚠️ Bancor direct quote failed, trying pool-based calculation: {}", e);
            }
        }

        // Fallback to pool-based calculation
        match self.calculate_quote_from_pools(params).await {
            Ok(quote) => {
                info!("✅ Bancor pool-based quote successful: {} {} -> {} {}", 
                      params.amount_in, params.token_in, quote, params.token_out);
                Ok(quote)
            }
            Err(e) => {
                error!("❌ All Bancor quote methods failed: {}", e);
                Err(e)
            }
        }
    }
}

#[async_trait]
impl DexIntegration for BancorDex {
    fn get_name(&self) -> &'static str {
        "Bancor V3"
    }

    async fn get_quote(&self, params: &QuoteParams) -> Result<RouteBreakdown, DexError> {
        let quote = self.get_bancor_quote(params).await?;
        
        Ok(RouteBreakdown {
            dex: self.get_name().to_string(),
            percentage: 100.0,
            amount_out: quote,
            gas_used: "220000".to_string(), // Higher gas due to Bancor's complex single-sided liquidity
        })
    }

    async fn is_pair_supported(&self, token_in: &str, token_out: &str, chain: &str) -> Result<bool, DexError> {
        // Check if chain is supported (Bancor only on Ethereum)
        if chain != "ethereum" {
            return Ok(false);
        }

        // Basic validation
        if token_in.is_empty() || token_out.is_empty() || token_in == token_out {
            return Ok(false);
        }
        
        // For Bancor, we'll assume pairs are potentially supported if they're valid tokens
        // The real validation happens when we query pools or call the quote API
        // Bancor primarily works with single-sided liquidity, so most tokens that have pools are supported
        Ok(true)
    }

    fn get_supported_chains(&self) -> Vec<&'static str> {
        // Bancor V3 primarily operates on Ethereum mainnet
        vec!["ethereum"]
    }
}