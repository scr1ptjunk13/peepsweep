use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub bridges: HashMap<String, BridgeSettings>,
    pub chains: HashMap<u64, ChainConfig>,
    pub monitoring: MonitoringConfig,
    pub rate_limits: RateLimitConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeSettings {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub api_url: String,
    pub timeout_seconds: u64,
    pub max_retries: u32,
    pub priority: u8, // 1-10, higher is better
    pub supported_chains: Vec<u64>,
    pub supported_tokens: Vec<String>,
    pub fee_multiplier: f64, // Adjust fees for this bridge
    pub custom_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: String,
    pub rpc_url: String,
    pub explorer_url: String,
    pub native_token: String,
    pub gas_price_multiplier: f64,
    pub confirmation_blocks: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub health_check_interval_seconds: u64,
    pub metrics_retention_hours: u64,
    pub alert_webhooks: Vec<String>,
    pub log_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub burst_size: u32,
    pub per_bridge_limits: HashMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub require_api_key: bool,
    pub allowed_origins: Vec<String>,
    pub max_amount_per_transfer: String,
    pub blacklisted_addresses: Vec<String>,
    pub enable_transaction_simulation: bool,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        let mut bridges = HashMap::new();
        
        // Hop Protocol
        bridges.insert("hop_protocol".to_string(), BridgeSettings {
            enabled: true,
            api_key: None,
            api_url: "https://api.hop.exchange".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            priority: 8,
            supported_chains: vec![1, 10, 42161, 137, 100, 42220],
            supported_tokens: vec!["ETH".to_string(), "USDC".to_string(), "USDT".to_string(), "DAI".to_string()],
            fee_multiplier: 1.0,
            custom_headers: HashMap::new(),
        });

        // Across Protocol
        bridges.insert("across_protocol".to_string(), BridgeSettings {
            enabled: true,
            api_key: None,
            api_url: "https://app.across.to/api".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            priority: 9,
            supported_chains: vec![1, 10, 42161, 137, 8453, 59144, 324],
            supported_tokens: vec!["ETH".to_string(), "USDC".to_string(), "USDT".to_string(), "DAI".to_string(), "WBTC".to_string()],
            fee_multiplier: 1.0,
            custom_headers: HashMap::new(),
        });

        // Stargate Finance
        bridges.insert("stargate_finance".to_string(), BridgeSettings {
            enabled: true,
            api_key: None,
            api_url: "https://stargate.finance/api".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            priority: 7,
            supported_chains: vec![1, 10, 42161, 137, 43114, 250, 56],
            supported_tokens: vec!["USDC".to_string(), "USDT".to_string(), "ETH".to_string()],
            fee_multiplier: 1.0,
            custom_headers: HashMap::new(),
        });

        // Synapse Protocol
        bridges.insert("synapse_protocol".to_string(), BridgeSettings {
            enabled: true,
            api_key: None,
            api_url: "https://api.synapseprotocol.com".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            priority: 6,
            supported_chains: vec![1, 10, 42161, 137, 43114, 250, 56, 8453],
            supported_tokens: vec!["USDC".to_string(), "USDT".to_string(), "DAI".to_string(), "ETH".to_string()],
            fee_multiplier: 1.0,
            custom_headers: HashMap::new(),
        });

        // Multichain
        bridges.insert("multichain".to_string(), BridgeSettings {
            enabled: true,
            api_key: None,
            api_url: "https://bridgeapi.anyswap.exchange".to_string(),
            timeout_seconds: 45,
            max_retries: 2,
            priority: 5,
            supported_chains: vec![1, 10, 42161, 137, 43114, 250, 56, 8453, 42220, 100],
            supported_tokens: vec!["USDC".to_string(), "USDT".to_string(), "DAI".to_string(), "ETH".to_string(), "WBTC".to_string()],
            fee_multiplier: 1.0,
            custom_headers: HashMap::new(),
        });

        let mut chains = HashMap::new();
        
        // Ethereum Mainnet
        chains.insert(1, ChainConfig {
            chain_id: 1,
            name: "Ethereum".to_string(),
            rpc_url: "https://eth.llamarpc.com".to_string(),
            explorer_url: "https://etherscan.io".to_string(),
            native_token: "ETH".to_string(),
            gas_price_multiplier: 1.1,
            confirmation_blocks: 12,
            enabled: true,
        });

        // Optimism
        chains.insert(10, ChainConfig {
            chain_id: 10,
            name: "Optimism".to_string(),
            rpc_url: "https://mainnet.optimism.io".to_string(),
            explorer_url: "https://optimistic.etherscan.io".to_string(),
            native_token: "ETH".to_string(),
            gas_price_multiplier: 1.0,
            confirmation_blocks: 1,
            enabled: true,
        });

        // Arbitrum One
        chains.insert(42161, ChainConfig {
            chain_id: 42161,
            name: "Arbitrum One".to_string(),
            rpc_url: "https://arb1.arbitrum.io/rpc".to_string(),
            explorer_url: "https://arbiscan.io".to_string(),
            native_token: "ETH".to_string(),
            gas_price_multiplier: 1.0,
            confirmation_blocks: 1,
            enabled: true,
        });

        // Polygon
        chains.insert(137, ChainConfig {
            chain_id: 137,
            name: "Polygon".to_string(),
            rpc_url: "https://polygon-rpc.com".to_string(),
            explorer_url: "https://polygonscan.com".to_string(),
            native_token: "MATIC".to_string(),
            gas_price_multiplier: 1.2,
            confirmation_blocks: 20,
            enabled: true,
        });

        // Avalanche
        chains.insert(43114, ChainConfig {
            chain_id: 43114,
            name: "Avalanche".to_string(),
            rpc_url: "https://api.avax.network/ext/bc/C/rpc".to_string(),
            explorer_url: "https://snowtrace.io".to_string(),
            native_token: "AVAX".to_string(),
            gas_price_multiplier: 1.0,
            confirmation_blocks: 1,
            enabled: true,
        });

        // Fantom
        chains.insert(250, ChainConfig {
            chain_id: 250,
            name: "Fantom".to_string(),
            rpc_url: "https://rpc.ftm.tools".to_string(),
            explorer_url: "https://ftmscan.com".to_string(),
            native_token: "FTM".to_string(),
            gas_price_multiplier: 1.0,
            confirmation_blocks: 1,
            enabled: true,
        });

        // BSC
        chains.insert(56, ChainConfig {
            chain_id: 56,
            name: "BNB Smart Chain".to_string(),
            rpc_url: "https://bsc-dataseed.binance.org".to_string(),
            explorer_url: "https://bscscan.com".to_string(),
            native_token: "BNB".to_string(),
            gas_price_multiplier: 1.0,
            confirmation_blocks: 3,
            enabled: true,
        });

        // Base
        chains.insert(8453, ChainConfig {
            chain_id: 8453,
            name: "Base".to_string(),
            rpc_url: "https://mainnet.base.org".to_string(),
            explorer_url: "https://basescan.org".to_string(),
            native_token: "ETH".to_string(),
            gas_price_multiplier: 1.0,
            confirmation_blocks: 1,
            enabled: true,
        });

        Self {
            bridges,
            chains,
            monitoring: MonitoringConfig {
                enabled: true,
                health_check_interval_seconds: 60,
                metrics_retention_hours: 168, // 7 days
                alert_webhooks: vec![],
                log_level: "info".to_string(),
            },
            rate_limits: RateLimitConfig {
                requests_per_minute: 100,
                burst_size: 20,
                per_bridge_limits: HashMap::new(),
            },
            security: SecurityConfig {
                require_api_key: false,
                allowed_origins: vec!["*".to_string()],
                max_amount_per_transfer: "1000000000000000000000".to_string(), // 1000 ETH equivalent
                blacklisted_addresses: vec![],
                enable_transaction_simulation: true,
            },
        }
    }
}

impl BridgeConfig {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Loading bridge configuration from: {}", path);
        
        if !std::path::Path::new(path).exists() {
            warn!("Configuration file not found at {}, creating default config", path);
            let default_config = Self::default();
            default_config.save_to_file(path)?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)?;
        let config: BridgeConfig = serde_json::from_str(&content)?;
        
        info!("Bridge configuration loaded successfully");
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        info!("Bridge configuration saved to: {}", path);
        Ok(())
    }

    pub fn load_from_env() -> Self {
        let mut config = Self::default();
        
        // Load environment variables
        if let Ok(log_level) = env::var("BRIDGE_LOG_LEVEL") {
            config.monitoring.log_level = log_level;
        }

        if let Ok(health_interval) = env::var("BRIDGE_HEALTH_CHECK_INTERVAL") {
            if let Ok(interval) = health_interval.parse::<u64>() {
                config.monitoring.health_check_interval_seconds = interval;
            }
        }

        if let Ok(rate_limit) = env::var("BRIDGE_RATE_LIMIT") {
            if let Ok(limit) = rate_limit.parse::<u32>() {
                config.rate_limits.requests_per_minute = limit;
            }
        }

        if let Ok(max_amount) = env::var("BRIDGE_MAX_AMOUNT") {
            config.security.max_amount_per_transfer = max_amount;
        }

        // Load bridge-specific API keys
        for (bridge_name, bridge_settings) in config.bridges.iter_mut() {
            let env_key = format!("BRIDGE_{}_API_KEY", bridge_name.to_uppercase());
            if let Ok(api_key) = env::var(&env_key) {
                bridge_settings.api_key = Some(api_key);
                info!("Loaded API key for bridge: {}", bridge_name);
            }

            let env_url = format!("BRIDGE_{}_API_URL", bridge_name.to_uppercase());
            if let Ok(api_url) = env::var(&env_url) {
                bridge_settings.api_url = api_url;
            }

            let env_enabled = format!("BRIDGE_{}_ENABLED", bridge_name.to_uppercase());
            if let Ok(enabled) = env::var(&env_enabled) {
                bridge_settings.enabled = enabled.parse().unwrap_or(true);
            }
        }

        // Load chain-specific RPC URLs
        for (chain_id, chain_config) in config.chains.iter_mut() {
            let env_key = format!("CHAIN_{}_RPC_URL", chain_id);
            if let Ok(rpc_url) = env::var(&env_key) {
                chain_config.rpc_url = rpc_url;
                info!("Loaded custom RPC URL for chain {}: {}", chain_id, chain_config.rpc_url);
            }
        }

        info!("Bridge configuration loaded from environment variables");
        config
    }

    pub fn get_bridge_settings(&self, bridge_name: &str) -> Option<&BridgeSettings> {
        self.bridges.get(bridge_name)
    }

    pub fn get_chain_config(&self, chain_id: u64) -> Option<&ChainConfig> {
        self.chains.get(&chain_id)
    }

    pub fn is_bridge_enabled(&self, bridge_name: &str) -> bool {
        self.bridges.get(bridge_name)
            .map(|settings| settings.enabled)
            .unwrap_or(false)
    }

    pub fn is_chain_enabled(&self, chain_id: u64) -> bool {
        self.chains.get(&chain_id)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }

    pub fn get_enabled_bridges(&self) -> Vec<String> {
        self.bridges.iter()
            .filter(|(_, settings)| settings.enabled)
            .map(|(name, _)| name.clone())
            .collect()
    }

    pub fn get_enabled_chains(&self) -> Vec<u64> {
        self.chains.iter()
            .filter(|(_, config)| config.enabled)
            .map(|(chain_id, _)| *chain_id)
            .collect()
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate bridges
        for (bridge_name, settings) in &self.bridges {
            if settings.enabled {
                if settings.api_url.is_empty() {
                    errors.push(format!("Bridge {} has empty API URL", bridge_name));
                }
                
                if settings.timeout_seconds == 0 {
                    errors.push(format!("Bridge {} has zero timeout", bridge_name));
                }

                if settings.priority == 0 || settings.priority > 10 {
                    errors.push(format!("Bridge {} has invalid priority (must be 1-10)", bridge_name));
                }

                if settings.supported_chains.is_empty() {
                    errors.push(format!("Bridge {} has no supported chains", bridge_name));
                }
            }
        }

        // Validate chains
        for (chain_id, config) in &self.chains {
            if config.enabled {
                if config.rpc_url.is_empty() {
                    errors.push(format!("Chain {} has empty RPC URL", chain_id));
                }

                if config.name.is_empty() {
                    errors.push(format!("Chain {} has empty name", chain_id));
                }

                if config.confirmation_blocks == 0 {
                    errors.push(format!("Chain {} has zero confirmation blocks", chain_id));
                }
            }
        }

        // Validate monitoring
        if self.monitoring.enabled && self.monitoring.health_check_interval_seconds == 0 {
            errors.push("Monitoring enabled but health check interval is zero".to_string());
        }

        // Validate rate limits
        if self.rate_limits.requests_per_minute == 0 {
            errors.push("Rate limit requests per minute cannot be zero".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn update_bridge_setting(&mut self, bridge_name: &str, key: &str, value: serde_json::Value) -> Result<(), String> {
        let bridge_settings = self.bridges.get_mut(bridge_name)
            .ok_or_else(|| format!("Bridge {} not found", bridge_name))?;

        match key {
            "enabled" => {
                bridge_settings.enabled = value.as_bool()
                    .ok_or_else(|| "enabled must be a boolean".to_string())?;
            }
            "api_key" => {
                bridge_settings.api_key = value.as_str().map(|s| s.to_string());
            }
            "api_url" => {
                bridge_settings.api_url = value.as_str()
                    .ok_or_else(|| "api_url must be a string".to_string())?
                    .to_string();
            }
            "timeout_seconds" => {
                bridge_settings.timeout_seconds = value.as_u64()
                    .ok_or_else(|| "timeout_seconds must be a number".to_string())?;
            }
            "priority" => {
                let priority = value.as_u64()
                    .ok_or_else(|| "priority must be a number".to_string())? as u8;
                if priority == 0 || priority > 10 {
                    return Err("priority must be between 1 and 10".to_string());
                }
                bridge_settings.priority = priority;
            }
            "fee_multiplier" => {
                bridge_settings.fee_multiplier = value.as_f64()
                    .ok_or_else(|| "fee_multiplier must be a number".to_string())?;
            }
            _ => {
                return Err(format!("Unknown setting key: {}", key));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = BridgeConfig::default();
        assert!(!config.bridges.is_empty());
        assert!(!config.chains.is_empty());
        assert!(config.monitoring.enabled);
    }

    #[test]
    fn test_config_validation() {
        let config = BridgeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_bridge_settings_update() {
        let mut config = BridgeConfig::default();
        
        let result = config.update_bridge_setting(
            "hop_protocol", 
            "enabled", 
            serde_json::Value::Bool(false)
        );
        assert!(result.is_ok());
        assert!(!config.bridges["hop_protocol"].enabled);
    }

    #[test]
    fn test_config_file_operations() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_config.json");
        let file_path_str = file_path.to_str().unwrap();

        let config = BridgeConfig::default();
        assert!(config.save_to_file(file_path_str).is_ok());

        let loaded_config = BridgeConfig::load_from_file(file_path_str).unwrap();
        assert_eq!(config.bridges.len(), loaded_config.bridges.len());
    }

    #[test]
    fn test_enabled_bridges() {
        let config = BridgeConfig::default();
        let enabled_bridges = config.get_enabled_bridges();
        assert!(!enabled_bridges.is_empty());
        
        for bridge_name in &enabled_bridges {
            assert!(config.is_bridge_enabled(bridge_name));
        }
    }
}
