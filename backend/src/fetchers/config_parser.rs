use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use alloy::primitives::Address;
use anyhow::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtocolConfig {
    pub protocol: ProtocolInfo,
    pub position_detection: PositionDetection,
    pub position_details: PositionDetails,
    pub position_type: String,
    pub risk_calculation: RiskCalculation,
    pub cache_strategy: CacheStrategy,
    pub performance: PerformanceConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtocolInfo {
    pub name: String,
    pub r#type: String,
    pub version: String,
    pub chains: HashMap<u32, ChainConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChainConfig {
    pub factory: Option<String>,
    pub position_manager: Option<String>,
    pub multicall: Option<String>,
    pub quoter: Option<String>,
    pub router: Option<String>,
}

impl ChainConfig {
    pub fn factory_address(&self) -> Result<Address> {
        self.factory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Factory address not configured"))?
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid factory address: {}", e))
    }

    pub fn position_manager_address(&self) -> Result<Address> {
        self.position_manager
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Position manager address not configured"))?
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid position manager address: {}", e))
    }

    pub fn multicall_address(&self) -> Result<Option<Address>> {
        if let Some(addr) = &self.multicall {
            Ok(Some(addr.parse().map_err(|e| anyhow::anyhow!("Invalid multicall address: {}", e))?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PositionDetection {
    pub method: String,
    pub contract_function: Option<ContractFunction>,
    pub token_enumeration: Option<ContractFunction>,
    pub pair_discovery: Option<Vec<DiscoveryMethod>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContractFunction {
    pub name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub function: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiscoveryMethod {
    pub method: String,
    pub event: Option<String>,
    pub contract: Option<String>,
    pub from_block: Option<u64>,
    pub min_balance: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PositionDetails {
    pub function: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<OutputField>,
    pub batch_calls: Option<Vec<ContractFunction>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputField {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskCalculation {
    pub il_formula: String,
    pub volatility_window: u32,
    pub rebalancing_frequency: u32,
    pub risk_factors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheStrategy {
    pub ttl: u32,
    pub invalidation_triggers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    pub batch_size: u32,
    pub parallel_requests: u32,
    pub timeout_ms: u32,
}

pub struct ConfigParser;

impl ConfigParser {
    pub fn load_protocol_configs() -> Result<HashMap<String, ProtocolConfig>> {
        let mut configs = HashMap::new();
        
        let config_dir = std::path::Path::new("backend/configs/protocols");
        
        if !config_dir.exists() {
            return Err(anyhow::anyhow!("Protocol configs directory not found: {:?}", config_dir));
        }
        
        for entry in std::fs::read_dir(config_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension() == Some(std::ffi::OsStr::new("yaml")) || 
               path.extension() == Some(std::ffi::OsStr::new("yml")) {
                
                let content = std::fs::read_to_string(&path)?;
                let config: ProtocolConfig = serde_yaml::from_str(&content)
                    .map_err(|e| anyhow::anyhow!("Failed to parse config {:?}: {}", path, e))?;
                
                let protocol_name = config.protocol.name.clone();
                configs.insert(protocol_name.clone(), config);
                
                tracing::info!("Loaded protocol config: {} from {:?}", protocol_name, path);
            }
        }
        
        if configs.is_empty() {
            return Err(anyhow::anyhow!("No protocol configurations found in {:?}", config_dir));
        }
        
        tracing::info!("Loaded {} protocol configurations", configs.len());
        Ok(configs)
    }
    
    pub fn load_single_protocol(protocol_name: &str) -> Result<ProtocolConfig> {
        let config_path = format!("backend/configs/protocols/{}.yaml", protocol_name);
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to read config {}: {}", config_path, e))?;
        
        let config: ProtocolConfig = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config {}: {}", config_path, e))?;
        
        Ok(config)
    }
    
    pub fn validate_config(config: &ProtocolConfig) -> Result<()> {
        // Validate protocol info
        if config.protocol.name.is_empty() {
            return Err(anyhow::anyhow!("Protocol name cannot be empty"));
        }
        
        if config.protocol.chains.is_empty() {
            return Err(anyhow::anyhow!("Protocol must support at least one chain"));
        }
        
        // Validate position detection method
        match config.position_detection.method.as_str() {
            "nft_ownership" => {
                if config.position_detection.contract_function.is_none() {
                    return Err(anyhow::anyhow!("NFT ownership method requires contract_function"));
                }
                if config.position_detection.token_enumeration.is_none() {
                    return Err(anyhow::anyhow!("NFT ownership method requires token_enumeration"));
                }
            },
            "erc20_balance" => {
                if config.position_detection.pair_discovery.is_none() {
                    return Err(anyhow::anyhow!("ERC20 balance method requires pair_discovery"));
                }
            },
            _ => {
                return Err(anyhow::anyhow!("Unsupported position detection method: {}", config.position_detection.method));
            }
        }
        
        // Validate position type
        match config.position_type.as_str() {
            "nft" | "erc20" => {},
            _ => {
                return Err(anyhow::anyhow!("Unsupported position type: {}", config.position_type));
            }
        }
        
        // Validate IL formula
        match config.risk_calculation.il_formula.as_str() {
            "uniswap_v3_concentrated" | "uniswap_v2_constant_product" => {},
            _ => {
                return Err(anyhow::anyhow!("Unsupported IL formula: {}", config.risk_calculation.il_formula));
            }
        }
        
        // Validate chain configurations
        for (chain_id, chain_config) in &config.protocol.chains {
            match config.position_detection.method.as_str() {
                "nft_ownership" => {
                    if chain_config.position_manager.is_none() {
                        return Err(anyhow::anyhow!("Chain {} missing position_manager for NFT ownership", chain_id));
                    }
                },
                "erc20_balance" => {
                    if chain_config.factory.is_none() {
                        return Err(anyhow::anyhow!("Chain {} missing factory for ERC20 balance", chain_id));
                    }
                },
                _ => {}
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_uniswap_v3_config() {
        let config = ConfigParser::load_single_protocol("uniswap_v3").unwrap();
        assert_eq!(config.protocol.name, "uniswap_v3");
        assert_eq!(config.position_detection.method, "nft_ownership");
        assert_eq!(config.position_type, "nft");
        assert_eq!(config.risk_calculation.il_formula, "uniswap_v3_concentrated");
    }

    #[test]
    fn test_validate_config() {
        let config = ConfigParser::load_single_protocol("uniswap_v3").unwrap();
        ConfigParser::validate_config(&config).unwrap();
    }

    #[test]
    fn test_chain_config_address_parsing() {
        let config = ConfigParser::load_single_protocol("uniswap_v3").unwrap();
        let ethereum_config = config.protocol.chains.get(&1).unwrap();
        
        let factory_addr = ethereum_config.factory_address().unwrap();
        assert_eq!(factory_addr.to_string().to_lowercase(), "0x1f98431c8ad98523631ae4a59f267346ea31f984");
        
        let position_manager_addr = ethereum_config.position_manager_address().unwrap();
        assert_eq!(position_manager_addr.to_string().to_lowercase(), "0xc36442b4a4522e871399cd717abdd847ab11fe88");
    }
}
