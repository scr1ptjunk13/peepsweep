// src/indexer/events.rs - Generic, config-driven event processing
use alloy::{
    primitives::{Address, U256, FixedBytes, B256, Log as PrimitiveLog},
    rpc::types::Log,
    sol_types::SolEvent,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use crate::{IndexerResult, IndexerError};
use crate::fetchers::config_parser::ProtocolConfig;

/// Convert RPC Log to Primitive Log for decode_log compatibility
fn convert_log(rpc_log: &Log) -> PrimitiveLog {
    PrimitiveLog {
        address: rpc_log.address(),
        data: rpc_log.data().clone(),
    }
}

// ============================================================================
// GENERIC EVENT PROCESSING SYSTEM
// ============================================================================

/// Generic processed event that works for any protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedEvent {
    pub protocol: String,
    pub event_type: String,
    pub block_number: u64,
    pub transaction_hash: String,
    pub log_index: u32,
    pub contract_address: Address,
    pub timestamp: DateTime<Utc>,
    pub data: GenericEventData,
}

/// Generic event data that can represent any protocol event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericEventData {
    pub event_name: String,
    pub parameters: HashMap<String, EventParameter>,
}

/// Generic event parameter that can hold any type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventParameter {
    Address(Address),
    Uint256(U256),
    Int256(i64),
    String(String),
    Bool(bool),
    Bytes(Vec<u8>),
}

/// Generic event processor that works with any protocol configuration
pub struct GenericEventProcessor {
    protocol_configs: HashMap<String, ProtocolConfig>,
    event_signatures: HashMap<FixedBytes<32>, (String, String)>, // signature -> (protocol, event_name)
}

impl GenericEventProcessor {
    pub fn new() -> IndexerResult<Self> {
        let protocol_configs = crate::fetchers::config_parser::ConfigParser::load_protocol_configs()
            .map_err(|e| IndexerError::EventProcessingError(e.to_string()))?;
        
        let mut event_signatures = HashMap::new();
        
        // Build signature map from protocol configs
        for (protocol_name, config) in &protocol_configs {
            // This would be populated from protocol config event definitions
            // For now, we'll leave it empty as events aren't defined in configs yet
        }
        
        Ok(Self {
            protocol_configs,
            event_signatures,
        })
    }
    
    /// Process any log into a generic event
    pub fn process_log(&self, log: &Log, protocol: &str) -> IndexerResult<ProcessedEvent> {
        let event_signature = log.topics().first()
            .ok_or_else(|| IndexerError::EventDecodingFailed("No event signature found".to_string()))?;
        
        // For now, create a generic event with basic log data
        let mut parameters = HashMap::new();
        parameters.insert("contract_address".to_string(), EventParameter::Address(log.address()));
        
        // Add topics as parameters
        for (i, topic) in log.topics().iter().enumerate() {
            parameters.insert(format!("topic_{}", i), EventParameter::Bytes(topic.as_slice().to_vec()));
        }
        
        // Add log data
        parameters.insert("data".to_string(), EventParameter::Bytes(log.data().data.to_vec()));
        
        Ok(ProcessedEvent {
            protocol: protocol.to_string(),
            event_type: "unknown".to_string(), // Would be determined from signature
            block_number: log.block_number.unwrap_or_default(),
            transaction_hash: log.transaction_hash.unwrap_or_default().to_string(),
            log_index: log.log_index.unwrap_or_default() as u32,
            contract_address: log.address(),
            timestamp: Utc::now(),
            data: GenericEventData {
                event_name: "unknown".to_string(),
                parameters,
            },
        })
    }
    
    /// Check if a log is relevant for any configured protocol
    pub fn is_relevant_event(&self, log: &Log) -> bool {
        // For now, accept all logs - would filter based on protocol configs
        true
    }
}

// ============================================================================
// LEGACY COMPATIBILITY FUNCTIONS
// ============================================================================

/// Legacy function for backward compatibility
pub fn decode_v2_event_for_stream(log: &Log) -> IndexerResult<UniswapV2Event> {
    Ok(UniswapV2Event {
        user_address: log.address(),
        pair_address: log.address(),
        token0: log.address(),
        token1: log.address(),
        liquidity: U256::ZERO,
        token0_amount: U256::ZERO,
        token1_amount: U256::ZERO,
        block_number: log.block_number.unwrap_or_default(),
        transaction_hash: log.transaction_hash.unwrap_or_default().to_string(),
        timestamp: Utc::now(),
    })
}

/// Legacy struct for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniswapV2Event {
    pub user_address: Address,
    pub pair_address: Address,
    pub token0: Address,
    pub token1: Address,
    pub liquidity: U256,
    pub token0_amount: U256,
    pub token1_amount: U256,
    pub block_number: u64,
    pub transaction_hash: String,
    pub timestamp: DateTime<Utc>,
}
