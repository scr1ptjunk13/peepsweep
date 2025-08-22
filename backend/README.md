# PeepSweep Backend

High-performance Rust backend for DeFi portfolio tracking and impermanent loss calculations.

## Features

- **Real-time Event Indexing** - Tracks Uniswap V3 positions using alloy-rs
- **Impermanent Loss Calculations** - Accurate IL tracking with fee compensation
- **Redis Caching** - Optimized performance with intelligent caching strategies
- **PostgreSQL Database** - Robust data storage with optimized queries
- **REST API** - Clean endpoints for frontend integration

## Architecture

```
backend/
├── src/
│   ├── main.rs                 # Axum server entry point
│   ├── lib.rs                  # Shared types and utilities
│   ├── config/                 # Environment configuration
│   ├── indexer/                # Event indexing & processing
│   ├── database/               # PostgreSQL models & queries
│   ├── api/                    # REST API endpoints
│   ├── calculations/           # IL & fee calculation engines
│   └── cache/                  # Redis caching layer
└── Cargo.toml                  # Dependencies (alloy-rs, axum, sqlx)
```

## Quick Start

1. **Setup Environment**
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

2. **Install Dependencies**
   ```bash
   cargo build
   ```

3. **Setup Database**
   ```bash
   # Run PostgreSQL and create database
   createdb peepsweep
   ```

4. **Run Server**
   ```bash
   cargo run
   ```

## API Endpoints

- `GET /api/v1/positions/:address` - Get user positions
- `GET /api/v1/calculations/il/:position_id` - Calculate impermanent loss
- `GET /api/v1/calculations/fees/:position_id` - Calculate fees earned

## Dependencies

- **alloy-rs** - Ethereum client (no ethers.js)
- **axum** - Web framework
- **sqlx** - PostgreSQL driver
- **redis** - Caching layer
- **tokio** - Async runtime

## Performance Features

- Debounced ENS resolution (300ms)
- 5-minute position caching
- Optimized SQL queries with indexes
- Real-time event streaming
- Batch historical backfill
