use bralaladex_backend::crosschain::chain_abstractor::{
    ChainAbstractor, OperationType, StepType, ExecutionStatus, UnifiedQuote, RouteStep
};
use bralaladex_backend::bridges::BridgeManager;
use bralaladex_backend::dexes::DexManager;
use std::sync::Arc;

#[tokio::test]
async fn test_chain_abstractor_creation() {
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    
    let abstractor = ChainAbstractor::new(bridge_manager, dex_manager);
    
    // Verify chain configs are loaded
    assert!(abstractor.get_chain_config(1).is_some());
    assert!(abstractor.get_chain_config(137).is_some());
    assert!(abstractor.get_chain_config(42161).is_some());
}

#[tokio::test]
async fn test_gas_price_management() {
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    let abstractor = ChainAbstractor::new(bridge_manager, dex_manager);

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
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    let abstractor = ChainAbstractor::new(bridge_manager, dex_manager);

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

    let execution_time = abstractor.estimate_execution_time(&route_steps);
    
    // Should be > 0 and account for both swap and bridge times
    assert!(execution_time > 300); // At least 5 minutes for bridge
    assert!(execution_time < 1000); // But reasonable upper bound
}

#[tokio::test]
async fn test_get_chain_config() {
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    let abstractor = ChainAbstractor::new(bridge_manager, dex_manager);

    // Test existing chain
    let eth_config = abstractor.get_chain_config(1).unwrap();
    assert_eq!(eth_config.name, "Ethereum");
    assert_eq!(eth_config.native_token, "ETH");
    assert_eq!(eth_config.block_time_ms, 12000);

    // Test non-existent chain
    assert!(abstractor.get_chain_config(999).is_none());
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
