use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedToken {
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub chain_addresses: HashMap<u64, String>,
    pub coingecko_id: Option<String>,
    pub token_type: TokenType,
    pub is_native: bool,
    pub logo_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenType {
    Native,
    ERC20,
    Wrapped,
    Stable,
    Synthetic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub token: UnifiedToken,
    pub balance: String,
    pub balance_formatted: String,
    pub value_usd: f64,
    pub chain_id: u64,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    pub symbol: String,
    pub price_usd: f64,
    pub change_24h: f64,
    pub market_cap: Option<f64>,
    pub volume_24h: Option<f64>,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainTokenMapping {
    pub base_symbol: String,
    pub mappings: HashMap<u64, TokenMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMapping {
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
    pub is_canonical: bool,
    pub bridge_support: Vec<String>,
}

pub struct UnifiedTokenInterface {
    tokens: Arc<RwLock<HashMap<String, UnifiedToken>>>,
    price_cache: Arc<RwLock<HashMap<String, TokenPrice>>>,
    cross_chain_mappings: Arc<RwLock<HashMap<String, CrossChainTokenMapping>>>,
    http_client: Client,
}

impl UnifiedTokenInterface {
    pub fn new() -> Self {
        let mut tokens = HashMap::new();
        let mut cross_chain_mappings = HashMap::new();

        // Initialize common tokens with cross-chain mappings
        Self::initialize_common_tokens(&mut tokens, &mut cross_chain_mappings);

        Self {
            tokens: Arc::new(RwLock::new(tokens)),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            cross_chain_mappings: Arc::new(RwLock::new(cross_chain_mappings)),
            http_client: Client::new(),
        }
    }

    fn initialize_common_tokens(
        tokens: &mut HashMap<String, UnifiedToken>,
        mappings: &mut HashMap<String, CrossChainTokenMapping>,
    ) {
        // ETH/WETH
        let mut eth_addresses = HashMap::new();
        eth_addresses.insert(1, "0x0000000000000000000000000000000000000000".to_string()); // Native ETH
        eth_addresses.insert(10, "0x0000000000000000000000000000000000000000".to_string()); // Native ETH on Optimism
        eth_addresses.insert(42161, "0x0000000000000000000000000000000000000000".to_string()); // Native ETH on Arbitrum
        eth_addresses.insert(8453, "0x0000000000000000000000000000000000000000".to_string()); // Native ETH on Base

        tokens.insert("ETH".to_string(), UnifiedToken {
            symbol: "ETH".to_string(),
            name: "Ethereum".to_string(),
            decimals: 18,
            chain_addresses: eth_addresses.clone(),
            coingecko_id: Some("ethereum".to_string()),
            token_type: TokenType::Native,
            is_native: true,
            logo_uri: Some("https://assets.coingecko.com/coins/images/279/small/ethereum.png".to_string()),
        });

        // WETH
        let mut weth_addresses = HashMap::new();
        weth_addresses.insert(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string());
        weth_addresses.insert(10, "0x4200000000000000000000000000000000000006".to_string());
        weth_addresses.insert(42161, "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string());
        weth_addresses.insert(8453, "0x4200000000000000000000000000000000000006".to_string());

        tokens.insert("WETH".to_string(), UnifiedToken {
            symbol: "WETH".to_string(),
            name: "Wrapped Ethereum".to_string(),
            decimals: 18,
            chain_addresses: weth_addresses.clone(),
            coingecko_id: Some("weth".to_string()),
            token_type: TokenType::Wrapped,
            is_native: false,
            logo_uri: Some("https://assets.coingecko.com/coins/images/2518/small/weth.png".to_string()),
        });

        // USDC
        let mut usdc_addresses = HashMap::new();
        usdc_addresses.insert(1, "0xA0b86a33E6441E6C7D3B4b5F9B4B4B4B4B4B4B4B".to_string());
        usdc_addresses.insert(137, "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string());
        usdc_addresses.insert(10, "0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string());
        usdc_addresses.insert(42161, "0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".to_string());
        usdc_addresses.insert(8453, "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string());

        tokens.insert("USDC".to_string(), UnifiedToken {
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            chain_addresses: usdc_addresses.clone(),
            coingecko_id: Some("usd-coin".to_string()),
            token_type: TokenType::Stable,
            is_native: false,
            logo_uri: Some("https://assets.coingecko.com/coins/images/6319/small/USD_Coin_icon.png".to_string()),
        });

        // USDT
        let mut usdt_addresses = HashMap::new();
        usdt_addresses.insert(1, "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string());
        usdt_addresses.insert(137, "0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string());
        usdt_addresses.insert(10, "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58".to_string());
        usdt_addresses.insert(42161, "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string());

        tokens.insert("USDT".to_string(), UnifiedToken {
            symbol: "USDT".to_string(),
            name: "Tether USD".to_string(),
            decimals: 6,
            chain_addresses: usdt_addresses.clone(),
            coingecko_id: Some("tether".to_string()),
            token_type: TokenType::Stable,
            is_native: false,
            logo_uri: Some("https://assets.coingecko.com/coins/images/325/small/Tether-logo.png".to_string()),
        });

        // DAI
        let mut dai_addresses = HashMap::new();
        dai_addresses.insert(1, "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string());
        dai_addresses.insert(137, "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063".to_string());
        dai_addresses.insert(10, "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".to_string());
        dai_addresses.insert(42161, "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".to_string());

        tokens.insert("DAI".to_string(), UnifiedToken {
            symbol: "DAI".to_string(),
            name: "Dai Stablecoin".to_string(),
            decimals: 18,
            chain_addresses: dai_addresses.clone(),
            coingecko_id: Some("dai".to_string()),
            token_type: TokenType::Stable,
            is_native: false,
            logo_uri: Some("https://assets.coingecko.com/coins/images/9956/small/4943.png".to_string()),
        });

        // WBTC
        let mut wbtc_addresses = HashMap::new();
        wbtc_addresses.insert(1, "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string());
        wbtc_addresses.insert(137, "0x1BFD67037B42Cf73acF2047067bd4F2C47D9BfD6".to_string());
        wbtc_addresses.insert(10, "0x68f180fcCe6836688e9084f035309E29Bf0A2095".to_string());
        wbtc_addresses.insert(42161, "0x2f2a2543B76A4166549F7aaB2e75Bef0aefC5B0f".to_string());

        tokens.insert("WBTC".to_string(), UnifiedToken {
            symbol: "WBTC".to_string(),
            name: "Wrapped Bitcoin".to_string(),
            decimals: 8,
            chain_addresses: wbtc_addresses.clone(),
            coingecko_id: Some("wrapped-bitcoin".to_string()),
            token_type: TokenType::Wrapped,
            is_native: false,
            logo_uri: Some("https://assets.coingecko.com/coins/images/7598/small/wrapped_bitcoin_wbtc.png".to_string()),
        });

        // Initialize cross-chain mappings
        Self::create_cross_chain_mapping(mappings, "ETH", &eth_addresses);
        Self::create_cross_chain_mapping(mappings, "WETH", &weth_addresses);
        Self::create_cross_chain_mapping(mappings, "USDC", &usdc_addresses);
        Self::create_cross_chain_mapping(mappings, "USDT", &usdt_addresses);
        Self::create_cross_chain_mapping(mappings, "DAI", &dai_addresses);
        Self::create_cross_chain_mapping(mappings, "WBTC", &wbtc_addresses);
    }

    fn create_cross_chain_mapping(
        mappings: &mut HashMap<String, CrossChainTokenMapping>,
        symbol: &str,
        addresses: &HashMap<u64, String>,
    ) {
        let mut chain_mappings = HashMap::new();
        
        for (&chain_id, address) in addresses {
            chain_mappings.insert(chain_id, TokenMapping {
                address: address.clone(),
                symbol: symbol.to_string(),
                decimals: match symbol {
                    "USDC" | "USDT" => 6,
                    "WBTC" => 8,
                    _ => 18,
                },
                is_canonical: true,
                bridge_support: vec![
                    "Stargate".to_string(),
                    "Hop".to_string(),
                    "Across".to_string(),
                    "Synapse".to_string(),
                ],
            });
        }

        mappings.insert(symbol.to_string(), CrossChainTokenMapping {
            base_symbol: symbol.to_string(),
            mappings: chain_mappings,
        });
    }

    /// Get unified token by symbol
    pub async fn get_token(&self, symbol: &str) -> Option<UnifiedToken> {
        let tokens = self.tokens.read().await;
        tokens.get(symbol).cloned()
    }

    /// Get token address for specific chain
    pub async fn get_token_address(&self, symbol: &str, chain_id: u64) -> Option<String> {
        let tokens = self.tokens.read().await;
        tokens.get(symbol)?.chain_addresses.get(&chain_id).cloned()
    }

    /// Get all supported tokens
    pub async fn get_all_tokens(&self) -> Vec<UnifiedToken> {
        let tokens = self.tokens.read().await;
        tokens.values().cloned().collect()
    }

    /// Get tokens supported on specific chain
    pub async fn get_tokens_for_chain(&self, chain_id: u64) -> Vec<UnifiedToken> {
        let tokens = self.tokens.read().await;
        tokens.values()
            .filter(|token| token.chain_addresses.contains_key(&chain_id))
            .cloned()
            .collect()
    }

    /// Check if token is supported on chain
    pub async fn is_token_supported(&self, symbol: &str, chain_id: u64) -> bool {
        self.get_token_address(symbol, chain_id).await.is_some()
    }

    /// Get cross-chain mapping for token
    pub async fn get_cross_chain_mapping(&self, symbol: &str) -> Option<CrossChainTokenMapping> {
        let mappings = self.cross_chain_mappings.read().await;
        mappings.get(symbol).cloned()
    }

    /// Find equivalent tokens across chains
    pub async fn find_equivalent_tokens(&self, symbol: &str, target_chain_id: u64) -> Vec<String> {
        let mappings = self.cross_chain_mappings.read().await;
        
        if let Some(mapping) = mappings.get(symbol) {
            if mapping.mappings.contains_key(&target_chain_id) {
                return vec![symbol.to_string()];
            }
        }

        // Look for wrapped/unwrapped equivalents
        match symbol {
            "ETH" => {
                if self.is_token_supported("WETH", target_chain_id).await {
                    vec!["WETH".to_string()]
                } else {
                    vec![]
                }
            }
            "WETH" => {
                if self.is_token_supported("ETH", target_chain_id).await {
                    vec!["ETH".to_string()]
                } else {
                    vec![]
                }
            }
            _ => vec![]
        }
    }

    /// Get token price from cache or fetch from API
    pub async fn get_token_price(&self, symbol: &str) -> Result<TokenPrice, Box<dyn std::error::Error>> {
        // Check cache first
        {
            let cache = self.price_cache.read().await;
            if let Some(price) = cache.get(symbol) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                
                // Use cached price if less than 5 minutes old
                if now - price.last_updated < 300 {
                    return Ok(price.clone());
                }
            }
        }

        // Fetch from CoinGecko API
        let token = self.get_token(symbol).await
            .ok_or_else(|| format!("Token {} not found", symbol))?;

        let coingecko_id = token.coingecko_id
            .ok_or_else(|| format!("No CoinGecko ID for token {}", symbol))?;

        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true&include_market_cap=true&include_24hr_vol=true",
            coingecko_id
        );

        let response: Value = self.http_client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let price_data = response.get(&coingecko_id)
            .ok_or_else(|| format!("Price data not found for {}", symbol))?;

        let price = TokenPrice {
            symbol: symbol.to_string(),
            price_usd: price_data["usd"].as_f64().unwrap_or(0.0),
            change_24h: price_data["usd_24h_change"].as_f64().unwrap_or(0.0),
            market_cap: price_data["usd_market_cap"].as_f64(),
            volume_24h: price_data["usd_24h_vol"].as_f64(),
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        // Update cache
        {
            let mut cache = self.price_cache.write().await;
            cache.insert(symbol.to_string(), price.clone());
        }

        Ok(price)
    }

    /// Format token amount with proper decimals
    pub async fn format_token_amount(&self, symbol: &str, amount: &str) -> Result<String, Box<dyn std::error::Error>> {
        let token = self.get_token(symbol).await
            .ok_or_else(|| format!("Token {} not found", symbol))?;

        let amount_u128: u128 = amount.parse()?;
        let divisor = 10u128.pow(token.decimals as u32);
        let formatted = amount_u128 as f64 / divisor as f64;

        Ok(format!("{:.6}", formatted).trim_end_matches('0').trim_end_matches('.').to_string())
    }

    /// Parse formatted amount to raw amount
    pub async fn parse_token_amount(&self, symbol: &str, formatted_amount: &str) -> Result<String, Box<dyn std::error::Error>> {
        let token = self.get_token(symbol).await
            .ok_or_else(|| format!("Token {} not found", symbol))?;

        let amount_f64: f64 = formatted_amount.parse()?;
        let multiplier = 10u128.pow(token.decimals as u32);
        let raw_amount = (amount_f64 * multiplier as f64) as u128;

        Ok(raw_amount.to_string())
    }

    /// Get token balance with USD value
    pub async fn get_token_balance_with_value(
        &self,
        symbol: &str,
        balance: &str,
        chain_id: u64,
    ) -> Result<TokenBalance, Box<dyn std::error::Error>> {
        let token = self.get_token(symbol).await
            .ok_or_else(|| format!("Token {} not found", symbol))?;

        let formatted_balance = self.format_token_amount(symbol, balance).await?;
        let price = self.get_token_price(symbol).await?;
        let balance_f64: f64 = formatted_balance.parse().unwrap_or(0.0);
        let value_usd = balance_f64 * price.price_usd;

        Ok(TokenBalance {
            token,
            balance: balance.to_string(),
            balance_formatted: formatted_balance,
            value_usd,
            chain_id,
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        })
    }

    /// Add or update token
    pub async fn add_token(&self, token: UnifiedToken) {
        let mut tokens = self.tokens.write().await;
        tokens.insert(token.symbol.clone(), token);
    }

    /// Remove token
    pub async fn remove_token(&self, symbol: &str) {
        let mut tokens = self.tokens.write().await;
        tokens.remove(symbol);
    }

    /// Get bridgeable tokens between two chains
    pub async fn get_bridgeable_tokens(&self, from_chain_id: u64, to_chain_id: u64) -> Vec<String> {
        let tokens = self.tokens.read().await;
        tokens.values()
            .filter(|token| {
                token.chain_addresses.contains_key(&from_chain_id) &&
                token.chain_addresses.contains_key(&to_chain_id)
            })
            .map(|token| token.symbol.clone())
            .collect()
    }

    /// Get recommended bridge token for cross-chain swap
    pub async fn get_recommended_bridge_token(&self, from_chain_id: u64, to_chain_id: u64) -> Option<String> {
        let bridgeable = self.get_bridgeable_tokens(from_chain_id, to_chain_id).await;
        
        // Prioritize stablecoins for bridging
        for stable in &["USDC", "USDT", "DAI"] {
            if bridgeable.contains(&stable.to_string()) {
                return Some(stable.to_string());
            }
        }

        // Fall back to WETH if available
        if bridgeable.contains(&"WETH".to_string()) {
            return Some("WETH".to_string());
        }

        // Return first available token
        bridgeable.first().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_interface_creation() {
        let interface = UnifiedTokenInterface::new();
        let tokens = interface.get_all_tokens().await;
        
        assert!(!tokens.is_empty());
        assert!(tokens.iter().any(|t| t.symbol == "ETH"));
        assert!(tokens.iter().any(|t| t.symbol == "USDC"));
        assert!(tokens.iter().any(|t| t.symbol == "WETH"));
    }

    #[tokio::test]
    async fn test_get_token() {
        let interface = UnifiedTokenInterface::new();
        
        let eth = interface.get_token("ETH").await.unwrap();
        assert_eq!(eth.symbol, "ETH");
        assert_eq!(eth.decimals, 18);
        assert!(eth.is_native);
        
        let usdc = interface.get_token("USDC").await.unwrap();
        assert_eq!(usdc.symbol, "USDC");
        assert_eq!(usdc.decimals, 6);
        assert!(!usdc.is_native);
    }

    #[tokio::test]
    async fn test_get_token_address() {
        let interface = UnifiedTokenInterface::new();
        
        // ETH on Ethereum (native)
        let eth_address = interface.get_token_address("ETH", 1).await.unwrap();
        assert_eq!(eth_address, "0x0000000000000000000000000000000000000000");
        
        // WETH on Ethereum
        let weth_address = interface.get_token_address("WETH", 1).await.unwrap();
        assert_eq!(weth_address, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    }

    #[tokio::test]
    async fn test_tokens_for_chain() {
        let interface = UnifiedTokenInterface::new();
        
        let eth_tokens = interface.get_tokens_for_chain(1).await;
        assert!(!eth_tokens.is_empty());
        assert!(eth_tokens.iter().any(|t| t.symbol == "ETH"));
        assert!(eth_tokens.iter().any(|t| t.symbol == "USDC"));
        
        let polygon_tokens = interface.get_tokens_for_chain(137).await;
        assert!(!polygon_tokens.is_empty());
        assert!(polygon_tokens.iter().any(|t| t.symbol == "USDC"));
    }

    #[tokio::test]
    async fn test_is_token_supported() {
        let interface = UnifiedTokenInterface::new();
        
        assert!(interface.is_token_supported("ETH", 1).await);
        assert!(interface.is_token_supported("USDC", 137).await);
        assert!(!interface.is_token_supported("NONEXISTENT", 1).await);
    }

    #[tokio::test]
    async fn test_find_equivalent_tokens() {
        let interface = UnifiedTokenInterface::new();
        
        let eth_equivalents = interface.find_equivalent_tokens("ETH", 1).await;
        assert!(eth_equivalents.contains(&"ETH".to_string()));
        
        let weth_equivalents = interface.find_equivalent_tokens("WETH", 1).await;
        assert!(weth_equivalents.contains(&"WETH".to_string()));
    }

    #[tokio::test]
    async fn test_format_token_amount() {
        let interface = UnifiedTokenInterface::new();
        
        // ETH (18 decimals)
        let formatted = interface.format_token_amount("ETH", "1000000000000000000").await.unwrap();
        assert_eq!(formatted, "1");
        
        // USDC (6 decimals)
        let formatted = interface.format_token_amount("USDC", "1000000").await.unwrap();
        assert_eq!(formatted, "1");
    }

    #[tokio::test]
    async fn test_parse_token_amount() {
        let interface = UnifiedTokenInterface::new();
        
        // ETH (18 decimals)
        let parsed = interface.parse_token_amount("ETH", "1.5").await.unwrap();
        assert_eq!(parsed, "1500000000000000000");
        
        // USDC (6 decimals)
        let parsed = interface.parse_token_amount("USDC", "100.5").await.unwrap();
        assert_eq!(parsed, "100500000");
    }

    #[tokio::test]
    async fn test_get_bridgeable_tokens() {
        let interface = UnifiedTokenInterface::new();
        
        let bridgeable = interface.get_bridgeable_tokens(1, 137).await;
        assert!(!bridgeable.is_empty());
        assert!(bridgeable.contains(&"USDC".to_string()));
        assert!(bridgeable.contains(&"USDT".to_string()));
    }

    #[tokio::test]
    async fn test_get_recommended_bridge_token() {
        let interface = UnifiedTokenInterface::new();
        
        let recommended = interface.get_recommended_bridge_token(1, 137).await;
        assert!(recommended.is_some());
        
        let token = recommended.unwrap();
        assert!(["USDC", "USDT", "DAI"].contains(&token.as_str()));
    }

    #[tokio::test]
    async fn test_cross_chain_mapping() {
        let interface = UnifiedTokenInterface::new();
        
        let usdc_mapping = interface.get_cross_chain_mapping("USDC").await.unwrap();
        assert_eq!(usdc_mapping.base_symbol, "USDC");
        assert!(usdc_mapping.mappings.contains_key(&1));
        assert!(usdc_mapping.mappings.contains_key(&137));
    }
}
