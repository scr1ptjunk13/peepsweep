use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};

use crate::user_retention::trading_insights::{
    DashboardService, MarketIntelligenceEngine, PersonalizationEngine, PredictiveAnalytics,
    PersonalizedInsight, MarketOpportunity, TimingRecommendation, MarketSentimentAnalysis,
    PricePrediction, LiquidityForecast,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightMessage {
    pub message_type: InsightMessageType,
    pub user_id: Option<Uuid>,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InsightMessageType {
    PersonalizedInsights,
    MarketOpportunities,
    TimingRecommendations,
    PricePredictions,
    LiquidityForecasts,
    MarketSentiment,
    GasOptimization,
    RiskAlerts,
    MarketOverview,
    DashboardUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    pub action: SubscriptionAction,
    pub message_types: Vec<InsightMessageType>,
    pub user_id: Option<Uuid>,
    pub filters: Option<SubscriptionFilters>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionAction {
    Subscribe,
    Unsubscribe,
    UpdateFilters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionFilters {
    pub min_confidence: Option<f64>,
    pub risk_levels: Option<Vec<String>>,
    pub tokens: Option<Vec<String>>,
    pub dexes: Option<Vec<String>>,
    pub chains: Option<Vec<u64>>,
}

#[derive(Debug, Clone)]
pub struct ClientSubscription {
    pub user_id: Option<Uuid>,
    pub message_types: Vec<InsightMessageType>,
    pub filters: Option<SubscriptionFilters>,
    pub connected_at: DateTime<Utc>,
}

pub struct InsightsWebSocketServer {
    dashboard_service: Arc<DashboardService>,
    market_intelligence: Arc<MarketIntelligenceEngine>,
    personalization_engine: Arc<PersonalizationEngine>,
    predictive_analytics: Arc<PredictiveAnalytics>,
    broadcast_tx: broadcast::Sender<InsightMessage>,
    subscriptions: Arc<RwLock<HashMap<String, ClientSubscription>>>,
}

impl InsightsWebSocketServer {
    pub fn new(
        dashboard_service: Arc<DashboardService>,
        market_intelligence: Arc<MarketIntelligenceEngine>,
        personalization_engine: Arc<PersonalizationEngine>,
        predictive_analytics: Arc<PredictiveAnalytics>,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);
        
        Self {
            dashboard_service,
            market_intelligence,
            personalization_engine,
            predictive_analytics,
            broadcast_tx,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Handle WebSocket upgrade and connection
    pub async fn handle_websocket(
        &self,
        ws: WebSocketUpgrade,
        client_id: String,
    ) -> Response {
        let server = self.clone();
        ws.on_upgrade(move |socket| server.handle_socket(socket, client_id))
    }

    /// Handle individual WebSocket connection
    async fn handle_socket(self, socket: WebSocket, client_id: String) {
        let (mut sender, mut receiver) = socket.split();
        let mut broadcast_rx = self.broadcast_tx.subscribe();

        // Spawn task to handle incoming messages from client
        let subscriptions_clone = self.subscriptions.clone();
        let client_id_clone = client_id.clone();
        let incoming_task = tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                if let Ok(msg) = msg {
                    if let Ok(text) = msg.to_text() {
                        if let Ok(request) = serde_json::from_str::<SubscriptionRequest>(text) {
                            Self::handle_subscription_request(
                                &subscriptions_clone,
                                &client_id_clone,
                                request,
                            ).await;
                        }
                    }
                }
            }
        });

        // Spawn task to handle outgoing messages to client
        let subscriptions_clone = self.subscriptions.clone();
        let client_id_clone = client_id.clone();
        let outgoing_task = tokio::spawn(async move {
            while let Ok(message) = broadcast_rx.recv().await {
                // Check if client is subscribed to this message type
                if Self::should_send_message(&subscriptions_clone, &client_id_clone, &message).await {
                    if let Ok(json) = serde_json::to_string(&message) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        // Wait for either task to complete (connection closed)
        tokio::select! {
            _ = incoming_task => {},
            _ = outgoing_task => {},
        }

        // Clean up subscription
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(&client_id);
    }

    /// Handle subscription requests from clients
    async fn handle_subscription_request(
        subscriptions: &Arc<RwLock<HashMap<String, ClientSubscription>>>,
        client_id: &str,
        request: SubscriptionRequest,
    ) {
        let mut subs = subscriptions.write().await;
        
        match request.action {
            SubscriptionAction::Subscribe => {
                let subscription = ClientSubscription {
                    user_id: request.user_id,
                    message_types: request.message_types,
                    filters: request.filters,
                    connected_at: Utc::now(),
                };
                subs.insert(client_id.to_string(), subscription);
            }
            SubscriptionAction::Unsubscribe => {
                subs.remove(client_id);
            }
            SubscriptionAction::UpdateFilters => {
                if let Some(subscription) = subs.get_mut(client_id) {
                    subscription.filters = request.filters;
                    subscription.message_types = request.message_types;
                }
            }
        }
    }

    /// Check if a message should be sent to a specific client
    async fn should_send_message(
        subscriptions: &Arc<RwLock<HashMap<String, ClientSubscription>>>,
        client_id: &str,
        message: &InsightMessage,
    ) -> bool {
        let subs = subscriptions.read().await;
        
        if let Some(subscription) = subs.get(client_id) {
            // Check if subscribed to message type
            if !subscription.message_types.contains(&message.message_type) {
                return false;
            }

            // Check user ID filter
            if let (Some(sub_user_id), Some(msg_user_id)) = (subscription.user_id, message.user_id) {
                if sub_user_id != msg_user_id {
                    return false;
                }
            }

            // Apply additional filters if present
            if let Some(filters) = &subscription.filters {
                if !Self::message_passes_filters(message, filters) {
                    return false;
                }
            }

            return true;
        }

        false
    }

    /// Check if message passes subscription filters
    fn message_passes_filters(message: &InsightMessage, filters: &SubscriptionFilters) -> bool {
        // Apply confidence filter for relevant message types
        if let Some(min_confidence) = filters.min_confidence {
            match &message.message_type {
                InsightMessageType::PersonalizedInsights => {
                    if let Ok(insights) = serde_json::from_value::<Vec<PersonalizedInsight>>(message.data.clone()) {
                        if !insights.iter().any(|i| i.confidence_score >= min_confidence) {
                            return false;
                        }
                    }
                }
                InsightMessageType::MarketOpportunities => {
                    if let Ok(opportunities) = serde_json::from_value::<Vec<MarketOpportunity>>(message.data.clone()) {
                        if !opportunities.iter().any(|o| o.confidence >= min_confidence) {
                            return false;
                        }
                    }
                }
                InsightMessageType::PricePredictions => {
                    if let Ok(predictions) = serde_json::from_value::<Vec<PricePrediction>>(message.data.clone()) {
                        if !predictions.iter().any(|p| p.confidence >= min_confidence) {
                            return false;
                        }
                    }
                }
                _ => {}
            }
        }

        // Apply token filter
        if let Some(tokens) = &filters.tokens {
            // This would need to be implemented based on message content
            // For now, we'll assume all messages pass this filter
        }

        // Apply DEX filter
        if let Some(dexes) = &filters.dexes {
            // This would need to be implemented based on message content
            // For now, we'll assume all messages pass this filter
        }

        // Apply chain filter
        if let Some(chains) = &filters.chains {
            // This would need to be implemented based on message content
            // For now, we'll assume all messages pass this filter
        }

        true
    }

    /// Broadcast personalized insights to subscribed clients
    pub async fn broadcast_personalized_insights(
        &self,
        user_id: Uuid,
        insights: Vec<PersonalizedInsight>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = InsightMessage {
            message_type: InsightMessageType::PersonalizedInsights,
            user_id: Some(user_id),
            data: serde_json::to_value(insights)?,
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_tx.send(message);
        Ok(())
    }

    /// Broadcast market opportunities to subscribed clients
    pub async fn broadcast_market_opportunities(
        &self,
        user_id: Uuid,
        opportunities: Vec<MarketOpportunity>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = InsightMessage {
            message_type: InsightMessageType::MarketOpportunities,
            user_id: Some(user_id),
            data: serde_json::to_value(opportunities)?,
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_tx.send(message);
        Ok(())
    }

    /// Broadcast timing recommendations to subscribed clients
    pub async fn broadcast_timing_recommendations(
        &self,
        user_id: Uuid,
        recommendations: Vec<TimingRecommendation>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = InsightMessage {
            message_type: InsightMessageType::TimingRecommendations,
            user_id: Some(user_id),
            data: serde_json::to_value(recommendations)?,
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_tx.send(message);
        Ok(())
    }

    /// Broadcast price predictions to all subscribed clients
    pub async fn broadcast_price_predictions(
        &self,
        predictions: Vec<PricePrediction>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = InsightMessage {
            message_type: InsightMessageType::PricePredictions,
            user_id: None, // Global message
            data: serde_json::to_value(predictions)?,
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_tx.send(message);
        Ok(())
    }

    /// Broadcast liquidity forecasts to all subscribed clients
    pub async fn broadcast_liquidity_forecasts(
        &self,
        forecasts: Vec<LiquidityForecast>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = InsightMessage {
            message_type: InsightMessageType::LiquidityForecasts,
            user_id: None, // Global message
            data: serde_json::to_value(forecasts)?,
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_tx.send(message);
        Ok(())
    }

    /// Broadcast market sentiment analysis to all subscribed clients
    pub async fn broadcast_market_sentiment(
        &self,
        sentiment: MarketSentimentAnalysis,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = InsightMessage {
            message_type: InsightMessageType::MarketSentiment,
            user_id: None, // Global message
            data: serde_json::to_value(sentiment)?,
            timestamp: Utc::now(),
        };

        let _ = self.broadcast_tx.send(message);
        Ok(())
    }

    /// Start background task to generate and broadcast insights periodically
    pub async fn start_background_broadcasting(&self) {
        let server = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Generate and broadcast market sentiment
                if let Ok(sentiment) = server.predictive_analytics.analyze_market_sentiment().await {
                    let _ = server.broadcast_market_sentiment(sentiment).await;
                }

                // Generate and broadcast price predictions for major tokens
                let tokens = vec!["ETH".to_string(), "WBTC".to_string(), "LINK".to_string()];
                if let Ok(predictions) = server.predictive_analytics
                    .predict_price_trends(tokens, Duration::hours(1)).await {
                    let _ = server.broadcast_price_predictions(predictions).await;
                }

                // Generate and broadcast liquidity forecasts
                let pairs = vec!["ETH/USDC".to_string(), "WBTC/ETH".to_string()];
                if let Ok(forecasts) = server.predictive_analytics
                    .forecast_liquidity(pairs, Duration::hours(24)).await {
                    let _ = server.broadcast_liquidity_forecasts(forecasts).await;
                }
            }
        });
    }

    /// Get active subscription count
    pub async fn get_active_subscriptions(&self) -> usize {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.len()
    }

    /// Get subscription statistics
    pub async fn get_subscription_stats(&self) -> HashMap<String, usize> {
        let subscriptions = self.subscriptions.read().await;
        let mut stats = HashMap::new();
        
        for subscription in subscriptions.values() {
            for message_type in &subscription.message_types {
                let key = format!("{:?}", message_type);
                *stats.entry(key).or_insert(0) += 1;
            }
        }
        
        stats
    }
}

impl Clone for InsightsWebSocketServer {
    fn clone(&self) -> Self {
        Self {
            dashboard_service: self.dashboard_service.clone(),
            market_intelligence: self.market_intelligence.clone(),
            personalization_engine: self.personalization_engine.clone(),
            predictive_analytics: self.predictive_analytics.clone(),
            broadcast_tx: self.broadcast_tx.clone(),
            subscriptions: self.subscriptions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_retention::performance_analytics::UserPerformanceAnalyzer;
    use crate::user_retention::trading_insights::{MarketIntelligenceEngine, PersonalizationEngine, DashboardService};
    use crate::dex_aggregator::DEXAggregator;
    use crate::cache::RiskCache;
    use redis::Client;

    #[tokio::test]
    async fn test_websocket_server_creation() {
        let redis_client = Client::open("redis://127.0.0.1:6379/").unwrap();
        let cache = Arc::new(RiskCache::new(redis_client.clone()));
        let dex_aggregator = Arc::new(DEXAggregator::new(cache.clone(), redis_client.clone()));
        
        let user_analyzer = Arc::new(UserPerformanceAnalyzer::new(cache.clone()));
        let market_intelligence = Arc::new(MarketIntelligenceEngine::new(dex_aggregator, cache.clone()));
        let personalization_engine = Arc::new(PersonalizationEngine::new(
            user_analyzer.clone(),
            market_intelligence.clone(),
            cache.clone(),
        ));
        let dashboard_service = Arc::new(DashboardService::new(
            market_intelligence.clone(),
            personalization_engine.clone(),
            user_analyzer,
            cache.clone(),
        ));
        let predictive_analytics = Arc::new(PredictiveAnalytics::new(
            market_intelligence.clone(),
            user_analyzer,
            cache,
        ));
        
        let server = InsightsWebSocketServer::new(
            dashboard_service,
            market_intelligence,
            personalization_engine,
            predictive_analytics,
        );
        
        // Test initial state
        assert_eq!(server.get_active_subscriptions().await, 0);
        
        // Test stats
        let stats = server.get_subscription_stats().await;
        assert!(stats.is_empty());
    }

    #[tokio::test]
    async fn test_message_filtering() {
        let filters = SubscriptionFilters {
            min_confidence: Some(0.8),
            risk_levels: None,
            tokens: None,
            dexes: None,
            chains: None,
        };

        // Create a test message with high confidence insights
        let high_confidence_insight = PersonalizedInsight {
            insight_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            insight_type: crate::user_retention::trading_insights::personalization_engine::InsightType::TradingOpportunity,
            title: "Test".to_string(),
            description: "Test".to_string(),
            action_items: vec![],
            confidence_score: 0.9,
            priority: crate::user_retention::trading_insights::personalization_engine::InsightPriority::High,
            relevant_tokens: vec![],
            relevant_dexes: vec![],
            estimated_impact: crate::user_retention::trading_insights::personalization_engine::EstimatedImpact {
                potential_profit: None,
                risk_reduction: None,
                gas_savings: None,
                time_savings: None,
            },
            expires_at: None,
            created_at: Utc::now(),
        };

        let message = InsightMessage {
            message_type: InsightMessageType::PersonalizedInsights,
            user_id: Some(Uuid::new_v4()),
            data: serde_json::to_value(vec![high_confidence_insight]).unwrap(),
            timestamp: Utc::now(),
        };

        // Test that high confidence message passes filter
        assert!(InsightsWebSocketServer::message_passes_filters(&message, &filters));
    }
}
