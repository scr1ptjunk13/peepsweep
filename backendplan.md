# Complete File Architecture & Interconnections Guide

## 🏗️ Architecture Overview

The codebase follows a modular, domain-driven design where each module has a specific responsibility. Data flows from blockchain events → indexer → database → cache → API → client.

---

## 📁 File-by-File Breakdown & Interconnections

### **Core Entry Points**

#### `src/main.rs` ✅ *[PROVIDED]*
**Purpose:** Application bootstrap and orchestration
**What it does:**
- Initializes all services (database, Redis, indexer, cache)
- Sets up Axum web server with middleware stack
- Starts background services (event streaming, price updates, IL snapshots)
- Handles graceful shutdown
**Connections:**
- Imports all modules and creates AppState
- Spawns background tasks from indexer and cache modules
- Routes HTTP requests to api module

#### `src/lib.rs` ❌ *[MISSING]*
**Purpose:** Shared types, utilities, and re-exports
**What it needs:**
- Common error types (ApiError, DatabaseError, IndexerError)
- Shared structs (Position, TokenInfo, PriceData)
- Utility functions (address validation, number formatting)
- Re-export main types for external crates
**Connections:**
- Used by ALL other modules for shared types
- Imported by main.rs for error handling
- Contains trait definitions used across modules

---

### **🔌 API Layer**

#### `src/api/mod.rs` ❌ *[MISSING]*
**Purpose:** API module orchestration and shared middleware
**What it needs:**
- Re-exports from positions.rs and calculations.rs
- Shared API middleware (authentication, validation, error handling)
- Common response types (ApiResponse<T>, ErrorResponse)
- Request validation utilities
**Connections:**
- Imported by main.rs to get route handlers
- Uses lib.rs for shared types
- Coordinates between positions.rs and calculations.rs

#### `src/api/positions.rs` ✅ *[PROVIDED]*
**Purpose:** Position-related API endpoints
**What it does:**
- GET /positions/:address - fetch user positions with caching
- GET /positions/:address/history - historical position data
- Parallel IL calculation and aggregation
**Connections:**
- Uses database/queries.rs for data fetching
- Uses calculations/impermanent_loss.rs for IL computation
- Uses cache/strategies.rs for multi-layer caching

#### `src/api/calculations.rs` ❌ *[MISSING]*
**Purpose:** Calculation-specific API endpoints
**What it needs:**
- POST /calculate/il - manual IL calculation endpoint
- GET /calculate/fees/:pool - fee calculation for specific pools
- POST /calculate/batch - batch calculation for multiple positions
- Real-time calculation endpoints for frontend widgets
**Connections:**
- Uses calculations/* modules for all math operations
- Uses database/queries.rs for historical data
- Uses cache/mod.rs for caching expensive calculations

---

### **🗄️ Database Layer**

#### `src/database/mod.rs` ❌ *[MISSING]*
**Purpose:** Database connection management and utilities
**What it needs:**
- PostgreSQL connection pool setup and configuration
- Migration runner integration
- Transaction management utilities
- Health check functions for database connectivity
**Connections:**
- Used by main.rs for database initialization
- Provides connection pool to indexer and API modules
- Coordinates with models.rs and queries.rs

#### `src/database/migrations.sql` ✅ *[PROVIDED]*
**Purpose:** Database schema and optimizations
**What it does:**
- Creates partitioned tables for positions_v2 and positions_v3
- Sets up materialized views for fast queries
- Creates optimized indexes for user lookups

#### `src/database/models.rs` ❌ *[MISSING]*
**Purpose:** Database model structs and conversions
**What it needs:**
- Position structs that map to database rows
- Conversion functions between database types and API types
- Validation logic for database constraints
- Helper methods for complex queries
**Connections:**
- Used by queries.rs for type-safe database operations
- Used by api/positions.rs for data serialization
- Imports shared types from lib.rs

#### `src/database/queries.rs` ❌ *[MISSING]*
**Purpose:** Optimized SQL queries and database operations
**What it needs:**
- get_user_positions() - fetch positions from materialized view
- insert_position_batch() - efficient bulk inserts
- update_position_il() - update IL calculations
- get_position_history() - time-series queries
- Complex aggregation queries for analytics
**Connections:**
- Uses models.rs for type definitions
- Called by api/positions.rs and indexer/processor.rs
- Uses sqlx for query execution

---

### **📊 Cache Layer**

#### `src/cache/mod.rs` ❌ *[MISSING]*
**Purpose:** Cache management and coordination
**What it needs:**
- CacheManager struct that coordinates all caching strategies
- Generic cache interface for different data types
- Cache invalidation logic and TTL management
- Health monitoring for Redis connections
**Connections:**
- Used by main.rs to initialize cache system
- Coordinates between strategies.rs and Redis
- Used by API layer for all caching operations

#### `src/cache/strategies.rs` ❌ *[MISSING]*
**Purpose:** Specific caching strategies and policies
**What it needs:**
- L1 (in-memory) cache using moka crate
- L2 (Redis) cache with serialization
- Cache-aside pattern implementation
- Different TTL strategies for positions vs prices
- Cache warming for popular addresses
**Connections:**
- Used by api/positions.rs for multi-layer caching
- Uses lib.rs for shared data types
- Coordinates with cache/mod.rs for management

---

### **⚡ Indexer Layer**

#### `src/indexer/mod.rs` ❌ *[MISSING]*
**Purpose:** Event indexer orchestration and coordination
**What it needs:**
- EventIndexer struct that manages all indexing operations
- Coordination between streaming, processing, and backfilling
- Error handling and retry logic for failed events
- Metrics collection for indexing performance
**Connections:**
- Used by main.rs to start background indexing
- Coordinates stream.rs, processor.rs, and backfill.rs
- Uses database/mod.rs for data persistence

#### `src/indexer/events.rs` ❌ *[MISSING]*
**Purpose:** Event definitions and ABI decoding
**What it needs:**
- Uniswap V2 event definitions (Mint, Burn, Swap, Transfer)
- Uniswap V3 event definitions (Mint, Burn, Swap, IncreaseLiquidity, etc.)
- ABI encoding/decoding utilities using alloy
- Event filtering and validation logic
**Connections:**
- Used by stream.rs for event filtering
- Used by processor.rs for event decoding
- Defines data structures used throughout indexer

#### `src/indexer/stream.rs` ✅ *[PROVIDED]*
**Purpose:** Real-time event streaming from blockchain
**What it does:**
- WebSocket connections to multiple RPC providers
- Event filtering for Uniswap V2/V3 contracts
- Failover logic for RPC provider redundancy
- Parallel processing of different event types

#### `src/indexer/processor.rs` ❌ *[MISSING]*
**Purpose:** Event processing and database updates
**What it needs:**
- decode_and_process_v2_event() - processes V2 Mint/Burn/Transfer events
- decode_and_process_v3_event() - processes V3 position changes
- update_position_in_db() - atomic database updates
- handle_position_changes() - calculates position deltas
- Error handling for malformed events
**Connections:**
- Called by stream.rs when new events arrive
- Uses events.rs for event decoding
- Uses database/queries.rs for data persistence
- Uses calculations/mod.rs for position calculations

#### `src/indexer/backfill.rs` ❌ *[MISSING]*
**Purpose:** Historical data backfilling for new users
**What it needs:**
- backfill_user_positions() - fetch historical positions for address
- batch_fetch_events() - efficient bulk event fetching
- progress_tracking() - track backfill progress
- rate_limiting() - avoid hitting RPC rate limits
**Connections:**
- Called by api/positions.rs when user not found in cache
- Uses events.rs for event definitions
- Uses processor.rs for event processing logic
- Uses database/queries.rs for data storage

---

### **🧮 Calculations Layer**

#### `src/calculations/mod.rs` ❌ *[MISSING]*
**Purpose:** Calculation module coordination and utilities
**What it needs:**
- Re-exports from impermanent_loss.rs, fees.rs, pricing.rs
- Common calculation utilities (decimal handling, rounding)
- Error types specific to calculations (InvalidPosition, PriceNotFound)
- Validation for calculation inputs
**Connections:**
- Used by api/calculations.rs and api/positions.rs
- Coordinates between all calculation submodules
- Imports shared types from lib.rs

#### `src/calculations/impermanent_loss.rs` ✅ *[PROVIDED]*
**Purpose:** Advanced IL calculation engine
**What it does:**
- V2 and V3 IL calculations with concentrated liquidity
- Risk scoring and breakeven analysis
- Batch processing for multiple positions
- Historical IL tracking

#### `src/calculations/fees.rs` ❌ *[MISSING]*
**Purpose:** Fee calculation and tracking
**What it needs:**
- calculate_fees_earned() - compute total fees from LP positions
- estimate_future_fees() - project fee earnings based on volume
- fee_apr_calculation() - annual percentage rate for fees
- compare_fee_tiers() - V3 fee tier analysis (0.05%, 0.3%, 1%)
**Connections:**
- Used by impermanent_loss.rs for net profit calculations
- Uses pricing.rs for USD conversions
- Called by api/positions.rs for fee display

#### `src/calculations/pricing.rs` ❌ *[MISSING]*
**Purpose:** Token pricing and price feed management
**What it needs:**
- get_token_price() - fetch current token price with caching
- get_historical_prices() - time-series price data
- calculate_twap() - time-weighted average price
- price_impact_calculation() - estimate slippage for large positions
- Integration with Chainlink oracles and CoinGecko API
**Connections:**
- Used by ALL calculation modules for price data
- Uses cache/strategies.rs for price caching
- Called by indexer/processor.rs for real-time updates

---

### **⚙️ Configuration**

#### `src/config/mod.rs` ✅ *[PROVIDED]*
**Purpose:** Application configuration management
**What it does:**
- Environment variable parsing and validation
- Database, Redis, and RPC URL configuration
- Rate limiting and caching configuration
- Monitoring and logging setup

---

## 🔄 Data Flow & Interconnections

### **Real-time Event Processing Flow:**
```
Blockchain Events → indexer/stream.rs → indexer/events.rs → indexer/processor.rs → database/queries.rs → cache invalidation
```

### **API Request Flow:**
```
HTTP Request → api/positions.rs → cache/strategies.rs (L1/L2 check) → database/queries.rs → calculations/impermanent_loss.rs → Response
```

### **Background Services Flow:**
```
main.rs spawns:
├── indexer/mod.rs (event streaming)
├── cache/mod.rs (price updates)
└── calculations/mod.rs (IL snapshots)
```

---

## 🎯 Implementation Priority

### **Phase 1 (Days 1-3): Core Foundation**
1. `lib.rs` - shared types and errors
2. `database/mod.rs` - connection management
3. `database/models.rs` - basic structs
4. `indexer/events.rs` - event definitions

### **Phase 2 (Days 4-6): Basic Indexing**
5. `indexer/mod.rs` - coordinator
6. `indexer/processor.rs` - V2 processing only
7. `database/queries.rs` - basic CRUD operations
8. `calculations/pricing.rs` - simple price fetching

### **Phase 3 (Days 7-8): API Layer**
9. `api/mod.rs` - basic middleware
10. `cache/mod.rs` - simple caching
11. `cache/strategies.rs` - L1 cache only
12. `calculations/fees.rs` - basic fee calculation

### **Phase 4 (Days 9-10): Advanced Features**
13. `api/calculations.rs` - calculation endpoints
14. `indexer/backfill.rs` - historical data
15. `calculations/mod.rs` - advanced utilities

---

## 🔗 Key Dependencies Between Files

**Critical Path:** `lib.rs` → `database/models.rs` → `database/queries.rs` → `api/positions.rs`

**Indexer Chain:** `indexer/events.rs` → `indexer/processor.rs` → `database/queries.rs`

**Calculation Chain:** `calculations/pricing.rs` → `calculations/fees.rs` → `calculations/impermanent_loss.rs`

**Cache Chain:** `cache/mod.rs` → `cache/strategies.rs` → `api/positions.rs`

---

## 💡 Pro Tips for Implementation

1. **Start with lib.rs** - defines contracts for all other modules
2. **Keep database/models.rs simple** - focus on core Position struct first
3. **Mock expensive operations** - use dummy data for calculations initially
4. **Implement caching last** - get basic functionality working first
5. **Use feature flags** - enable V3 support after V2 is stable

This modular architecture ensures each file has a single responsibility while maintaining clear data flow and minimal coupling between components.