use crate::types::{QuoteParams, RouteBreakdown};
use std::collections::{HashMap, VecDeque, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument};
use std::time::Instant;

use super::advanced_router::{TokenAddress, PathfindingGraph, DirectRoute};

pub struct MultiHopRouter {
    graph: Arc<RwLock<PathfindingGraph>>,
    max_hops: u8,
    intermediate_tokens: Vec<String>, // Common tokens used as bridges
}

impl MultiHopRouter {
    pub async fn new(graph: Arc<RwLock<PathfindingGraph>>) -> Self {
        let intermediate_tokens = vec![
            "WETH".to_string(),
            "USDC".to_string(),
            "USDT".to_string(),
            "DAI".to_string(),
            "WBTC".to_string(),
        ];

        let router = Self {
            graph,
            max_hops: 3,
            intermediate_tokens,
        };

        // Initialize the pathfinding graph
        router.initialize_graph().await;
        
        router
    }

    async fn initialize_graph(&self) {
        let mut graph = self.graph.write().await;
        
        // Initialize nodes (tokens)
        let tokens = vec![
            "ETH", "WETH", "USDC", "USDT", "DAI", "WBTC", 
            "UNI", "LINK", "AAVE", "COMP", "MKR", "SNX",
            "CRV", "BAL", "SUSHI", "YFI", "1INCH"
        ];

        for token in &tokens {
            graph.nodes.insert(token.to_string(), Vec::new());
        }

        // Create connections between tokens (edges)
        self.add_major_pairs(&mut graph).await;
        
        info!("Initialized pathfinding graph with {} nodes and {} edges", 
              graph.nodes.len(), graph.edges.len());
    }

    async fn add_major_pairs(&self, graph: &mut PathfindingGraph) {
        // ETH pairs
        self.add_bidirectional_edge(graph, "ETH", "USDC", vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640".to_string(),
                fee_tier: 500,
                liquidity: 50_000_000,
                estimated_gas: 150_000,
            }
        ]).await;

        self.add_bidirectional_edge(graph, "ETH", "USDT", vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x11b815efb8f581194ae79006d24e0d814b7697f6".to_string(),
                fee_tier: 3000,
                liquidity: 180_000_000,
                estimated_gas: 150_000,
            }
        ]).await;

        self.add_bidirectional_edge(graph, "ETH", "WBTC", vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0xcbcdf9626bc03e24f779434178a73a0b4bad62ed".to_string(),
                fee_tier: 3000,
                liquidity: 75_000_000,
                estimated_gas: 150_000,
            }
        ]).await;

        // Stablecoin pairs
        self.add_bidirectional_edge(graph, "USDC", "USDT", vec![
            DirectRoute {
                dex: "Curve".to_string(),
                pool_address: "0xbebc44782c7db0a1a60cb6fe97d0b483032ff1c7".to_string(),
                fee_tier: 4,
                liquidity: 500_000_000,
                estimated_gas: 120_000,
            }
        ]).await;

        self.add_bidirectional_edge(graph, "USDC", "DAI", vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x5777d92f208679db4b9778590fa3cab3ac9e2168".to_string(),
                fee_tier: 100,
                liquidity: 80_000_000,
                estimated_gas: 150_000,
            }
        ]).await;

        // DeFi token pairs
        self.add_bidirectional_edge(graph, "ETH", "UNI", vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0x1d42064fc4beb5f8aaf85f4617ae8b3b5b8bd801".to_string(),
                fee_tier: 3000,
                liquidity: 30_000_000,
                estimated_gas: 150_000,
            }
        ]).await;

        self.add_bidirectional_edge(graph, "ETH", "LINK", vec![
            DirectRoute {
                dex: "Uniswap V3".to_string(),
                pool_address: "0xa6cc3c2531fdaa6ae1a3ca84c2855806728693e8".to_string(),
                fee_tier: 3000,
                liquidity: 25_000_000,
                estimated_gas: 150_000,
            }
        ]).await;
    }

    async fn add_bidirectional_edge(&self, graph: &mut PathfindingGraph, token_a: &str, token_b: &str, routes: Vec<DirectRoute>) {
        // Add A -> B
        graph.nodes.entry(token_a.to_string())
            .or_insert_with(Vec::new)
            .push(token_b.to_string());
        
        graph.edges.insert(
            (token_a.to_string(), token_b.to_string()),
            routes.clone()
        );

        // Add B -> A
        graph.nodes.entry(token_b.to_string())
            .or_insert_with(Vec::new)
            .push(token_a.to_string());
        
        graph.edges.insert(
            (token_b.to_string(), token_a.to_string()),
            routes
        );
    }

    #[instrument(skip(self))]
    pub async fn find_multi_hop_routes(&self, params: &QuoteParams) -> Result<Vec<RouteBreakdown>, anyhow::Error> {
        let start = Instant::now();
        
        let paths = self.find_all_paths(&params.token_in, &params.token_out).await;
        
        if paths.is_empty() {
            info!("No multi-hop paths found for {}->{}", params.token_in, params.token_out);
            return Ok(Vec::new());
        }

        let mut route_breakdowns = Vec::new();
        
        for path in paths.iter().take(5) { // Limit to top 5 paths
            if let Some(route) = self.path_to_route_breakdown(path, params).await {
                route_breakdowns.push(route);
            }
        }

        // Sort by estimated output (best first)
        route_breakdowns.sort_by(|a, b| {
            let amount_a = a.amount_out.parse::<u64>().unwrap_or(0);
            let amount_b = b.amount_out.parse::<u64>().unwrap_or(0);
            amount_b.cmp(&amount_a)
        });

        let elapsed = start.elapsed().as_millis();
        info!("Found {} multi-hop routes in {}ms", route_breakdowns.len(), elapsed);
        
        Ok(route_breakdowns)
    }

    async fn find_all_paths(&self, start: &str, end: &str) -> Vec<Vec<String>> {
        let graph = self.graph.read().await;
        let mut paths = Vec::new();
        let mut queue = VecDeque::new();
        
        // BFS to find all paths up to max_hops
        queue.push_back((start.to_string(), vec![start.to_string()], HashSet::new()));
        
        while let Some((current, path, visited)) = queue.pop_front() {
            if path.len() > self.max_hops as usize + 1 {
                continue;
            }
            
            if current == end && path.len() > 1 {
                paths.push(path.clone());
                continue;
            }
            
            if let Some(neighbors) = graph.nodes.get(&current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        let mut new_path = path.clone();
                        new_path.push(neighbor.clone());
                        
                        let mut new_visited = visited.clone();
                        new_visited.insert(current.clone());
                        
                        queue.push_back((neighbor.clone(), new_path, new_visited));
                    }
                }
            }
        }
        
        // Sort paths by length (shorter paths first)
        paths.sort_by_key(|path| path.len());
        paths
    }

    async fn path_to_route_breakdown(&self, path: &[String], params: &QuoteParams) -> Option<RouteBreakdown> {
        if path.len() < 2 {
            return None;
        }

        let graph = self.graph.read().await;
        let mut total_gas = 0u64;
        let mut route_description = Vec::new();
        let mut estimated_output = params.amount_in.parse::<u64>().unwrap_or(0) as f64;

        // Calculate route through each hop
        for i in 0..path.len() - 1 {
            let from = &path[i];
            let to = &path[i + 1];
            
            if let Some(routes) = graph.edges.get(&(from.clone(), to.clone())) {
                if let Some(best_route) = routes.first() {
                    total_gas += best_route.estimated_gas;
                    route_description.push(format!("{}->{} via {}", from, to, best_route.dex));
                    
                    // Simplified output estimation for multi-hop
                    estimated_output = self.estimate_hop_output(estimated_output, from, to);
                }
            } else {
                return None; // Path not viable
            }
        }

        Some(RouteBreakdown {
            dex: format!("Multi-hop: {}", route_description.join(" → ")),
            percentage: 100.0,
            amount_out: (estimated_output as u64).to_string(),
            gas_used: total_gas.to_string(),
        })
    }

    fn estimate_hop_output(&self, input_amount: f64, from: &str, to: &str) -> f64 {
        // Simplified conversion rates with slippage
        let base_rate = match (from, to) {
            ("ETH", "USDC") | ("ETH", "USDT") => input_amount / 1e18 * 3400.0 * 1e6,
            ("USDC", "ETH") | ("USDT", "ETH") => input_amount / 1e6 / 3400.0 * 1e18,
            ("USDC", "USDT") | ("USDT", "USDC") => input_amount * 0.9999,
            ("ETH", "WBTC") => input_amount / 1e18 * 3400.0 / 65000.0 * 1e8, // ETH to BTC rate
            ("WBTC", "ETH") => input_amount / 1e8 * 65000.0 / 3400.0 * 1e18,
            _ => input_amount * 0.95, // Default 5% conversion for unknown pairs
        };

        // Apply multi-hop slippage (0.3% per hop)
        base_rate * 0.997
    }

    pub async fn add_token_pair(&self, token_a: String, token_b: String, routes: Vec<DirectRoute>) {
        let mut graph = self.graph.write().await;
        self.add_bidirectional_edge(&mut graph, &token_a, &token_b, routes).await;
    }

    pub async fn get_path_count(&self) -> usize {
        let graph = self.graph.read().await;
        graph.edges.len()
    }
}
