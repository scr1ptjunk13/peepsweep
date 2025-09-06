use std::collections::HashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};
use tokio::time::{sleep, Duration};

use super::{DiscoveredToken, TokenRegistryConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValidationResult {
    pub address: String,
    pub is_valid: bool,
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub is_contract: bool,
    pub verification_source: String,
    pub error: Option<String>,
}

pub struct TokenValidationService {
    config: TokenRegistryConfig,
    http_client: Client,
    rate_limiter: HashMap<u64, u64>, // chain_id -> last_request_time
}

impl TokenValidationService {
    pub fn new(config: TokenRegistryConfig) -> Self {
        Self {
            config,
            http_client: Client::new(),
            rate_limiter: HashMap::new(),
        }
    }

    /// Validate a list of tokens for a specific chain
    pub async fn validate_tokens(&mut self, tokens: Vec<DiscoveredToken>, chain_id: u64) -> Vec<DiscoveredToken> {
        if !self.config.enable_verification {
            return tokens;
        }

        info!("Validating {} tokens for chain {}", tokens.len(), chain_id);
        let mut validated_tokens = Vec::new();
        let mut validated_count = 0;

        for mut token in tokens {
            // Rate limiting
            self.apply_rate_limit(chain_id).await;

            match self.validate_token_contract(&token, chain_id).await {
                Ok(validation_result) => {
                    if validation_result.is_valid {
                        let mut validated_token = token;
                        
                        // Update token info from validation if available
                        if let Some(decimals) = validation_result.decimals {
                            validated_token.decimals = decimals;
                        }
                        if let Some(symbol) = validation_result.symbol {
                            validated_token.symbol = symbol;
                        }
                        if let Some(name) = validation_result.name {
                            validated_token.name = name;
                        }
                        
                        validated_token.verified = true;
                        validated_tokens.push(validated_token);
                        validated_count += 1;
                    } else {
                        warn!("Token validation failed for {}: {}", 
                              token.address, validation_result.error.unwrap_or_default());
                    }
                }
                Err(e) => {
                    error!("Error validating token {}: {}", token.address, e);
                    // Include unvalidated token but mark as unverified
                    let mut unvalidated_token = token;
                    unvalidated_token.verified = false;
                    validated_tokens.push(unvalidated_token);
                }
            }
        }

        info!("Validated {}/{} tokens for chain {}", validated_count, validated_tokens.len(), chain_id);
        validated_tokens
    }

    /// Validate a single token contract
    async fn validate_token_contract(&self, token: &DiscoveredToken, chain_id: u64) -> Result<TokenValidationResult, Box<dyn std::error::Error>> {
        // Try different validation methods in order of preference
        
        // 1. Try chain explorer API first
        if let Ok(result) = self.validate_via_explorer(token, chain_id).await {
            return Ok(result);
        }

        // 2. Try RPC call as fallback
        if let Ok(result) = self.validate_via_rpc(token, chain_id).await {
            return Ok(result);
        }

        // 3. Basic validation as last resort
        Ok(self.basic_validation(token))
    }

    /// Validate token via chain explorer API
    async fn validate_via_explorer(&self, token: &DiscoveredToken, chain_id: u64) -> Result<TokenValidationResult, Box<dyn std::error::Error>> {
        let explorer_url = self.get_explorer_api_url(chain_id)?;
        let api_key = self.get_explorer_api_key(chain_id);
        
        let mut url = format!("{}?module=contract&action=getsourcecode&address={}", 
                             explorer_url, token.address);
        
        if let Some(key) = api_key {
            url.push_str(&format!("&apikey={}", key));
        }

        let response: Value = self.http_client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        if let Some(result_array) = response.get("result").and_then(|r| r.as_array()) {
            if let Some(contract_info) = result_array.first() {
                let is_contract = contract_info.get("SourceCode")
                    .and_then(|s| s.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);

                if is_contract {
                    // Try to get token info via additional API call
                    let token_info = self.get_token_info_from_explorer(token, chain_id).await?;
                    
                    return Ok(TokenValidationResult {
                        address: token.address.clone(),
                        is_valid: true,
                        decimals: token_info.decimals,
                        symbol: token_info.symbol,
                        name: token_info.name,
                        is_contract: true,
                        verification_source: "explorer".to_string(),
                        error: None,
                    });
                }
            }
        }

        Err("Contract not found or not verified".into())
    }

    /// Get token info from explorer
    async fn get_token_info_from_explorer(&self, token: &DiscoveredToken, chain_id: u64) -> Result<TokenInfo, Box<dyn std::error::Error>> {
        let explorer_url = self.get_explorer_api_url(chain_id)?;
        let api_key = self.get_explorer_api_key(chain_id);
        
        // Get token decimals
        let mut decimals_url = format!("{}?module=proxy&action=eth_call&to={}&data=0x313ce567", 
                                      explorer_url, token.address);
        if let Some(key) = &api_key {
            decimals_url.push_str(&format!("&apikey={}", key));
        }

        let decimals = self.parse_decimals_from_response(&decimals_url).await.unwrap_or(token.decimals);

        // Get token symbol
        let mut symbol_url = format!("{}?module=proxy&action=eth_call&to={}&data=0x95d89b41", 
                                    explorer_url, token.address);
        if let Some(key) = &api_key {
            symbol_url.push_str(&format!("&apikey={}", key));
        }

        let symbol = self.parse_string_from_response(&symbol_url).await.unwrap_or_else(|| token.symbol.clone());

        // Get token name
        let mut name_url = format!("{}?module=proxy&action=eth_call&to={}&data=0x06fdde03", 
                                  explorer_url, token.address);
        if let Some(key) = api_key {
            name_url.push_str(&format!("&apikey={}", key));
        }

        let name = self.parse_string_from_response(&name_url).await.unwrap_or_else(|| token.name.clone());

        Ok(TokenInfo {
            decimals: Some(decimals),
            symbol: Some(symbol),
            name: Some(name),
        })
    }

    /// Validate token via RPC call
    async fn validate_via_rpc(&self, token: &DiscoveredToken, chain_id: u64) -> Result<TokenValidationResult, Box<dyn std::error::Error>> {
        let rpc_url = self.get_rpc_url(chain_id)?;
        
        // Check if address is a contract by getting code
        let code_request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getCode",
            "params": [token.address, "latest"],
            "id": 1
        });

        let response: Value = self.http_client
            .post(&rpc_url)
            .json(&code_request)
            .send()
            .await?
            .json()
            .await?;

        let code = response.get("result")
            .and_then(|r| r.as_str())
            .unwrap_or("0x");

        let is_contract = code != "0x" && code != "0x0";

        if is_contract {
            Ok(TokenValidationResult {
                address: token.address.clone(),
                is_valid: true,
                decimals: Some(token.decimals),
                symbol: Some(token.symbol.clone()),
                name: Some(token.name.clone()),
                is_contract: true,
                verification_source: "rpc".to_string(),
                error: None,
            })
        } else {
            Ok(TokenValidationResult {
                address: token.address.clone(),
                is_valid: false,
                decimals: None,
                symbol: None,
                name: None,
                is_contract: false,
                verification_source: "rpc".to_string(),
                error: Some("Address is not a contract".to_string()),
            })
        }
    }

    /// Basic validation without external calls
    fn basic_validation(&self, token: &DiscoveredToken) -> TokenValidationResult {
        let is_valid = !token.address.is_empty() 
            && token.address.len() == 42 
            && token.address.starts_with("0x")
            && !token.symbol.is_empty();

        TokenValidationResult {
            address: token.address.clone(),
            is_valid,
            decimals: Some(token.decimals),
            symbol: Some(token.symbol.clone()),
            name: Some(token.name.clone()),
            is_contract: true, // Assume it's a contract
            verification_source: "basic".to_string(),
            error: if is_valid { None } else { Some("Basic validation failed".to_string()) },
        }
    }

    /// Get explorer API URL for chain
    fn get_explorer_api_url(&self, chain_id: u64) -> Result<String, Box<dyn std::error::Error>> {
        let url = match chain_id {
            1 => "https://api.etherscan.io/api",
            137 => "https://api.polygonscan.com/api",
            43114 => "https://api.snowtrace.io/api",
            42161 => "https://api.arbiscan.io/api",
            10 => "https://api-optimistic.etherscan.io/api",
            8453 => "https://api.basescan.org/api",
            56 => "https://api.bscscan.com/api",
            250 => "https://api.ftmscan.com/api",
            100 => "https://api.gnosisscan.io/api",
            _ => return Err(format!("No explorer API for chain {}", chain_id).into()),
        };
        Ok(url.to_string())
    }

    /// Get explorer API key (would be from environment variables in production)
    fn get_explorer_api_key(&self, chain_id: u64) -> Option<String> {
        // In production, these would come from environment variables
        // For now, return None to use rate-limited free tier
        None
    }

    /// Get RPC URL for chain
    fn get_rpc_url(&self, chain_id: u64) -> Result<String, Box<dyn std::error::Error>> {
        let url = match chain_id {
            1 => "https://eth.llamarpc.com",
            137 => "https://polygon.llamarpc.com",
            43114 => "https://avalanche.public-rpc.com",
            42161 => "https://arbitrum.llamarpc.com",
            10 => "https://optimism.llamarpc.com",
            8453 => "https://base.llamarpc.com",
            56 => "https://binance.llamarpc.com",
            250 => "https://fantom.llamarpc.com",
            100 => "https://gnosis.llamarpc.com",
            _ => return Err(format!("No RPC URL for chain {}", chain_id).into()),
        };
        Ok(url.to_string())
    }

    /// Apply rate limiting for chain requests
    async fn apply_rate_limit(&mut self, chain_id: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if let Some(&last_request) = self.rate_limiter.get(&chain_id) {
            let time_since_last = now - last_request;
            if time_since_last < 1 { // 1 second between requests per chain
                sleep(Duration::from_secs(1 - time_since_last)).await;
            }
        }

        self.rate_limiter.insert(chain_id, now);
    }

    /// Parse decimals from hex response
    async fn parse_decimals_from_response(&self, url: &str) -> Option<u8> {
        if let Ok(response) = self.http_client.get(url).send().await {
            if let Ok(json) = response.json::<Value>().await {
                if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                    if let Ok(hex_value) = u64::from_str_radix(&result[2..], 16) {
                        return Some(hex_value as u8);
                    }
                }
            }
        }
        None
    }

    /// Parse string from hex response
    async fn parse_string_from_response(&self, url: &str) -> Option<String> {
        if let Ok(response) = self.http_client.get(url).send().await {
            if let Ok(json) = response.json::<Value>().await {
                if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                    // Decode hex string (simplified)
                    if result.len() > 2 {
                        let hex_data = &result[2..];
                        if let Ok(bytes) = hex::decode(hex_data) {
                            // Skip first 32 bytes (offset) and next 32 bytes (length)
                            if bytes.len() > 64 {
                                let string_bytes = &bytes[64..];
                                if let Ok(decoded) = String::from_utf8(string_bytes.to_vec()) {
                                    return Some(decoded.trim_end_matches('\0').to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
struct TokenInfo {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    pub name: Option<String>,
}
