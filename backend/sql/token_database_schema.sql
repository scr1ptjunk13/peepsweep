-- Comprehensive Token Database Schema
-- Optimized for fast lookups, comprehensive data, and scalability

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_trgm"; -- For fast text search

-- Drop existing tables if they exist (for clean reinstall)
DROP TABLE IF EXISTS discovery_jobs CASCADE;
DROP TABLE IF EXISTS token_liquidity CASCADE;
DROP TABLE IF EXISTS token_security CASCADE;
DROP TABLE IF EXISTS token_tags CASCADE;
DROP TABLE IF EXISTS token_sources CASCADE;
DROP TABLE IF EXISTS token_market_data CASCADE;
DROP TABLE IF EXISTS token_logos CASCADE;
DROP TABLE IF EXISTS token_addresses CASCADE;
DROP TABLE IF EXISTS tokens CASCADE;
DROP TABLE IF EXISTS chains CASCADE;

-- Create ENUM types
CREATE TYPE token_type AS ENUM ('Native', 'ERC20', 'ERC721', 'ERC1155', 'Wrapped', 'Stable');
CREATE TYPE verification_level AS ENUM ('Unverified', 'Community', 'DEX', 'Verified', 'Official');
CREATE TYPE job_status AS ENUM ('Pending', 'Running', 'Completed', 'Failed');

-- 1. Chains table - supported blockchain networks
CREATE TABLE chains (
    id BIGINT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    native_currency_symbol VARCHAR(10) NOT NULL,
    native_currency_decimals INTEGER NOT NULL DEFAULT 18,
    rpc_urls TEXT[] NOT NULL,
    block_explorer_url VARCHAR(255),
    is_testnet BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Insert supported chains
INSERT INTO chains (id, name, symbol, native_currency_symbol, native_currency_decimals, rpc_urls, block_explorer_url) VALUES
(1, 'Ethereum', 'ETH', 'ETH', 18, ARRAY['https://eth.llamarpc.com', 'https://rpc.ankr.com/eth'], 'https://etherscan.io'),
(56, 'BNB Smart Chain', 'BSC', 'BNB', 18, ARRAY['https://bsc-dataseed.binance.org', 'https://rpc.ankr.com/bsc'], 'https://bscscan.com'),
(137, 'Polygon', 'MATIC', 'MATIC', 18, ARRAY['https://polygon-rpc.com', 'https://rpc.ankr.com/polygon'], 'https://polygonscan.com'),
(42161, 'Arbitrum', 'ARB', 'ETH', 18, ARRAY['https://arb1.arbitrum.io/rpc', 'https://rpc.ankr.com/arbitrum'], 'https://arbiscan.io'),
(10, 'Optimism', 'OP', 'ETH', 18, ARRAY['https://mainnet.optimism.io', 'https://rpc.ankr.com/optimism'], 'https://optimistic.etherscan.io'),
(8453, 'Base', 'BASE', 'ETH', 18, ARRAY['https://mainnet.base.org', 'https://base.llamarpc.com'], 'https://basescan.org'),
(59144, 'Linea', 'LINEA', 'ETH', 18, ARRAY['https://rpc.linea.build', 'https://linea.drpc.org'], 'https://lineascan.build'),
(324, 'zkSync Era', 'ZKSYNC', 'ETH', 18, ARRAY['https://mainnet.era.zksync.io', 'https://zksync-era.blockpi.network/v1/rpc/public'], 'https://explorer.zksync.io'),
(43114, 'Avalanche', 'AVAX', 'AVAX', 18, ARRAY['https://api.avax.network/ext/bc/C/rpc', 'https://rpc.ankr.com/avalanche'], 'https://snowtrace.io'),
(250, 'Fantom', 'FTM', 'FTM', 18, ARRAY['https://rpc.ftm.tools', 'https://rpc.ankr.com/fantom'], 'https://ftmscan.com');

-- 2. Tokens table - main token registry
CREATE TABLE tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol VARCHAR(50) NOT NULL,
    name VARCHAR(200) NOT NULL,
    coingecko_id VARCHAR(100),
    token_type token_type NOT NULL DEFAULT 'ERC20',
    decimals INTEGER NOT NULL DEFAULT 18,
    total_supply NUMERIC(78, 0), -- Support very large numbers
    is_verified BOOLEAN NOT NULL DEFAULT FALSE,
    verification_level verification_level NOT NULL DEFAULT 'Unverified',
    description TEXT,
    website_url VARCHAR(500),
    twitter_handle VARCHAR(100),
    telegram_url VARCHAR(500),
    discord_url VARCHAR(500),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 3. Token addresses - multi-chain address mapping
CREATE TABLE token_addresses (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_id UUID NOT NULL REFERENCES tokens(id) ON DELETE CASCADE,
    chain_id BIGINT NOT NULL REFERENCES chains(id),
    address VARCHAR(100) NOT NULL,
    is_native BOOLEAN NOT NULL DEFAULT FALSE,
    is_wrapped BOOLEAN NOT NULL DEFAULT FALSE,
    proxy_address VARCHAR(100),
    implementation_address VARCHAR(100),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(chain_id, address)
);

-- 4. Token logos - image storage and CDN
CREATE TABLE token_logos (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_id UUID NOT NULL REFERENCES tokens(id) ON DELETE CASCADE,
    logo_url VARCHAR(1000),
    local_path VARCHAR(500),
    cdn_url VARCHAR(1000),
    image_format VARCHAR(10) DEFAULT 'png',
    image_size INTEGER,
    width INTEGER,
    height INTEGER,
    is_cached BOOLEAN NOT NULL DEFAULT FALSE,
    cache_expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 5. Token market data - price and trading information
CREATE TABLE token_market_data (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_id UUID NOT NULL REFERENCES tokens(id) ON DELETE CASCADE,
    price_usd NUMERIC(20, 8),
    market_cap_usd NUMERIC(20, 2),
    volume_24h_usd NUMERIC(20, 2),
    volume_7d_usd NUMERIC(20, 2),
    price_change_24h NUMERIC(10, 4),
    price_change_7d NUMERIC(10, 4),
    circulating_supply NUMERIC(78, 0),
    max_supply NUMERIC(78, 0),
    ath_usd NUMERIC(20, 8),
    atl_usd NUMERIC(20, 8),
    liquidity_usd NUMERIC(20, 2),
    holders_count BIGINT,
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 6. Token sources - discovery source tracking
CREATE TABLE token_sources (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_id UUID NOT NULL REFERENCES tokens(id) ON DELETE CASCADE,
    source_name VARCHAR(50) NOT NULL,
    source_priority INTEGER NOT NULL DEFAULT 5,
    first_discovered_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_seen_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB
);

-- 7. Token tags - categorization system
CREATE TABLE token_tags (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_id UUID NOT NULL REFERENCES tokens(id) ON DELETE CASCADE,
    tag VARCHAR(50) NOT NULL,
    category VARCHAR(30) NOT NULL DEFAULT 'general',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 8. Token security - security analysis
CREATE TABLE token_security (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_id UUID NOT NULL REFERENCES tokens(id) ON DELETE CASCADE,
    is_honeypot BOOLEAN DEFAULT FALSE,
    is_rugpull_risk BOOLEAN DEFAULT FALSE,
    contract_verified BOOLEAN DEFAULT FALSE,
    proxy_contract BOOLEAN DEFAULT FALSE,
    mint_function BOOLEAN DEFAULT FALSE,
    burn_function BOOLEAN DEFAULT FALSE,
    pause_function BOOLEAN DEFAULT FALSE,
    blacklist_function BOOLEAN DEFAULT FALSE,
    security_score INTEGER DEFAULT 50 CHECK (security_score >= 0 AND security_score <= 100),
    last_analyzed TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 9. Token liquidity - DEX liquidity tracking
CREATE TABLE token_liquidity (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    token_id UUID NOT NULL REFERENCES tokens(id) ON DELETE CASCADE,
    chain_id BIGINT NOT NULL REFERENCES chains(id),
    dex_name VARCHAR(50) NOT NULL,
    pool_address VARCHAR(100),
    pair_token_symbol VARCHAR(50),
    liquidity_usd NUMERIC(20, 2),
    volume_24h_usd NUMERIC(20, 2),
    fee_tier NUMERIC(5, 4),
    last_updated TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- 10. Discovery jobs - background job tracking
CREATE TABLE discovery_jobs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    job_type VARCHAR(50) NOT NULL,
    chain_id BIGINT REFERENCES chains(id),
    source_name VARCHAR(50),
    status job_status NOT NULL DEFAULT 'Pending',
    tokens_processed INTEGER DEFAULT 0,
    tokens_added INTEGER DEFAULT 0,
    tokens_updated INTEGER DEFAULT 0,
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    error_message TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Performance Indexes
-- Fast token lookups
CREATE INDEX idx_tokens_symbol ON tokens(symbol);
CREATE INDEX idx_tokens_symbol_upper ON tokens(UPPER(symbol));
CREATE INDEX idx_tokens_verification ON tokens(verification_level, is_verified);

-- Token address lookups
CREATE INDEX idx_token_addresses_chain_address ON token_addresses(chain_id, address);
CREATE INDEX idx_token_addresses_token_chain ON token_addresses(token_id, chain_id);
CREATE INDEX idx_token_addresses_native ON token_addresses(chain_id) WHERE is_native = TRUE;

-- Search functionality using trigram indexes
CREATE INDEX idx_tokens_name_trgm ON tokens USING gin(name gin_trgm_ops);
CREATE INDEX idx_tokens_symbol_trgm ON tokens USING gin(symbol gin_trgm_ops);

-- Market data queries
CREATE INDEX idx_market_data_token ON token_market_data(token_id);
CREATE INDEX idx_market_data_price ON token_market_data(price_usd DESC NULLS LAST);
CREATE INDEX idx_market_data_volume ON token_market_data(volume_24h_usd DESC NULLS LAST);
CREATE INDEX idx_market_data_market_cap ON token_market_data(market_cap_usd DESC NULLS LAST);
CREATE INDEX idx_market_data_updated ON token_market_data(last_updated DESC);

-- Source tracking
CREATE INDEX idx_token_sources_token ON token_sources(token_id);
CREATE INDEX idx_token_sources_name ON token_sources(source_name);
CREATE INDEX idx_token_sources_active ON token_sources(is_active, last_seen_at);

-- Logo caching
CREATE INDEX idx_token_logos_token ON token_logos(token_id);
CREATE INDEX idx_token_logos_cached ON token_logos(is_cached, cache_expires_at);

-- Tags and categories
CREATE INDEX idx_token_tags_token ON token_tags(token_id);
CREATE INDEX idx_token_tags_category ON token_tags(category, tag);

-- Liquidity tracking
CREATE INDEX idx_token_liquidity_token_chain ON token_liquidity(token_id, chain_id);
CREATE INDEX idx_token_liquidity_dex ON token_liquidity(dex_name, chain_id);
CREATE INDEX idx_token_liquidity_volume ON token_liquidity(volume_24h_usd DESC NULLS LAST);

-- Job tracking
CREATE INDEX idx_discovery_jobs_status ON discovery_jobs(status, created_at);
CREATE INDEX idx_discovery_jobs_type ON discovery_jobs(job_type, status);

-- Update triggers for updated_at columns
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_chains_updated_at BEFORE UPDATE ON chains FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_tokens_updated_at BEFORE UPDATE ON tokens FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_token_addresses_updated_at BEFORE UPDATE ON token_addresses FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_token_logos_updated_at BEFORE UPDATE ON token_logos FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert essential native tokens to bootstrap the system
INSERT INTO tokens (symbol, name, token_type, decimals, is_verified, verification_level) VALUES
('ETH', 'Ethereum', 'Native', 18, TRUE, 'Official'),
('BNB', 'BNB', 'Native', 18, TRUE, 'Official'),
('MATIC', 'Polygon', 'Native', 18, TRUE, 'Official'),
('AVAX', 'Avalanche', 'Native', 18, TRUE, 'Official'),
('FTM', 'Fantom', 'Native', 18, TRUE, 'Official');

-- Insert native token addresses for all supported chains (simplified approach)
-- We'll insert these after getting the token IDs from the previous inserts

-- Create views for common queries
CREATE VIEW v_unified_tokens AS
SELECT 
    t.id,
    t.symbol,
    t.name,
    t.token_type,
    t.decimals,
    t.is_verified,
    t.verification_level,
    json_object_agg(ta.chain_id::text, ta.address) FILTER (WHERE ta.address IS NOT NULL) as chain_addresses,
    tl.logo_url,
    tl.cdn_url,
    tm.price_usd,
    tm.market_cap_usd,
    tm.volume_24h_usd,
    t.updated_at
FROM tokens t
LEFT JOIN token_addresses ta ON t.id = ta.token_id
LEFT JOIN token_logos tl ON t.id = tl.token_id
LEFT JOIN token_market_data tm ON t.id = tm.token_id
GROUP BY t.id, t.symbol, t.name, t.token_type, t.decimals, t.is_verified, 
         t.verification_level, tl.logo_url, tl.cdn_url, tm.price_usd, 
         tm.market_cap_usd, tm.volume_24h_usd, t.updated_at;

-- Create view for token search with ranking
CREATE VIEW v_token_search AS
SELECT 
    t.*,
    ta.chain_id,
    ta.address,
    tl.logo_url,
    tm.price_usd,
    tm.volume_24h_usd,
    CASE 
        WHEN t.verification_level = 'Official' THEN 5
        WHEN t.verification_level = 'Verified' THEN 4
        WHEN t.verification_level = 'DEX' THEN 3
        WHEN t.verification_level = 'Community' THEN 2
        ELSE 1
    END as verification_score
FROM tokens t
LEFT JOIN token_addresses ta ON t.id = ta.token_id
LEFT JOIN token_logos tl ON t.id = tl.token_id
LEFT JOIN token_market_data tm ON t.id = tm.token_id;

-- Grant permissions (adjust as needed for your setup)
-- GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO your_app_user;
-- GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO your_app_user;

-- Success message
SELECT 'Token database schema created successfully! 🚀' as status,
       COUNT(*) as chains_created FROM chains;
