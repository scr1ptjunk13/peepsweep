use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::{info, warn, error, debug};

use super::types::*;

#[derive(Clone)]
pub struct TradeEventStreamer {
    config: TradeStreamingConfig,
    // User subscriptions: user_id -> subscription details
    subscriptions: Arc<RwLock<HashMap<Uuid, TradeSubscription>>>,
    // Event broadcasting channels
    execution_tx: broadcast::Sender<TradeExecutionEvent>,
    routing_tx: broadcast::Sender<RoutingDecisionEvent>,
    slippage_tx: broadcast::Sender<SlippageUpdateEvent>,
    failure_tx: broadcast::Sender<FailedTransactionEvent>,
    // Statistics tracking
    stats: Arc<RwLock<TradeStreamingStats>>,
    start_time: DateTime<Utc>,
}

impl TradeEventStreamer {
    pub fn new(config: TradeStreamingConfig) -> Result<Self> {
        let (execution_tx, _) = broadcast::channel(config.event_buffer_size);
        let (routing_tx, _) = broadcast::channel(config.event_buffer_size);
        let (slippage_tx, _) = broadcast::channel(config.event_buffer_size);
        let (failure_tx, _) = broadcast::channel(config.event_buffer_size);

        let stats = TradeStreamingStats {
            active_subscriptions: 0,
            events_emitted_total: 0,
            events_per_second: 0.0,
            average_latency_ms: 0.0,
            error_rate: 0.0,
            uptime_seconds: 0,
        };

        let streamer = Self {
            config,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            execution_tx,
            routing_tx,
            slippage_tx,
            failure_tx,
            stats: Arc::new(RwLock::new(stats)),
            start_time: Utc::now(),
        };

        // Start background tasks
        streamer.start_cleanup_task();
        streamer.start_stats_updater();

        info!("🚀 Trade Event Streamer initialized with {} max subscribers", 
              streamer.config.max_subscribers);

        Ok(streamer)
    }

    /// Subscribe to trade execution events only
    pub async fn subscribe_to_trade_events(&self, user_id: Uuid) -> Result<mpsc::UnboundedReceiver<TradeEventMessage>> {
        self.create_subscription(user_id, vec!["executions".to_string()]).await
    }

    /// Subscribe to routing decision events only
    pub async fn subscribe_to_routing_events(&self, user_id: Uuid) -> Result<mpsc::UnboundedReceiver<TradeEventMessage>> {
        self.create_subscription(user_id, vec!["routing".to_string()]).await
    }

    /// Subscribe to slippage update events only
    pub async fn subscribe_to_slippage_events(&self, user_id: Uuid) -> Result<mpsc::UnboundedReceiver<TradeEventMessage>> {
        self.create_subscription(user_id, vec!["slippage".to_string()]).await
    }

    /// Subscribe to transaction failure events only
    pub async fn subscribe_to_failure_events(&self, user_id: Uuid) -> Result<mpsc::UnboundedReceiver<TradeEventMessage>> {
        self.create_subscription(user_id, vec!["failures".to_string()]).await
    }

    /// Subscribe to all trade events
    pub async fn subscribe_to_all_events(&self, user_id: Uuid) -> Result<mpsc::UnboundedReceiver<TradeEventMessage>> {
        self.create_subscription(user_id, vec!["all".to_string()]).await
    }

    /// Create a subscription with specified event types
    async fn create_subscription(&self, user_id: Uuid, subscription_types: Vec<String>) -> Result<mpsc::UnboundedReceiver<TradeEventMessage>> {
        let mut subscriptions = self.subscriptions.write().await;
        
        // Check subscription limit
        if subscriptions.len() >= self.config.max_subscribers {
            return Err(anyhow::anyhow!("Maximum subscribers limit reached"));
        }

        let (sender, receiver) = mpsc::unbounded_channel();

        // Create subscription
        let subscription = TradeSubscription {
            user_id,
            subscription_types: subscription_types.clone(),
            sender: sender.clone(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
        };

        subscriptions.insert(user_id, subscription);

        // Send subscription acknowledgment
        let ack = TradeEventMessage::SubscriptionAck(TradeSubscriptionAck {
            user_id,
            subscription_type: subscription_types.join(","),
            status: "subscribed".to_string(),
            timestamp: Utc::now(),
        });

        if let Err(e) = sender.send(ack) {
            warn!("Failed to send subscription ack to {}: {}", user_id, e);
        }

        // Start event forwarding for this user
        self.start_event_forwarding(user_id).await;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_subscriptions = subscriptions.len() as u64;
        }

        info!("✅ User {} subscribed to trade events: {:?}", user_id, subscription_types);
        Ok(receiver)
    }

    /// Start forwarding broadcast events to user's channel
    async fn start_event_forwarding(&self, user_id: Uuid) {
        let subscriptions = self.subscriptions.clone();
        let execution_rx = self.execution_tx.subscribe();
        let routing_rx = self.routing_tx.subscribe();
        let slippage_rx = self.slippage_tx.subscribe();
        let failure_rx = self.failure_tx.subscribe();

        // Spawn task for execution events
        let subscriptions_clone = subscriptions.clone();
        tokio::spawn(async move {
            let mut rx = execution_rx;
            while let Ok(event) = rx.recv().await {
                if event.user_id == user_id {
                    Self::forward_event_to_user(&subscriptions_clone, user_id, "executions", TradeEventMessage::TradeExecution(event)).await;
                }
            }
        });

        // Spawn task for routing events
        let subscriptions_clone = subscriptions.clone();
        tokio::spawn(async move {
            let mut rx = routing_rx;
            while let Ok(event) = rx.recv().await {
                if event.user_id == user_id {
                    Self::forward_event_to_user(&subscriptions_clone, user_id, "routing", TradeEventMessage::RoutingDecision(event)).await;
                }
            }
        });

        // Spawn task for slippage events
        let subscriptions_clone = subscriptions.clone();
        tokio::spawn(async move {
            let mut rx = slippage_rx;
            while let Ok(event) = rx.recv().await {
                if event.user_id == user_id {
                    Self::forward_event_to_user(&subscriptions_clone, user_id, "slippage", TradeEventMessage::SlippageUpdate(event)).await;
                }
            }
        });

        // Spawn task for failure events
        let subscriptions_clone = subscriptions.clone();
        tokio::spawn(async move {
            let mut rx = failure_rx;
            while let Ok(event) = rx.recv().await {
                if event.user_id == user_id {
                    Self::forward_event_to_user(&subscriptions_clone, user_id, "failures", TradeEventMessage::TransactionFailure(event)).await;
                }
            }
        });
    }

    /// Forward event to user if they're subscribed to that event type
    async fn forward_event_to_user(
        subscriptions: &Arc<RwLock<HashMap<Uuid, TradeSubscription>>>,
        user_id: Uuid,
        event_type: &str,
        message: TradeEventMessage,
    ) {
        let subscriptions_guard = subscriptions.read().await;
        if let Some(subscription) = subscriptions_guard.get(&user_id) {
            // Check if user is subscribed to this event type
            if subscription.subscription_types.contains(&"all".to_string()) || 
               subscription.subscription_types.contains(&event_type.to_string()) {
                
                if let Err(e) = subscription.sender.send(message) {
                    warn!("Failed to forward {} event to user {}: {}", event_type, user_id, e);
                } else {
                    debug!("📤 Forwarded {} event to user {}", event_type, user_id);
                }
            }
        }
    }

    /// Emit trade execution event
    pub async fn emit_trade_execution(&self, event: TradeExecutionEvent) -> Result<()> {
        debug!("📊 Emitting trade execution event for user {}: {} -> {}", 
               event.user_id, event.token_in, event.token_out);

        if let Err(e) = self.execution_tx.send(event) {
            error!("Failed to emit trade execution event: {}", e);
            return Err(anyhow::anyhow!("Failed to emit trade execution event: {}", e));
        }

        self.increment_event_counter().await;
        Ok(())
    }

    /// Emit routing decision event
    pub async fn emit_routing_decision(&self, event: RoutingDecisionEvent) -> Result<()> {
        debug!("🛣️ Emitting routing decision for user {}: {} routes considered", 
               event.user_id, event.alternative_routes.len() + 1);

        // Forward to subscribed users immediately
        self.forward_routing_decision_to_users(&event).await;

        if let Err(e) = self.routing_tx.send(event) {
            error!("Failed to emit routing decision event: {}", e);
            return Err(anyhow::anyhow!("Failed to emit routing decision event: {}", e));
        }

        self.increment_event_counter().await;
        Ok(())
    }

    /// Forward routing decision to subscribed users
    async fn forward_routing_decision_to_users(&self, event: &RoutingDecisionEvent) {
        let subscriptions = self.subscriptions.read().await;
        
        for (user_id, subscription) in subscriptions.iter() {
            // Only forward to the user who initiated the routing decision or users subscribed to all events
            if *user_id == event.user_id || subscription.subscription_types.contains(&"all".to_string()) 
                || subscription.subscription_types.contains(&"routing_decisions".to_string()) {
                
                let message = TradeEventMessage::RoutingDecision(event.clone());
                
                if let Err(e) = subscription.sender.send(message) {
                    warn!("Failed to forward routing decision to user {}: {}", user_id, e);
                } else {
                    debug!("📤 Forwarded routing decision to user {}", user_id);
                }
            }
        }
    }

    /// Emit slippage update event
    pub async fn emit_slippage_update(&self, event: SlippageUpdateEvent) -> Result<()> {
        debug!("📈 Emitting slippage update for user {}: {:.3}% slippage on {}", 
               event.user_id, event.slippage_percentage, event.dex_name);

        // Forward to subscribed users immediately
        self.forward_slippage_update_to_users(&event).await;

        if let Err(e) = self.slippage_tx.send(event) {
            error!("Failed to emit slippage update event: {}", e);
            return Err(anyhow::anyhow!("Failed to emit slippage update event: {}", e));
        }

        self.increment_event_counter().await;
        Ok(())
    }

    /// Forward slippage update to subscribed users
    async fn forward_slippage_update_to_users(&self, event: &SlippageUpdateEvent) {
        let subscriptions = self.subscriptions.read().await;
        
        for (user_id, subscription) in subscriptions.iter() {
            // Forward to the user who initiated the trade or users subscribed to slippage events
            if *user_id == event.user_id || subscription.subscription_types.contains(&"all".to_string()) 
                || subscription.subscription_types.contains(&"slippage_updates".to_string()) {
                
                let message = TradeEventMessage::SlippageUpdate(event.clone());
                
                if let Err(e) = subscription.sender.send(message) {
                    warn!("Failed to forward slippage update to user {}: {}", user_id, e);
                } else {
                    debug!("📤 Forwarded slippage update to user {}", user_id);
                }
            }
        }
    }

    /// Emit transaction failure event
    pub async fn emit_transaction_failure(&self, event: FailedTransactionEvent) -> Result<()> {
        warn!("❌ Emitting transaction failure for user {}: {} - {}", 
              event.user_id, event.error_code, event.failure_reason);

        // Forward to subscribed users immediately
        self.forward_failure_event_to_users(&event).await;

        if let Err(e) = self.failure_tx.send(event) {
            error!("Failed to emit transaction failure event: {}", e);
            return Err(anyhow::anyhow!("Failed to emit transaction failure event: {}", e));
        }

        self.increment_event_counter().await;
        Ok(())
    }

    /// Forward transaction failure to subscribed users
    async fn forward_failure_event_to_users(&self, event: &FailedTransactionEvent) {
        let subscriptions = self.subscriptions.read().await;
        
        for (user_id, subscription) in subscriptions.iter() {
            // Forward to the user who initiated the failed transaction or users subscribed to failure events
            if *user_id == event.user_id || subscription.subscription_types.contains(&"all".to_string()) 
                || subscription.subscription_types.contains(&"transaction_failures".to_string()) {
                
                let message = TradeEventMessage::TransactionFailure(event.clone());
                
                if let Err(e) = subscription.sender.send(message) {
                    warn!("Failed to forward transaction failure to user {}: {}", user_id, e);
                } else {
                    debug!("📤 Forwarded transaction failure to user {}", user_id);
                }
            }
        }
    }

    /// Get number of active subscriptions
    pub async fn get_active_subscriptions(&self) -> usize {
        self.subscriptions.read().await.len()
    }

    /// Check if streamer is healthy
    pub async fn is_healthy(&self) -> bool {
        let subscriptions = self.subscriptions.read().await;
        subscriptions.len() <= self.config.max_subscribers
    }

    /// Get streaming statistics
    pub async fn get_stats(&self) -> TradeStreamingStats {
        let mut stats = self.stats.read().await.clone();
        stats.uptime_seconds = (Utc::now() - self.start_time).num_seconds() as u64;
        stats
    }

    /// Unsubscribe user from all events
    pub async fn unsubscribe_user(&self, user_id: Uuid) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;
        
        if subscriptions.remove(&user_id).is_some() {
            info!("🔌 User {} unsubscribed from trade events", user_id);
            
            // Update stats
            let mut stats = self.stats.write().await;
            stats.active_subscriptions = subscriptions.len() as u64;
            
            Ok(())
        } else {
            Err(anyhow::anyhow!("User {} not found in subscriptions", user_id))
        }
    }

    /// Increment event counter for statistics
    async fn increment_event_counter(&self) {
        let mut stats = self.stats.write().await;
        stats.events_emitted_total += 1;
    }

    /// Start background cleanup task for inactive subscriptions
    fn start_cleanup_task(&self) {
        let subscriptions = self.subscriptions.clone();
        let cleanup_interval = self.config.cleanup_interval_secs;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(cleanup_interval));
            
            loop {
                interval.tick().await;
                
                let mut subscriptions_guard = subscriptions.write().await;
                let now = Utc::now();
                let timeout_duration = chrono::Duration::minutes(30); // 30 minutes timeout
                
                let mut to_remove = Vec::new();
                
                for (user_id, subscription) in subscriptions_guard.iter() {
                    if now - subscription.last_activity > timeout_duration {
                        to_remove.push(*user_id);
                    }
                }
                
                for user_id in to_remove {
                    subscriptions_guard.remove(&user_id);
                    info!("🧹 Cleaned up inactive subscription for user {}", user_id);
                }
            }
        });
    }

    /// Start background statistics updater
    fn start_stats_updater(&self) {
        let stats = self.stats.clone();
        let start_time = self.start_time;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            let mut last_event_count = 0u64;
            let mut last_update = Utc::now();
            
            loop {
                interval.tick().await;
                
                let now = Utc::now();
                let mut stats_guard = stats.write().await;
                
                // Calculate events per second
                let events_diff = stats_guard.events_emitted_total - last_event_count;
                let time_diff = (now - last_update).num_seconds() as f64;
                
                if time_diff > 0.0 {
                    stats_guard.events_per_second = events_diff as f64 / time_diff;
                }
                
                // Update uptime
                stats_guard.uptime_seconds = (now - start_time).num_seconds() as u64;
                
                last_event_count = stats_guard.events_emitted_total;
                last_update = now;
            }
        });
    }
}
