use std::collections::HashMap;
use std::sync::Arc;
use alloy::primitives::Address;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

use crate::fetchers::generic_fetcher::{GenericFetcher, StandardPosition, ImpermanentLossInfo};
use crate::cache::CacheManager;
use crate::database::DatabaseManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPositionSummary {
    pub user_address: Address,
    pub chain_id: u32,
    pub positions: Vec<StandardPosition>,
    pub protocol_stats: HashMap<String, ProtocolStats>,
    pub portfolio_risk: PortfolioRisk,
    pub total_value_usd: f64,
    pub total_il_usd: f64,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStats {
    pub position_count: usize,
    pub total_value_usd: f64,
    pub avg_il_percentage: f64,
    pub total_unclaimed_fees_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioRisk {
    pub overall_risk_score: f64,
    pub diversification_score: f64,
    pub liquidity_risk: f64,
    pub impermanent_loss_risk: f64,
    pub concentration_risk: f64,
}

pub struct PositionOrchestrator {
    generic_fetcher: GenericFetcher,
    cache: Arc<CacheManager>,
    database: Arc<DatabaseManager>,
}

impl PositionOrchestrator {
    pub async fn new(
        database: Arc<DatabaseManager>,
        cache: Arc<CacheManager>,
    ) -> Result<Self> {
        let generic_fetcher = GenericFetcher::new(cache.clone()).await?;
        
        Ok(Self {
            generic_fetcher,
            cache,
            database,
        })
    }
    
    /// Get comprehensive position summary for a user across all protocols
    pub async fn get_user_positions(
        &self,
        chain_id: u32,
        user_address: Address,
    ) -> Result<UserPositionSummary> {
        let cache_key = format!("user_positions:{}:{:?}", chain_id, user_address);
        
        // Check cache first
        if let Ok(Some(cached)) = self.cache.get::<UserPositionSummary>(&cache_key).await {
            info!("Cache hit for user positions: {}", cache_key);
            return Ok(cached);
        }
        
        let mut all_positions = Vec::new();
        let mut protocol_stats = HashMap::new();
        
        // Fetch positions from all loaded protocols
        for protocol_name in self.generic_fetcher.get_protocol_names() {
            match self.generic_fetcher
                .fetch_positions_for_protocol(protocol_name, chain_id, user_address)
                .await 
            {
                Ok(positions) => {
                    if !positions.is_empty() {
                        let stats = self.calculate_protocol_stats(&positions);
                        protocol_stats.insert(protocol_name.clone(), stats);
                        all_positions.extend(positions);
                        
                        info!("Found {} positions for {} on {}", 
                              protocol_stats[protocol_name].position_count, 
                              protocol_name, 
                              chain_id);
                    }
                },
                Err(e) => {
                    warn!("Failed to fetch positions for {} on chain {}: {}", 
                          protocol_name, chain_id, e);
                    continue;
                }
            }
        }
        
        // Calculate portfolio-wide metrics
        let portfolio_risk = self.calculate_portfolio_risk(&all_positions);
        let total_value_usd = protocol_stats.values().map(|s| s.total_value_usd).sum();
        let total_il_usd = all_positions.iter()
            .filter_map(|p| p.impermanent_loss.as_ref())
            .map(|il| il.usd_amount)
            .sum();
        
        let summary = UserPositionSummary {
            user_address,
            chain_id,
            positions: all_positions,
            protocol_stats,
            portfolio_risk,
            total_value_usd,
            total_il_usd,
            fetched_at: chrono::Utc::now(),
        };
        
        // Cache the results
        let _ = self.cache.set(&cache_key, &summary, 300).await; // 5 minute cache
        
        info!("Generated position summary for {:?}: {} positions, ${:.2} total value", 
              user_address, summary.positions.len(), summary.total_value_usd);
        
        Ok(summary)
    }
    
    /// Get positions for a specific protocol (useful for IL Shield MVP)
    pub async fn get_protocol_positions(
        &self,
        protocol_name: &str,
        chain_id: u32,
        user_address: Address,
    ) -> Result<Vec<StandardPosition>> {
        self.generic_fetcher
            .fetch_positions_for_protocol(protocol_name, chain_id, user_address)
            .await
    }
    
    /// Get positions across multiple chains for a user
    pub async fn get_multichain_positions(
        &self,
        chain_ids: Vec<u32>,
        user_address: Address,
    ) -> Result<HashMap<u32, UserPositionSummary>> {
        let mut results = HashMap::new();
        
        // Fetch positions for each chain in parallel
        let mut handles = Vec::new();
        
        for chain_id in chain_ids {
            let orchestrator = self.clone_for_async();
            let handle = tokio::spawn(async move {
                (chain_id, orchestrator.get_user_positions(chain_id, user_address).await)
            });
            handles.push(handle);
        }
        
        // Collect results
        for handle in handles {
            let (chain_id, result) = handle.await?;
            match result {
                Ok(summary) => {
                    results.insert(chain_id, summary);
                },
                Err(e) => {
                    warn!("Failed to fetch positions for chain {}: {}", chain_id, e);
                }
            }
        }
        
        Ok(results)
    }
    
    /// Find whale positions (for IL Shield MVP)
    pub async fn find_whale_positions(
        &self,
        protocol_name: &str,
        chain_id: u32,
        min_value_usd: f64,
    ) -> Result<Vec<(Address, Vec<StandardPosition>)>> {
        // This would typically query a database of known whale addresses
        // For now, we'll return empty as this requires historical data
        warn!("Whale position detection requires historical address analysis");
        Ok(Vec::new())
    }
    
    /// Calculate high-risk positions for IL alerts
    pub async fn get_high_risk_positions(
        &self,
        chain_id: u32,
        user_address: Address,
        il_threshold: f64,
    ) -> Result<Vec<StandardPosition>> {
        let summary = self.get_user_positions(chain_id, user_address).await?;
        
        let high_risk_positions = summary.positions
            .into_iter()
            .filter(|position| {
                if let Some(il) = &position.impermanent_loss {
                    il.predicted_24h > il_threshold
                } else {
                    false
                }
            })
            .collect();
        
        Ok(high_risk_positions)
    }
    
    fn calculate_protocol_stats(&self, positions: &[StandardPosition]) -> ProtocolStats {
        let position_count = positions.len();
        let total_value_usd = positions.iter().map(|p| p.value_usd).sum();
        
        let avg_il_percentage = if position_count > 0 {
            positions.iter()
                .filter_map(|p| p.impermanent_loss.as_ref())
                .map(|il| il.percentage)
                .sum::<f64>() / position_count as f64
        } else {
            0.0
        };
        
        let total_unclaimed_fees_usd = 0.0; // TODO: Calculate based on token prices
        
        ProtocolStats {
            position_count,
            total_value_usd,
            avg_il_percentage,
            total_unclaimed_fees_usd,
        }
    }
    
    fn calculate_portfolio_risk(&self, positions: &[StandardPosition]) -> PortfolioRisk {
        if positions.is_empty() {
            return PortfolioRisk {
                overall_risk_score: 0.0,
                diversification_score: 0.0,
                liquidity_risk: 0.0,
                impermanent_loss_risk: 0.0,
                concentration_risk: 0.0,
            };
        }
        
        // Calculate diversification (number of unique protocols and token pairs)
        let unique_protocols: std::collections::HashSet<_> = positions.iter()
            .map(|p| &p.protocol)
            .collect();
        let unique_pairs: std::collections::HashSet<_> = positions.iter()
            .map(|p| (&p.token0, &p.token1))
            .collect();
        
        let diversification_score = (unique_protocols.len() as f64 * 0.3 + 
                                   unique_pairs.len() as f64 * 0.7) / positions.len() as f64;
        
        // Calculate average IL risk
        let avg_il_risk = positions.iter()
            .filter_map(|p| p.impermanent_loss.as_ref())
            .map(|il| il.percentage.abs())
            .sum::<f64>() / positions.len() as f64;
        
        // Calculate concentration risk (largest position as % of total)
        let total_value: f64 = positions.iter().map(|p| p.value_usd).sum();
        let max_position_value = positions.iter()
            .map(|p| p.value_usd)
            .fold(0.0, f64::max);
        let concentration_risk = if total_value > 0.0 {
            max_position_value / total_value
        } else {
            0.0
        };
        
        // Simple liquidity risk calculation (higher for V3 concentrated positions)
        let liquidity_risk = positions.iter()
            .map(|p| {
                if p.tick_lower.is_some() && p.tick_upper.is_some() {
                    0.7 // V3 concentrated liquidity is higher risk
                } else {
                    0.3 // V2 full-range is lower risk
                }
            })
            .sum::<f64>() / positions.len() as f64;
        
        // Overall risk score (weighted average)
        let overall_risk_score = (avg_il_risk * 0.4 + 
                                 concentration_risk * 0.3 + 
                                 liquidity_risk * 0.2 + 
                                 (1.0 - diversification_score) * 0.1).min(1.0);
        
        PortfolioRisk {
            overall_risk_score,
            diversification_score,
            liquidity_risk,
            impermanent_loss_risk: avg_il_risk,
            concentration_risk,
        }
    }
    
    // Helper method for async cloning
    fn clone_for_async(&self) -> Self {
        Self {
            generic_fetcher: self.generic_fetcher.clone(),
            cache: self.cache.clone(),
            database: self.database.clone(),
        }
    }
}

impl Clone for PositionOrchestrator {
    fn clone(&self) -> Self {
        Self {
            generic_fetcher: self.generic_fetcher.clone(),
            cache: self.cache.clone(),
            database: self.database.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_orchestrator_initialization() {
        let cache = Arc::new(CacheManager::new().await.unwrap());
        let database = Arc::new(DatabaseManager::new("sqlite::memory:").await.unwrap());
        
        let orchestrator = PositionOrchestrator::new(database, cache).await.unwrap();
        
        // Test that we can get protocol names
        let protocols = orchestrator.generic_fetcher.get_protocol_names();
        assert!(!protocols.is_empty());
    }
    
    #[tokio::test]
    async fn test_portfolio_risk_calculation() {
        let cache = Arc::new(CacheManager::new().await.unwrap());
        let database = Arc::new(DatabaseManager::new("sqlite::memory:").await.unwrap());
        let orchestrator = PositionOrchestrator::new(database, cache).await.unwrap();
        
        // Test with empty positions
        let risk = orchestrator.calculate_portfolio_risk(&[]);
        assert_eq!(risk.overall_risk_score, 0.0);
        assert_eq!(risk.diversification_score, 0.0);
    }
}
