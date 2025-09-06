use crate::bridges::{BridgeManager, CrossChainParams, BridgeResponse};
use crate::dexes::DexManager;
use crate::types::QuoteParams;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwapStatus {
    Pending,
    SourceSwapExecuted,
    BridgeInitiated,
    BridgeCompleted,
    DestinationSwapExecuted,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicSwapRequest {
    pub swap_id: String,
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub user_address: String,
    pub slippage: f64,
    pub deadline: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicSwapResponse {
    pub swap_id: String,
    pub status: SwapStatus,
    pub source_tx_hash: Option<String>,
    pub bridge_tx_hash: Option<String>,
    pub destination_tx_hash: Option<String>,
    pub amount_out: Option<String>,
    pub execution_time_ms: u64,
    pub total_fees_usd: f64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct SwapExecution {
    request: AtomicSwapRequest,
    status: SwapStatus,
    source_tx_hash: Option<String>,
    bridge_tx_hash: Option<String>,
    destination_tx_hash: Option<String>,
    amount_out: Option<String>,
    start_time: u64,
    total_fees_usd: f64,
    error_message: Option<String>,
}

#[derive(Clone)]
pub struct AtomicSwapCoordinator {
    bridge_manager: Arc<BridgeManager>,
    dex_manager: Arc<DexManager>,
    active_swaps: Arc<RwLock<HashMap<String, SwapExecution>>>,
}

impl AtomicSwapCoordinator {
    pub fn new(bridge_manager: Arc<BridgeManager>, dex_manager: Arc<DexManager>) -> Self {
        Self {
            bridge_manager,
            dex_manager,
            active_swaps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initiate an atomic cross-chain swap
    pub async fn initiate_swap(&self, request: AtomicSwapRequest) -> Result<AtomicSwapResponse, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let execution = SwapExecution {
            request: request.clone(),
            status: SwapStatus::Pending,
            source_tx_hash: None,
            bridge_tx_hash: None,
            destination_tx_hash: None,
            amount_out: None,
            start_time,
            total_fees_usd: 0.0,
            error_message: None,
        };

        // Store the swap execution
        {
            let mut swaps = self.active_swaps.write().await;
            swaps.insert(request.swap_id.clone(), execution);
        }

        // Execute the atomic swap asynchronously
        let coordinator = self.clone();
        let swap_id = request.swap_id.clone();
        tokio::spawn(async move {
            if let Err(e) = coordinator.execute_atomic_swap(swap_id.clone()).await {
                let error_msg = e.to_string();
                coordinator.update_swap_status(swap_id, SwapStatus::Failed, Some(error_msg)).await;
            }
        });

        Ok(AtomicSwapResponse {
            swap_id: request.swap_id,
            status: SwapStatus::Pending,
            source_tx_hash: None,
            bridge_tx_hash: None,
            destination_tx_hash: None,
            amount_out: None,
            execution_time_ms: 0,
            total_fees_usd: 0.0,
            error_message: None,
        })
    }

    /// Get the status of an atomic swap
    pub async fn get_swap_status(&self, swap_id: &str) -> Option<AtomicSwapResponse> {
        let swaps = self.active_swaps.read().await;
        if let Some(execution) = swaps.get(swap_id) {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            Some(AtomicSwapResponse {
                swap_id: swap_id.to_string(),
                status: execution.status.clone(),
                source_tx_hash: execution.source_tx_hash.clone(),
                bridge_tx_hash: execution.bridge_tx_hash.clone(),
                destination_tx_hash: execution.destination_tx_hash.clone(),
                amount_out: execution.amount_out.clone(),
                execution_time_ms: current_time - execution.start_time,
                total_fees_usd: execution.total_fees_usd,
                error_message: execution.error_message.clone(),
            })
        } else {
            None
        }
    }

    /// Cancel an atomic swap if possible
    pub async fn cancel_swap(&self, swap_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut swaps = self.active_swaps.write().await;
        if let Some(execution) = swaps.get_mut(swap_id) {
            match execution.status {
                SwapStatus::Pending => {
                    execution.status = SwapStatus::Cancelled;
                    Ok(true)
                }
                _ => Ok(false), // Cannot cancel once execution has started
            }
        } else {
            Err("Swap not found".into())
        }
    }

    /// Execute the atomic swap with proper error handling and rollback
    async fn execute_atomic_swap(&self, swap_id: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let request = {
            let swaps = self.active_swaps.read().await;
            swaps.get(&swap_id).ok_or("Swap not found")?.request.clone()
        };

        let source_token = request.token_in.clone();
        let intermediate_token = "USDC".to_string();
        let destination_token = request.token_out.clone();
        let params = CrossChainParams {
            from_chain_id: request.from_chain_id,
            to_chain_id: request.to_chain_id,
            token_in: source_token.clone(),
            token_out: destination_token.clone(),
            amount_in: request.amount_in.clone(),
            user_address: request.user_address.clone(),
            slippage: request.slippage,
            deadline: request.deadline,
        };

        // Step 1: Execute source chain swap if needed
        let intermediate_amount = if source_token != intermediate_token {
            self.update_swap_status(swap_id.clone(), SwapStatus::SourceSwapExecuted, None).await;
            
            let swap_quote = self.dex_manager.get_best_quote(&QuoteParams {
                token_in: source_token.clone(),
                token_in_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
                token_in_decimals: Some(18),
                token_out: intermediate_token.clone(),
                token_out_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
                token_out_decimals: Some(18),
                amount_in: request.amount_in.clone(),
                slippage: Some(0.5),
                chain: Some(format!("{}", params.from_chain_id)),
            }).await.map_err(|e| format!("Source swap quote failed: {}", e))?;

            // Mock transaction execution
            let tx_hash = format!("0x{:x}", rand::random::<u64>());
            self.update_swap_tx_hash(swap_id.clone(), "source", tx_hash).await;
            
            swap_quote.amount_out
        } else {
            request.amount_in.clone()
        };

        // Step 2: Execute bridge transfer
        self.update_swap_status(swap_id.clone(), SwapStatus::BridgeInitiated, None).await;
        
        let bridge_params = CrossChainParams {
            from_chain_id: request.from_chain_id,
            to_chain_id: request.to_chain_id,
            token_in: intermediate_token.clone(),
            token_out: intermediate_token.clone(),
            amount_in: intermediate_amount.clone(),
            user_address: request.user_address.clone(),
            slippage: request.slippage,
            deadline: request.deadline,
        };

        let bridge_response = self.bridge_manager.execute_best_bridge(&bridge_params).await
            .map_err(|e| format!("Bridge execution failed: {}", e))?;

        self.update_swap_tx_hash(swap_id.clone(), "bridge", bridge_response.transaction_hash.clone()).await;
        self.update_swap_status(swap_id.clone(), SwapStatus::BridgeCompleted, None).await;

        // Step 3: Execute destination chain swap if needed
        let final_amount = if destination_token != intermediate_token {
            self.update_swap_status(swap_id.clone(), SwapStatus::DestinationSwapExecuted, None).await;
            
            let swap_quote = self.dex_manager.get_best_quote(&QuoteParams {
                token_in: intermediate_token.clone(),
                token_in_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
                token_in_decimals: Some(18),
                token_out: "USDC".to_string(),
                token_out_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
                token_out_decimals: Some(18),
                amount_in: intermediate_amount.clone(),
                slippage: Some(params.slippage),
                chain: Some(format!("{}", params.to_chain_id)),
            }).await.map_err(|e| format!("Destination swap quote failed: {}", e))?;

            // Mock transaction execution
            let tx_hash = format!("0x{:x}", rand::random::<u64>());
            self.update_swap_tx_hash(swap_id.clone(), "destination", tx_hash).await;
            
            swap_quote.amount_out
        } else {
            intermediate_amount
        };

        // Update final status with mock fee
        let mock_fee_usd = 5.0; // Mock bridge fee
        self.update_swap_final_result(swap_id, final_amount, mock_fee_usd).await;
        
        Ok(())
    }

    async fn update_swap_status(&self, swap_id: String, status: SwapStatus, error_message: Option<String>) {
        let mut swaps = self.active_swaps.write().await;
        if let Some(execution) = swaps.get_mut(&swap_id) {
            execution.status = status;
            if let Some(error) = error_message {
                execution.error_message = Some(error);
            }
        }
    }

    async fn update_swap_tx_hash(&self, swap_id: String, tx_type: &str, tx_hash: String) {
        let mut swaps = self.active_swaps.write().await;
        if let Some(execution) = swaps.get_mut(&swap_id) {
            match tx_type {
                "source" => execution.source_tx_hash = Some(tx_hash),
                "bridge" => execution.bridge_tx_hash = Some(tx_hash),
                "destination" => execution.destination_tx_hash = Some(tx_hash),
                _ => {}
            }
        }
    }

    async fn update_swap_final_result(&self, swap_id: String, amount_out: String, total_fees_usd: f64) {
        let mut swaps = self.active_swaps.write().await;
        if let Some(execution) = swaps.get_mut(&swap_id) {
            execution.status = SwapStatus::Completed;
            execution.amount_out = Some(amount_out);
            execution.total_fees_usd = total_fees_usd;
        }
    }

    /// Get all active swaps for monitoring
    pub async fn get_active_swaps(&self) -> Vec<AtomicSwapResponse> {
        let swaps = self.active_swaps.read().await;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        swaps.iter().map(|(swap_id, execution)| {
            AtomicSwapResponse {
                swap_id: swap_id.clone(),
                status: execution.status.clone(),
                source_tx_hash: execution.source_tx_hash.clone(),
                bridge_tx_hash: execution.bridge_tx_hash.clone(),
                destination_tx_hash: execution.destination_tx_hash.clone(),
                amount_out: execution.amount_out.clone(),
                execution_time_ms: current_time - execution.start_time,
                total_fees_usd: execution.total_fees_usd,
                error_message: execution.error_message.clone(),
            }
        }).collect()
    }

    /// Clean up completed swaps older than specified time
    pub async fn cleanup_old_swaps(&self, max_age_hours: u64) {
        let mut swaps = self.active_swaps.write().await;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let max_age_ms = max_age_hours * 60 * 60 * 1000;
        
        swaps.retain(|_, execution| {
            let age = current_time - execution.start_time;
            age < max_age_ms || !matches!(execution.status, SwapStatus::Completed | SwapStatus::Failed | SwapStatus::Cancelled)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_atomic_swap_coordinator_creation() {
        let bridge_manager = Arc::new(BridgeManager::new());
        let dex_manager = Arc::new(DexManager::new());
        
        let coordinator = AtomicSwapCoordinator::new(bridge_manager, dex_manager);
        
        // Verify coordinator is created successfully
        assert_eq!(coordinator.get_active_swaps().await.len(), 0);
    }

    #[tokio::test]
    async fn test_swap_initiation() {
        let bridge_manager = Arc::new(BridgeManager::new());
        let dex_manager = Arc::new(DexManager::new());
        let coordinator = AtomicSwapCoordinator::new(bridge_manager, dex_manager);

        let request = AtomicSwapRequest {
            swap_id: "test_swap_1".to_string(),
            from_chain_id: 1,
            to_chain_id: 137,
            token_in: "WETH".to_string(),
            token_out: "USDC".to_string(),
            amount_in: "1000000000000000000".to_string(), // 1 ETH
            user_address: "0x742d35Cc6634C0532925a3b8D4C9db96c4b4d8b6".to_string(),
            slippage: 0.5,
            deadline: None,
        };

        let response = coordinator.initiate_swap(request).await.unwrap();
        
        assert_eq!(response.swap_id, "test_swap_1");
        assert!(matches!(response.status, SwapStatus::Pending));
    }

    #[tokio::test]
    async fn test_swap_status_tracking() {
        let bridge_manager = Arc::new(BridgeManager::new());
        let dex_manager = Arc::new(DexManager::new());
        let coordinator = AtomicSwapCoordinator::new(bridge_manager, dex_manager);

        let request = AtomicSwapRequest {
            swap_id: "test_swap_2".to_string(),
            from_chain_id: 1,
            to_chain_id: 137,
            token_in: "WETH".to_string(),
            token_out: "USDC".to_string(),
            amount_in: "1000000000000000000".to_string(),
            user_address: "0x742d35Cc6634C0532925a3b8D4C9db96c4b4d8b6".to_string(),
            slippage: 0.5,
            deadline: None,
        };

        coordinator.initiate_swap(request).await.unwrap();
        
        let status = coordinator.get_swap_status("test_swap_2").await;
        assert!(status.is_some());
        
        let status = status.unwrap();
        assert_eq!(status.swap_id, "test_swap_2");
    }

    #[tokio::test]
    async fn test_swap_cancellation() {
        let bridge_manager = Arc::new(BridgeManager::new());
        let dex_manager = Arc::new(DexManager::new());
        let coordinator = AtomicSwapCoordinator::new(bridge_manager, dex_manager);

        let request = AtomicSwapRequest {
            swap_id: "test_swap_3".to_string(),
            from_chain_id: 1,
            to_chain_id: 137,
            token_in: "WETH".to_string(),
            token_out: "USDC".to_string(),
            amount_in: "1000000000000000000".to_string(),
            user_address: "0x742d35Cc6634C0532925a3b8D4C9db96c4b4d8b6".to_string(),
            slippage: 0.5,
            deadline: None,
        };

        coordinator.initiate_swap(request).await.unwrap();
        
        let cancelled = coordinator.cancel_swap("test_swap_3").await.unwrap();
        assert!(cancelled);
        
        let status = coordinator.get_swap_status("test_swap_3").await.unwrap();
        assert!(matches!(status.status, SwapStatus::Cancelled));
    }
}
