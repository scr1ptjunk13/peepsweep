-- Additional tables for price snapshots and backfill checkpoints

-- Price snapshots for historical analysis
CREATE TABLE price_snapshots (
    id BIGSERIAL PRIMARY KEY,
    token_address VARCHAR(42) NOT NULL,
    price_usd NUMERIC(20, 8) NOT NULL,
    volume_24h NUMERIC(20, 8),
    market_cap NUMERIC(20, 8),
    source VARCHAR(50) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_price_snapshots_token_timestamp 
ON price_snapshots (token_address, timestamp DESC);

-- Backfill checkpoints for tracking indexing progress
CREATE TABLE backfill_checkpoints (
    user_address VARCHAR(42) PRIMARY KEY,
    last_processed_block BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Partitioned tables for v2 positions (referenced in queries)
CREATE TABLE positions_v2_partitioned (
    LIKE positions_v2 INCLUDING ALL
) PARTITION BY HASH (user_address);

-- Create partitions for v2_partitioned
DO $$ 
DECLARE 
    i INTEGER;
BEGIN 
    FOR i IN 0..15 LOOP
        EXECUTE format('CREATE TABLE positions_v2_partitioned_%s PARTITION OF positions_v2_partitioned 
                       FOR VALUES WITH (modulus 16, remainder %s)', i, i);
    END LOOP;
END $$;
