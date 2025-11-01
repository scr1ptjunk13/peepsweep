# Token Database Implementation Plan

## 🎯 Architecture Decision: PHF + FxHashMap Hybrid

Based on comprehensive research of Rust performance patterns and DeFi application requirements, we're implementing a **Perfect Hash Function (PHF) + FxHashMap fallback** architecture for maximum performance.

### Research Findings
- **PHF**: O(1) lookup with zero collisions, fastest possible
- **FxHashMap**: 2-3x faster than std HashMap for string keys
- **Embedded data**: 80% faster initialization vs JSON parsing
- **Compile-time generation**: Moves work from runtime to build time

## 📊 Performance Targets

| Operation | Target Time | Current Baseline |
|-----------|-------------|------------------|
| Token lookup by address | < 0.01ms | N/A (hardcoded) |
| Fuzzy search (10 results) | < 1ms | N/A |
| Database initialization | < 5ms | N/A |
| Binary size increase | < 10MB | Current: ~2MB |

## 🏗️ Core Architecture

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: String,        // Lowercase hex address
    pub chain_id: u64,         // Chain identifier
    pub symbol: String,         // Token symbol (e.g., "ETH")
    pub name: String,           // Full name (e.g., "Ethereum")
    pub decimals: u8,           // Token decimals
    pub logo_uri: Option<String>, // Logo URL
    pub verified: bool,         // Verified by major lists
    pub market_cap_usd: Option<f64>, // Market cap for ranking
    pub price_usd: Option<f64>, // Current price
    pub tags: Vec<String>,      // Categories (stablecoin, defi, etc.)
}

pub struct TokenDatabase {
    // Core tokens (top 1000) - PHF for maximum speed
    core_tokens: phf::Map<&'static str, TokenInfo>,
    
    // Extended tokens - FxHashMap for good performance
    extended_tokens: FxHashMap<String, TokenInfo>,
    
    // Search indices for fuzzy matching
    symbol_index: FxHashMap<String, Vec<String>>, // symbol -> addresses
    name_index: FxHashMap<String, Vec<String>>,   // name -> addresses
    
    // Chain-specific lookups
    chain_tokens: FxHashMap<u64, Vec<String>>,    // chain_id -> addresses
    
    // Metadata
    last_updated: SystemTime,
    total_tokens: usize,
}
```

### Key Design Decisions

1. **Two-Tier Storage**:
   - **Tier 1 (PHF)**: Top 1000 tokens by market cap, embedded at compile time
   - **Tier 2 (FxHashMap)**: Extended tokens, loaded at runtime

2. **Composite Keys**: `(address, chain_id)` for unique identification

3. **Search Optimization**: Separate indices for symbol and name lookups

4. **Memory Efficiency**: String interning for repeated values

## 📦 Build System Architecture

### Compile-Time Generation (build.rs)

```rust
// build.rs workflow:
// 1. Download token lists from multiple sources
// 2. Merge and deduplicate tokens
// 3. Rank by market cap and verification status
// 4. Generate PHF map for top 1000 tokens
// 5. Generate fallback data structures
// 6. Embed in binary via include! macro

fn main() {
    // Data sources (in priority order)
    let sources = vec![
        TokenSource::Uniswap("https://tokens.uniswap.org"),
        TokenSource::OneInch("https://api.1inch.io/v4.0/1/tokens"),
        TokenSource::CoinGecko("https://tokens.coingecko.com/uniswap/all.json"),
        TokenSource::Compound("https://compound.finance/tokens"),
        TokenSource::Aave("https://aave.github.io/aave-addresses/mainnet.json"),
    ];
    
    let merged_tokens = download_and_merge_tokens(sources)?;
    let ranked_tokens = rank_tokens_by_importance(merged_tokens)?;
    
    // Generate core PHF map (top 1000)
    let core_tokens = select_core_tokens(ranked_tokens, 1000);
    generate_phf_map("core_tokens.rs", core_tokens)?;
    
    // Generate extended token data
    let extended_tokens = select_extended_tokens(ranked_tokens, 1000);
    generate_extended_data("extended_tokens.json", extended_tokens)?;
    
    println!("cargo:rerun-if-changed=build.rs");
}
```

### Token Ranking Algorithm

```rust
fn calculate_token_importance(token: &TokenInfo) -> f64 {
    let mut score = 0.0;
    
    // Market cap weight (40%)
    if let Some(market_cap) = token.market_cap_usd {
        score += (market_cap.log10() / 12.0) * 0.4; // Normalize to 0-1
    }
    
    // Verification weight (30%)
    if token.verified {
        score += 0.3;
    }
    
    // Volume weight (20%) - if available
    // Trading pair count weight (10%) - if available
    
    score
}
```

## 🔍 Search Implementation

### Fuzzy Search Algorithm

```rust
impl TokenDatabase {
    pub fn search(&self, query: &str, chain_id: Option<u64>, limit: usize) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        
        // 1. Exact symbol matches (score: 100)
        if let Some(addresses) = self.symbol_index.get(&query_lower) {
            for addr in addresses {
                if let Some(token) = self.get_token_by_address(addr, chain_id) {
                    results.push(SearchResult::new(token, 100));
                }
            }
        }
        
        // 2. Symbol prefix matches (score: 90)
        for (symbol, addresses) in &self.symbol_index {
            if symbol.starts_with(&query_lower) && symbol != &query_lower {
                for addr in addresses {
                    if let Some(token) = self.get_token_by_address(addr, chain_id) {
                        results.push(SearchResult::new(token, 90));
                    }
                }
            }
        }
        
        // 3. Name contains matches (score: 80)
        for (name, addresses) in &self.name_index {
            if name.contains(&query_lower) {
                for addr in addresses {
                    if let Some(token) = self.get_token_by_address(addr, chain_id) {
                        results.push(SearchResult::new(token, 80));
                    }
                }
            }
        }
        
        // 4. Address prefix matches (score: 70)
        if query_lower.starts_with("0x") && query_lower.len() >= 4 {
            // Search through all tokens for address matches
            // This is slower but necessary for address lookups
        }
        
        // Sort by score, then by market cap, then by verification
        results.sort_by(|a, b| {
            b.score.cmp(&a.score)
                .then_with(|| b.token.market_cap_usd.partial_cmp(&a.token.market_cap_usd).unwrap_or(Ordering::Equal))
                .then_with(|| b.token.verified.cmp(&a.token.verified))
        });
        
        results.truncate(limit);
        results
    }
}
```

## 🚀 Performance Optimizations

### Memory Layout Optimization

1. **String Interning**: Common strings (chain names, categories) stored once
2. **Compact Representation**: Use `u32` for indices where possible
3. **Cache-Friendly Layout**: Group related data together

### Lookup Optimization

1. **PHF for Hot Path**: Most common tokens use perfect hash
2. **FxHash for Cold Path**: Extended tokens use fast hash function
3. **Bloom Filter**: Quick negative lookups for non-existent tokens
4. **Prefix Trees**: For efficient prefix matching in search

### Build-Time Optimization

1. **Parallel Downloads**: Fetch token lists concurrently
2. **Incremental Updates**: Only rebuild if source data changes
3. **Compression**: Compress extended token data
4. **Caching**: Cache processed data between builds

## 📱 TUI Integration

### Token Selector Component

```rust
pub struct TokenSelector {
    database: Arc<TokenDatabase>,
    search_query: String,
    selected_chain: Option<u64>,
    search_results: Vec<SearchResult>,
    selected_index: usize,
    input_mode: bool,
}

impl TokenSelector {
    pub fn render(&self, f: &mut Frame, area: Rect) {
        // Search input box
        // Chain filter dropdown
        // Results list with:
        //   - Token symbol and name
        //   - Address (truncated)
        //   - Market cap and price
        //   - Verification badge
        //   - Chain icon
    }
    
    pub fn handle_input(&mut self, key: KeyEvent) -> Result<Option<TokenInfo>> {
        match key.code {
            KeyCode::Char(c) if self.input_mode => {
                self.search_query.push(c);
                self.update_search_results()?;
            }
            KeyCode::Backspace if self.input_mode => {
                self.search_query.pop();
                self.update_search_results()?;
            }
            KeyCode::Enter => {
                if let Some(result) = self.search_results.get(self.selected_index) {
                    return Ok(Some(result.token.clone()));
                }
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Tab => self.cycle_chain_filter(),
            _ => {}
        }
        Ok(None)
    }
}
```

### UI Design

```
┌─ Select Token ─────────────────────────────────────────────────┐
│ Search: eth█                          Chain: [Ethereum ▼]      │
├────────────────────────────────────────────────────────────────┤
│ Results (3 of 847):                                            │
│                                                                │
│ ► 🟢 ETH - Ethereum                                  $3,794.30 │
│   📍 0xC02a...756Cc2  💰 $456.2B  ⭐ Verified                 │
│                                                                │
│   🟢 WETH - Wrapped Ethereum                         $3,794.29 │
│   📍 0xC02a...756Cc2  💰 $12.1B   ⭐ Verified                 │
│                                                                │
│   🟡 SETH - Synth Ethereum                           $3,790.15 │
│   📍 0x5e74...31cb    💰 $45.2M    ⚠️  Unverified             │
│                                                                │
│ [↑↓] Navigate [Enter] Select [Tab] Chain [Esc] Cancel          │
└────────────────────────────────────────────────────────────────┘
```

## 🔄 Update Mechanism

### Background Updates

```rust
pub struct TokenUpdater {
    database: Arc<RwLock<TokenDatabase>>,
    update_interval: Duration,
    sources: Vec<TokenSource>,
}

impl TokenUpdater {
    pub async fn start_background_updates(&self) {
        let mut interval = tokio::time::interval(self.update_interval);
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.update_extended_tokens().await {
                eprintln!("Token update failed: {}", e);
            }
        }
    }
    
    async fn update_extended_tokens(&self) -> Result<()> {
        // 1. Download latest token lists
        let new_tokens = self.fetch_latest_tokens().await?;
        
        // 2. Merge with existing data
        let merged = self.merge_token_data(new_tokens).await?;
        
        // 3. Update database (write lock)
        {
            let mut db = self.database.write().await;
            db.update_extended_tokens(merged)?;
            db.rebuild_search_indices()?;
        }
        
        // 4. Persist to cache
        self.save_to_cache().await?;
        
        Ok(())
    }
}
```

## 📊 Implementation Phases

### Phase 1: Core Infrastructure (Week 1)
- [ ] Define TokenInfo struct and core types
- [ ] Implement basic TokenDatabase with HashMap
- [ ] Create build.rs for token list downloading
- [ ] Basic search functionality
- [ ] Unit tests for core functionality

### Phase 2: Performance Optimization (Week 2)
- [ ] Integrate PHF for core tokens
- [ ] Implement FxHashMap for extended tokens
- [ ] Add search indices and ranking
- [ ] Benchmark and optimize lookup performance
- [ ] Memory usage optimization

### Phase 3: TUI Integration (Week 3)
- [ ] Create TokenSelector component
- [ ] Implement search UI with real-time updates
- [ ] Add chain filtering and sorting
- [ ] Integration with existing swap interface
- [ ] User experience testing

### Phase 4: Production Features (Week 4)
- [ ] Background update mechanism
- [ ] Error handling and fallbacks
- [ ] Logging and metrics
- [ ] Documentation and examples
- [ ] Performance benchmarking suite

## 🎯 Success Metrics

### Performance Benchmarks
- Token lookup: < 0.01ms (target: 0.001ms)
- Search 10 results: < 1ms (target: 0.5ms)
- Database init: < 5ms (target: 2ms)
- Memory usage: < 50MB (target: 30MB)

### User Experience
- Zero network delays for common tokens
- Sub-second search response time
- Intuitive keyboard navigation
- Comprehensive token coverage (>10k tokens)

### Technical Quality
- 100% test coverage for core functions
- Zero panics in production code
- Graceful degradation on network issues
- Efficient memory usage patterns

## 🔧 Dependencies

### Required Crates
```toml
[dependencies]
phf = { version = "0.11", features = ["macros"] }
rustc-hash = "1.1"  # FxHashMap
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
anyhow = "1.0"
thiserror = "1.0"

[build-dependencies]
phf_codegen = "0.11"
reqwest = { version = "0.11", features = ["json", "blocking"] }
serde_json = "1.0"
anyhow = "1.0"
```

### External Data Sources
- Uniswap Token Lists (primary)
- 1inch API (secondary)
- CoinGecko Lists (tertiary)
- Compound/Aave Protocol Lists (DeFi focus)

This implementation plan provides a comprehensive, performance-optimized token database that will give PeepSweep a significant competitive advantage in token discovery and lookup speed.
