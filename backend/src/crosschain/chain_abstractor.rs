use crate::bridges::{BridgeManager, CrossChainParams, BridgeQuote};
use crate::dexes::{DexManager, DexError};
use crate::types::{QuoteParams, RouteBreakdown};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use uuid::Uuid;
use super::unified_token_interface::{UnifiedTokenInterface, UnifiedToken, TokenBalance};
use tracing::{error, info, warn};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedQuote {
    pub operation_type: OperationType,
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out: String,
    pub estimated_gas: String,
    pub execution_time_seconds: u64,
    pub route_steps: Vec<RouteStep>,
    pub total_fees_usd: f64,
    pub price_impact: f64,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    SameChainSwap,
    CrossChainBridge,
    CrossChainSwap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    pub step_type: StepType,
    pub chain_id: u64,
    pub protocol: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out: String,
    pub gas_estimate: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepType {
    Swap,
    Bridge,
    Wrap,
    Unwrap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub transaction_hash: String,
    pub status: ExecutionStatus,
    pub gas_used: String,
    pub actual_amount_out: String,
    pub execution_time_ms: u64,
    pub block_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Success,
    Failed,
    Reverted,
}

#[derive(Clone)]
pub struct ChainAbstractor {
    chains: Arc<RwLock<HashMap<u64, ChainConfig>>>,
    gas_price_cache: Arc<RwLock<HashMap<u64, f64>>>,
    token_interface: Arc<UnifiedTokenInterface>,
    http_client: Client,
}

#[derive(Debug, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: String,
    pub native_token: String,
    pub rpc_url: String,
    pub block_time_ms: u64,
    pub gas_limit: u64,
}

impl ChainAbstractor {
    pub fn new() -> Self {
        let mut chains = HashMap::new();
        
        // Initialize supported chains
        chains.insert(1, ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            native_token: "ETH".to_string(),
            rpc_url: "https://eth-mainnet.g.alchemy.com/v2/demo".to_string(),
            block_time_ms: 12000,
            gas_limit: 21000,
        });
        
        chains.insert(137, ChainConfig {
            chain_id: 137,
            name: "Polygon".to_string(),
            native_token: "MATIC".to_string(),
            rpc_url: "https://polygon-rpc.com".to_string(),
            block_time_ms: 2000,
            gas_limit: 21000,
        });
        
        chains.insert(10, ChainConfig {
            chain_id: 10,
            name: "Optimism".to_string(),
            native_token: "ETH".to_string(),
            rpc_url: "https://mainnet.optimism.io".to_string(),
            block_time_ms: 2000,
            gas_limit: 21000,
        });
        
        chains.insert(42161, ChainConfig {
            chain_id: 42161,
            name: "Arbitrum".to_string(),
            native_token: "ETH".to_string(),
            rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
            block_time_ms: 1000,
            gas_limit: 21000,
        });
        
        Self {
            chains: Arc::new(RwLock::new(chains)),
            gas_price_cache: Arc::new(RwLock::new(HashMap::new())),
            token_interface: Arc::new(UnifiedTokenInterface::new()),
            http_client: Client::new(),
        }
    }

    /// Get unified quote for any operation type
    pub async fn get_unified_quote(
        &self,
        from_token: &str,
        to_token: &str,
        amount: &str,
        from_chain_id: u64,
        to_chain_id: u64,
        user_address: &str,
    ) -> Result<UnifiedQuote, Box<dyn std::error::Error>> {
        // Validate tokens exist and are supported on respective chains
        if !self.token_interface.is_token_supported(from_token, from_chain_id).await {
            return Err(format!("Token {} not supported on chain {}", from_token, from_chain_id).into());
        }

        let operation_type = self.determine_operation_type(from_chain_id, to_chain_id, from_token, to_token).await;
        
        match operation_type {
            OperationType::SameChainSwap => {
                self.get_same_chain_swap_quote(from_chain_id, from_token, to_token, amount).await
            }
            OperationType::CrossChainBridge => {
                self.get_cross_chain_bridge_quote(from_chain_id, to_chain_id, from_token, amount, user_address).await
            }
            OperationType::CrossChainSwap => {
                self.get_cross_chain_swap_quote(from_chain_id, to_chain_id, from_token, to_token, amount, user_address).await
            }
        }
    }

    /// Get token interface reference
    pub fn get_token_interface(&self) -> &Arc<UnifiedTokenInterface> {
        &self.token_interface
    }


    /// Get supported tokens for a chain
    pub async fn get_supported_tokens(&self, chain_id: u64) -> Vec<UnifiedToken> {
        self.token_interface.get_tokens_for_chain(chain_id).await
    }

    /// Get token address for specific chain
    pub async fn get_token_address(&self, symbol: &str, chain_id: u64) -> Option<String> {
        self.token_interface.get_token_address(symbol, chain_id).await
    }

    /// Format token amount with proper decimals
    pub async fn format_token_amount(&self, symbol: &str, amount: &str) -> Result<String, Box<dyn std::error::Error>> {
        self.token_interface.format_token_amount(symbol, amount).await
    }

    /// Get bridgeable tokens between chains
    pub async fn get_bridgeable_tokens(&self, from_chain_id: u64, to_chain_id: u64) -> Vec<String> {
        self.token_interface.get_bridgeable_tokens(from_chain_id, to_chain_id).await
    }

    /// Get recommended bridge token
    pub async fn get_recommended_bridge_token(&self, from_chain_id: u64, to_chain_id: u64) -> Option<String> {
        self.token_interface.get_recommended_bridge_token(from_chain_id, to_chain_id).await
    }

    /// Execute a unified operation
    pub async fn execute_unified_operation(
        &self,
        quote: &UnifiedQuote,
        user_address: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        match quote.operation_type {
            OperationType::SameChainSwap => {
                self.execute_same_chain_swap(quote, user_address).await
            }
            OperationType::CrossChainBridge => {
                self.execute_cross_chain_bridge(quote, user_address).await
            }
            OperationType::CrossChainSwap => {
                self.execute_cross_chain_swap(quote, user_address).await
            }
        }
    }

    /// Get chain configuration
    pub async fn get_chain_config(&self, chain_id: u64) -> Option<ChainConfig> {
        let chains = self.chains.read().await;
        chains.get(&chain_id).cloned()
    }

    /// Update gas prices for all chains
    pub async fn update_gas_prices(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut gas_prices = self.gas_price_cache.write().await;
        
        // Mock gas prices - in real implementation, fetch from chain RPCs
        gas_prices.insert(1, 30.0);     // Ethereum: 30 gwei
        gas_prices.insert(137, 50.0);   // Polygon: 50 gwei
        gas_prices.insert(42161, 0.1);  // Arbitrum: 0.1 gwei
        gas_prices.insert(10, 0.001);   // Optimism: 0.001 gwei
        
        Ok(())
    }

    /// Get current gas price for a chain
    pub async fn get_gas_price(&self, chain_id: u64) -> Option<f64> {
        let gas_prices = self.gas_price_cache.read().await;
        gas_prices.get(&chain_id).copied()
    }

    /// Estimate total execution time for a route
    pub async fn estimate_execution_time(&self, route_steps: &[RouteStep]) -> u64 {
        let mut total_time = 0u64;
        
        for step in route_steps {
            let chain_config = self.get_chain_config(step.chain_id).await;
            let base_time = match step.step_type {
                StepType::Swap => 30,      // 30 seconds for DEX swap
                StepType::Bridge => 300,   // 5 minutes for bridge
                StepType::Wrap => 15,      // 15 seconds for wrap
                StepType::Unwrap => 15,    // 15 seconds for unwrap
            };
            
            // Add chain-specific block time
            let block_time = chain_config
                .map(|c| c.block_time_ms / 1000)
                .unwrap_or(300);
            
            total_time += base_time + block_time * 2; // 2 block confirmations
        }
        
        total_time
    }

    // Private helper methods
    async fn determine_operation_type(
        &self,
        from_chain_id: u64,
        to_chain_id: u64,
        token_in: &str,
        token_out: &str,
    ) -> OperationType {
        if from_chain_id == to_chain_id {
            OperationType::SameChainSwap
        } else if token_in == token_out {
            OperationType::CrossChainBridge
        } else {
            OperationType::CrossChainSwap
        }
    }

    async fn get_same_chain_swap_quote(
        &self,
        chain_id: u64,
        token_in: &str,
        token_out: &str,
        amount_in: &str,
    ) -> Result<UnifiedQuote, Box<dyn std::error::Error>> {
        // Mock implementation - replace with actual DEX integration
        let amount_out = "1000000000000000000"; // Mock 1 ETH output

        let route_step = RouteStep {
            step_type: StepType::Swap,
            chain_id,
            protocol: "DEX".to_string(),
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            amount_in: amount_in.to_string(),
            amount_out: amount_out.to_string(),
            gas_estimate: "150000".to_string(),
        };

        Ok(UnifiedQuote {
            operation_type: OperationType::SameChainSwap,
            from_chain_id: chain_id,
            to_chain_id: chain_id,
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            amount_in: amount_in.to_string(),
            amount_out: amount_out.to_string(),
            estimated_gas: "150000".to_string(),
            execution_time_seconds: self.estimate_execution_time(&[route_step.clone()]).await,
            route_steps: vec![route_step],
            total_fees_usd: 0.0, // Fee calculation based on 0.3% estimate
            price_impact: 0.1, // Mock price impact
            confidence_score: 0.95,
        })
    }

    async fn get_cross_chain_bridge_quote(
        &self,
        from_chain_id: u64,
        to_chain_id: u64,
        token: &str,
        amount_in: &str,
        user_address: &str,
    ) -> Result<UnifiedQuote, Box<dyn std::error::Error>> {
        // Mock implementation - replace with actual bridge integration
        let amount_out = amount_in; // 1:1 for same token bridging

        let route_step = RouteStep {
            step_type: StepType::Bridge,
            chain_id: from_chain_id,
            protocol: "MockBridge".to_string(),
            token_in: token.to_string(),
            token_out: token.to_string(),
            amount_in: amount_in.to_string(),
            amount_out: amount_out.to_string(),
            gas_estimate: "200000".to_string(),
        };

        Ok(UnifiedQuote {
            operation_type: OperationType::CrossChainBridge,
            from_chain_id,
            to_chain_id,
            token_in: token.to_string(),
            token_out: token.to_string(),
            amount_in: amount_in.to_string(),
            amount_out: amount_out.to_string(),
            estimated_gas: "200000".to_string(),
            execution_time_seconds: self.estimate_execution_time(&[route_step.clone()]).await,
            route_steps: vec![route_step],
            total_fees_usd: 5.0, // Mock bridge fee
            price_impact: 0.0,
            confidence_score: 0.85,
        })
    }

    async fn get_cross_chain_swap_quote(
        &self,
        from_chain_id: u64,
        to_chain_id: u64,
        token_in: &str,
        token_out: &str,
        amount_in: &str,
        user_address: &str,
    ) -> Result<UnifiedQuote, Box<dyn std::error::Error>> {
        // For cross-chain swaps, we need to:
        // 1. Swap token_in to bridgeable token on source chain
        // 2. Bridge the bridgeable token
        // 3. Swap bridgeable token to token_out on destination chain

        // For simplicity, assume USDC is the bridgeable token
        let bridge_token = "USDC";
        let mut route_steps = Vec::new();
        let mut total_fees = 0.0;
        let mut total_gas = 0u64;

        // Step 1: Swap on source chain (if needed)
        let intermediate_amount = if token_in != bridge_token {
            // Mock swap implementation
            let swap_amount = amount_in;
            
            route_steps.push(RouteStep {
                step_type: StepType::Swap,
                chain_id: from_chain_id,
                protocol: "DEX".to_string(),
                token_in: token_in.to_string(),
                token_out: bridge_token.to_string(),
                amount_in: amount_in.to_string(),
                amount_out: swap_amount.to_string(),
                gas_estimate: "150000".to_string(),
            });

            total_fees += amount_in.parse::<f64>().unwrap_or(0.0) * 0.003;
            total_gas += 150000;
            swap_amount.to_string()
        } else {
            amount_in.to_string()
        };

        // Step 2: Bridge (mock implementation)
        let bridge_amount = intermediate_amount.clone();
        
        route_steps.push(RouteStep {
            step_type: StepType::Bridge,
            chain_id: from_chain_id,
            protocol: "MockBridge".to_string(),
            token_in: bridge_token.to_string(),
            token_out: bridge_token.to_string(),
            amount_in: intermediate_amount,
            amount_out: bridge_amount.clone(),
            gas_estimate: "200000".to_string(),
        });

        total_fees += 5.0; // Mock bridge fee
        total_gas += 200000;

        // Step 3: Swap on destination chain if needed
        let final_amount = if token_out != bridge_token {
            // Mock swap implementation
            let final_swap_amount = bridge_amount.clone();

            route_steps.push(RouteStep {
                step_type: StepType::Swap,
                chain_id: to_chain_id,
                protocol: "DEX".to_string(),
                token_in: bridge_token.to_string(),
                token_out: token_out.to_string(),
                amount_in: bridge_amount.clone(),
                amount_out: final_swap_amount.clone(),
                gas_estimate: "150000".to_string(),
            });

            total_fees += bridge_amount.parse::<f64>().unwrap_or(0.0) * 0.003;
            total_gas += 150000;
            final_swap_amount
        } else {
            bridge_amount
        };

        Ok(UnifiedQuote {
            operation_type: OperationType::CrossChainSwap,
            from_chain_id,
            to_chain_id,
            token_in: token_in.to_string(),
            token_out: token_out.to_string(),
            amount_in: amount_in.to_string(),
            amount_out: final_amount,
            estimated_gas: total_gas.to_string(),
            execution_time_seconds: self.estimate_execution_time(&route_steps).await,
            route_steps,
            total_fees_usd: total_fees,
            price_impact: 0.5, // Estimated combined price impact
            confidence_score: 0.85, // Lower confidence for complex routes
        })
    }

    async fn execute_same_chain_swap(
        &self,
        _quote: &UnifiedQuote,
        _user_address: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        // Mock execution - in real implementation, this would interact with DEX contracts
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
        
        Ok(ExecutionResult {
            transaction_hash: format!("0x{:x}", now),
            status: ExecutionStatus::Success,
            gas_used: "21000".to_string(),
            actual_amount_out: "1000.0".to_string(),
            execution_time_ms: 3000,
            block_number: 18000000,
        })
    }

    async fn execute_cross_chain_bridge(
        &self,
        _quote: &UnifiedQuote,
        _user_address: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        // Mock execution - in real implementation, this would use bridge contracts
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
        
        Ok(ExecutionResult {
            transaction_hash: format!("0x{:x}", now),
            status: ExecutionStatus::Pending, // Bridge operations start as pending
            gas_used: "150000".to_string(),
            actual_amount_out: "995.0".to_string(), // After bridge fees
            execution_time_ms: 300000, // 5 minutes
            block_number: 18000001,
        })
    }

    async fn execute_cross_chain_swap(
        &self,
        _quote: &UnifiedQuote,
        _user_address: &str,
    ) -> Result<ExecutionResult, Box<dyn std::error::Error>> {
        // Mock execution - in real implementation, this would execute multiple steps
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;
        
        Ok(ExecutionResult {
            transaction_hash: format!("0x{:x}", now),
            status: ExecutionStatus::Pending, // Multi-step operations start as pending
            gas_used: "300000".to_string(),
            actual_amount_out: "990.0".to_string(), // After all fees
            execution_time_ms: 600000, // 10 minutes
            block_number: 18000002,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridges::BridgeManager;
    use crate::dexes::DexManager;

    #[tokio::test]
    async fn test_chain_abstractor_creation() {
        let abstractor = ChainAbstractor::new();
        
        // Verify chain configs are loaded
        let chains = abstractor.chains.read().await;
        assert_eq!(chains.len(), 4);
        assert!(abstractor.get_chain_config(1).await.is_some());
        assert!(abstractor.get_chain_config(137).await.is_some());
        assert!(abstractor.get_chain_config(42161).await.is_some());
    }

    #[tokio::test]
    async fn test_determine_operation_type() {
        let abstractor = ChainAbstractor::new();

        // Same chain swap
        let op_type = abstractor.determine_operation_type(1, 1, "USDC", "WETH").await;
        assert!(matches!(op_type, OperationType::SameChainSwap));

        // Cross-chain bridge (same token)
        let op_type = abstractor.determine_operation_type(1, 137, "USDC", "USDC").await;
        assert!(matches!(op_type, OperationType::CrossChainBridge));

        // Cross-chain swap (different tokens)
        let op_type = abstractor.determine_operation_type(1, 137, "USDC", "WETH").await;
        assert!(matches!(op_type, OperationType::CrossChainSwap));
    }

    #[tokio::test]
    async fn test_gas_price_management() {
        let abstractor = ChainAbstractor::new();

        // Initially no gas prices
        assert_eq!(abstractor.get_gas_price(1).await, None);

        // Update gas prices
        abstractor.update_gas_prices().await.unwrap();

        // Verify gas prices are set
        assert_eq!(abstractor.get_gas_price(1).await, Some(30.0));
        assert_eq!(abstractor.get_gas_price(137).await, Some(50.0));
        assert_eq!(abstractor.get_gas_price(42161).await, Some(0.1));
    }

    #[tokio::test]
    async fn test_estimate_execution_time() {
        let abstractor = ChainAbstractor::new();

        let route_steps = vec![
            RouteStep {
                step_type: StepType::Swap,
                chain_id: 1,
                protocol: "Uniswap".to_string(),
                token_in: "USDC".to_string(),
                token_out: "WETH".to_string(),
                amount_in: "1000".to_string(),
                amount_out: "0.3".to_string(),
                gas_estimate: "150000".to_string(),
            },
            RouteStep {
                step_type: StepType::Bridge,
                chain_id: 1,
                protocol: "Hop".to_string(),
                token_in: "WETH".to_string(),
                token_out: "WETH".to_string(),
                amount_in: "0.3".to_string(),
                amount_out: "0.299".to_string(),
                gas_estimate: "200000".to_string(),
            },
        ];

        let execution_time = abstractor.estimate_execution_time(&route_steps).await;
        
        // Should be > 0 and account for both swap and bridge times
        assert!(execution_time > 300); // At least 5 minutes for bridge
        assert!(execution_time < 1000); // But reasonable upper bound
    }

    #[tokio::test]
    async fn test_get_chain_config() {
        let abstractor = ChainAbstractor::new();

        // Test existing chain
        let eth_config = abstractor.get_chain_config(1).await.unwrap();
        assert_eq!(eth_config.name, "Ethereum");
        assert_eq!(eth_config.native_token, "ETH");
        assert_eq!(eth_config.block_time_ms, 12000);

        // Test non-existent chain
        assert!(abstractor.get_chain_config(999).await.is_none());
    }

    #[tokio::test]
    async fn test_execution_result_creation() {
        let abstractor = ChainAbstractor::new();

        let quote = UnifiedQuote {
            operation_type: OperationType::SameChainSwap,
            from_chain_id: 1,
            to_chain_id: 1,
            token_in: "USDC".to_string(),
            token_out: "WETH".to_string(),
            amount_in: "1000".to_string(),
            amount_out: "0.3".to_string(),
            estimated_gas: "150000".to_string(),
            execution_time_seconds: 60,
            route_steps: vec![],
            total_fees_usd: 5.0,
            price_impact: 0.1,
            confidence_score: 0.95,
        };

        let result = abstractor.execute_same_chain_swap(&quote, "0x123").await.unwrap();
        
        assert!(!result.transaction_hash.is_empty());
        assert!(matches!(result.status, ExecutionStatus::Success));
        assert_eq!(result.gas_used, "21000");
        assert!(result.execution_time_ms > 0);
        assert!(result.block_number > 0);
    }

    #[tokio::test]
    async fn test_operation_type_serialization() {
        // Test that our enums can be serialized/deserialized
        let op_type = OperationType::CrossChainSwap;
        let serialized = serde_json::to_string(&op_type).unwrap();
        let deserialized: OperationType = serde_json::from_str(&serialized).unwrap();
        
        assert!(matches!(deserialized, OperationType::CrossChainSwap));
    }

    #[tokio::test]
    async fn test_step_type_serialization() {
        let step_type = StepType::Bridge;
        let serialized = serde_json::to_string(&step_type).unwrap();
        let deserialized: StepType = serde_json::from_str(&serialized).unwrap();
        
        assert!(matches!(deserialized, StepType::Bridge));
    }

    #[tokio::test]
    async fn test_execution_status_serialization() {
        let status = ExecutionStatus::Pending;
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: ExecutionStatus = serde_json::from_str(&serialized).unwrap();
        
        assert!(matches!(deserialized, ExecutionStatus::Pending));
    }
}
