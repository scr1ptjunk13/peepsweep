use crate::crosschain::arbitrage_detector::{ArbitrageDetector, ArbitrageOpportunity, PriceMonitoringStatus, CrossChainPrice, PriceAnomaly};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct ArbitrageApiState {
    pub detector: Arc<RwLock<ArbitrageDetector>>,
    pub opportunities_cache: Arc<RwLock<Vec<ArbitrageOpportunity>>>,
    pub last_update: Arc<RwLock<u64>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OpportunitiesQuery {
    pub min_profit_usd: Option<f64>,
    pub min_profit_percentage: Option<f64>,
    pub from_chain: Option<u64>,
    pub to_chain: Option<u64>,
    pub token: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct PriceQuery {
    pub token: String,
    pub chain_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AnomalyQuery {
    pub token: String,
    pub threshold_percentage: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ArbitrageApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Serialize)]
pub struct OpportunitiesResponse {
    pub opportunities: Vec<ArbitrageOpportunity>,
    pub total_count: usize,
    pub filters_applied: OpportunitiesQuery,
    pub last_updated: u64,
}

#[derive(Debug, Serialize)]
pub struct PriceResponse {
    pub prices: Vec<CrossChainPrice>,
    pub token: String,
    pub price_spread_percentage: f64,
    pub highest_price_chain: String,
    pub lowest_price_chain: String,
}

#[derive(Debug, Serialize)]
pub struct MonitoringResponse {
    pub status: PriceMonitoringStatus,
    pub recent_opportunities_count: usize,
    pub total_chains_monitored: u32,
    pub average_profit_usd: f64,
    pub top_profitable_pairs: Vec<String>,
}

impl ArbitrageApiState {
    pub fn new(detector: ArbitrageDetector) -> Self {
        Self {
            detector: Arc::new(RwLock::new(detector)),
            opportunities_cache: Arc::new(RwLock::new(Vec::new())),
            last_update: Arc::new(RwLock::new(0)),
        }
    }

    pub fn from_shared(detector: Arc<RwLock<ArbitrageDetector>>) -> Self {
        Self {
            detector,
            opportunities_cache: Arc::new(RwLock::new(Vec::new())),
            last_update: Arc::new(RwLock::new(0)),
        }
    }

    /// Update opportunities cache in background
    pub async fn update_opportunities_cache(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut detector = self.detector.write().await;
        let opportunities = detector.detect_opportunities().await?;
        
        let mut cache = self.opportunities_cache.write().await;
        *cache = opportunities;
        
        let mut last_update = self.last_update.write().await;
        *last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        info!("📊 Updated arbitrage opportunities cache with {} opportunities", cache.len());
        Ok(())
    }
}

pub fn create_arbitrage_router(state: ArbitrageApiState) -> Router {
    Router::new()
        .route("/opportunities", get(get_arbitrage_opportunities))
        .route("/opportunities/refresh", get(|| async { "refresh endpoint" }))
        .route("/prices", get(get_cross_chain_prices))
        .route("/anomalies", get(get_price_anomalies))
        .route("/monitoring", get(get_monitoring_status))
        .route("/health", get(get_arbitrage_health))
        .with_state(state)
}

/// GET /arbitrage/opportunities - Get current arbitrage opportunities
async fn get_arbitrage_opportunities(
    State(state): State<ArbitrageApiState>,
    Query(query): Query<OpportunitiesQuery>,
) -> Result<Json<ArbitrageApiResponse<OpportunitiesResponse>>, StatusCode> {
    let cache = state.opportunities_cache.read().await;
    let last_update = *state.last_update.read().await;
    
    let mut filtered_opportunities: Vec<ArbitrageOpportunity> = cache.clone();
    
    // Apply filters
    if let Some(min_profit_usd) = query.min_profit_usd {
        filtered_opportunities.retain(|op| op.profit_usd >= min_profit_usd);
    }
    
    if let Some(min_profit_percentage) = query.min_profit_percentage {
        filtered_opportunities.retain(|op| op.profit_percentage >= min_profit_percentage);
    }
    
    if let Some(from_chain) = query.from_chain {
        filtered_opportunities.retain(|op| op.from_chain_id == from_chain);
    }
    
    if let Some(to_chain) = query.to_chain {
        filtered_opportunities.retain(|op| op.to_chain_id == to_chain);
    }
    
    if let Some(token) = &query.token {
        filtered_opportunities.retain(|op| op.token_in == *token);
    }
    
    // Apply limit
    if let Some(limit) = query.limit {
        filtered_opportunities.truncate(limit);
    }
    
    let total_count = filtered_opportunities.len();
    
    let response = ArbitrageApiResponse {
        success: true,
        data: Some(OpportunitiesResponse {
            opportunities: filtered_opportunities,
            total_count,
            filters_applied: query,
            last_updated: last_update,
        }),
        error: None,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    Ok(Json(response))
}

/// POST /arbitrage/opportunities/refresh - Refresh cached opportunities
async fn refresh_opportunities(
    State(state): State<ArbitrageApiState>,
) -> Result<Json<ArbitrageApiResponse<String>>, StatusCode> {
    match state.update_opportunities_cache().await {
        Ok(_) => {
            let response = ArbitrageApiResponse {
                success: true,
                data: Some("Opportunities refreshed successfully".to_string()),
                error: None,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            Ok(Json(response))
        }
        Err(e) => {
            warn!("Failed to refresh opportunities: {}", e);
            let response = ArbitrageApiResponse {
                success: false,
                data: None,
                error: Some(format!("Failed to refresh opportunities: {}", e)),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            Ok(Json(response))
        }
    }
}

/// GET /arbitrage/prices - Get cross-chain prices for a token
async fn get_cross_chain_prices(
    State(state): State<ArbitrageApiState>,
    Query(query): Query<PriceQuery>,
) -> Result<Json<ArbitrageApiResponse<PriceResponse>>, StatusCode> {
    let detector = state.detector.read().await;
    let prices = (*detector).get_cross_chain_prices(&query.token);
    
    if prices.is_empty() {
        let response = ArbitrageApiResponse {
            success: false,
            data: None,
            error: Some(format!("No price data found for token: {}", query.token)),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        return Ok(Json(response));
    }
    
    // Calculate price spread
    let highest_price = prices.iter().map(|p| p.price_usd).fold(0.0, f64::max);
    let lowest_price = prices.iter().map(|p| p.price_usd).fold(f64::INFINITY, f64::min);
    let price_spread_percentage = if lowest_price > 0.0 {
        ((highest_price - lowest_price) / lowest_price) * 100.0
    } else {
        0.0
    };
    
    let highest_price_chain = prices.iter()
        .max_by(|a, b| a.price_usd.partial_cmp(&b.price_usd).unwrap())
        .map(|p| p.chain_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    
    let lowest_price_chain = prices.iter()
        .min_by(|a, b| a.price_usd.partial_cmp(&b.price_usd).unwrap())
        .map(|p| p.chain_name.clone())
        .unwrap_or_else(|| "Unknown".to_string());
    
    // Convert ChainPrice to CrossChainPrice for API response
    let cross_chain_prices: Vec<CrossChainPrice> = prices.into_iter().map(|p| CrossChainPrice {
        chain_id: 1, // Default chain ID
        chain_name: p.chain_name,
        token: query.token.clone(),
        price_usd: p.price_usd,
        liquidity_usd: p.liquidity_usd,
        timestamp: p.last_updated,
        dex_source: "aggregated".to_string(),
    }).collect();
    
    let price_spread = if !cross_chain_prices.is_empty() {
        ((highest_price - lowest_price) / lowest_price) * 100.0
    } else {
        0.0
    };

    let response = ArbitrageApiResponse {
        success: true,
        data: Some(PriceResponse {
            prices: cross_chain_prices,
            token: query.token,
            price_spread_percentage: price_spread,
            highest_price_chain,
            lowest_price_chain,
        }),
        error: None,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    Ok(Json(response))
}

/// GET /arbitrage/anomalies - Get price anomalies for a token
async fn get_price_anomalies(
    State(state): State<ArbitrageApiState>,
    Query(query): Query<AnomalyQuery>,
) -> Result<Json<ArbitrageApiResponse<Vec<PriceAnomaly>>>, StatusCode> {
    let detector = state.detector.read().await;
    let threshold = query.threshold_percentage.unwrap_or(2.0); // Default 2% threshold
    let anomalies = (*detector).detect_price_anomalies(&query.token, threshold);
    
    let response = ArbitrageApiResponse {
        success: true,
        data: Some(anomalies),
        error: None,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    Ok(Json(response))
}

/// GET /arbitrage/monitoring - Get monitoring status
async fn get_monitoring_status(
    State(state): State<ArbitrageApiState>,
) -> Result<Json<ArbitrageApiResponse<MonitoringResponse>>, StatusCode> {
    let detector = state.detector.read().await;
    let cache = state.opportunities_cache.read().await;
    
    let status = (*detector).get_monitoring_status();
    let recent_opportunities_count = cache.len();
    
    let average_profit_usd = if !cache.is_empty() {
        cache.iter().map(|op| op.profit_usd).sum::<f64>() / cache.len() as f64
    } else {
        0.0
    };
    
    // Get top 5 most profitable pairs
    let mut pair_profits: HashMap<String, f64> = HashMap::new();
    for opportunity in cache.iter() {
        let pair_key = format!("{}-{}", opportunity.from_chain_id, opportunity.to_chain_id);
        *pair_profits.entry(pair_key).or_insert(0.0) += opportunity.profit_usd;
    }
    
    let mut top_pairs: Vec<(String, f64)> = pair_profits.into_iter().collect();
    top_pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let top_profitable_pairs: Vec<String> = top_pairs
        .into_iter()
        .take(5)
        .map(|(pair, _)| pair)
        .collect();
    
    let response = ArbitrageApiResponse {
        success: true,
        data: Some(MonitoringResponse {
            status,
            recent_opportunities_count,
            total_chains_monitored: 5, // Number of supported chains
            average_profit_usd,
            top_profitable_pairs,
        }),
        error: None,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };
    
    Ok(Json(response))
}

/// GET /arbitrage/health - Health check for arbitrage system
async fn get_arbitrage_health(
    State(state): State<ArbitrageApiState>,
) -> Result<Json<ArbitrageApiResponse<HashMap<String, serde_json::Value>>>, StatusCode> {
    let detector = state.detector.read().await;
    let cache = state.opportunities_cache.read().await;
    let last_update = *state.last_update.read().await;
    
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let mut health_data = HashMap::new();
    health_data.insert("service".to_string(), serde_json::Value::String("arbitrage_detector".to_string()));
    health_data.insert("status".to_string(), serde_json::Value::String("healthy".to_string()));
    health_data.insert("uptime_seconds".to_string(), serde_json::Value::Number(serde_json::Number::from(current_time)));
    health_data.insert("cached_opportunities".to_string(), serde_json::Value::Number(serde_json::Number::from(cache.len())));
    health_data.insert("last_update_seconds_ago".to_string(), serde_json::Value::Number(serde_json::Number::from(current_time - last_update)));
    let monitoring_status = (*detector).get_monitoring_status();
    health_data.insert("monitoring_active".to_string(), serde_json::Value::Bool(monitoring_status.is_active));
    health_data.insert("total_chains".to_string(), serde_json::Value::Number(serde_json::Number::from(monitoring_status.total_chains)));
    health_data.insert("total_tokens".to_string(), serde_json::Value::Number(serde_json::Number::from(monitoring_status.total_tokens)));
    
    let response = ArbitrageApiResponse {
        success: true,
        data: Some(health_data),
        error: None,
        timestamp: current_time,
    };
    
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridges::BridgeManager;
    use crate::dexes::DexManager;
    use std::sync::Arc;
    
    #[tokio::test]
    async fn test_arbitrage_api_state_creation() {
        let bridge_manager = Arc::new(BridgeManager::new());
        let dex_manager = Arc::new(DexManager::new());
        let detector = ArbitrageDetector::new(bridge_manager, dex_manager);
        
        let api_state = ArbitrageApiState::new(detector);
        
        let cache = api_state.opportunities_cache.read().await;
        assert!(cache.is_empty());
        
        let last_update = *api_state.last_update.read().await;
        assert_eq!(last_update, 0);
    }
    
    #[tokio::test]
    async fn test_opportunities_cache_update() {
        let bridge_manager = Arc::new(BridgeManager::new());
        let dex_manager = Arc::new(DexManager::new());
        let detector = ArbitrageDetector::new(bridge_manager, dex_manager);
        
        let api_state = ArbitrageApiState::new(detector);
        
        // Update the cache
        api_state.update_opportunities_cache().await.unwrap();
        
        let last_update = *api_state.last_update.read().await;
        assert!(last_update > 0);
    }
}
