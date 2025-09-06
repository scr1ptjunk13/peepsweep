use crate::aggregator::DEXAggregator;
use crate::routing::user_preferences::{RoutingPreferences, OptimizationStrategy, MevProtectionLevel, UserPreferenceManager};
use crate::types::{Route, RouteRequest};
use crate::risk_management::types::{UserId, RiskError};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use std::str::FromStr;

/// Preference-aware routing engine that customizes route selection based on user preferences
pub struct PreferenceRouter {
    base_aggregator: Arc<DEXAggregator>,
    preference_manager: Arc<UserPreferenceManager>,
    learning_engine: RouteLearningEngine,
}

impl PreferenceRouter {
    /// Create new preference router
    pub fn new(
        base_aggregator: Arc<DEXAggregator>,
        preference_manager: Arc<UserPreferenceManager>,
    ) -> Self {
        Self {
            base_aggregator,
            preference_manager,
            learning_engine: RouteLearningEngine::new(),
        }
    }

    /// Get optimized route based on user preferences
    pub async fn get_preference_optimized_route(
        &self,
        user_id: UserId,
        params: RouteRequest,
    ) -> Result<PreferenceOptimizedRoute, RiskError> {
        // Get user preferences
        let preferences = self.preference_manager.get_preferences(user_id).await?;
        
        // Get base routes from aggregator by converting to QuoteParams
        let quote_params = crate::types::QuoteParams {
            token_in: params.from_token.clone(),
            token_in_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
            token_in_decimals: Some(18),
            token_out: "USDC".to_string(),
            token_out_address: Some("0x0000000000000000000000000000000000000000".to_string()), // Placeholder
            token_out_decimals: Some(18),
            amount_in: params.amount.to_string(),
            slippage: Some(0.5),
            chain: None,
        };
        
        let quote_response = self.base_aggregator.get_optimal_route(quote_params)
            .await.map_err(|e| RiskError::RoutingError(e.to_string()))?;
        
        // Convert QuoteResponse to Route format for preference processing
        let base_routes = vec![self.convert_quote_to_route(&quote_response, &params)?];

        // Filter routes based on preferences
        let filtered_routes = self.filter_routes_by_preferences(&base_routes, &preferences)?;
        
        if filtered_routes.is_empty() {
            return Err(RiskError::RoutingError("No routes available after applying preferences".to_string()));
        }

        // Score and rank routes based on preferences
        let scored_routes = self.score_routes_by_preferences(&filtered_routes, &preferences)?;
        
        // Select best route
        let best_route = scored_routes.into_iter()
            .max_by(|a, b| a.preference_score.partial_cmp(&b.preference_score).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| RiskError::RoutingError("Failed to select best route".to_string()))?;

        // Apply learning feedback
        self.learning_engine.record_route_selection(user_id, &best_route).await;

        Ok(best_route)
    }

    /// Filter routes based on user preferences
    fn filter_routes_by_preferences(
        &self,
        routes: &[Route],
        preferences: &RoutingPreferences,
    ) -> Result<Vec<Route>, RiskError> {
        let mut filtered_routes = Vec::new();

        for route in routes {
            // Check hop count limit
            if route.hops.len() > preferences.max_hop_count as usize {
                continue;
            }

            // Check slippage tolerance
            if route.estimated_slippage > preferences.max_slippage_tolerance {
                continue;
            }

            // Check liquidity threshold
            if route.liquidity_usd < preferences.min_liquidity_threshold {
                continue;
            }

            // Check DEX allowances
            let mut route_allowed = true;
            for hop in &route.hops {
                if !preferences.is_dex_allowed(&hop.dex_name) || !preferences.is_dex_enabled(&hop.dex_name) {
                    route_allowed = false;
                    break;
                }
            }
            if !route_allowed {
                continue;
            }

            // Check token allowances
            if !preferences.is_token_allowed(&route.from_token) || !preferences.is_token_allowed(&route.to_token) {
                continue;
            }

            filtered_routes.push(route.clone());
        }

        Ok(filtered_routes)
    }

    /// Convert QuoteResponse to Route format for preference processing
    fn convert_quote_to_route(
        &self,
        quote_response: &crate::types::QuoteResponse,
        request: &RouteRequest,
    ) -> Result<Route, RiskError> {
        let mut hops = Vec::new();
        
        // Convert route breakdowns to hops
        for route_breakdown in &quote_response.routes {
            let hop = crate::types::RouteHop {
                dex_name: route_breakdown.dex.clone(),
                from_token: request.from_token.clone(),
                to_token: request.to_token.clone(),
                amount_in: request.amount,
                amount_out: Decimal::from_str(&route_breakdown.amount_out)
                    .map_err(|e| RiskError::RoutingError(format!("Invalid amount_out: {}", e)))?,
                gas_estimate: Decimal::from_str(&route_breakdown.gas_used)
                    .map_err(|e| RiskError::RoutingError(format!("Invalid gas_used: {}", e)))?,
                pool_address: None,
            };
            hops.push(hop);
        }

        let route = Route {
            from_token: request.from_token.clone(),
            to_token: request.to_token.clone(),
            input_amount: request.amount,
            output_amount: Decimal::from_str(&quote_response.amount_out)
                .map_err(|e| RiskError::RoutingError(format!("Invalid amount_out: {}", e)))?,
            hops,
            gas_estimate: Decimal::from_str(&quote_response.gas_estimate)
                .map_err(|e| RiskError::RoutingError(format!("Invalid gas_estimate: {}", e)))?,
            estimated_slippage: Decimal::try_from(quote_response.price_impact).unwrap_or(Decimal::ZERO),
            liquidity_usd: Decimal::new(1000000, 0), // Default $1M liquidity estimate
            execution_time_estimate_ms: quote_response.response_time as u64,
            mev_protection_level: "standard".to_string(),
        };

        Ok(route)
    }

    /// Score routes based on user preferences
    fn score_routes_by_preferences(
        &self,
        routes: &[Route],
        preferences: &RoutingPreferences,
    ) -> Result<Vec<PreferenceOptimizedRoute>, RiskError> {
        let mut scored_routes = Vec::new();

        for route in routes {
            let score = self.calculate_preference_score(route, preferences)?;
            
            scored_routes.push(PreferenceOptimizedRoute {
                route: route.clone(),
                preference_score: score,
                optimization_breakdown: self.get_optimization_breakdown(route, preferences)?,
                user_id: preferences.user_id,
                applied_strategy: preferences.optimization_strategy.clone(),
            });
        }

        Ok(scored_routes)
    }

    /// Calculate preference score for a route
    fn calculate_preference_score(
        &self,
        route: &Route,
        preferences: &RoutingPreferences,
    ) -> Result<Decimal, RiskError> {
        let strategy_weights = self.get_strategy_weights(&preferences.optimization_strategy);
        
        // Base scoring factors
        let price_score = self.calculate_price_score(route)?;
        let speed_score = self.calculate_speed_score(route)?;
        let gas_score = self.calculate_gas_score(route)?;
        let security_score = self.calculate_security_score(route, &preferences.mev_protection_level)?;
        let liquidity_score = self.calculate_liquidity_score(route)?;
        
        // DEX preference multipliers
        let dex_preference_multiplier = self.calculate_dex_preference_multiplier(route, preferences)?;
        
        // Weighted score calculation
        let weighted_score = (
            price_score * strategy_weights.price_weight +
            speed_score * strategy_weights.speed_weight +
            gas_score * strategy_weights.gas_weight +
            security_score * strategy_weights.security_weight +
            liquidity_score * strategy_weights.liquidity_weight
        ) * dex_preference_multiplier;

        Ok(weighted_score)
    }

    /// Get strategy weights based on optimization strategy
    fn get_strategy_weights(&self, strategy: &OptimizationStrategy) -> StrategyWeights {
        match strategy {
            OptimizationStrategy::SpeedFirst => StrategyWeights {
                speed_weight: Decimal::new(40, 2), // 0.4
                price_weight: Decimal::new(20, 2), // 0.2
                gas_weight: Decimal::new(10, 2),   // 0.1
                security_weight: Decimal::new(15, 2), // 0.15
                liquidity_weight: Decimal::new(15, 2), // 0.15
            },
            OptimizationStrategy::BestPrice => StrategyWeights {
                speed_weight: Decimal::new(10, 2), // 0.1
                price_weight: Decimal::new(50, 2), // 0.5
                gas_weight: Decimal::new(15, 2),   // 0.15
                security_weight: Decimal::new(10, 2), // 0.1
                liquidity_weight: Decimal::new(15, 2), // 0.15
            },
            OptimizationStrategy::MevProtected => StrategyWeights {
                speed_weight: Decimal::new(15, 2), // 0.15
                price_weight: Decimal::new(20, 2), // 0.2
                gas_weight: Decimal::new(10, 2),   // 0.1
                security_weight: Decimal::new(40, 2), // 0.4
                liquidity_weight: Decimal::new(15, 2), // 0.15
            },
            OptimizationStrategy::GasOptimized => StrategyWeights {
                speed_weight: Decimal::new(15, 2), // 0.15
                price_weight: Decimal::new(20, 2), // 0.2
                gas_weight: Decimal::new(40, 2),   // 0.4
                security_weight: Decimal::new(10, 2), // 0.1
                liquidity_weight: Decimal::new(15, 2), // 0.15
            },
            OptimizationStrategy::Balanced => StrategyWeights {
                speed_weight: Decimal::new(20, 2), // 0.2
                price_weight: Decimal::new(25, 2), // 0.25
                gas_weight: Decimal::new(20, 2),   // 0.2
                security_weight: Decimal::new(20, 2), // 0.2
                liquidity_weight: Decimal::new(15, 2), // 0.15
            },
            OptimizationStrategy::Custom(custom) => StrategyWeights {
                speed_weight: custom.speed_weight,
                price_weight: custom.price_weight,
                gas_weight: custom.gas_weight,
                security_weight: custom.security_weight,
                liquidity_weight: custom.liquidity_weight,
            },
        }
    }

    /// Calculate price score (higher is better)
    fn calculate_price_score(&self, route: &Route) -> Result<Decimal, RiskError> {
        // Normalize based on expected output amount (higher output = better score)
        let max_possible_output = route.input_amount; // Theoretical maximum
        let score = route.output_amount / max_possible_output;
        Ok(score.min(Decimal::ONE))
    }

    /// Calculate speed score (lower execution time = higher score)
    fn calculate_speed_score(&self, route: &Route) -> Result<Decimal, RiskError> {
        // Score based on number of hops (fewer hops = faster)
        let hop_penalty = Decimal::new(1, 1) * Decimal::from(route.hops.len()); // 0.1 per hop
        let score = Decimal::ONE - hop_penalty;
        Ok(score.max(Decimal::ZERO))
    }

    /// Calculate gas score (lower gas = higher score)
    fn calculate_gas_score(&self, route: &Route) -> Result<Decimal, RiskError> {
        // Normalize gas cost (assuming max reasonable gas is 500k)
        let max_gas = Decimal::new(500000, 0);
        let score = (max_gas - route.gas_estimate) / max_gas;
        Ok(score.max(Decimal::ZERO))
    }

    /// Calculate security score based on MEV protection level
    fn calculate_security_score(
        &self,
        route: &Route,
        mev_protection: &MevProtectionLevel,
    ) -> Result<Decimal, RiskError> {
        let base_score = match mev_protection {
            MevProtectionLevel::None => Decimal::new(50, 2), // 0.5
            MevProtectionLevel::Basic => Decimal::new(70, 2), // 0.7
            MevProtectionLevel::Medium => Decimal::try_from(1.0).unwrap(), // 1.0
            MevProtectionLevel::High => Decimal::new(85, 2), // 0.85
            MevProtectionLevel::Maximum => Decimal::new(95, 2), // 0.95
        };

        // Adjust based on route characteristics
        let mut score = base_score;
        
        // Private mempools get security bonus
        for hop in &route.hops {
            if hop.dex_name.contains("CoW") || hop.dex_name.contains("Flashbots") {
                score += Decimal::new(10, 2); // 0.1 bonus
            }
        }

        Ok(score.min(Decimal::ONE))
    }

    /// Calculate liquidity score
    fn calculate_liquidity_score(&self, route: &Route) -> Result<Decimal, RiskError> {
        // Normalize liquidity (assuming $10M is excellent liquidity)
        let excellent_liquidity = Decimal::new(10_000_000, 0);
        let score = (route.liquidity_usd / excellent_liquidity).min(Decimal::ONE);
        Ok(score)
    }

    /// Calculate DEX preference multiplier
    fn calculate_dex_preference_multiplier(
        &self,
        route: &Route,
        preferences: &RoutingPreferences,
    ) -> Result<Decimal, RiskError> {
        let mut total_weight = Decimal::ZERO;
        let mut hop_count = Decimal::ZERO;

        for hop in &route.hops {
            let dex_weight = preferences.get_dex_weight(&hop.dex_name);
            total_weight += dex_weight;
            hop_count += Decimal::ONE;
        }

        if hop_count > Decimal::ZERO {
            Ok(total_weight / hop_count)
        } else {
            Ok(Decimal::ONE)
        }
    }

    /// Get optimization breakdown for transparency
    fn get_optimization_breakdown(
        &self,
        route: &Route,
        preferences: &RoutingPreferences,
    ) -> Result<OptimizationBreakdown, RiskError> {
        Ok(OptimizationBreakdown {
            price_score: self.calculate_price_score(route)?,
            speed_score: self.calculate_speed_score(route)?,
            gas_score: self.calculate_gas_score(route)?,
            security_score: self.calculate_security_score(route, &preferences.mev_protection_level)?,
            liquidity_score: self.calculate_liquidity_score(route)?,
            dex_preference_multiplier: self.calculate_dex_preference_multiplier(route, preferences)?,
            applied_filters: self.get_applied_filters(preferences),
        })
    }

    /// Get list of applied filters for transparency
    fn get_applied_filters(&self, preferences: &RoutingPreferences) -> Vec<String> {
        let mut filters = Vec::new();
        
        if preferences.max_hop_count < 5 {
            filters.push(format!("Max {} hops", preferences.max_hop_count));
        }
        
        if !preferences.blacklisted_dexs.is_empty() {
            filters.push(format!("Blacklisted {} DEXs", preferences.blacklisted_dexs.len()));
        }
        
        if preferences.whitelisted_dexs.is_some() {
            filters.push("DEX whitelist active".to_string());
        }
        
        if preferences.max_slippage_tolerance < Decimal::new(1, 0) {
            filters.push(format!("Max {}% slippage", preferences.max_slippage_tolerance));
        }
        
        filters
    }
}

/// Strategy weights for route optimization
#[derive(Debug, Clone)]
struct StrategyWeights {
    pub speed_weight: Decimal,
    pub price_weight: Decimal,
    pub gas_weight: Decimal,
    pub security_weight: Decimal,
    pub liquidity_weight: Decimal,
}

/// Preference-optimized route with scoring details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceOptimizedRoute {
    pub route: Route,
    pub preference_score: Decimal,
    pub optimization_breakdown: OptimizationBreakdown,
    pub user_id: UserId,
    pub applied_strategy: OptimizationStrategy,
}

/// Detailed breakdown of optimization scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationBreakdown {
    pub price_score: Decimal,
    pub speed_score: Decimal,
    pub gas_score: Decimal,
    pub security_score: Decimal,
    pub liquidity_score: Decimal,
    pub dex_preference_multiplier: Decimal,
    pub applied_filters: Vec<String>,
}

/// Learning engine to improve preferences over time
pub struct RouteLearningEngine {
    route_history: Arc<tokio::sync::RwLock<HashMap<UserId, Vec<RoutePerformance>>>>,
}

impl RouteLearningEngine {
    pub fn new() -> Self {
        Self {
            route_history: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Record route selection for learning
    pub async fn record_route_selection(&self, user_id: UserId, route: &PreferenceOptimizedRoute) {
        let performance = RoutePerformance {
            route_id: format!("{}_{}", route.route.from_token, route.route.to_token),
            selected_at: chrono::Utc::now().timestamp() as u64,
            preference_score: route.preference_score,
            actual_performance: None, // Will be updated when execution completes
        };

        let mut history = self.route_history.write().await;
        history.entry(user_id).or_insert_with(Vec::new).push(performance);
    }

    /// Update route performance after execution
    pub async fn update_route_performance(
        &self,
        user_id: UserId,
        route_id: &str,
        actual_slippage: Decimal,
        execution_time_ms: u64,
        gas_used: Decimal,
    ) {
        let mut history = self.route_history.write().await;
        if let Some(user_history) = history.get_mut(&user_id) {
            for performance in user_history.iter_mut().rev() {
                if performance.route_id == route_id && performance.actual_performance.is_none() {
                    performance.actual_performance = Some(ActualPerformance {
                        actual_slippage,
                        execution_time_ms,
                        gas_used,
                        success: true,
                    });
                    break;
                }
            }
        }
    }

    /// Get learning insights for preference optimization
    pub async fn get_learning_insights(&self, user_id: UserId) -> Option<LearningInsights> {
        let history = self.route_history.read().await;
        let user_history = history.get(&user_id)?;
        
        if user_history.len() < 5 {
            return None; // Need more data
        }

        // Analyze performance patterns
        let successful_routes: Vec<_> = user_history.iter()
            .filter(|p| p.actual_performance.as_ref().map_or(false, |ap| ap.success))
            .collect();

        if successful_routes.is_empty() {
            return None;
        }

        let avg_slippage: Decimal = successful_routes.iter()
            .filter_map(|p| p.actual_performance.as_ref().map(|ap| ap.actual_slippage))
            .sum::<Decimal>() / Decimal::from(successful_routes.len());

        let avg_gas: Decimal = successful_routes.iter()
            .filter_map(|p| p.actual_performance.as_ref().map(|ap| ap.gas_used))
            .sum::<Decimal>() / Decimal::from(successful_routes.len());

        Some(LearningInsights {
            total_routes: user_history.len(),
            successful_routes: successful_routes.len(),
            average_slippage: avg_slippage,
            average_gas_used: avg_gas,
            recommendations: self.generate_recommendations(&successful_routes),
        })
    }

    /// Generate preference recommendations based on learning
    fn generate_recommendations(&self, successful_routes: &[&RoutePerformance]) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Analyze slippage patterns
        let high_slippage_count = successful_routes.iter()
            .filter(|p| p.actual_performance.as_ref()
                .map_or(false, |ap| ap.actual_slippage > Decimal::new(1, 2))) // > 1%
            .count();
        
        if high_slippage_count > successful_routes.len() / 2 {
            recommendations.push("Consider lowering max slippage tolerance".to_string());
        }

        // Analyze gas usage patterns
        let high_gas_count = successful_routes.iter()
            .filter(|p| p.actual_performance.as_ref()
                .map_or(false, |ap| ap.gas_used > Decimal::new(300000, 0))) // > 300k gas
            .count();
        
        if high_gas_count > successful_routes.len() / 2 {
            recommendations.push("Consider enabling gas optimization strategy".to_string());
        }

        recommendations
    }
}

/// Route performance tracking for learning
#[derive(Debug, Clone)]
struct RoutePerformance {
    pub route_id: String,
    pub selected_at: u64,
    pub preference_score: Decimal,
    pub actual_performance: Option<ActualPerformance>,
}

/// Actual execution performance
#[derive(Debug, Clone)]
struct ActualPerformance {
    pub actual_slippage: Decimal,
    pub execution_time_ms: u64,
    pub gas_used: Decimal,
    pub success: bool,
}

/// Learning insights for preference optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningInsights {
    pub total_routes: usize,
    pub successful_routes: usize,
    pub average_slippage: Decimal,
    pub average_gas_used: Decimal,
    pub recommendations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_strategy_weights() {
        let router = create_test_router();
        
        let speed_weights = router.get_strategy_weights(&OptimizationStrategy::SpeedFirst);
        assert_eq!(speed_weights.speed_weight, Decimal::new(40, 2));
        
        let price_weights = router.get_strategy_weights(&OptimizationStrategy::BestPrice);
        assert_eq!(price_weights.price_weight, Decimal::new(50, 2));
    }

    #[test]
    fn test_score_calculations() {
        let router = create_test_router();
        let route = create_test_route();
        
        let price_score = router.calculate_price_score(&route).unwrap();
        assert!(price_score >= Decimal::ZERO && price_score <= Decimal::ONE);
        
        let speed_score = router.calculate_speed_score(&route).unwrap();
        assert!(speed_score >= Decimal::ZERO && speed_score <= Decimal::ONE);
    }

    fn create_test_router() -> PreferenceRouter {
        use crate::aggregator::DEXAggregator;
        use crate::routing::user_preferences::UserPreferenceManager;
        use std::sync::Arc;
        use tokio::runtime::Runtime;
        
        // Create runtime for async operations
        let rt = Runtime::new().unwrap();
        
        // Create mock Redis client for testing
        let redis_client = redis::Client::open("redis://127.0.0.1/").unwrap();
        let dex_aggregator = rt.block_on(async {
            Arc::new(DEXAggregator::new(redis_client).await.unwrap())
        });
        let preference_manager = Arc::new(UserPreferenceManager::new());
        
        PreferenceRouter::new(dex_aggregator, preference_manager)
    }

    fn create_test_route() -> Route {
        use crate::types::{Route, RouteHop};
        use rust_decimal::Decimal;
        
        Route {
            from_token: "ETH".to_string(),
            to_token: "USDC".to_string(),
            input_amount: Decimal::new(1000, 0),
            output_amount: Decimal::new(950, 0),
            hops: vec![RouteHop {
                dex_name: "uniswap".to_string(),
                from_token: "ETH".to_string(),
                to_token: "USDC".to_string(),
                amount_in: Decimal::new(1000, 0),
                amount_out: Decimal::new(950, 0),
                gas_estimate: Decimal::new(150000, 0),
                pool_address: Some("0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640".to_string()),
            }],
            gas_estimate: Decimal::new(150000, 0),
            estimated_slippage: Decimal::new(5, 3), // 0.5%
            liquidity_usd: Decimal::new(1000000, 0), // $1M
            execution_time_estimate_ms: 15000, // 15 seconds
            mev_protection_level: "medium".to_string(),
        }
    }
}
