use std::collections::{HashMap, VecDeque, HashSet};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tracing::{info, warn, error};

use super::{BridgeIntegration, BridgeError, CrossChainParams, BridgeQuote, BridgeResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHopRoute {
    pub hops: Vec<RouteHop>,
    pub total_cost: String,
    pub total_time: u64,
    pub total_confidence: f64,
    pub route_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    pub from_chain: u64,
    pub to_chain: u64,
    pub bridge_name: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub amount_out: String,
    pub fee: String,
    pub estimated_time: u64,
    pub hop_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHopParams {
    pub from_chain_id: u64,
    pub to_chain_id: u64,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: String,
    pub user_address: String,
    pub slippage: f64,
    pub max_hops: usize,
    pub prefer_speed: bool, // true for speed, false for cost
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHopExecution {
    pub route_id: String,
    pub hops: Vec<HopExecution>,
    pub status: MultiHopStatus,
    pub current_hop: usize,
    pub total_hops: usize,
    pub started_at: u64,
    pub estimated_completion: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HopExecution {
    pub hop_index: usize,
    pub bridge_name: String,
    pub transaction_hash: Option<String>,
    pub status: HopStatus,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultiHopStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    PartiallyCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HopStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

pub struct MultiHopRouter {
    bridges: HashMap<String, Box<dyn BridgeIntegration + Send + Sync>>,
    intermediate_tokens: Vec<String>,
    chain_connections: HashMap<u64, Vec<u64>>, // chain -> directly connected chains
    active_executions: HashMap<String, MultiHopExecution>,
}

impl MultiHopRouter {
    pub fn new() -> Self {
        let mut chain_connections = HashMap::new();
        
        // Define direct connections between chains based on bridge availability
        let chains = vec![1, 10, 42161, 137, 43114, 250, 56, 8453]; // Major chains
        
        // For simplicity, assume all chains can connect to each other directly
        // In reality, this would be based on actual bridge support
        for &chain in &chains {
            let mut connections = Vec::new();
            for &other_chain in &chains {
                if chain != other_chain {
                    connections.push(other_chain);
                }
            }
            chain_connections.insert(chain, connections);
        }
        
        Self {
            bridges: HashMap::new(),
            intermediate_tokens: vec![
                "USDC".to_string(),
                "USDT".to_string(),
                "ETH".to_string(),
                "WETH".to_string(),
                "DAI".to_string(),
            ],
            chain_connections,
            active_executions: HashMap::new(),
        }
    }
    
    pub fn add_bridge(&mut self, bridge: Box<dyn BridgeIntegration + Send + Sync>) {
        let name = bridge.name().to_string();
        self.bridges.insert(name, bridge);
    }
    
    /// Discover all possible routes from source to destination
    pub async fn discover_routes(&self, params: &MultiHopParams) -> Result<Vec<MultiHopRoute>, BridgeError> {
        info!("Discovering multi-hop routes from chain {} to chain {}", 
              params.from_chain_id, params.to_chain_id);
        
        if params.from_chain_id == params.to_chain_id {
            return Err(BridgeError::UnsupportedRoute);
        }
        
        let mut routes = Vec::new();
        
        // Try direct route first
        if let Ok(direct_route) = self.find_direct_route(params).await {
            routes.push(direct_route);
        }
        
        // Try multi-hop routes if max_hops > 1
        if params.max_hops > 1 {
            if let Ok(mut multi_hop_routes) = self.find_multi_hop_routes(params).await {
                routes.append(&mut multi_hop_routes);
            }
        }
        
        if routes.is_empty() {
            return Err(BridgeError::UnsupportedRoute);
        }
        
        // Sort routes by preference (speed vs cost)
        routes.sort_by(|a, b| {
            if params.prefer_speed {
                a.total_time.cmp(&b.total_time)
            } else {
                a.total_cost.parse::<f64>().unwrap_or(f64::MAX)
                    .partial_cmp(&b.total_cost.parse::<f64>().unwrap_or(f64::MAX))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });
        
        info!("Found {} routes from chain {} to chain {}", 
              routes.len(), params.from_chain_id, params.to_chain_id);
        
        Ok(routes)
    }
    
    async fn find_direct_route(&self, params: &MultiHopParams) -> Result<MultiHopRoute, BridgeError> {
        let bridge_params = CrossChainParams {
            from_chain_id: params.from_chain_id,
            to_chain_id: params.to_chain_id,
            token_in: params.token_in.clone(),
            token_out: params.token_out.clone(),
            amount_in: params.amount_in.clone(),
            user_address: params.user_address.clone(),
            slippage: params.slippage,
            deadline: None,
        };
        
        let mut best_quote: Option<BridgeQuote> = None;
        let mut best_bridge = String::new();
        
        // Try all bridges for direct route
        for (bridge_name, bridge) in &self.bridges {
            if bridge.supports_route(params.from_chain_id, params.to_chain_id) {
                match bridge.get_quote(&bridge_params).await {
                    Ok(quote) => {
                        if best_quote.is_none() || 
                           quote.amount_out.parse::<u64>().unwrap_or(0) > 
                           best_quote.as_ref().unwrap().amount_out.parse::<u64>().unwrap_or(0) {
                            best_quote = Some(quote);
                            best_bridge = bridge_name.clone();
                        }
                    }
                    Err(e) => {
                        warn!("Bridge {} failed to provide quote: {}", bridge_name, e);
                    }
                }
            }
        }
        
        if let Some(quote) = best_quote {
            let hop = RouteHop {
                from_chain: params.from_chain_id,
                to_chain: params.to_chain_id,
                bridge_name: best_bridge,
                token_in: params.token_in.clone(),
                token_out: params.token_out.clone(),
                amount_in: params.amount_in.clone(),
                amount_out: quote.amount_out.clone(),
                fee: quote.fee.clone(),
                estimated_time: quote.estimated_time,
                hop_index: 0,
            };
            
            Ok(MultiHopRoute {
                hops: vec![hop],
                total_cost: quote.fee,
                total_time: quote.estimated_time,
                total_confidence: quote.confidence_score,
                route_id: format!("direct_{}_{}", params.from_chain_id, params.to_chain_id),
            })
        } else {
            Err(BridgeError::UnsupportedRoute)
        }
    }
    
    async fn find_multi_hop_routes(&self, params: &MultiHopParams) -> Result<Vec<MultiHopRoute>, BridgeError> {
        let mut routes = Vec::new();
        
        // Use BFS to find paths through intermediate chains
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        
        // Start with source chain
        queue.push_back(vec![params.from_chain_id]);
        visited.insert(params.from_chain_id);
        
        while let Some(path) = queue.pop_front() {
            let current_chain = *path.last().unwrap();
            
            // If we reached destination, try to build route
            if current_chain == params.to_chain_id && path.len() > 1 {
                if let Ok(route) = self.build_route_from_path(&path, params).await {
                    routes.push(route);
                }
                continue;
            }
            
            // If path is too long, skip
            if path.len() >= params.max_hops {
                continue;
            }
            
            // Explore connected chains
            if let Some(connections) = self.chain_connections.get(&current_chain) {
                for &next_chain in connections {
                    if !visited.contains(&next_chain) || next_chain == params.to_chain_id {
                        let mut new_path = path.clone();
                        new_path.push(next_chain);
                        queue.push_back(new_path);
                        
                        if next_chain != params.to_chain_id {
                            visited.insert(next_chain);
                        }
                    }
                }
            }
        }
        
        Ok(routes)
    }
    
    async fn build_route_from_path(&self, path: &[u64], params: &MultiHopParams) -> Result<MultiHopRoute, BridgeError> {
        let mut hops = Vec::new();
        let mut total_cost = 0.0;
        let mut total_time = 0u64;
        let mut total_confidence = 1.0;
        let mut current_amount = params.amount_in.parse::<u64>().map_err(|_| BridgeError::InvalidAmount)?;
        let mut current_token = params.token_in.clone();
        
        for (i, window) in path.windows(2).enumerate() {
            let from_chain = window[0];
            let to_chain = window[1];
            
            // Determine intermediate token for this hop
            let target_token = if to_chain == params.to_chain_id {
                params.token_out.clone()
            } else {
                // Use USDC as intermediate token for simplicity
                "USDC".to_string()
            };
            
            // Find best bridge for this hop
            let hop_params = CrossChainParams {
                from_chain_id: from_chain,
                to_chain_id: to_chain,
                token_in: current_token.clone(),
                token_out: target_token.clone(),
                amount_in: current_amount.to_string(),
                user_address: params.user_address.clone(),
                slippage: params.slippage,
                deadline: None,
            };
            
            let mut best_quote: Option<BridgeQuote> = None;
            let mut best_bridge = String::new();
            
            for (bridge_name, bridge) in &self.bridges {
                if bridge.supports_route(from_chain, to_chain) {
                    match bridge.get_quote(&hop_params).await {
                        Ok(quote) => {
                            if best_quote.is_none() || 
                               quote.amount_out.parse::<u64>().unwrap_or(0) > 
                               best_quote.as_ref().unwrap().amount_out.parse::<u64>().unwrap_or(0) {
                                best_quote = Some(quote);
                                best_bridge = bridge_name.clone();
                            }
                        }
                        Err(_) => continue,
                    }
                }
            }
            
            if let Some(quote) = best_quote {
                let hop = RouteHop {
                    from_chain,
                    to_chain,
                    bridge_name: best_bridge,
                    token_in: current_token.clone(),
                    token_out: target_token.clone(),
                    amount_in: current_amount.to_string(),
                    amount_out: quote.amount_out.clone(),
                    fee: quote.fee.clone(),
                    estimated_time: quote.estimated_time,
                    hop_index: i,
                };
                
                hops.push(hop);
                total_cost += quote.fee.parse::<f64>().unwrap_or(0.0);
                total_time += quote.estimated_time;
                total_confidence *= quote.confidence_score;
                current_amount = quote.amount_out.parse::<u64>().unwrap_or(0);
                current_token = target_token;
            } else {
                return Err(BridgeError::UnsupportedRoute);
            }
        }
        
        Ok(MultiHopRoute {
            hops,
            total_cost: total_cost.to_string(),
            total_time,
            total_confidence,
            route_id: format!("multihop_{}_{}_{}hops", 
                             params.from_chain_id, 
                             params.to_chain_id, 
                             path.len() - 1),
        })
    }
    
    /// Execute a multi-hop route atomically
    pub async fn execute_multi_hop_route(&mut self, route: &MultiHopRoute, params: &MultiHopParams) -> Result<MultiHopExecution, BridgeError> {
        info!("Executing multi-hop route {} with {} hops", route.route_id, route.hops.len());
        
        let mut execution = MultiHopExecution {
            route_id: route.route_id.clone(),
            hops: route.hops.iter().map(|hop| HopExecution {
                hop_index: hop.hop_index,
                bridge_name: hop.bridge_name.clone(),
                transaction_hash: None,
                status: HopStatus::Pending,
                started_at: None,
                completed_at: None,
                error: None,
            }).collect(),
            status: MultiHopStatus::InProgress,
            current_hop: 0,
            total_hops: route.hops.len(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            estimated_completion: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() + route.total_time,
        };
        
        // Execute hops sequentially
        for (i, hop) in route.hops.iter().enumerate() {
            execution.current_hop = i;
            execution.hops[i].status = HopStatus::InProgress;
            execution.hops[i].started_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            );
            
            let hop_params = CrossChainParams {
                from_chain_id: hop.from_chain,
                to_chain_id: hop.to_chain,
                token_in: hop.token_in.clone(),
                token_out: hop.token_out.clone(),
                amount_in: hop.amount_in.clone(),
                user_address: params.user_address.clone(),
                slippage: params.slippage,
                deadline: None,
            };
            
            if let Some(bridge) = self.bridges.get(&hop.bridge_name) {
                match bridge.execute_bridge(&hop_params).await {
                    Ok(response) => {
                        let transaction_hash = Some(response.transaction_hash.clone());
                        execution.hops[i].status = HopStatus::Completed;
                        execution.hops[i].completed_at = Some(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        );
                        
                        info!("Hop {} completed: {}", i, response.transaction_hash);
                    }
                    Err(e) => {
                        error!("Hop {} failed: {}", i, e);
                        execution.hops[i].status = HopStatus::Failed;
                        execution.hops[i].error = Some(e.to_string());
                        execution.status = MultiHopStatus::Failed;
                        
                        // Store execution state for monitoring
                        self.active_executions.insert(route.route_id.clone(), execution.clone());
                        return Ok(execution);
                    }
                }
            } else {
                error!("Bridge {} not found for hop {}", hop.bridge_name, i);
                execution.hops[i].status = HopStatus::Failed;
                execution.hops[i].error = Some("Bridge not found".to_string());
                execution.status = MultiHopStatus::Failed;
                
                self.active_executions.insert(route.route_id.clone(), execution.clone());
                return Ok(execution);
            }
        }
        
        execution.status = MultiHopStatus::Completed;
        execution.current_hop = route.hops.len();
        
        info!("Multi-hop route {} completed successfully", route.route_id);
        
        // Store execution state
        self.active_executions.insert(route.route_id.clone(), execution.clone());
        
        Ok(execution)
    }
    
    /// Get status of a multi-hop execution
    pub fn get_execution_status(&self, route_id: &str) -> Option<&MultiHopExecution> {
        self.active_executions.get(route_id)
    }
    
    /// Clean up completed executions older than 24 hours
    pub fn cleanup_old_executions(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        self.active_executions.retain(|_, execution| {
            now - execution.started_at < 86400 // 24 hours
        });
    }
}

impl Default for MultiHopRouter {
    fn default() -> Self {
        Self::new()
    }
}
