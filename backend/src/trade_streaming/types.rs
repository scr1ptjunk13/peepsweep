use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeStreamingConfig {
    pub max_subscribers: usize,
    pub event_buffer_size: usize,
    pub cleanup_interval_secs: u64,
}

impl Default for TradeStreamingConfig {
    fn default() -> Self {
        Self {
            max_subscribers: 10000,
            event_buffer_size: 100000,
            cleanup_interval_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TradeEventMessage {
    #[serde(rename = "trade_execution")]
    TradeExecution(TradeExecutionEvent),
    #[serde(rename = "routing_decision")]
    RoutingDecision(RoutingDecisionEvent),
    #[serde(rename = "slippage_update")]
    SlippageUpdate(SlippageUpdateEvent),
    #[serde(rename = "transaction_failure")]
    TransactionFailure(FailedTransactionEvent),
    #[serde(rename = "subscription_ack")]
    SubscriptionAck(TradeSubscriptionAck),
    #[serde(rename = "error")]
    Error(TradeErrorMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecutionEvent {
    pub trade_id: Uuid,
    pub user_id: Uuid,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: Decimal,
    pub amount_out: Decimal,
    pub dex_name: String,
    pub transaction_hash: String,
    pub gas_used: u64,
    pub gas_price: Decimal,
    pub execution_time_ms: u64,
    pub status: String, // "pending", "confirmed", "failed"
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecisionEvent {
    pub quote_id: Uuid,
    pub user_id: Uuid,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: Decimal,
    pub selected_route: Vec<(String, Decimal)>, // (dex_name, percentage)
    pub alternative_routes: Vec<Vec<(String, Decimal)>>,
    pub selection_reason: String,
    pub expected_output: Decimal,
    pub estimated_gas: u64,
    pub price_impact: Decimal,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageUpdateEvent {
    pub trade_id: Uuid,
    pub user_id: Uuid,
    pub token_pair: (String, String),
    pub expected_price: Decimal,
    pub actual_price: Decimal,
    pub slippage_percentage: Decimal,
    pub price_impact: Decimal,
    pub liquidity_depth: Decimal,
    pub market_conditions: String, // "normal", "volatile", "low_liquidity"
    pub dex_name: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedTransactionEvent {
    pub trade_id: Uuid,
    pub user_id: Uuid,
    pub transaction_hash: Option<String>,
    pub failure_reason: String,
    pub error_code: String,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub gas_price: Decimal,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: Decimal,
    pub dex_name: String,
    pub retry_possible: bool,
    pub suggested_gas_limit: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSubscriptionAck {
    pub user_id: Uuid,
    pub subscription_type: String, // "all", "executions", "routing", "slippage", "failures"
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeErrorMessage {
    pub code: String,
    pub message: String,
    pub user_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeSubscription {
    pub user_id: Uuid,
    pub subscription_types: Vec<String>,
    pub sender: tokio::sync::mpsc::UnboundedSender<TradeEventMessage>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeStreamingStats {
    pub active_subscriptions: u64,
    pub events_emitted_total: u64,
    pub events_per_second: f64,
    pub average_latency_ms: f64,
    pub error_rate: f64,
    pub uptime_seconds: u64,
}

// Event types for internal routing
#[derive(Debug, Clone)]
pub enum TradeEvent {
    Execution(TradeExecutionEvent),
    Routing(RoutingDecisionEvent),
    Slippage(SlippageUpdateEvent),
    Failure(FailedTransactionEvent),
}

impl From<TradeEvent> for TradeEventMessage {
    fn from(event: TradeEvent) -> Self {
        match event {
            TradeEvent::Execution(e) => TradeEventMessage::TradeExecution(e),
            TradeEvent::Routing(e) => TradeEventMessage::RoutingDecision(e),
            TradeEvent::Slippage(e) => TradeEventMessage::SlippageUpdate(e),
            TradeEvent::Failure(e) => TradeEventMessage::TransactionFailure(e),
        }
    }
}
