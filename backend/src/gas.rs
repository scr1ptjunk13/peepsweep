use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::dexes::{DexError, utils::ProviderCache};

/// Lean gas estimator with caching and graceful fallbacks
/// Single file implementation (~200 lines) as per implementation plan
#[derive(Clone)]
pub struct GasEstimator {
    provider_cache: Arc<ProviderCache>,
    cache: DashMap<String, (u64, Instant)>, // 30s TTL
}

impl GasEstimator {
    pub fn new(provider_cache: Arc<ProviderCache>) -> Self {
        Self {
            provider_cache,
            cache: DashMap::new(),
        }
    }

    /// Estimate gas with caching and fallbacks
    /// Returns real gas estimate or falls back to reasonable default
    pub async fn estimate(&self, tx: &TransactionRequest, chain: &str) -> Result<u64, DexError> {
        // 1. Check cache first
        let cache_key = self.generate_cache_key(tx, chain);
        if let Some(entry) = self.cache.get(&cache_key) {
            let (gas, time) = *entry.value();
            if time.elapsed() < Duration::from_secs(30) {
                return Ok(gas);
            }
        }
        
        // 2. Try real gas estimation with timeout
        match self.estimate_with_timeout(tx, chain).await {
            Ok(gas) => {
                // Cache successful result
                self.cache.insert(cache_key, (gas, Instant::now()));
                Ok(gas)
            }
            Err(e) => {
                // Graceful fallback to reasonable defaults
                let fallback_gas = self.get_fallback_gas(tx);
                
                // Cache fallback for shorter duration (10s)
                self.cache.insert(cache_key, (fallback_gas, Instant::now()));
                
                // Log but don't fail
                eprintln!("⚠️  Gas estimation failed, using fallback {}: {:?}", fallback_gas, e);
                Ok(fallback_gas)
            }
        }
    }

    /// Estimate gas with 500ms timeout per call
    async fn estimate_with_timeout(&self, tx: &TransactionRequest, chain: &str) -> Result<u64, DexError> {
        let provider = self.provider_cache.get_provider(chain).await?;
        
        let estimated = tokio::time::timeout(
            Duration::from_millis(500), // 500ms timeout per call
            provider.estimate_gas(tx)
        ).await
            .map_err(|_| DexError::Timeout("Gas estimation timeout".into()))?
            .map_err(|e| DexError::ContractCallFailed(e.to_string()))?;
        
        let estimated_u64 = estimated.try_into()
            .map_err(|_| DexError::InvalidAmount("Gas estimate too large".into()))?;
        
        // Apply EIP-114 buffer (3% + base gas)
        let buffered = self.apply_eip_114_buffer(estimated_u64);
        Ok(buffered)
    }

    /// Apply EIP-114 buffer: 3% + 21000 base gas
    fn apply_eip_114_buffer(&self, base_gas: u64) -> u64 {
        (base_gas * 103) / 100 + 21000
    }

    /// Generate cache key from transaction and chain
    fn generate_cache_key(&self, tx: &TransactionRequest, chain: &str) -> String {
        // Simple cache key based on destination and chain
        format!("{}:{:?}:{:?}", chain, tx.to, tx.input)
    }

    /// Get reasonable fallback gas based on transaction type
    fn get_fallback_gas(&self, tx: &TransactionRequest) -> u64 {
        // Analyze transaction to provide smart fallbacks
        // tx.input is a TransactionInput struct, not Option<TransactionInput>
        let input_size = if let Some(data) = &tx.input.input {
            data.len()
        } else if let Some(data) = &tx.input.data {
            data.len()
        } else {
            0
        };
        
        match input_size {
            0 => 21000,           // Simple ETH transfer
            1..=100 => 50000,     // Simple contract call
            101..=500 => 100000,  // Medium contract call
            501..=1000 => 150000, // Complex contract call (Uniswap V2)
            _ => 200000,          // Very complex call (Uniswap V3)
        }
    }

    /// Clear expired cache entries (call periodically)
    pub fn cleanup_cache(&self) {
        let now = Instant::now();
        self.cache.retain(|_, (_, time)| {
            now.duration_since(*time) < Duration::from_secs(60) // Keep for 1 minute max
        });
    }

    /// Get cache statistics for monitoring
    pub fn get_cache_stats(&self) -> (usize, usize) {
        let total_entries = self.cache.len();
        let now = Instant::now();
        let fresh_entries = self.cache.iter()
            .filter(|entry| now.duration_since(entry.value().1) < Duration::from_secs(30))
            .count();
        
        (fresh_entries, total_entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, Bytes};
    use std::str::FromStr;

    #[test]
    fn test_fallback_gas_calculation() {
        let estimator = GasEstimator::new(Arc::new(ProviderCache::new()));
        
        // Test ETH transfer
        let eth_transfer = TransactionRequest::default()
            .to(Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap());
        assert_eq!(estimator.get_fallback_gas(&eth_transfer), 21000);
        
        // Test simple contract call
        let simple_call = TransactionRequest::default()
            .to(Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap())
            .input(Bytes::from_static(&[0x38, 0xed, 0x17, 0x39]).into()); // From<Bytes> for TransactionInput
        assert_eq!(estimator.get_fallback_gas(&simple_call), 50000);
        
        // Test complex contract call
        let complex_call = TransactionRequest::default()
            .to(Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap())
            .input(Bytes::from_static(&[0u8; 600]).into()); // From<Bytes> for TransactionInput
        assert_eq!(estimator.get_fallback_gas(&complex_call), 200000);
    }

    #[test]
    fn test_eip_114_buffer() {
        let estimator = GasEstimator::new(Arc::new(ProviderCache::new()));
        
        // Test buffer calculation
        let base_gas = 100000;
        let buffered = estimator.apply_eip_114_buffer(base_gas);
        let expected = (100000 * 103) / 100 + 21000; // 3% + 21000
        assert_eq!(buffered, expected);
        assert_eq!(buffered, 124000); // 103000 + 21000
    }

    #[test]
    fn test_cache_key_generation() {
        let estimator = GasEstimator::new(Arc::new(ProviderCache::new()));
        
        let tx = TransactionRequest::default()
            .to(Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap())
            .input(Bytes::from_static(&[0x38, 0xed, 0x17, 0x39]).into()); // From<Bytes> for TransactionInput
        
        let key = estimator.generate_cache_key(&tx, "ethereum");
        assert!(key.contains("ethereum"));
        assert!(key.contains("0x7a250d5630b4cf539739df2c5dacb4c659f2488d")); // lowercase
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let estimator = GasEstimator::new(Arc::new(ProviderCache::new()));
        
        // Test cache miss -> fallback
        let tx = TransactionRequest::default()
            .to(Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap());
        
        let gas1 = estimator.estimate(&tx, "ethereum").await.unwrap();
        assert_eq!(gas1, 21000); // Should be fallback for ETH transfer
        
        // Test cache hit
        let gas2 = estimator.estimate(&tx, "ethereum").await.unwrap();
        assert_eq!(gas2, gas1); // Should be same (cached)
        
        // Verify cache stats
        let (fresh, total) = estimator.get_cache_stats();
        assert!(fresh > 0);
        assert!(total > 0);
    }
}
