#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::user_preferences::{UserPreferenceManager, RoutingPreferences, MevProtectionLevel, OptimizationStrategy};
    use crate::routing::strategy_templates::{StrategyTemplateManager, RiskLevel};
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_custom_routing_preferences_complete_flow() {
        println!("🧪 Testing Custom Routing Preferences Complete Flow");
        
        // Test 1: User Preference Manager CRUD Operations
        println!("\n📋 Test 1: User Preference Manager CRUD Operations");
        
        // Create preference manager without Redis cache for testing
        let mut preference_manager = UserPreferenceManager::new();
        
        let user_id = Uuid::new_v4();
        println!("   User ID: {}", user_id);
        
        // Create default preferences
        let mut preferences = RoutingPreferences::default();
        preferences.max_hop_count = 3;
        preferences.max_slippage_tolerance = Decimal::from_str("0.005").unwrap(); // 0.5%
        preferences.min_liquidity_threshold = Decimal::from_str("100000").unwrap(); // $100k
        preferences.gas_vs_price_preference = Decimal::from_str("0.7").unwrap(); // 70% price focus
        preferences.mev_protection_level = MevProtectionLevel::Medium;
        preferences.blacklisted_dexs = vec!["sushiswap".to_string()];
        preferences.whitelisted_dexs = Some(vec!["uniswap".to_string(), "curve".to_string()]);
        
        // Set preferences
        let result = preference_manager.set_preferences(user_id, preferences.clone()).await;
        assert!(result.is_ok(), "Failed to set user preferences: {:?}", result);
        println!("   ✅ User preferences set successfully");
        
        // Get preferences
        let retrieved = preference_manager.get_preferences(user_id).await;
        assert!(retrieved.is_ok(), "Failed to get user preferences: {:?}", retrieved);
        let retrieved_prefs = retrieved.unwrap();
        
        assert_eq!(retrieved_prefs.max_hop_count, 3);
        assert_eq!(retrieved_prefs.max_slippage_tolerance, Decimal::from_str("0.005").unwrap());
        assert_eq!(retrieved_prefs.mev_protection_level, MevProtectionLevel::Medium);
        assert_eq!(retrieved_prefs.blacklisted_dexs.len(), 1);
        assert_eq!(retrieved_prefs.whitelisted_dexs.as_ref().unwrap().len(), 2);
        println!("   ✅ User preferences retrieved and validated successfully");
        
        // Update preferences
        let mut updated_prefs = retrieved_prefs.clone();
        updated_prefs.max_hop_count = 2;
        updated_prefs.mev_protection_level = MevProtectionLevel::High;
        
        let update_result = preference_manager.set_preferences(user_id, updated_prefs).await;
        assert!(update_result.is_ok(), "Failed to update user preferences");
        
        let final_prefs = preference_manager.get_preferences(user_id).await.unwrap();
        assert_eq!(final_prefs.max_hop_count, 2);
        assert_eq!(final_prefs.mev_protection_level, MevProtectionLevel::High);
        println!("   ✅ User preferences updated successfully");
        
        // Test 2: Strategy Template System
        println!("\n📋 Test 2: Strategy Template System");
        
        let template_manager = StrategyTemplateManager::new();
        
        // Get all templates
        let all_templates = template_manager.get_all_templates();
        assert!(!all_templates.is_empty(), "No strategy templates found");
        println!("   Found {} strategy templates", all_templates.len());
        
        // Test specific templates exist
        let speed_template = template_manager.get_template("speed_first");
        assert!(speed_template.is_some(), "Speed First template not found");
        let speed_template = speed_template.unwrap();
        
        assert_eq!(speed_template.name, "Speed First");
        assert_eq!(speed_template.risk_level, RiskLevel::Moderate);
        assert_eq!(speed_template.max_hop_count, 2);
        println!("   ✅ Speed First template validated");
        
        let mev_template = template_manager.get_template("mev_protected");
        assert!(mev_template.is_some(), "MEV Protected template not found");
        let mev_template = mev_template.unwrap();
        
        assert_eq!(mev_template.name, "MEV Protected");
        assert_eq!(mev_template.risk_level, RiskLevel::Conservative);
        assert_eq!(mev_template.mev_protection, MevProtectionLevel::Maximum);
        println!("   ✅ MEV Protected template validated");
        
        // Test template to preferences conversion
        let converted_prefs = speed_template.to_routing_preferences(user_id);
        assert_eq!(converted_prefs.max_hop_count, speed_template.max_hop_count);
        assert_eq!(converted_prefs.optimization_strategy, speed_template.strategy);
        println!("   ✅ Template to preferences conversion working");
        
        // Test 3: Template Recommendation System
        println!("\n📋 Test 3: Template Recommendation System");
        
        let trade_amount = Decimal::from_str("10000").unwrap(); // $10k trade
        let recommendations = template_manager.recommend_templates(
            trade_amount,
            true, // time sensitive
            &RiskLevel::Moderate
        );
        
        assert!(!recommendations.is_empty(), "No template recommendations generated");
        println!("   Generated {} recommendations for $10k time-sensitive trade", recommendations.len());
        
        // Verify speed_first is recommended for time-sensitive trades
        assert!(recommendations.contains(&"speed_first".to_string()), 
                "Speed First should be recommended for time-sensitive trades");
        println!("   ✅ Speed First correctly recommended for time-sensitive trade");
        
        // Test 4: Data Structure Defaults and Validation
        println!("\n📋 Test 4: Data Structure Defaults and Validation");
        
        let default_prefs = RoutingPreferences::default();
        assert_eq!(default_prefs.max_hop_count, 3);
        assert_eq!(default_prefs.max_slippage_tolerance, Decimal::from_str("0.5").unwrap()); // 0.5%
        assert_eq!(default_prefs.optimization_strategy, OptimizationStrategy::Balanced);
        assert_eq!(default_prefs.mev_protection_level, MevProtectionLevel::Medium);
        assert!(default_prefs.blacklisted_dexs.is_empty());
        assert!(default_prefs.whitelisted_dexs.is_none());
        println!("   ✅ Default routing preferences validated");
        
        // Test 5: Multiple Users Preference Management
        println!("\n📋 Test 5: Multiple Users Preference Management");
        
        let user2_id = Uuid::new_v4();
        let user3_id = Uuid::new_v4();
        
        // Set different preferences for each user
        let mut user2_prefs = RoutingPreferences::default();
        user2_prefs.optimization_strategy = OptimizationStrategy::GasOptimized;
        user2_prefs.mev_protection_level = MevProtectionLevel::Basic;
        
        let mut user3_prefs = RoutingPreferences::default();
        user3_prefs.optimization_strategy = OptimizationStrategy::BestPrice;
        user3_prefs.mev_protection_level = MevProtectionLevel::High;
        
        // Set preferences for multiple users
        assert!(preference_manager.set_preferences(user2_id, user2_prefs.clone()).await.is_ok());
        assert!(preference_manager.set_preferences(user3_id, user3_prefs.clone()).await.is_ok());
        
        // Verify each user has their own preferences
        let retrieved_user2 = preference_manager.get_preferences(user2_id).await.unwrap();
        let retrieved_user3 = preference_manager.get_preferences(user3_id).await.unwrap();
        
        assert_eq!(retrieved_user2.optimization_strategy, OptimizationStrategy::GasOptimized);
        assert_eq!(retrieved_user2.mev_protection_level, MevProtectionLevel::Basic);
        
        assert_eq!(retrieved_user3.optimization_strategy, OptimizationStrategy::BestPrice);
        assert_eq!(retrieved_user3.mev_protection_level, MevProtectionLevel::High);
        
        // Verify original user preferences unchanged
        let original_user_prefs = preference_manager.get_preferences(user_id).await.unwrap();
        assert_eq!(original_user_prefs.max_hop_count, 2); // From our earlier update
        assert_eq!(original_user_prefs.mev_protection_level, MevProtectionLevel::High);
        
        println!("   ✅ Multiple users with independent preferences validated");
        
        println!("\n🎉 All Custom Routing Preferences tests passed successfully!");
        println!("   - User preference CRUD operations: ✅");
        println!("   - Strategy template system: ✅");
        println!("   - Template recommendations: ✅");
        println!("   - Data structure defaults: ✅");
        println!("   - Multiple users management: ✅");
    }
}
