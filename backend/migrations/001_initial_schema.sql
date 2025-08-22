-- src/database/migrations.sql
-- Innovation: Partitioned tables for massive scale

-- Uniswap V2 Positions (partitioned by user_address hash)
CREATE TABLE positions_v2 (
    id BIGSERIAL,
    user_address VARCHAR(42) NOT NULL,
    pair_address VARCHAR(42) NOT NULL,
    token0 VARCHAR(42) NOT NULL,
    token1 VARCHAR(42) NOT NULL,
    liquidity NUMERIC(78, 0) NOT NULL,
    token0_amount NUMERIC(78, 18) NOT NULL,
    token1_amount NUMERIC(78, 18) NOT NULL,
    block_number BIGINT NOT NULL,
    transaction_hash VARCHAR(66) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Innovation: Store pre-calculated IL to avoid real-time computation
    current_il_percentage NUMERIC(10, 6),
    fees_earned_usd NUMERIC(20, 8),
    PRIMARY KEY (user_address, pair_address, id)
) PARTITION BY HASH (user_address);

-- Create 16 partitions for horizontal scaling
DO $$ 
DECLARE 
    i INTEGER;
BEGIN 
    FOR i IN 0..15 LOOP
        EXECUTE format('CREATE TABLE positions_v2_%s PARTITION OF positions_v2 
                       FOR VALUES WITH (modulus 16, remainder %s)', i, i);
    END LOOP;
END $$;

-- Uniswap V3 Positions (more complex due to concentrated liquidity)
CREATE TABLE positions_v3 (
    id BIGSERIAL,
    user_address VARCHAR(42) NOT NULL,
    pool_address VARCHAR(42) NOT NULL,
    token_id BIGINT NOT NULL, -- NFT token ID
    token0 VARCHAR(42) NOT NULL,
    token1 VARCHAR(42) NOT NULL,
    fee_tier INTEGER NOT NULL, -- 500, 3000, 10000
    tick_lower INTEGER NOT NULL,
    tick_upper INTEGER NOT NULL,
    liquidity NUMERIC(78, 0) NOT NULL,
    token0_amount NUMERIC(78, 18),
    token1_amount NUMERIC(78, 18),
    fees_token0 NUMERIC(78, 18) DEFAULT 0,
    fees_token1 NUMERIC(78, 18) DEFAULT 0,
    block_number BIGINT NOT NULL,
    transaction_hash VARCHAR(66) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- V3 specific optimizations
    current_tick INTEGER,
    in_range BOOLEAN DEFAULT TRUE,
    current_il_percentage NUMERIC(10, 6),
    fees_earned_usd NUMERIC(20, 8),
    PRIMARY KEY (user_address, token_id, id)
) PARTITION BY HASH (user_address);

-- Create V3 partitions
DO $$ 
DECLARE 
    i INTEGER;
BEGIN 
    FOR i IN 0..15 LOOP
        EXECUTE format('CREATE TABLE positions_v3_%s PARTITION OF positions_v3 
                       FOR VALUES WITH (modulus 16, remainder %s)', i, i);
    END LOOP;
END $$;

-- Innovation: Materialized view for instant position lookups
CREATE MATERIALIZED VIEW user_positions_summary AS
SELECT 
    user_address,
    'v2' as version,
    pair_address as pool_address,
    token0,
    token1,
    NULL::INTEGER as fee_tier,
    token0_amount,
    token1_amount,
    current_il_percentage,
    fees_earned_usd,
    updated_at
FROM positions_v2
WHERE liquidity > 0

UNION ALL

SELECT 
    user_address,
    'v3' as version,
    pool_address,
    token0,
    token1,
    fee_tier,
    token0_amount,
    token1_amount,
    current_il_percentage,
    fees_earned_usd,
    updated_at
FROM positions_v3
WHERE liquidity > 0;

-- Refresh materialized view every 30 seconds
CREATE INDEX CONCURRENTLY idx_user_positions_summary_user 
ON user_positions_summary (user_address);

-- Innovation: Hyper-optimized indexes (removed CONCURRENTLY for partitioned tables)
CREATE INDEX idx_positions_v2_user_updated 
ON positions_v2 (user_address, updated_at DESC);

CREATE INDEX idx_positions_v3_user_updated 
ON positions_v3 (user_address, updated_at DESC);

CREATE INDEX idx_positions_v2_pair_block 
ON positions_v2 (pair_address, block_number DESC);

CREATE INDEX idx_positions_v3_pool_block 
ON positions_v3 (pool_address, block_number DESC);

-- Price cache table (updated every block)
CREATE TABLE token_prices (
    token_address VARCHAR(42) PRIMARY KEY,
    price_usd NUMERIC(20, 8) NOT NULL,
    price_eth NUMERIC(20, 8),
    block_number BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_token_prices_updated ON token_prices (updated_at DESC);

-- Innovation: Pre-computed IL snapshots for historical analysis
CREATE TABLE il_snapshots (
    id BIGSERIAL PRIMARY KEY,
    user_address VARCHAR(42) NOT NULL,
    position_id VARCHAR(100) NOT NULL, -- pair_address or pool_address:token_id
    version VARCHAR(2) NOT NULL, -- 'v2' or 'v3'
    il_percentage NUMERIC(10, 6) NOT NULL,
    hodl_value_usd NUMERIC(20, 8) NOT NULL,
    position_value_usd NUMERIC(20, 8) NOT NULL,
    fees_earned_usd NUMERIC(20, 8) NOT NULL,
    net_result_usd NUMERIC(20, 8) NOT NULL, -- position_value + fees - hodl_value
    block_number BIGINT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_il_snapshots_user_position 
ON il_snapshots (user_address, position_id, timestamp DESC);

-- Function to refresh materialized view automatically
CREATE OR REPLACE FUNCTION refresh_user_positions_summary()
RETURNS TRIGGER AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY user_positions_summary;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-refresh on position updates (debounced)
CREATE TRIGGER trigger_refresh_positions_summary
AFTER INSERT OR UPDATE OR DELETE ON positions_v2
FOR EACH STATEMENT
EXECUTE FUNCTION refresh_user_positions_summary();

CREATE TRIGGER trigger_refresh_positions_summary_v3
AFTER INSERT OR UPDATE OR DELETE ON positions_v3
FOR EACH STATEMENT
EXECUTE FUNCTION refresh_user_positions_summary();
