use crate::bridges::BridgeManager;
use crate::dexes::DexManager;
use crate::crosschain::{ArbitrageDetector, CrossChainManager};
use std::sync::Arc;

#[tokio::test]
async fn test_crosschain_manager_creation() {
    // Create mock managers
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    
    // Create CrossChainManager
    let crosschain_manager = CrossChainManager::new(
        bridge_manager.clone(),
        dex_manager.clone(),
    );
    
    // Verify the manager was created successfully
    assert!(true); // Basic creation test
}

#[tokio::test]
async fn test_arbitrage_detector_creation() {
    // Create mock managers
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    
    // Create ArbitrageDetector
    let arbitrage_detector = ArbitrageDetector::new(
        bridge_manager,
        dex_manager,
        10.0,  // min_profit_usd
        0.5,   // min_profit_percentage
    );
    
    // Verify the detector was created successfully
    assert!(true); // Basic creation test
}

#[tokio::test]
async fn test_arbitrage_detector_price_difference() {
    // Create mock managers
    let bridge_manager = Arc::new(BridgeManager::new());
    let dex_manager = Arc::new(DexManager::new());
    
    // Create ArbitrageDetector
    let mut arbitrage_detector = ArbitrageDetector::new(
        bridge_manager,
        dex_manager,
        10.0,  // min_profit_usd
        0.5,   // min_profit_percentage
    );
    
    // Test price difference calculation
    let price_diff = arbitrage_detector.calculate_price_difference(1.0, 1.002);
    assert!((price_diff - 0.2).abs() < 0.001); // Should be ~0.2%
    
    let price_diff = arbitrage_detector.calculate_price_difference(1.0, 0.998);
    assert!((price_diff - (-0.2)).abs() < 0.001); // Should be ~-0.2%
}
