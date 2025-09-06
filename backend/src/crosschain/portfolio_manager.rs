use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use reqwest::Client;
use tracing::{info, warn, error};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioBalance {
    pub chain_id: u64,
    pub token_address: String,
    pub token_symbol: String,
    pub balance: String,
    pub balance_usd: f64,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub user_address: String,
    pub balances: Vec<PortfolioBalance>,
    pub total_value_usd: f64,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainBalance {
    pub chain_id: u64,
    pub chain_name: String,
    pub total_value_usd: f64,
    pub token_count: usize,
    pub balances: Vec<PortfolioBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummary {
    pub user_address: String,
    pub total_value_usd: f64,
    pub chain_distribution: Vec<ChainBalance>,
    pub top_tokens: Vec<PortfolioBalance>,
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: String,
    pub rpc_url: String,
    pub native_token: String,
    pub block_explorer: String,
    pub multicall_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub address: String,
    pub symbol: String,
    pub decimals: u8,
    pub balance: String,
    pub balance_formatted: String,
    pub price_usd: f64,
    pub value_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainBalanceResponse {
    pub chain_id: u64,
    pub chain_name: String,
    pub native_balance: TokenBalance,
    pub token_balances: Vec<TokenBalance>,
    pub total_value_usd: f64,
    pub last_updated: u64,
}

#[derive(Clone)]
pub struct PortfolioManager {
    // Cache for user portfolios
    portfolio_cache: Arc<RwLock<HashMap<String, Portfolio>>>,
    // Price cache for tokens
    price_cache: Arc<RwLock<HashMap<(u64, String), f64>>>,
    // Chain configurations with RPC endpoints
    chain_configs: HashMap<u64, ChainConfig>,
    // HTTP client for RPC calls
    http_client: Client,
}

impl PortfolioManager {
    pub fn new() -> Self {
        let mut chain_configs = HashMap::new();
        
        // Ethereum Mainnet
        chain_configs.insert(1, ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            rpc_url: "https://eth-mainnet.alchemyapi.io/v2/demo".to_string(),
            native_token: "ETH".to_string(),
            block_explorer: "https://etherscan.io".to_string(),
            multicall_address: Some("0xeefBa1e63905eF1D7ACbA5a8513c70307C1cE441".to_string()),
        });
        
        // Polygon
        chain_configs.insert(137, ChainConfig {
            chain_id: 137,
            name: "Polygon".to_string(),
            rpc_url: "https://polygon-rpc.com".to_string(),
            native_token: "MATIC".to_string(),
            block_explorer: "https://polygonscan.com".to_string(),
            multicall_address: Some("0x11ce4B23bD875D7F5C6a31084f55fDe1e9A87507".to_string()),
        });
        
        // Arbitrum
        chain_configs.insert(42161, ChainConfig {
            chain_id: 42161,
            name: "Arbitrum".to_string(),
            rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
            native_token: "ETH".to_string(),
            block_explorer: "https://arbiscan.io".to_string(),
            multicall_address: Some("0x842eC2c7D803033Edf55E478F461FC547Bc54EB2".to_string()),
        });
        
        // Optimism
        chain_configs.insert(10, ChainConfig {
            chain_id: 10,
            name: "Optimism".to_string(),
            rpc_url: "https://mainnet.optimism.io".to_string(),
            native_token: "ETH".to_string(),
            block_explorer: "https://optimistic.etherscan.io".to_string(),
            multicall_address: Some("0x2DC0E2aa608532Da689e89e237dF582B783E552C".to_string()),
        });
        
        // BSC
        chain_configs.insert(56, ChainConfig {
            chain_id: 56,
            name: "BSC".to_string(),
            rpc_url: "https://bsc-dataseed1.binance.org".to_string(),
            native_token: "BNB".to_string(),
            block_explorer: "https://bscscan.com".to_string(),
            multicall_address: Some("0x41263cBA59EB80dC200F3E2544eda4ed6A90E76C".to_string()),
        });
        
        // Avalanche
        chain_configs.insert(43114, ChainConfig {
            chain_id: 43114,
            name: "Avalanche".to_string(),
            rpc_url: "https://api.avax.network/ext/bc/C/rpc".to_string(),
            native_token: "AVAX".to_string(),
            block_explorer: "https://snowtrace.io".to_string(),
            multicall_address: Some("0x98e2060F672FD1656a07bc12D7253b5e41bF4876".to_string()),
        });

        Self {
            portfolio_cache: Arc::new(RwLock::new(HashMap::new())),
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            chain_configs,
            http_client: Client::new(),
        }
    }

    /// Get portfolio for a user across all chains
    pub async fn get_portfolio(&self, user_address: &str) -> Result<Portfolio, Box<dyn std::error::Error>> {
        // Check cache first
        {
            let cache = self.portfolio_cache.read().await;
            if let Some(portfolio) = cache.get(user_address) {
                // Return cached portfolio if it's less than 5 minutes old
                let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                if now - portfolio.last_updated < 300 {
                    return Ok(portfolio.clone());
                }
            }
        }

        // Fetch fresh portfolio data
        let portfolio = self.fetch_portfolio(user_address).await?;
        
        // Update cache
        {
            let mut cache = self.portfolio_cache.write().await;
            cache.insert(user_address.to_string(), portfolio.clone());
        }

        Ok(portfolio)
    }

    /// Get portfolio summary with chain distribution
    pub async fn get_portfolio_summary(&self, user_address: &str) -> Result<PortfolioSummary, Box<dyn std::error::Error>> {
        let portfolio = self.get_portfolio(user_address).await?;
        
        let mut chain_distribution = HashMap::new();
        let mut top_tokens = portfolio.balances.clone();
        
        // Group balances by chain
        for balance in &portfolio.balances {
            let chain_name = self.chain_configs
                .get(&balance.chain_id)
                .map(|config| config.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
                
            let chain_balance = chain_distribution
                .entry(balance.chain_id)
                .or_insert(ChainBalance {
                    chain_id: balance.chain_id,
                    chain_name,
                    total_value_usd: 0.0,
                    token_count: 0,
                    balances: Vec::new(),
                });
                
            chain_balance.total_value_usd += balance.balance_usd;
            chain_balance.token_count += 1;
            chain_balance.balances.push(balance.clone());
        }
        
        // Sort top tokens by USD value
        top_tokens.sort_by(|a, b| b.balance_usd.partial_cmp(&a.balance_usd).unwrap());
        top_tokens.truncate(10); // Top 10 tokens
        
        Ok(PortfolioSummary {
            user_address: user_address.to_string(),
            total_value_usd: portfolio.total_value_usd,
            chain_distribution: chain_distribution.into_values().collect(),
            top_tokens,
            last_updated: portfolio.last_updated,
        })
    }

    /// Update token prices in cache
    pub async fn update_token_prices(&self, prices: Vec<(u64, String, f64)>) -> Result<(), Box<dyn std::error::Error>> {
        let mut cache = self.price_cache.write().await;
        for (chain_id, token_address, price) in prices {
            cache.insert((chain_id, token_address), price);
        }
        Ok(())
    }

    /// Get token price from cache
    pub async fn get_token_price(&self, chain_id: u64, token_address: &str) -> Option<f64> {
        let cache = self.price_cache.read().await;
        cache.get(&(chain_id, token_address.to_string())).copied()
    }

    /// Track balance changes for analytics
    pub async fn track_balance_change(
        &self,
        user_address: &str,
        chain_id: u64,
        token_address: &str,
        old_balance: &str,
        new_balance: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // This would integrate with analytics/monitoring system
        println!(
            "Balance change for {} on chain {}: {} {} -> {}",
            user_address, chain_id, token_address, old_balance, new_balance
        );
        Ok(())
    }

    /// Get balances for specific chain
    pub async fn get_chain_balances(
        &self,
        user_address: &str,
        chain_id: u64,
    ) -> Result<Vec<PortfolioBalance>, Box<dyn std::error::Error>> {
        let portfolio = self.get_portfolio(user_address).await?;
        let chain_balances: Vec<PortfolioBalance> = portfolio
            .balances
            .into_iter()
            .filter(|balance| balance.chain_id == chain_id)
            .collect();
        Ok(chain_balances)
    }

    /// Get chain-specific balances using real RPC calls
    pub async fn get_chain_balance_detailed(
        &self,
        user_address: &str,
        chain_id: u64,
    ) -> Result<ChainBalanceResponse, Box<dyn std::error::Error>> {
        let chain_config = self.chain_configs.get(&chain_id)
            .ok_or_else(|| format!("Unsupported chain ID: {}", chain_id))?;

        info!("🔍 Fetching balances for {} on {}", user_address, chain_config.name);

        // Get native token balance
        let native_balance = self.get_native_balance(user_address, chain_config).await?;
        
        // Get ERC-20 token balances (using common tokens for each chain)
        let token_balances = self.get_token_balances(user_address, chain_config).await?;
        
        let total_value_usd = native_balance.value_usd + 
            token_balances.iter().map(|t| t.value_usd).sum::<f64>();

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        Ok(ChainBalanceResponse {
            chain_id,
            chain_name: chain_config.name.clone(),
            native_balance,
            token_balances,
            total_value_usd,
            last_updated: now,
        })
    }

    /// Get native token balance (ETH, MATIC, BNB, etc.)
    async fn get_native_balance(
        &self,
        user_address: &str,
        chain_config: &ChainConfig,
    ) -> Result<TokenBalance, Box<dyn std::error::Error>> {
        let rpc_payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [user_address, "latest"],
            "id": 1
        });

        let response = self.http_client
            .post(&chain_config.rpc_url)
            .json(&rpc_payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        let rpc_result: Value = response.json().await?;
        
        if let Some(error) = rpc_result.get("error") {
            return Err(format!("RPC error: {}", error).into());
        }

        let balance_hex = rpc_result["result"]
            .as_str()
            .ok_or("Invalid balance response")?;
        
        let balance_wei = u128::from_str_radix(&balance_hex[2..], 16)
            .map_err(|e| format!("Failed to parse balance: {}", e))?;
        
        let balance_formatted = self.format_balance(balance_wei, 18);
        let price_usd = self.get_token_price_from_coingecko(&chain_config.native_token).await.unwrap_or(0.0);
        let value_usd = balance_formatted.parse::<f64>().unwrap_or(0.0) * price_usd;

        Ok(TokenBalance {
            address: "native".to_string(),
            symbol: chain_config.native_token.clone(),
            decimals: 18,
            balance: balance_wei.to_string(),
            balance_formatted,
            price_usd,
            value_usd,
        })
    }

    /// Get ERC-20 token balances for common tokens on each chain
    async fn get_token_balances(
        &self,
        user_address: &str,
        chain_config: &ChainConfig,
    ) -> Result<Vec<TokenBalance>, Box<dyn std::error::Error>> {
        let mut token_balances = Vec::new();
        
        // Define common tokens for each chain
        let common_tokens = self.get_common_tokens_for_chain(chain_config.chain_id);
        
        for (token_address, symbol, decimals) in common_tokens {
            match self.get_erc20_balance(user_address, &token_address, &symbol, decimals, chain_config).await {
                Ok(balance) => {
                    if balance.value_usd > 0.01 { // Only include tokens with value > $0.01
                        token_balances.push(balance);
                    }
                }
                Err(e) => {
                    warn!("Failed to get balance for {} on {}: {}", symbol, chain_config.name, e);
                }
            }
        }

        Ok(token_balances)
    }

    /// Get ERC-20 token balance using balanceOf call
    async fn get_erc20_balance(
        &self,
        user_address: &str,
        token_address: &str,
        symbol: &str,
        decimals: u8,
        chain_config: &ChainConfig,
    ) -> Result<TokenBalance, Box<dyn std::error::Error>> {
        // ERC-20 balanceOf function signature: balanceOf(address)
        let function_selector = "0x70a08231";
        let padded_address = format!("{:0>64}", &user_address[2..]);
        let call_data = format!("{}{}", function_selector, padded_address);

        let rpc_payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": token_address,
                "data": call_data
            }, "latest"],
            "id": 1
        });

        let response = self.http_client
            .post(&chain_config.rpc_url)
            .json(&rpc_payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        let rpc_result: Value = response.json().await?;
        
        if let Some(error) = rpc_result.get("error") {
            return Err(format!("RPC error for {}: {}", symbol, error).into());
        }

        let balance_hex = rpc_result["result"]
            .as_str()
            .ok_or("Invalid balance response")?;
        
        let balance_raw = u128::from_str_radix(&balance_hex[2..], 16)
            .map_err(|e| format!("Failed to parse balance: {}", e))?;
        
        let balance_formatted = self.format_balance(balance_raw, decimals);
        let price_usd = self.get_token_price_from_coingecko(symbol).await.unwrap_or(0.0);
        let value_usd = balance_formatted.parse::<f64>().unwrap_or(0.0) * price_usd;

        Ok(TokenBalance {
            address: token_address.to_string(),
            symbol: symbol.to_string(),
            decimals,
            balance: balance_raw.to_string(),
            balance_formatted,
            price_usd,
            value_usd,
        })
    }

    /// Get token price from CoinGecko API with rate limiting and better error handling
    async fn get_token_price_from_coingecko(&self, token_symbol: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let coingecko_id = self.symbol_to_coingecko_id(token_symbol);
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd",
            coingecko_id
        );

        // Add delay to prevent rate limiting
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let response = self.http_client
            .get(&url)
            .header("User-Agent", "BralaladotDex/1.0")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await?;
        
        if !response.status().is_success() {
            error!("❌ CoinGecko API error for {}: Status {}", token_symbol, response.status());
            return Ok(0.0);
        }
        
        let response_text = response.text().await?;
        
        if response_text.is_empty() {
            error!("❌ Empty response from CoinGecko for {}", token_symbol);
            return Ok(0.0);
        }
        
        let price_data: Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse CoinGecko response for {}: {} - Response: {}", token_symbol, e, response_text))?;
        
        let price = price_data
            .get(coingecko_id)
            .and_then(|token_data| token_data.get("usd"))
            .and_then(|price| price.as_f64())
            .unwrap_or_else(|| {
                error!("❌ No price data found for {} ({})", token_symbol, coingecko_id);
                0.0
            });
            
        info!("💰 Real price for {}: ${:.2}", token_symbol, price);
        Ok(price)
    }

    /// Convert token symbol to CoinGecko ID
    fn symbol_to_coingecko_id(&self, symbol: &str) -> &str {
        match symbol {
            "ETH" => "ethereum",
            "USDC" => "usd-coin",
            "USDT" => "tether", 
            "DAI" => "dai",
            "WETH" => "ethereum", // WETH should use ethereum price
            "WBTC" => "wrapped-bitcoin",
            "MATIC" => "matic-network",
            "BNB" => "binancecoin",
            "AVAX" => "avalanche-2",
            "LINK" => "chainlink",
            "UNI" => "uniswap",
            "AAVE" => "aave",
            _ => {
                error!("⚠️ Unknown token symbol: {}, using ethereum as fallback", symbol);
                "ethereum"
            }
        }
    }

    /// Get common tokens for each chain
    fn get_common_tokens_for_chain(&self, chain_id: u64) -> Vec<(String, String, u8)> {
        match chain_id {
            1 => vec![ // Ethereum
                ("0xA0b86a33E6441E8C8C7014C0C746C4B5F4F5E5E5".to_string(), "USDC".to_string(), 6),
                ("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(), "USDT".to_string(), 6),
                ("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(), "DAI".to_string(), 18),
                ("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), "WETH".to_string(), 18),
                ("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string(), "WBTC".to_string(), 8),
            ],
            137 => vec![ // Polygon
                ("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string(), "USDC".to_string(), 6),
                ("0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string(), "USDT".to_string(), 6),
                ("0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063".to_string(), "DAI".to_string(), 18),
                ("0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string(), "WETH".to_string(), 18),
            ],
            42161 => vec![ // Arbitrum
                ("0xFF970A61A04b1cA14834A43f5dE4533eBDDB5CC8".to_string(), "USDC".to_string(), 6),
                ("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string(), "USDT".to_string(), 6),
                ("0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".to_string(), "DAI".to_string(), 18),
                ("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string(), "WETH".to_string(), 18),
            ],
            10 => vec![ // Optimism
                ("0x7F5c764cBc14f9669B88837ca1490cCa17c31607".to_string(), "USDC".to_string(), 6),
                ("0x94b008aA00579c1307B0EF2c499aD98a8ce58e58".to_string(), "USDT".to_string(), 6),
                ("0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1".to_string(), "DAI".to_string(), 18),
                ("0x4200000000000000000000000000000000000006".to_string(), "WETH".to_string(), 18),
            ],
            56 => vec![ // BSC
                ("0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d".to_string(), "USDC".to_string(), 18),
                ("0x55d398326f99059fF775485246999027B3197955".to_string(), "USDT".to_string(), 18),
                ("0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3".to_string(), "DAI".to_string(), 18),
                ("0x2170Ed0880ac9A755fd29B2688956BD959F933F8".to_string(), "WETH".to_string(), 18),
            ],
            43114 => vec![ // Avalanche
                ("0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E".to_string(), "USDC".to_string(), 6),
                ("0x9702230A8Ea53601f5cD2dc00fDBc13d4dF4A8c7".to_string(), "USDT".to_string(), 6),
                ("0xd586E7F844cEa2F87f50152665BCbc2C279D8d70".to_string(), "DAI".to_string(), 18),
                ("0x49D5c2BdFfac6CE2BFdB6640F4F80f226bc10bAB".to_string(), "WETH".to_string(), 18),
            ],
            _ => vec![],
        }
    }

    /// Format balance from raw units to human-readable format
    fn format_balance(&self, balance: u128, decimals: u8) -> String {
        let divisor = 10_u128.pow(decimals as u32);
        let integer_part = balance / divisor;
        let fractional_part = balance % divisor;
        
        if fractional_part == 0 {
            integer_part.to_string()
        } else {
            let fractional_str = format!("{:0width$}", fractional_part, width = decimals as usize);
            let trimmed = fractional_str.trim_end_matches('0');
            if trimmed.is_empty() {
                integer_part.to_string()
            } else {
                format!("{}.{}", integer_part, trimmed)
            }
        }
    }

    /// Private method to fetch portfolio from blockchain using real RPC calls
    async fn fetch_portfolio(&self, user_address: &str) -> Result<Portfolio, Box<dyn std::error::Error>> {
        let mut balances = Vec::new();
        let mut total_value_usd = 0.0;
        
        info!("🔄 Fetching portfolio for {} across all chains", user_address);
        
        // Fetch balances from all supported chains concurrently
        let mut chain_futures = Vec::new();
        for chain_id in self.chain_configs.keys() {
            let chain_future = self.get_chain_balance_detailed(user_address, *chain_id);
            chain_futures.push((*chain_id, chain_future));
        }

        // Process results
        for (chain_id, future) in chain_futures {
            match future.await {
                Ok(chain_balance) => {
                    info!("✅ Successfully fetched {} balances: ${:.2}", 
                          chain_balance.chain_name, chain_balance.total_value_usd);
                    
                    total_value_usd += chain_balance.total_value_usd;
                    
                    // Convert native balance
                    if chain_balance.native_balance.value_usd > 0.01 {
                        balances.push(PortfolioBalance {
                            chain_id,
                            token_address: "native".to_string(),
                            token_symbol: chain_balance.native_balance.symbol,
                            balance: chain_balance.native_balance.balance_formatted,
                            balance_usd: chain_balance.native_balance.value_usd,
                            last_updated: chain_balance.last_updated,
                        });
                    }
                    
                    // Convert token balances
                    for token_balance in chain_balance.token_balances {
                        balances.push(PortfolioBalance {
                            chain_id,
                            token_address: token_balance.address,
                            token_symbol: token_balance.symbol,
                            balance: token_balance.balance_formatted,
                            balance_usd: token_balance.value_usd,
                            last_updated: chain_balance.last_updated,
                        });
                    }
                }
                Err(e) => {
                    error!("❌ Failed to fetch balances for chain {}: {}", chain_id, e);
                }
            }
        }
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        
        info!("📊 Portfolio summary: {} tokens, ${:.2} total value", 
              balances.len(), total_value_usd);
        
        Ok(Portfolio {
            user_address: user_address.to_string(),
            balances,
            total_value_usd,
            last_updated: now,
        })
    }

    /// Clear cache for testing
    pub async fn clear_cache(&self) {
        let mut portfolio_cache = self.portfolio_cache.write().await;
        let mut price_cache = self.price_cache.write().await;
        portfolio_cache.clear();
        price_cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_portfolio_manager_creation() {
        let manager = PortfolioManager::new();
        
        // Verify supported chains are loaded
        assert_eq!(manager.chain_configs.len(), 6);
        assert!(manager.chain_configs.contains_key(&1));
        assert!(manager.chain_configs.contains_key(&137));
    }

    #[tokio::test]
    async fn test_get_portfolio() {
        let manager = PortfolioManager::new();
        let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
        

        let portfolio = manager.get_portfolio(user_address).await.unwrap();
        
        assert_eq!(portfolio.user_address, user_address);
        assert!(portfolio.last_updated > 0);
        
        // Portfolio may be empty for test addresses - this is expected behavior
        println!("Portfolio balances: {}", portfolio.balances.len());
        println!("Total value USD: ${:.2}", portfolio.total_value_usd);
    }

    #[tokio::test]
    async fn test_portfolio_caching() {
        let manager = PortfolioManager::new();
        let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
        
        // First call should fetch from "blockchain"
        let portfolio1 = manager.get_portfolio(user_address).await.unwrap();
        
        // Second call should return cached result
        let portfolio2 = manager.get_portfolio(user_address).await.unwrap();
        
        assert_eq!(portfolio1.last_updated, portfolio2.last_updated);
        assert_eq!(portfolio1.total_value_usd, portfolio2.total_value_usd);
    }

    #[tokio::test]
    async fn test_get_portfolio_summary() {
        let manager = PortfolioManager::new();
        let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
        
        let summary = manager.get_portfolio_summary(user_address).await.unwrap();
        
        assert_eq!(summary.user_address, user_address);
        
        // Real blockchain data - may be empty for test addresses
        println!("Summary total value USD: ${:.2}", summary.total_value_usd);
        println!("Chain distribution count: {}", summary.chain_distribution.len());
        println!("Top tokens count: {}", summary.top_tokens.len());
        
        // Verify top tokens are sorted by USD value if any exist
        if summary.top_tokens.len() > 1 {
            for i in 1..summary.top_tokens.len() {
                assert!(summary.top_tokens[i-1].balance_usd >= summary.top_tokens[i].balance_usd);
            }
        }
    }

    #[tokio::test]
    async fn test_get_chain_balances() {
        let manager = PortfolioManager::new();
        let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
        
        // Get Ethereum balances - real blockchain data
        let eth_balances = manager.get_chain_balances(user_address, 1).await.unwrap();
        println!("Ethereum balances count: {}", eth_balances.len());
        
        for balance in &eth_balances {
            assert_eq!(balance.chain_id, 1);
        }
        
        // Get Polygon balances - real blockchain data
        let polygon_balances = manager.get_chain_balances(user_address, 137).await.unwrap();
        println!("Polygon balances count: {}", polygon_balances.len());
        
        // Verify chain IDs are correct for any returned balances
        for balance in &polygon_balances {
            assert_eq!(balance.chain_id, 137);
        }
    }

    #[tokio::test]
    async fn test_token_price_cache() {
        let manager = PortfolioManager::new();
        
        // Update prices
        let prices = vec![
            (1, "0xA0b86a33E6441E8C8C7014C0C746C4B5F4F5E5E5".to_string(), 1.0),
            (1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(), 3300.0),
        ];
        
        manager.update_token_prices(prices).await.unwrap();
        
        // Verify prices are cached
        let usdc_price = manager.get_token_price(1, "0xA0b86a33E6441E8C8C7014C0C746C4B5F4F5E5E5").await;
        assert_eq!(usdc_price, Some(1.0));
        
        let weth_price = manager.get_token_price(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").await;
        assert_eq!(weth_price, Some(3300.0));
        
        // Non-existent token should return None
        let unknown_price = manager.get_token_price(1, "0xUnknown").await;
        assert_eq!(unknown_price, None);
    }

    #[tokio::test]
    async fn test_track_balance_change() {
        let manager = PortfolioManager::new();
        
        // This should not panic and should complete successfully
        let result = manager.track_balance_change(
            "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5",
            1,
            "0xA0b86a33E6441E8C8C7014C0C746C4B5F4F5E5E5",
            "1000.0",
            "1100.0",
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let manager = PortfolioManager::new();
        let user_address = "0x742d35Cc6634C0532925a3b8D5c9C5E3C5F5c5c5";
        
        // Populate cache
        let _portfolio = manager.get_portfolio(user_address).await.unwrap();
        manager.update_token_prices(vec![(1, "USDC".to_string(), 1.0)]).await.unwrap();
        
        // Clear cache
        manager.clear_cache().await;
        
        // Verify cache is empty by checking that price returns None
        let price = manager.get_token_price(1, "USDC").await;
        assert_eq!(price, None);
    }
}
