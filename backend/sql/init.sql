-- TimescaleDB initialization script for risk management system
-- This creates the hypertables and indexes needed for high-performance time-series data

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Trade events hypertable for real-time trade tracking
CREATE TABLE IF NOT EXISTS trade_events (
    event_id VARCHAR(255) PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    token_in VARCHAR(42) NOT NULL,
    token_out VARCHAR(42) NOT NULL,
    amount_in DECIMAL(78, 18) NOT NULL,
    amount_out DECIMAL(78, 18) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    dex VARCHAR(50) NOT NULL,
    gas_used BIGINT NOT NULL,
    gas_price DECIMAL(78, 18) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('trade_events', 'timestamp', if_not_exists => TRUE);

-- User positions table for current position tracking
CREATE TABLE IF NOT EXISTS user_positions (
    user_id VARCHAR(255) PRIMARY KEY,
    positions JSONB NOT NULL,
    pnl DECIMAL(78, 18) NOT NULL,
    last_updated TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Risk metrics table for calculated risk data
CREATE TABLE IF NOT EXISTS risk_metrics (
    user_id VARCHAR(255) NOT NULL,
    total_portfolio_value DECIMAL(78, 18) NOT NULL,
    var_95 DECIMAL(78, 18) NOT NULL,
    var_99 DECIMAL(78, 18) NOT NULL,
    expected_shortfall DECIMAL(78, 18) NOT NULL,
    max_drawdown DECIMAL(78, 18) NOT NULL,
    sharpe_ratio DOUBLE PRECISION NOT NULL,
    sortino_ratio DOUBLE PRECISION NOT NULL,
    token_exposures JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert risk metrics to hypertable
SELECT create_hypertable('risk_metrics', 'timestamp', if_not_exists => TRUE);

-- Risk alerts table for alert tracking
CREATE TABLE IF NOT EXISTS risk_alerts (
    alert_id VARCHAR(255) PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    alert_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL,
    message TEXT NOT NULL,
    metadata JSONB,
    triggered_at TIMESTAMPTZ NOT NULL,
    acknowledged BOOLEAN DEFAULT FALSE,
    acknowledged_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert alerts to hypertable
SELECT create_hypertable('risk_alerts', 'triggered_at', if_not_exists => TRUE);

-- Exposure snapshots for historical exposure tracking
CREATE TABLE IF NOT EXISTS exposure_snapshots (
    snapshot_id VARCHAR(255) PRIMARY KEY,
    user_id VARCHAR(255) NOT NULL,
    total_exposure_usd DECIMAL(78, 18) NOT NULL,
    token_exposures JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Convert exposure snapshots to hypertable
SELECT create_hypertable('exposure_snapshots', 'timestamp', if_not_exists => TRUE);

-- Indexes for optimal query performance
CREATE INDEX IF NOT EXISTS idx_trade_events_user_id ON trade_events (user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_trade_events_tokens ON trade_events (token_in, token_out);
CREATE INDEX IF NOT EXISTS idx_trade_events_dex ON trade_events (dex, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_risk_metrics_user_id ON risk_metrics (user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_risk_alerts_user_id ON risk_alerts (user_id, triggered_at DESC);
CREATE INDEX IF NOT EXISTS idx_risk_alerts_type ON risk_alerts (alert_type, severity);

CREATE INDEX IF NOT EXISTS idx_exposure_snapshots_user_id ON exposure_snapshots (user_id, timestamp DESC);

-- Continuous aggregates for fast analytics queries
CREATE MATERIALIZED VIEW IF NOT EXISTS hourly_trade_volume
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', timestamp) AS hour,
    user_id,
    dex,
    COUNT(*) as trade_count,
    SUM(amount_out) as total_volume_out,
    AVG(gas_price) as avg_gas_price
FROM trade_events
GROUP BY hour, user_id, dex;

CREATE MATERIALIZED VIEW IF NOT EXISTS daily_risk_summary
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 day', timestamp) AS day,
    user_id,
    AVG(total_portfolio_value) as avg_portfolio_value,
    MAX(var_95) as max_var_95,
    AVG(sharpe_ratio) as avg_sharpe_ratio
FROM risk_metrics
GROUP BY day, user_id;

-- Retention policies for data management
SELECT add_retention_policy('trade_events', INTERVAL '1 year');
SELECT add_retention_policy('risk_metrics', INTERVAL '2 years');
SELECT add_retention_policy('risk_alerts', INTERVAL '1 year');
SELECT add_retention_policy('exposure_snapshots', INTERVAL '6 months');

-- Compression policies for storage optimization
SELECT add_compression_policy('trade_events', INTERVAL '7 days');
SELECT add_compression_policy('risk_metrics', INTERVAL '30 days');
SELECT add_compression_policy('exposure_snapshots', INTERVAL '7 days');

-- Grant permissions
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO postgres;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO postgres;
