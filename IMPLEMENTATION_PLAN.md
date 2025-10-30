# PeepSweep Gas/Slippage/Price Impact Implementation Plan

## 🚨 CRITICAL REALITY CHECK (DO THIS FIRST)

### **5-Minute RPC Test - MANDATORY**
```rust
// Add to main.rs temporarily - TEST BEFORE BUILDING ANYTHING
#[tokio::main]
async fn main() {
    let provider = ProviderCache::new().get_provider("ethereum").await.unwrap();
    
    let tx = TransactionRequest::default()
        .from(Address::ZERO)
        .to(Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap()) // Uniswap V2 Router
        .data(Bytes::from_static(&[0x38, 0xed, 0x17, 0x39])); // swapExactTokensForTokens selector
    
    let start = Instant::now();
    match provider.estimate_gas(&tx).await {
        Ok(gas) => println!("✅ Gas estimation works: {} gas in {:?}", gas, start.elapsed()),
        Err(e) => println!("❌ BLOCKED: Fix this first: {:?}", e),
    }
}
```
**If this prints ❌ → Fix RPC setup before implementing anything**
**If this prints ✅ → Continue with implementation**

### **Performance Reality**
- **10 DEXes × 200ms `eth_estimateGas` = 2s minimum**
- **Current response: 500ms**
- **Target: <1s is mathematically impossible without aggressive caching**
- **Accept 1-2s response time or cache heavily (30s TTL)**

## 🔍 CURRENT CODEBASE ANALYSIS

### **Architecture Overview**
```
peepsweep/backend/src/
├── aggregator.rs           # Main DEX aggregator with hardcoded values
├── types.rs               # Core types (QuoteParams, RouteBreakdown, etc.)
├── dexes/
│   ├── mod.rs             # DexIntegration trait
│   ├── utils/
│   │   ├── dex_utils.rs   # Universal utilities (GOOD foundation)
│   │   ├── provider_cache.rs # RPC provider management (EXCELLENT)
│   │   └── dex_template.rs # Template for new DEXes
│   ├── uniswap_v2.rs      # Hardcoded gas: "150000"
│   ├── uniswap_v3.rs      # Hardcoded gas: "180000"
│   ├── sushiswap.rs       # Hardcoded gas: "150000"
│   └── [other DEXes]      # All with hardcoded values
├── tui/
│   ├── app.rs             # Hardcoded slippage: 0.5%, price_impact: 0.2%
│   └── ui.rs              # Display layer
└── main.rs                # TUI entry point
```

### **Current Problems (CRITICAL)**
1. **Gas Estimation**: All DEXes return hardcoded strings (`"150000"`, `"180000"`)
2. **Slippage Calculation**: Hardcoded `0.5%` in `tui/app.rs:281`
3. **Price Impact**: Hardcoded `0.2%` in `tui/app.rs:282`
4. **No Real-Time Calculation**: Everything is static

### **Current Strengths (LEVERAGE)**
1. **✅ Excellent Provider Cache**: Multi-RPC with health monitoring
2. **✅ Universal DexUtils**: Safe amount parsing, address resolution
3. **✅ Modular DEX Architecture**: Clean trait-based system
4. **✅ Concurrent Quote Fetching**: Already implemented with timeouts
5. **✅ Circuit Breakers**: Failure handling for DEXes

---

## 🚀 LEAN IMPLEMENTATION PLAN (REVISED)

### **Week 1: Gas Estimation Only (Lean Approach)**

#### **1.1 Single File Implementation**
```rust
// File: src/gas.rs (NEW - Single file, ~200 lines)
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;
use dashmap::DashMap;
use std::time::{Duration, Instant};

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

    pub async fn estimate(&self, tx: &TransactionRequest, chain: &str) -> Result<u64, DexError> {
        // 1. Check cache
        let cache_key = format!("{}:{:?}", chain, tx.to);
        if let Some((gas, time)) = self.cache.get(&cache_key) {
            if time.elapsed() < Duration::from_secs(30) {
                return Ok(*gas);
            }
        }
        
        // 2. Simulate with timeout
        let provider = self.provider_cache.get_provider(chain).await?;
        let estimated = tokio::time::timeout(
            Duration::from_millis(500), // 500ms timeout per call
            provider.estimate_gas(tx)
        ).await
            .map_err(|_| DexError::Timeout("Gas estimation timeout".into()))?
            .map_err(|e| DexError::ContractCallFailed(e.to_string()))?
            .as_u64();
        
        // 3. Add EIP-114 buffer (3% + base gas)
        let buffered = (estimated * 103) / 100 + 21000;
        
        // 4. Cache and return
        self.cache.insert(cache_key, (buffered, Instant::now()));
        Ok(buffered)
    }
}
```

#### **1.2 Minimal DEX Trait Changes**
```rust
// File: src/dexes/mod.rs (MODIFY - Add ONE method)
#[async_trait]
pub trait DexIntegration: Send + Sync {
    // ... existing methods ...
    
    // NEW: Optional - only implement if you can build transaction easily
    async fn build_transaction(&self, params: &QuoteParams) -> Result<TransactionRequest, DexError> {
        Err(DexError::NotImplemented("build_transaction not implemented".into()))
    }
}
```

#### **1.3 Minimal Aggregator Changes**
```rust
// File: src/aggregator.rs (MODIFY - Only add gas estimation)
impl DEXAggregator {
    pub async fn get_quotes(&self, params: QuoteParams) -> Result<QuoteResponse> {
        let gas_estimator = GasEstimator::new(self.provider_cache.clone());
        
        // Your existing concurrent quote fetching...
        for dex in &self.dexes {
            let base_quote = dex.get_quote(&params).await?;
            
            // NEW: Real gas with fallback
            let gas = if let Ok(tx) = dex.build_transaction(&params).await {
                gas_estimator.estimate(&tx, &params.chain).await
                    .unwrap_or(150000) // Fallback to old value
            } else {
                150000 // Fallback if build_transaction not implemented
            };
            
            quotes.push(RouteBreakdown {
                gas_used: gas.to_string(),
                // Keep old hardcoded values for now
                ...base_quote
            });
        }
    }
}
```

**Test with 5 DEXes. If it works → continue. If not → fix before Week 2.**

---

### **Week 2: Smart Price Impact (Hybrid Approach)**

#### **2.1 Single File Implementation**
```rust
// File: src/price_impact.rs (NEW - Single file, ~150 lines)

// For Uniswap V2 style (covers 80% of DEXes)
pub fn calculate_v2_impact(amount_in: U256, reserve_in: U256, reserve_out: U256) -> f64 {
    let amount_in_with_fee = (amount_in * U256::from(997)) / U256::from(1000);
    let numerator = amount_in_with_fee * reserve_out;
    let denominator = reserve_in + amount_in_with_fee;
    let amount_out = numerator / denominator;
    
    let price_before = reserve_out.as_u128() as f64 / reserve_in.as_u128() as f64;
    let price_after = (reserve_out - amount_out).as_u128() as f64 / (reserve_in + amount_in).as_u128() as f64;
    
    ((price_before - price_after) / price_before * 100.0).abs()
}

// For Uniswap V3 (if IQuoterV2 exists)
pub async fn calculate_v3_impact(params: &QuoteParams, provider: &Provider) -> Result<f64, DexError> {
    // Check if IQuoterV2 exists on this chain
    let quoter_address = match params.chain.as_deref().unwrap_or("ethereum") {
        "ethereum" => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
        "arbitrum" => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
        "optimism" => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
        "polygon" => "0x61fFE014bA17989E743c5F6cB21bF9697530B21e",
        "base" => "0x3d4e44Eb1374240CE5F1B871ab261CD16335B76a", // Different on Base
        _ => return Err(DexError::UnsupportedChain("No IQuoterV2 for this chain".into())),
    };
    
    let quoter = IQuoterV2::new(Address::from_str(quoter_address)?, provider);
    let result = quoter.quoteExactInputSingle(...).await?;
    // Calculate from sqrtPriceX96After
    Ok(calculated_impact)
}

// Smart dispatcher
pub async fn calculate_price_impact(dex_name: &str, params: &QuoteParams, provider: &Provider) -> Result<f64, DexError> {
    match dex_name {
        "UniswapV2" | "SushiSwapV2" | "PancakeSwapV2" => {
            // Get reserves and use V2 formula
            if let Ok(reserves) = get_v2_reserves(params, provider).await {
                let amount_in = DexUtils::parse_amount_safe(&params.amount_in, 
                    params.token_in_decimals.unwrap_or(18))?;
                Ok(calculate_v2_impact(amount_in, reserves.0, reserves.1))
            } else {
                Ok(0.2) // Fallback to old hardcoded value
            }
        }
        "UniswapV3" => {
            // Try V3 precise calculation, fallback to V2 formula
            if let Ok(impact) = calculate_v3_impact(params, provider).await {
                Ok(impact)
            } else {
                // Fallback to V2 calculation
                if let Ok(reserves) = get_v2_reserves(params, provider).await {
                    let amount_in = DexUtils::parse_amount_safe(&params.amount_in, 
                        params.token_in_decimals.unwrap_or(18))?;
                    Ok(calculate_v2_impact(amount_in, reserves.0, reserves.1))
                } else {
                    Ok(0.2) // Final fallback
                }
            }
        }
        _ => {
            // Default V2 formula for unknown DEXes
            if let Ok(reserves) = get_v2_reserves(params, provider).await {
                let amount_in = DexUtils::parse_amount_safe(&params.amount_in, 
                    params.token_in_decimals.unwrap_or(18))?;
                Ok(calculate_v2_impact(amount_in, reserves.0, reserves.1))
            } else {
                Ok(0.2) // Fallback
            }
        }
    }
}
```

#### **2.2 Basic Slippage Estimation**
```rust
// File: src/slippage.rs (NEW - Single file, ~100 lines)
pub fn calculate_slippage_estimate(
    price_impact: f64,
    current_gas_price: u64,
    pool_liquidity_usd: f64,
    mempool_congestion: f64
) -> f64 {
    let base_slippage = 0.1; // Minimum 0.1%
    
    // Liquidity factor: less liquid = more slippage
    let liquidity_factor = if pool_liquidity_usd < 100000.0 { 
        2.0 
    } else if pool_liquidity_usd < 1000000.0 { 
        1.5 
    } else { 
        1.0 
    };
    
    // Gas price factor: high gas = more competition = more slippage
    let gas_factor = if current_gas_price > 50 { 1.5 } else { 1.0 };
    
    // Mempool congestion factor
    let congestion_factor = 1.0 + (mempool_congestion * 0.01);
    
    // Price impact contributes to slippage
    let impact_factor = 1.0 + (price_impact * 0.1);
    
    base_slippage * liquidity_factor * gas_factor * congestion_factor * impact_factor
}
```

#### **2.3 Add to DEX Trait**
```rust
// File: src/dexes/mod.rs (MODIFY - Add ONE method)
#[async_trait]
pub trait DexIntegration: Send + Sync {
    // ... existing methods ...
    
    // NEW: Optional - only implement if you can get reserves easily
    async fn get_reserves(&self, params: &QuoteParams) -> Result<(U256, U256), DexError> {
        Err(DexError::NotImplemented("get_reserves not implemented".into()))
    }
}
```

---

### **Week 3: Integration & Testing**

#### **3.1 Enhanced Aggregator with All Features**
```rust
// File: src/aggregator.rs (MAJOR UPDATE)
impl DEXAggregator {
    pub async fn get_optimal_route(&self, params: QuoteParams) -> Result<QuoteResponse, AggregatorError> {
        let start = Instant::now();
        
        // Initialize calculators
        let gas_estimator = GasEstimator::new(self.provider_cache.clone());
        
        // Concurrent quote fetching with enhanced data
        let enhanced_futures = self.create_enhanced_quote_futures(&params, &gas_estimator).await;
        
        let timeout_duration = Duration::from_millis(2000); // Accept 2s response time
        let results = tokio::time::timeout(timeout_duration, join_all(enhanced_futures)).await
            .map_err(|_| AggregatorError::AllDexesFailed)?;
        
        let mut enhanced_quotes = Vec::new();
        
        for result in results {
            match result {
                Ok((dex_name, enhanced_quote_result)) => {
                    match enhanced_quote_result {
                        Ok(enhanced_quote) => {
                            enhanced_quotes.push(enhanced_quote);
                            info!("✅ {} enhanced quote successful", dex_name);
                        }
                        Err(e) => {
                            warn!("❌ {} enhanced quote failed: {:?}", dex_name, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("❌ Task join failed: {:?}", e);
                }
            }
        }
        
        if enhanced_quotes.is_empty() {
            return Err(AggregatorError::NoValidRoutes);
        }
        
        // Sort by gas-adjusted price (best value)
        enhanced_quotes.sort_by(|a, b| {
            let a_adjusted = a.calculate_gas_adjusted_price();
            let b_adjusted = b.calculate_gas_adjusted_price();
            b_adjusted.partial_cmp(&a_adjusted).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        let response_time = start.elapsed().as_millis();
        
        Ok(QuoteResponse {
            amount_out: enhanced_quotes[0].amount_out.clone(),
            response_time,
            routes: enhanced_quotes.into_iter().map(|eq| eq.into_route_breakdown()).collect(),
            price_impact: enhanced_quotes[0].price_impact,
            gas_estimate: enhanced_quotes[0].gas_estimate.to_string(),
            savings: self.calculate_savings(&enhanced_quotes),
        })
    }
    
    async fn create_enhanced_quote_futures(
        &self,
        params: &QuoteParams,
        gas_estimator: &GasEstimator,
    ) -> Vec<tokio::task::JoinHandle<(String, Result<EnhancedQuote, AggregatorError>)>> {
        let mut futures = Vec::new();
        
        for dex in &self.dexes {
            let dex_name = dex.get_name().to_string();
            let params_clone = params.clone();
            let dex_clone = dex.clone_box();
            let gas_estimator_clone = gas_estimator.clone();
            
            let future = tokio::task::spawn(async move {
                // 1. Get base quote (amount_out)
                let base_quote = dex_clone.get_quote(&params_clone).await?;
                
                // 2. Calculate real gas estimate with fallback
                let gas_estimate = if let Ok(tx) = dex_clone.build_transaction(&params_clone).await {
                    gas_estimator_clone.estimate(&tx, 
                        params_clone.chain.as_deref().unwrap_or("ethereum")).await
                        .unwrap_or(150000) // Fallback
                } else {
                    150000 // Fallback if build_transaction not implemented
                };
                
                // 3. Calculate real price impact with fallback
                let price_impact = if let Ok(provider) = gas_estimator_clone.provider_cache.get_provider(
                    params_clone.chain.as_deref().unwrap_or("ethereum")).await {
                    calculate_price_impact(&dex_name, &params_clone, &provider).await
                        .unwrap_or(0.2) // Fallback
                } else {
                    0.2 // Fallback
                };
                
                // 4. Calculate slippage estimate
                let slippage = calculate_slippage_estimate(
                    price_impact,
                    30, // Default gas price
                    1000000.0, // Default pool liquidity
                    0.5 // Default mempool congestion
                );
                
                Ok(EnhancedQuote {
                    dex_name: dex_name.clone(),
                    amount_out: base_quote.amount_out,
                    gas_estimate,
                    slippage,
                    price_impact,
                })
            });
            
            futures.push(future);
        }
        
        futures
    }
}

#[derive(Debug, Clone)]
pub struct EnhancedQuote {
    pub dex_name: String,
    pub amount_out: String,
    pub gas_estimate: u64,
    pub slippage: f64,
    pub price_impact: f64,
}

impl EnhancedQuote {
    /// Calculate gas-adjusted price (output amount - gas cost in tokens)
    pub fn calculate_gas_adjusted_price(&self) -> f64 {
        let output_amount: f64 = self.amount_out.parse().unwrap_or(0.0);
        let gas_cost_usd = (self.gas_estimate as f64) * 30.0 * 0.000000001 * 2500.0; // Assume $2500 ETH, 30 gwei
        let gas_cost_tokens = gas_cost_usd / (output_amount / 1000.0); // Rough conversion
        
        output_amount - gas_cost_tokens
    }
    
    pub fn into_route_breakdown(self) -> RouteBreakdown {
        RouteBreakdown {
            dex: self.dex_name,
            percentage: 100.0,
            amount_out: self.amount_out,
            gas_used: self.gas_estimate.to_string(),
        }
    }
}
```

---

## 📊 LEAN IMPLEMENTATION TIMELINE (REVISED)

| Week | Focus | Files | Deliverables | Accuracy Improvement |
|------|-------|-------|--------------|---------------------|
| **Week 1** | Gas Estimation | 1 file (`gas.rs`) | Real `eth_estimateGas` with fallbacks | 60% → 90%+ |
| **Week 2** | Price Impact | 2 files (`price_impact.rs`, `slippage.rs`) | V2/V3 calculations with fallbacks | Static → Dynamic |
| **Week 3** | Integration | Update `aggregator.rs` | Complete enhanced quotes | Full system |

## 🎯 SUCCESS METRICS (REALISTIC)

### **Before Implementation**
- Gas Accuracy: ~60% (hardcoded values)
- Slippage: Static 0.5%
- Price Impact: Static 0.2%
- Response Time: <500ms

### **After Implementation**
- Gas Accuracy: 90%+ (simulation-based with fallbacks)
- Slippage: Dynamic 0.1-2.0% (based on price impact + liquidity)
- Price Impact: Real-time calculation (V2 formula + V3 where available)
- Response Time: 1-2s (accept reality of RPC latency)

## 🔧 TECHNICAL REQUIREMENTS

### **New Dependencies**
```toml
# Add to Cargo.toml
[dependencies]
# Already have dashmap for caching
dashmap = "6.0"

# For IQuoterV2 integration (if needed)
alloy-sol-types = "0.7"
alloy-contract = "0.1"
```

### **File Structure Changes (MINIMAL)**
```
src/
├── gas.rs                 # NEW: Universal gas estimation (~200 lines)
├── price_impact.rs        # NEW: Smart price impact calculation (~150 lines)
├── slippage.rs            # NEW: Slippage estimation (~100 lines)
├── aggregator.rs          # MODIFY: Enhanced quote fetching
├── dexes/mod.rs           # MODIFY: Add 2 optional trait methods
└── [existing files]       # MINIMAL CHANGES
```

## 🚨 CRITICAL IMPLEMENTATION NOTES

1. **RPC Test First**: Run the 5-minute test before building anything
2. **Graceful Fallbacks**: Always fallback to old hardcoded values if calculations fail
3. **Performance Acceptance**: Accept 1-2s response time for real calculations
4. **Caching Strategy**: Cache gas (30s), price impact (15s), slippage (10s)
5. **Contract Verification**: Check IQuoterV2 addresses per chain before using

## ✅ WHAT'S GOOD ABOUT THIS LEAN APPROACH

- **3 files vs 10+ files** from original plan
- **Works with existing code** - minimal changes
- **Graceful fallbacks** - if simulation fails, use old values
- **Fast iteration** - test each week independently
- **Realistic performance** - accepts RPC latency constraints

## ❌ WHAT WE'RE CUTTING (FOR NOW)

- ❌ Separate slippage calculators per DEX → Use universal formula
- ❌ Complex trait hierarchies → Simple functions
- ❌ MEV protection → Add later if needed
- ❌ Tenderly state overrides → Overkill for v1
- ❌ Sub-1s response time → Accept 1-2s reality

**This revised plan balances precision improvements with implementation reality. Start with Week 1 gas estimation and iterate based on results.**

