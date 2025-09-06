use crate::routing::user_preferences::{OptimizationStrategy, CustomStrategy, MevProtectionLevel, DexPreference, RoutingPreferences};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::risk_management::UserId;

/// Pre-defined routing strategy templates for common use cases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyTemplate {
    pub name: String,
    pub description: String,
    pub strategy: OptimizationStrategy,
    pub mev_protection: MevProtectionLevel,
    pub max_hop_count: u8,
    pub gas_vs_price_preference: Decimal,
    pub max_slippage_tolerance: Decimal,
    pub min_liquidity_threshold: Decimal,
    pub recommended_dex_preferences: HashMap<String, DexPreference>,
    pub blacklisted_dexs: Vec<String>,
    pub use_cases: Vec<String>,
    pub risk_level: RiskLevel,
}

/// Risk level classification for strategies
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Conservative,
    Moderate,
    Aggressive,
    Custom,
}

impl StrategyTemplate {
    /// Convert strategy template to routing preferences for a user
    pub fn to_routing_preferences(&self, user_id: UserId) -> RoutingPreferences {
        let mut preferences = RoutingPreferences::new(user_id);
        preferences.optimization_strategy = self.strategy.clone();
        preferences.mev_protection_level = self.mev_protection.clone();
        preferences.max_hop_count = self.max_hop_count;
        preferences.max_slippage_tolerance = self.max_slippage_tolerance;
        preferences.min_liquidity_threshold = self.min_liquidity_threshold;
        
        // Apply DEX preferences from template
        for (dex_name, dex_pref) in &self.recommended_dex_preferences {
            preferences.dex_preferences.insert(dex_name.clone(), dex_pref.clone());
        }
        
        preferences
    }
}

/// Strategy template manager
pub struct StrategyTemplateManager {
    templates: HashMap<String, StrategyTemplate>,
}

impl StrategyTemplateManager {
    /// Create new strategy template manager with pre-defined templates
    pub fn new() -> Self {
        let mut manager = Self {
            templates: HashMap::new(),
        };
        
        manager.load_default_templates();
        manager
    }

    /// Load default strategy templates
    fn load_default_templates(&mut self) {
        // Speed First Strategy
        self.templates.insert("speed_first".to_string(), StrategyTemplate {
            name: "Speed First".to_string(),
            description: "Prioritize fastest execution with minimal hops and high-speed DEXs".to_string(),
            strategy: OptimizationStrategy::SpeedFirst,
            mev_protection: MevProtectionLevel::Basic,
            max_hop_count: 2,
            gas_vs_price_preference: Decimal::new(3, 1), // 0.3 - favor gas efficiency
            max_slippage_tolerance: Decimal::new(1, 0), // 1%
            min_liquidity_threshold: Decimal::new(50000, 0), // $50k
            recommended_dex_preferences: Self::create_speed_dex_preferences(),
            blacklisted_dexs: vec!["SlowDEX".to_string()], // Example slow DEX
            use_cases: vec![
                "Arbitrage opportunities".to_string(),
                "Time-sensitive trades".to_string(),
                "High-frequency trading".to_string(),
            ],
            risk_level: RiskLevel::Moderate,
        });

        // Best Price Strategy
        self.templates.insert("best_price".to_string(), StrategyTemplate {
            name: "Best Price".to_string(),
            description: "Optimize for lowest slippage and best execution price".to_string(),
            strategy: OptimizationStrategy::BestPrice,
            mev_protection: MevProtectionLevel::Medium,
            max_hop_count: 4,
            gas_vs_price_preference: Decimal::new(8, 1), // 0.8 - favor price optimization
            max_slippage_tolerance: Decimal::new(3, 1), // 0.3%
            min_liquidity_threshold: Decimal::new(100000, 0), // $100k
            recommended_dex_preferences: Self::create_price_dex_preferences(),
            blacklisted_dexs: Vec::new(),
            use_cases: vec![
                "Large trades".to_string(),
                "Long-term investments".to_string(),
                "Price-sensitive operations".to_string(),
            ],
            risk_level: RiskLevel::Conservative,
        });

        // MEV Protected Strategy
        self.templates.insert("mev_protected".to_string(), StrategyTemplate {
            name: "MEV Protected".to_string(),
            description: "Maximum MEV protection with private mempools and secure routing".to_string(),
            strategy: OptimizationStrategy::MevProtected,
            mev_protection: MevProtectionLevel::Maximum,
            max_hop_count: 3,
            gas_vs_price_preference: Decimal::new(4, 1), // 0.4 - balanced but security-focused
            max_slippage_tolerance: Decimal::new(5, 1), // 0.5%
            min_liquidity_threshold: Decimal::new(200000, 0), // $200k
            recommended_dex_preferences: Self::create_mev_protected_dex_preferences(),
            blacklisted_dexs: vec!["PublicMempoolDEX".to_string()], // Example public mempool DEX
            use_cases: vec![
                "Institutional trading".to_string(),
                "Large value transfers".to_string(),
                "MEV-sensitive operations".to_string(),
            ],
            risk_level: RiskLevel::Conservative,
        });

        // Gas Optimized Strategy
        self.templates.insert("gas_optimized".to_string(), StrategyTemplate {
            name: "Gas Optimized".to_string(),
            description: "Minimize gas costs with efficient routing and low-gas DEXs".to_string(),
            strategy: OptimizationStrategy::GasOptimized,
            mev_protection: MevProtectionLevel::Basic,
            max_hop_count: 2,
            gas_vs_price_preference: Decimal::new(1, 1), // 0.1 - heavily favor gas efficiency
            max_slippage_tolerance: Decimal::new(8, 1), // 0.8%
            min_liquidity_threshold: Decimal::new(25000, 0), // $25k
            recommended_dex_preferences: Self::create_gas_optimized_dex_preferences(),
            blacklisted_dexs: vec!["HighGasDEX".to_string()], // Example high-gas DEX
            use_cases: vec![
                "Small trades".to_string(),
                "High gas price periods".to_string(),
                "Cost-conscious trading".to_string(),
            ],
            risk_level: RiskLevel::Moderate,
        });

        // Balanced Strategy
        self.templates.insert("balanced".to_string(), StrategyTemplate {
            name: "Balanced".to_string(),
            description: "Well-rounded approach balancing speed, price, gas, and security".to_string(),
            strategy: OptimizationStrategy::Balanced,
            mev_protection: MevProtectionLevel::Medium,
            max_hop_count: 3,
            gas_vs_price_preference: Decimal::new(5, 1), // 0.5 - perfectly balanced
            max_slippage_tolerance: Decimal::new(5, 1), // 0.5%
            min_liquidity_threshold: Decimal::new(75000, 0), // $75k
            recommended_dex_preferences: Self::create_balanced_dex_preferences(),
            blacklisted_dexs: Vec::new(),
            use_cases: vec![
                "General trading".to_string(),
                "Portfolio rebalancing".to_string(),
                "Regular DeFi operations".to_string(),
            ],
            risk_level: RiskLevel::Moderate,
        });

        // Aggressive Yield Strategy
        self.templates.insert("aggressive_yield".to_string(), StrategyTemplate {
            name: "Aggressive Yield".to_string(),
            description: "High-risk, high-reward strategy for maximum yield opportunities".to_string(),
            strategy: OptimizationStrategy::Custom(CustomStrategy {
                speed_weight: Decimal::new(30, 2), // 0.3
                price_weight: Decimal::new(40, 2), // 0.4
                gas_weight: Decimal::new(10, 2),   // 0.1
                security_weight: Decimal::new(5, 2), // 0.05
                liquidity_weight: Decimal::new(15, 2), // 0.15
            }),
            mev_protection: MevProtectionLevel::Basic,
            max_hop_count: 5,
            gas_vs_price_preference: Decimal::new(9, 1), // 0.9 - heavily favor price
            max_slippage_tolerance: Decimal::new(2, 0), // 2%
            min_liquidity_threshold: Decimal::new(10000, 0), // $10k
            recommended_dex_preferences: Self::create_aggressive_dex_preferences(),
            blacklisted_dexs: Vec::new(),
            use_cases: vec![
                "yield farming".to_string(),
                "liquidity mining".to_string(),
                "high-risk arbitrage".to_string(),
            ],
            risk_level: RiskLevel::Aggressive,
        });

        // Conservative DeFi Strategy
        self.templates.insert("conservative_defi".to_string(), StrategyTemplate {
            name: "Conservative DeFi".to_string(),
            description: "Low-risk strategy for conservative DeFi participation".to_string(),
            strategy: OptimizationStrategy::Custom(CustomStrategy {
                speed_weight: Decimal::new(15, 2), // 0.15
                price_weight: Decimal::new(25, 2), // 0.25
                gas_weight: Decimal::new(20, 2),   // 0.2
                security_weight: Decimal::new(30, 2), // 0.3
                liquidity_weight: Decimal::new(10, 2), // 0.1
            }),
            mev_protection: MevProtectionLevel::Maximum,
            max_hop_count: 2,
            gas_vs_price_preference: Decimal::new(3, 1), // 0.3 - favor gas efficiency
            max_slippage_tolerance: Decimal::new(2, 1), // 0.2%
            min_liquidity_threshold: Decimal::new(500000, 0), // $500k
            recommended_dex_preferences: Self::create_conservative_dex_preferences(),
            blacklisted_dexs: vec![
                "NewDEX".to_string(),
                "UnauditedDEX".to_string(),
            ],
            use_cases: vec![
                "Retirement funds".to_string(),
                "Conservative investing".to_string(),
                "Risk-averse trading".to_string(),
            ],
            risk_level: RiskLevel::Conservative,
        });
    }

    /// Create DEX preferences for speed-focused strategy
    fn create_speed_dex_preferences() -> HashMap<String, DexPreference> {
        let mut prefs = HashMap::new();
        
        // Favor fast, direct DEXs
        prefs.insert("Uniswap".to_string(), DexPreference::new("Uniswap".to_string())
            .with_weight(Decimal::new(15, 1)) // 1.5x
            .with_priority(9));
        
        prefs.insert("SushiSwap".to_string(), DexPreference::new("SushiSwap".to_string())
            .with_weight(Decimal::new(14, 1)) // 1.4x
            .with_priority(8));
        
        prefs.insert("Curve".to_string(), DexPreference::new("Curve".to_string())
            .with_weight(Decimal::new(12, 1)) // 1.2x (slower for complex routes)
            .with_priority(6));
        
        prefs
    }

    /// Create DEX preferences for price-focused strategy
    fn create_price_dex_preferences() -> HashMap<String, DexPreference> {
        let mut prefs = HashMap::new();
        
        // Favor DEXs with best pricing
        prefs.insert("Curve".to_string(), DexPreference::new("Curve".to_string())
            .with_weight(Decimal::new(16, 1)) // 1.6x (excellent for stablecoins)
            .with_priority(9));
        
        prefs.insert("Balancer".to_string(), DexPreference::new("Balancer".to_string())
            .with_weight(Decimal::new(15, 1)) // 1.5x
            .with_priority(8));
        
        prefs.insert("Uniswap".to_string(), DexPreference::new("Uniswap".to_string())
            .with_weight(Decimal::new(13, 1)) // 1.3x
            .with_priority(7));
        
        prefs
    }

    /// Create DEX preferences for MEV-protected strategy
    fn create_mev_protected_dex_preferences() -> HashMap<String, DexPreference> {
        let mut prefs = HashMap::new();
        
        // Favor DEXs with MEV protection
        prefs.insert("CoW Swap".to_string(), DexPreference::new("CoW Swap".to_string())
            .with_weight(Decimal::new(18, 1)) // 1.8x (batch auctions)
            .with_priority(10));
        
        prefs.insert("Flashbots".to_string(), DexPreference::new("Flashbots".to_string())
            .with_weight(Decimal::new(17, 1)) // 1.7x (private mempool)
            .with_priority(9));
        
        prefs.insert("Uniswap".to_string(), DexPreference::new("Uniswap".to_string())
            .with_weight(Decimal::new(10, 1)) // 1.0x (standard)
            .with_priority(5));
        
        prefs
    }

    /// Create DEX preferences for gas-optimized strategy
    fn create_gas_optimized_dex_preferences() -> HashMap<String, DexPreference> {
        let mut prefs = HashMap::new();
        
        // Favor low-gas DEXs
        prefs.insert("Uniswap V3".to_string(), DexPreference::new("Uniswap V3".to_string())
            .with_weight(Decimal::new(16, 1)) // 1.6x (concentrated liquidity)
            .with_priority(9));
        
        prefs.insert("SushiSwap".to_string(), DexPreference::new("SushiSwap".to_string())
            .with_weight(Decimal::new(14, 1)) // 1.4x
            .with_priority(8));
        
        prefs.insert("Curve".to_string(), DexPreference::new("Curve".to_string())
            .with_weight(Decimal::new(8, 1)) // 0.8x (can be gas-heavy)
            .with_priority(4));
        
        prefs
    }

    /// Create DEX preferences for balanced strategy
    fn create_balanced_dex_preferences() -> HashMap<String, DexPreference> {
        let mut prefs = HashMap::new();
        
        // Equal weighting for major DEXs
        prefs.insert("Uniswap".to_string(), DexPreference::new("Uniswap".to_string())
            .with_weight(Decimal::new(12, 1)) // 1.2x
            .with_priority(7));
        
        prefs.insert("Curve".to_string(), DexPreference::new("Curve".to_string())
            .with_weight(Decimal::new(12, 1)) // 1.2x
            .with_priority(7));
        
        prefs.insert("SushiSwap".to_string(), DexPreference::new("SushiSwap".to_string())
            .with_weight(Decimal::new(11, 1)) // 1.1x
            .with_priority(6));
        
        prefs.insert("Balancer".to_string(), DexPreference::new("Balancer".to_string())
            .with_weight(Decimal::new(11, 1)) // 1.1x
            .with_priority(6));
        
        prefs
    }

    /// Create DEX preferences for aggressive strategy
    fn create_aggressive_dex_preferences() -> HashMap<String, DexPreference> {
        let mut prefs = HashMap::new();
        
        // Include newer, potentially higher-yield DEXs
        prefs.insert("Uniswap V3".to_string(), DexPreference::new("Uniswap V3".to_string())
            .with_weight(Decimal::new(15, 1)) // 1.5x
            .with_priority(8));
        
        prefs.insert("Balancer V2".to_string(), DexPreference::new("Balancer V2".to_string())
            .with_weight(Decimal::new(14, 1)) // 1.4x
            .with_priority(7));
        
        prefs.insert("Curve".to_string(), DexPreference::new("Curve".to_string())
            .with_weight(Decimal::new(13, 1)) // 1.3x
            .with_priority(7));
        
        // Include newer DEXs for potential opportunities
        prefs.insert("Kyber".to_string(), DexPreference::new("Kyber".to_string())
            .with_weight(Decimal::new(12, 1)) // 1.2x
            .with_priority(6));
        
        prefs
    }

    /// Create DEX preferences for conservative strategy
    fn create_conservative_dex_preferences() -> HashMap<String, DexPreference> {
        let mut prefs = HashMap::new();
        
        // Only well-established, audited DEXs
        prefs.insert("Uniswap".to_string(), DexPreference::new("Uniswap".to_string())
            .with_weight(Decimal::new(15, 1)) // 1.5x
            .with_priority(9));
        
        prefs.insert("Curve".to_string(), DexPreference::new("Curve".to_string())
            .with_weight(Decimal::new(14, 1)) // 1.4x
            .with_priority(8));
        
        prefs.insert("Balancer".to_string(), DexPreference::new("Balancer".to_string())
            .with_weight(Decimal::new(12, 1)) // 1.2x
            .with_priority(7));
        
        prefs
    }

    /// Get strategy template by name
    pub fn get_template(&self, name: &str) -> Option<&StrategyTemplate> {
        self.templates.get(name)
    }

    /// Get all available strategy templates
    pub fn get_all_templates(&self) -> &HashMap<String, StrategyTemplate> {
        &self.templates
    }

    /// Get templates by risk level
    pub fn get_templates_by_risk_level(&self, risk_level: &RiskLevel) -> Vec<&StrategyTemplate> {
        self.templates.values()
            .filter(|template| std::mem::discriminant(&template.risk_level) == std::mem::discriminant(risk_level))
            .collect()
    }

    /// Get templates by use case
    pub fn get_templates_by_use_case(&self, use_case: &str) -> Vec<&StrategyTemplate> {
        self.templates.values()
            .filter(|template| template.use_cases.iter().any(|uc| uc.contains(use_case)))
            .collect()
    }

    /// Add custom strategy template
    pub fn add_custom_template(&mut self, name: String, template: StrategyTemplate) {
        self.templates.insert(name, template);
    }

    /// Remove strategy template
    pub fn remove_template(&mut self, name: &str) -> Option<StrategyTemplate> {
        self.templates.remove(name)
    }

    /// Get template recommendations based on trade characteristics
    pub fn recommend_templates(
        &self,
        trade_amount_usd: Decimal,
        is_time_sensitive: bool,
        risk_tolerance: &RiskLevel,
    ) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        // Time-sensitive trades
        if is_time_sensitive {
            recommendations.push("speed_first".to_string());
        }
        
        // Large trades
        if trade_amount_usd > Decimal::new(100000, 0) { // > $100k
            recommendations.push("best_price".to_string());
            recommendations.push("mev_protected".to_string());
        }
        
        // Small trades
        if trade_amount_usd < Decimal::new(1000, 0) { // < $1k
            recommendations.push("gas_optimized".to_string());
        }
        
        // Risk-based recommendations
        match risk_tolerance {
            RiskLevel::Conservative => {
                recommendations.push("conservative_defi".to_string());
                recommendations.push("mev_protected".to_string());
            }
            RiskLevel::Moderate => {
                recommendations.push("balanced".to_string());
            }
            RiskLevel::Aggressive => {
                recommendations.push("aggressive_yield".to_string());
            }
            RiskLevel::Custom => {
                recommendations.push("balanced".to_string());
            }
        }
        
        // Default recommendation
        if recommendations.is_empty() {
            recommendations.push("balanced".to_string());
        }
        
        recommendations
    }
}

impl Default for StrategyTemplateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_template_manager_creation() {
        let manager = StrategyTemplateManager::new();
        
        // Check that default templates are loaded
        assert!(manager.get_template("speed_first").is_some());
        assert!(manager.get_template("best_price").is_some());
        assert!(manager.get_template("mev_protected").is_some());
        assert!(manager.get_template("gas_optimized").is_some());
        assert!(manager.get_template("balanced").is_some());
        assert!(manager.get_template("aggressive_yield").is_some());
        assert!(manager.get_template("conservative_defi").is_some());
    }

    #[test]
    fn test_template_properties() {
        let manager = StrategyTemplateManager::new();
        
        let speed_template = manager.get_template("speed_first").unwrap();
        assert_eq!(speed_template.max_hop_count, 2);
        assert!(matches!(speed_template.strategy, OptimizationStrategy::SpeedFirst));
        assert!(matches!(speed_template.risk_level, RiskLevel::Moderate));
        
        let conservative_template = manager.get_template("conservative_defi").unwrap();
        assert!(matches!(conservative_template.mev_protection, MevProtectionLevel::Maximum));
        assert!(matches!(conservative_template.risk_level, RiskLevel::Conservative));
    }

    #[test]
    fn test_risk_level_filtering() {
        let manager = StrategyTemplateManager::new();
        
        let conservative_templates = manager.get_templates_by_risk_level(&RiskLevel::Conservative);
        assert!(!conservative_templates.is_empty());
        
        let aggressive_templates = manager.get_templates_by_risk_level(&RiskLevel::Aggressive);
        assert!(!aggressive_templates.is_empty());
    }

    #[test]
    fn test_use_case_filtering() {
        let manager = StrategyTemplateManager::new();
        
        let arbitrage_templates = manager.get_templates_by_use_case("arbitrage");
        assert!(!arbitrage_templates.is_empty());
        
        let yield_templates = manager.get_templates_by_use_case("yield");
        assert!(!yield_templates.is_empty());
    }

    #[test]
    fn test_template_recommendations() {
        let manager = StrategyTemplateManager::new();
        
        // Large trade recommendation
        let large_trade_recs = manager.recommend_templates(
            Decimal::new(500000, 0), // $500k
            false,
            &RiskLevel::Conservative,
        );
        assert!(large_trade_recs.contains(&"best_price".to_string()));
        assert!(large_trade_recs.contains(&"mev_protected".to_string()));
        
        // Small trade recommendation
        let small_trade_recs = manager.recommend_templates(
            Decimal::new(500, 0), // $500
            false,
            &RiskLevel::Moderate,
        );
        assert!(small_trade_recs.contains(&"gas_optimized".to_string()));
        
        // Time-sensitive trade recommendation
        let urgent_trade_recs = manager.recommend_templates(
            Decimal::new(10000, 0), // $10k
            true,
            &RiskLevel::Moderate,
        );
        assert!(urgent_trade_recs.contains(&"speed_first".to_string()));
    }

    #[test]
    fn test_custom_template_management() {
        let mut manager = StrategyTemplateManager::new();
        
        let custom_template = StrategyTemplate {
            name: "Custom Test".to_string(),
            description: "Test template".to_string(),
            strategy: OptimizationStrategy::Balanced,
            mev_protection: MevProtectionLevel::Medium,
            max_hop_count: 3,
            gas_vs_price_preference: Decimal::new(5, 1),
            max_slippage_tolerance: Decimal::new(5, 1),
            min_liquidity_threshold: Decimal::new(50000, 0),
            recommended_dex_preferences: HashMap::new(),
            blacklisted_dexs: Vec::new(),
            use_cases: vec!["testing".to_string()],
            risk_level: RiskLevel::Custom,
        };
        
        manager.add_custom_template("custom_test".to_string(), custom_template);
        assert!(manager.get_template("custom_test").is_some());
        
        let removed = manager.remove_template("custom_test");
        assert!(removed.is_some());
        assert!(manager.get_template("custom_test").is_none());
    }
}
