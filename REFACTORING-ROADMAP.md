# 🔄 Refactoring Roadmap: Current Codebase → Modular Architecture

## 🎯 **Migration Strategy Overview**

Based on the memories showing your codebase is **already compiling successfully**, we can implement the modular position fetcher architecture incrementally without breaking existing functionality.

## 📊 **Current State Analysis**

### **✅ What's Already Working**
- **Zero compilation errors** - Clean Rust codebase
- **Existing Uniswap V3 position fetching** - Working logic to extract
- **Database layer** - SQLx with proper type handling
- **API endpoints** - Axum-based REST API
- **Cache system** - Redis integration
- **IL calculations** - Basic impermanent loss math

### **🔧 What Needs Refactoring**
- **Monolithic position fetching** - Currently Uniswap-specific
- **Hardcoded contract addresses** - Need config-driven approach
- **Single-protocol IL calculations** - Need SIMD batch processing
- **Direct database queries in API** - Need orchestrator pattern

---

## 🗺️ **Phase-by-Phase Migration Plan**

### **Phase 1: Extract & Modularize (Week 1)**

#### **Step 1.1: Create Configuration System**
```bash
# Create new directories
mkdir -p backend/configs/protocols
mkdir -p backend/src/fetchers
```

**Files to Create:**
1. `backend/configs/protocols/uniswap_v3.yaml` - Extract current Uniswap logic
2. `backend/src/fetchers/mod.rs` - Module definitions
3. `backend/src/fetchers/config_parser.rs` - YAML config loading

#### **Step 1.2: Extract Current Uniswap Logic**
**Current Location**: Likely in `src/indexer/` or `src/api/positions.rs`

**Action**: Move existing Uniswap V3 position fetching logic into:
- `backend/configs/protocols/uniswap_v3.yaml` (configuration)
- `backend/src/fetchers/uniswap_v3_legacy.rs` (temporary wrapper)

#### **Step 1.3: Create Generic Fetcher Foundation**
```rust
// backend/src/fetchers/generic_fetcher.rs
pub struct GenericFetcher {
    configs: HashMap<String, ProtocolConfig>,
    // Keep existing RPC provider for now
    rpc_provider: Arc<dyn EthereumProvider>, 
}

impl GenericFetcher {
    // Start with just Uniswap V3 support
    pub async fn fetch_uniswap_v3_positions(
        &self,
        chain_id: u32,
        user_address: Address,
    ) -> anyhow::Result<Vec<Position>> {
        // Use existing logic but config-driven
    }
}
```

### **Phase 2: Implement Core Architecture (Week 2)**

#### **Step 2.1: Build Full Generic Fetcher**
- Implement `fetch_positions_for_protocol()` method
- Add support for NFT ownership detection
- Add support for ERC20 balance detection
- Integrate with existing cache system

#### **Step 2.2: Create Position Orchestrator**
```rust
// backend/src/fetchers/orchestrator.rs
pub struct PositionOrchestrator {
    generic_fetcher: GenericFetcher,
    cache: Arc<CacheManager>, // Use existing cache
    database: Arc<DatabaseManager>, // Use existing DB
}
```

#### **Step 2.3: Update API Layer**
**Modify**: `backend/src/api/positions.rs`
```rust
// Replace direct database queries with orchestrator calls
pub async fn get_positions(
    // ... existing parameters
) -> Result<Json<PositionResponse>, ApiError> {
    // OLD: Direct database query
    // let positions = database.get_user_positions(address).await?;
    
    // NEW: Use orchestrator
    let summary = app_state.position_orchestrator
        .get_user_positions(chain_id, user_address)
        .await?;
    
    // Convert to existing response format
    Ok(Json(PositionResponse::from(summary)))
}
```

### **Phase 3: Add Protocol Configs (Week 3)**

#### **Step 3.1: Add Uniswap V2 Support**
- Create `backend/configs/protocols/uniswap_v2.yaml`
- Test with existing V2 positions (if any)
- Verify zero code changes needed

#### **Step 3.2: Add SushiSwap Support**
- Create `backend/configs/protocols/sushiswap.yaml`
- Copy V2 config structure
- Test with SushiSwap positions

#### **Step 3.3: Validate Scalability**
- Add 2-3 more protocols via config only
- Measure performance impact
- Verify cache effectiveness

### **Phase 4: SIMD Optimization (Week 4)**

#### **Step 4.1: Create SIMD IL Engine**
```rust
// backend/src/calculations/simd.rs
pub struct SIMDCalculationEngine {
    // AVX2 optimized IL calculations
}
```

#### **Step 4.2: Integrate SIMD with Generic Fetcher**
- Replace existing IL calculations
- Implement batch processing
- Add performance benchmarks

#### **Step 4.3: IL Shield MVP Implementation**
- Create dedicated endpoint for Uniswap V3 whales
- Implement real-time IL alerts
- Add whale address detection

---

## 📁 **File-by-File Migration Guide**

### **Files to Modify**

#### **1. `backend/src/lib.rs`**
```rust
// ADD: New modules
pub mod fetchers;

// KEEP: Existing modules
pub mod api;
pub mod cache;
pub mod calculations;
// ... rest unchanged
```

#### **2. `backend/src/main.rs`**
```rust
// ADD: Initialize orchestrator
use crate::fetchers::orchestrator::PositionOrchestrator;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... existing initialization
    
    // NEW: Initialize position orchestrator
    let position_orchestrator = Arc::new(
        PositionOrchestrator::new(database.clone(), cache.clone()).await?
    );
    
    let app_state = AppState {
        database,
        cache,
        position_orchestrator, // ADD this field
    };
    
    // ... rest unchanged
}
```

#### **3. `backend/src/api/positions.rs`**
**Strategy**: Gradual replacement
1. Keep existing endpoints working
2. Add new orchestrator-based endpoints
3. Gradually migrate traffic
4. Remove old endpoints

```rust
// KEEP: Existing get_positions() for backward compatibility
pub async fn get_positions_legacy(/* ... */) -> Result<Json<PositionResponse>, ApiError> {
    // Existing implementation
}

// ADD: New orchestrator-based endpoint
pub async fn get_positions(/* ... */) -> Result<Json<PositionResponse>, ApiError> {
    // New implementation using orchestrator
}
```

### **Files to Create**

#### **1. Configuration Files**
- `backend/configs/protocols/uniswap_v3.yaml`
- `backend/configs/protocols/uniswap_v2.yaml`
- `backend/configs/protocols/sushiswap.yaml`

#### **2. Fetcher Modules**
- `backend/src/fetchers/mod.rs`
- `backend/src/fetchers/generic_fetcher.rs`
- `backend/src/fetchers/orchestrator.rs`
- `backend/src/fetchers/config_parser.rs`

#### **3. Enhanced Calculations**
- `backend/src/calculations/simd.rs`

---

## 🧪 **Testing Strategy**

### **Phase 1 Testing**
```bash
# Test config loading
cargo test test_config_parser

# Test generic fetcher with Uniswap V3
cargo test test_uniswap_v3_fetching

# Verify existing API still works
curl http://localhost:3000/api/positions/0x123...
```

### **Phase 2 Testing**
```bash
# Test orchestrator
cargo test test_position_orchestrator

# Test new API endpoints
curl http://localhost:3000/api/v2/positions/0x123...

# Performance benchmarks
cargo bench position_fetching
```

### **Phase 3 Testing**
```bash
# Test multiple protocols
cargo test test_multi_protocol_fetching

# Test config-driven scaling
cargo test test_add_new_protocol

# Integration tests
cargo test test_full_position_pipeline
```

### **Phase 4 Testing**
```bash
# SIMD performance tests
cargo bench simd_il_calculations

# IL Shield MVP tests
cargo test test_whale_detection
cargo test test_il_alerts

# Load testing
wrk -t12 -c400 -d30s http://localhost:3000/api/positions/whale_address
```

---

## 📈 **Success Metrics**

### **Performance Targets**
- **Position fetch time**: <100ms for single protocol
- **Multi-protocol fetch**: <500ms for 10 protocols
- **SIMD IL calculation**: 8x faster than current
- **Cache hit rate**: >80% for repeated requests

### **Scalability Targets**
- **Add new protocol**: <5 minutes (config only)
- **Support 50+ protocols**: Zero code changes
- **Handle 1000+ concurrent users**: Sub-second response times

### **IL Shield MVP Targets**
- **Whale detection**: Top 1000 Uniswap V3 LPs
- **IL prediction accuracy**: >85% confidence
- **Alert latency**: <30 seconds from price change

---

## 🚨 **Risk Mitigation**

### **Backward Compatibility**
- Keep existing API endpoints during migration
- Gradual traffic migration with feature flags
- Rollback plan for each phase

### **Performance Monitoring**
- Add metrics for each phase
- Monitor memory usage during SIMD implementation
- Database query performance tracking

### **Data Integrity**
- Compare old vs new position data
- Validate IL calculations match existing results
- Audit trail for configuration changes

---

## 🎯 **Next Immediate Actions**

1. **Create directory structure** for new modules
2. **Extract current Uniswap V3 logic** into config format
3. **Implement basic GenericFetcher** with Uniswap V3 support
4. **Test side-by-side** with existing implementation
5. **Gradually migrate API endpoints** to use orchestrator

This roadmap maintains your working codebase while systematically building the modular architecture that enables infinite protocol scaling.
