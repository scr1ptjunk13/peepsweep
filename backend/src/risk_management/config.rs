use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskManagementConfig {
    pub database: DatabaseConfig,
    pub cache: RedisCacheConfig,
    pub var_confidence_level: f64,
    pub position_limit: f64,
    pub max_drawdown_threshold: f64,
    pub correlation_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCacheConfig {
    pub url: String,
    pub pool_size: u32,
    pub connection_timeout: Duration,
    pub default_ttl: Duration,
}

impl Default for RiskManagementConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            cache: RedisCacheConfig::default(),
            var_confidence_level: 0.95,
            position_limit: 1000000.0,
            max_drawdown_threshold: 0.2,
            correlation_threshold: 0.8,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://postgres:password@localhost:5432/risk_management".to_string(),
            max_connections: 10,
            connection_timeout: Duration::from_secs(30),
        }
    }
}

impl Default for RedisCacheConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
            connection_timeout: Duration::from_secs(10),
            default_ttl: Duration::from_secs(300),
        }
    }
}
