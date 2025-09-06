use bralaladex_backend::crosschain::ArbitrageDetector;
use bralaladex_backend::bridges::BridgeManager;
use bralaladex_backend::dexes::DexManager;
use std::sync::Arc;

#[tokio::test]
async fn test_arbitrage_detector_creation() {
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    
    let detector = ArbitrageDetector::new(
        bridge_manager,
        dex_manager,
    );
    
    // Test price difference calculation
    let price_diff = detector.calculate_price_difference(1.0, 1.002);
    assert!((price_diff - 0.2).abs() < 0.001);
    
    let price_diff = detector.calculate_price_difference(1.0, 0.998);
    assert!((price_diff - (-0.2)).abs() < 0.001);
}

#[tokio::test]
async fn test_crosschain_manager_integration() {
    use bralaladex_backend::crosschain::CrossChainManager;
    
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    
    let _manager = CrossChainManager::new(bridge_manager, dex_manager);
    
    // Basic integration test - just verify creation works
    assert!(true);
}
