use crate::types::{QuoteParams, RouteBreakdown};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument};
use std::time::Instant;

use super::direct_routes::DirectRouteManager;
use super::multi_hop::MultiHopRouter;
use super::complex_routes::ComplexRouteManager;
use super::liquidity_tracker::LiquidityTracker;

pub type TokenAddress = String;

#[derive(Clone, Debug)]
pub struct DirectRoute {
    pub dex: String,
    pub pool_address: String,
    pub fee_tier: u32,
    pub liquidity: u64,
    pub estimated_gas: u64,
}

#[derive(Clone, Debug)]
pub struct PathfindingGraph {
    pub nodes: HashMap<TokenAddress, Vec<TokenAddress>>,
    pub edges: HashMap<(TokenAddress, TokenAddress), Vec<DirectRoute>>,
}

#[derive(Clone, Debug)]
pub struct AdvancedPathfinder {
    pub max_hops: u8,
    pub cross_chain_bridges: Vec<String>,
}

pub struct AdvancedRouter {
    // Tier 1: Direct DEX routing (fastest) - <5ms target
    direct_routes: Arc<RwLock<HashMap<(TokenAddress, TokenAddress), Vec<DirectRoute>>>>,
    
    // Tier 2: Multi-hop routing (2-3 hops) - <20ms target
    multi_hop_routes: Arc<RwLock<PathfindingGraph>>,
    
    // Tier 3: Complex routing (4+ hops, cross-chain) - <50ms target
    complex_routes: Arc<RwLock<AdvancedPathfinder>>,
    
    // Real-time liquidity tracking
    liquidity_monitor: Arc<LiquidityTracker>,
    
    // Route managers
    direct_manager: DirectRouteManager,
    multi_hop_manager: MultiHopRouter,
    complex_manager: ComplexRouteManager,
}

impl AdvancedRouter {
    pub async fn new() -> Self {
        let direct_routes = Arc::new(RwLock::new(HashMap::new()));
        let multi_hop_routes = Arc::new(RwLock::new(PathfindingGraph {
            nodes: HashMap::new(),
            edges: HashMap::new(),
        }));
        let complex_routes = Arc::new(RwLock::new(AdvancedPathfinder {
            max_hops: 6,
            cross_chain_bridges: vec![
                "Hop Protocol".to_string(),
                "Across Protocol".to_string(),
                "Stargate".to_string(),
            ],
        }));
        
        let liquidity_monitor = Arc::new(LiquidityTracker::new().await);
        
        // Initialize route managers
        let direct_manager = DirectRouteManager::new(direct_routes.clone()).await;
        let multi_hop_manager = MultiHopRouter::new(multi_hop_routes.clone()).await;
        let complex_manager = ComplexRouteManager::new(complex_routes.clone()).await;
        
        Self {
            direct_routes,
            multi_hop_routes,
            complex_routes,
            liquidity_monitor,
            direct_manager,
            multi_hop_manager,
            complex_manager,
        }
    }

    #[instrument(skip(self))]
    pub async fn get_optimal_route(&self, params: &QuoteParams) -> Result<Vec<RouteBreakdown>, anyhow::Error> {
        let start = Instant::now();
        info!("Starting 3-tier routing for {}->{}", params.token_in, params.token_out);

        // Tier 1: Direct routes (fastest, highest priority)
        let tier1_start = Instant::now();
        let direct_routes = self.direct_manager.find_routes(params).await?;
        let tier1_time = tier1_start.elapsed().as_millis();
        
        if !direct_routes.is_empty() && tier1_time < 5 {
            info!("Tier 1 direct route found in {}ms", tier1_time);
            return Ok(direct_routes);
        }

        // Tier 2: Multi-hop routes (2-3 hops)
        let tier2_start = Instant::now();
        let multi_hop_routes = self.multi_hop_manager.find_multi_hop_routes(params).await?;
        let tier2_time = tier2_start.elapsed().as_millis();
        
        if !multi_hop_routes.is_empty() && (tier1_time + tier2_time) < 20 {
            info!("Tier 2 multi-hop route found in {}ms (total: {}ms)", tier2_time, tier1_time + tier2_time);
            return Ok(self.combine_routes(direct_routes, multi_hop_routes));
        }

        // Tier 3: Complex routes (4+ hops, cross-chain)
        let tier3_start = Instant::now();
        let complex_routes = self.complex_manager.find_complex_routes(params).await?;
        let tier3_time = tier3_start.elapsed().as_millis();
        
        let total_time = start.elapsed().as_millis();
        info!("3-tier routing completed in {}ms (T1: {}ms, T2: {}ms, T3: {}ms)", 
              total_time, tier1_time, tier2_time, tier3_time);

        // Combine all routes and optimize
        let all_routes = self.combine_all_routes(direct_routes, multi_hop_routes, complex_routes);
        Ok(self.optimize_route_combination(all_routes))
    }

    fn combine_routes(&self, tier1: Vec<RouteBreakdown>, tier2: Vec<RouteBreakdown>) -> Vec<RouteBreakdown> {
        let mut combined = tier1;
        combined.extend(tier2);
        self.optimize_route_combination(combined)
    }

    fn combine_all_routes(
        &self, 
        tier1: Vec<RouteBreakdown>, 
        tier2: Vec<RouteBreakdown>, 
        tier3: Vec<RouteBreakdown>
    ) -> Vec<RouteBreakdown> {
        let mut all_routes = tier1;
        all_routes.extend(tier2);
        all_routes.extend(tier3);
        all_routes
    }

    fn optimize_route_combination(&self, routes: Vec<RouteBreakdown>) -> Vec<RouteBreakdown> {
        if routes.is_empty() {
            return routes;
        }

        // Sort by efficiency (amount_out / gas_used ratio)
        let mut sorted_routes = routes;
        sorted_routes.sort_by(|a, b| {
            let efficiency_a = self.calculate_efficiency(a);
            let efficiency_b = self.calculate_efficiency(b);
            efficiency_b.partial_cmp(&efficiency_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top 3 routes and optimize percentages
        let top_routes: Vec<_> = sorted_routes.into_iter().take(3).collect();
        self.calculate_optimal_percentages(top_routes)
    }

    fn calculate_efficiency(&self, route: &RouteBreakdown) -> f64 {
        let amount_out = route.amount_out.parse::<u64>().unwrap_or(0) as f64;
        let gas_used = route.gas_used.parse::<u64>().unwrap_or(1) as f64;
        amount_out / gas_used
    }

    fn calculate_optimal_percentages(&self, mut routes: Vec<RouteBreakdown>) -> Vec<RouteBreakdown> {
        let total_routes = routes.len();
        
        match total_routes {
            1 => {
                routes[0].percentage = 100.0;
                routes
            }
            2 => {
                // 70-30 split favoring better route
                routes[0].percentage = 70.0;
                routes[1].percentage = 30.0;
                routes
            }
            3 => {
                // 50-30-20 split
                routes[0].percentage = 50.0;
                routes[1].percentage = 30.0;
                routes[2].percentage = 20.0;
                routes
            }
            _ => {
                // Distribute evenly for more than 3 routes
                let percentage = 100.0 / total_routes as f64;
                for route in &mut routes {
                    route.percentage = percentage;
                }
                routes
            }
        }
    }

    pub async fn update_liquidity_data(&self) {
        self.liquidity_monitor.update_all_pools().await;
    }

    pub async fn get_route_statistics(&self) -> RouteStatistics {
        let direct_count = self.direct_routes.read().await.len();
        let multi_hop_count = self.multi_hop_routes.read().await.edges.len();
        let complex_count = self.complex_routes.read().await.cross_chain_bridges.len();

        RouteStatistics {
            direct_routes_available: direct_count,
            multi_hop_paths: multi_hop_count,
            complex_routes: complex_count,
            last_update: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteStatistics {
    pub direct_routes_available: usize,
    pub multi_hop_paths: usize,
    pub complex_routes: usize,
    pub last_update: Instant,
}
