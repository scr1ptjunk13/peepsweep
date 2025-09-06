// mod.rs - FIXED VERSION (Remove #[instrument] macros)

pub mod flashbots;
pub mod mempool;
pub mod sandwich_detector;
pub mod slippage_manager;
pub mod time_delays;

pub use flashbots::FlashbotsProtect;
pub use crate::types::{SwapParams, SwapResponse};
use crate::mev_protection::sandwich_detector::SandwichDetector;
use crate::mev_protection::slippage_manager::DynamicSlippageManager;
use crate::mev_protection::time_delays::TimeBasedDelayManager;
use crate::mev_protection::mempool::PrivateMempoolRouter;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error}; // REMOVED instrument import
use thiserror::Error;
use async_trait::async_trait;

#[derive(Error, Debug)]
pub enum MevProtectionError {
    #[error("Flashbots relay error: {0}")]
    FlashbotsError(String),
    #[error("Private mempool unavailable: {0}")]
    MempoolError(String),
    #[error("Sandwich attack detected: {0}")]
    SandwichDetected(String),
    #[error("Slippage adjustment failed: {0}")]
    SlippageError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("MEV protection is disabled")]
    Disabled,
}

#[async_trait]
pub trait MevProtection: Send + Sync {
    async fn protect_swap(&self, params: &SwapParams) -> Result<SwapResponse, MevProtectionError>;
    async fn is_enabled(&self) -> bool;
    fn get_protection_type(&self) -> &'static str;
}

/// Comprehensive MEV protection suite
pub struct MevProtectionSuite {
    pub flashbots: FlashbotsProtect,
    pub mempool_router: PrivateMempoolRouter,
    pub sandwich_detector: Arc<tokio::sync::Mutex<SandwichDetector>>,
    pub slippage_manager: DynamicSlippageManager,
    pub delay_manager: TimeBasedDelayManager,
    pub enabled: bool,
}

impl MevProtectionSuite {
    pub async fn new() -> Result<Self, MevProtectionError> {
        info!("🔄 Initializing MEV Protection Suite");
        Ok(Self {
            flashbots: FlashbotsProtect::new().await?,
            mempool_router: PrivateMempoolRouter::new().await?,
            sandwich_detector: Arc::new(tokio::sync::Mutex::new(SandwichDetector::new().await)),
            slippage_manager: DynamicSlippageManager::new().await,
            delay_manager: TimeBasedDelayManager::new(100, 2000), // 100ms - 2s delays
            enabled: true,
        })
    }

    /// Protect a transaction from MEV attacks - FIXED (No #[instrument])
    pub async fn protect_transaction(&self, params: &SwapParams) -> Result<SwapResponse, MevProtectionError> {
        println!("🛡️ PROTECT_TRANSACTION ENTRY: Starting MEV protection for {}->{}", params.token_in, params.token_out);
        info!("🛡️ PROTECT_TRANSACTION ENTRY: Starting MEV protection for {}->{}", params.token_in, params.token_out);
        
        if !self.enabled {
            warn!("MEV protection is disabled");
            return Err(MevProtectionError::Disabled);
        }

        println!("🔍 Step 1: Analyzing transaction for sandwich attacks");
        info!("🔍 Step 1: Analyzing transaction for sandwich attacks");
        // 1. Check for sandwich attacks
        match self.sandwich_detector.lock().await.analyze_transaction(params).await {
            Ok(_) => {
                println!("✅ Sandwich attack analysis passed");
                info!("✅ Sandwich attack analysis passed");
            },
            Err(e) => {
                println!("❌ Sandwich attack analysis failed: {:?}", e);
                error!("❌ Sandwich attack analysis failed: {:?}", e);
                return Err(MevProtectionError::SandwichDetected(e.to_string()));
            }
        }

        println!("⚙️ Step 2: Adjusting slippage dynamically");
        info!("⚙️ Step 2: Adjusting slippage dynamically");
        // 2. Adjust slippage dynamically
        let protected_params = match self.slippage_manager.adjust_slippage(params).await {
            Ok(params) => {
                println!("✅ Slippage adjustment completed");
                info!("✅ Slippage adjustment completed");
                params
            },
            Err(e) => {
                println!("❌ Slippage adjustment failed: {:?}", e);
                error!("❌ Slippage adjustment failed: {:?}", e);
                return Err(e);
            }
        };

        println!("⏳ Step 3: Applying time-based execution delays");
        info!("⏳ Step 3: Applying time-based execution delays");
        // 3. Apply time-based delays
        match self.delay_manager.apply_delay(&protected_params).await {
            Ok(_) => {
                println!("✅ Time-based delay completed");
                info!("✅ Time-based delay completed");
            },
            Err(e) => {
                println!("❌ Time-based delay failed: {:?}", e);
                error!("❌ Time-based delay failed: {:?}", e);
                return Err(e);
            }
        }

        println!("🛡️ Step 4: Routing through Flashbots Protect");
        info!("🛡️ Step 4: Routing through Flashbots Protect");
        // 3. Route through Flashbots Protect
        match self.flashbots.protect_swap(&protected_params).await {
            Ok(response) => {
                println!("✅ Flashbots protection completed successfully");
                info!("✅ Flashbots protection completed successfully");
                Ok(response)
            },
            Err(e) => {
                println!("❌ Flashbots protection failed: {:?}", e);
                error!("❌ Flashbots protection failed: {:?}", e);
                Err(e)
            }
        }
    }

    pub async fn enable(&mut self) {
        self.enabled = true;
        info!("MEV protection enabled");
    }

    pub async fn disable(&mut self) {
        self.enabled = false;
        warn!("MEV protection disabled");
    }
}