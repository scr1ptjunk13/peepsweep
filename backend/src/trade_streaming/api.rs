use axum::{
    extract::{Path, Query, State, WebSocketUpgrade, ws::{WebSocket, Message}},
    http::StatusCode,
    response::{Json, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn, error, debug};
use uuid::Uuid;
use tokio::sync::mpsc;

use super::{TradeWebSocketIntegration, TradeStreamingStats};
use super::types::*;

#[derive(Clone)]
pub struct TradeStreamingApiState {
    pub trade_streaming: Arc<TradeWebSocketIntegration>,
}

impl TradeStreamingApiState {
    pub fn new(trade_streaming: Arc<TradeWebSocketIntegration>) -> Self {
        Self { trade_streaming }
    }
}

#[derive(Deserialize)]
pub struct SubscriptionQuery {
    pub user_id: Uuid,
    pub event_types: Option<String>, // Comma-separated list: "trade_executions,routing_decisions"
}

#[derive(Serialize)]
pub struct SubscriptionResponse {
    pub success: bool,
    pub message: String,
    pub subscription_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct StreamingStatsResponse {
    pub stats: TradeStreamingStats,
    pub health_status: String,
}

/// Create trade streaming API router
pub fn create_trade_streaming_router() -> Router<TradeStreamingApiState> {
    Router::new()
        .route("/ws", get(websocket_handler))
        .route("/ws/:subscription_type", get(websocket_handler))
        .route("/subscribe", post(subscribe_to_events))
        .route("/unsubscribe/:user_id", post(unsubscribe_from_events))
        .route("/stats", get(get_streaming_stats))
        .route("/health", get(get_streaming_health))
        .route("/emit/execution", post(emit_trade_execution))
        .route("/emit/routing", post(emit_routing_decision))
        .route("/emit/slippage", post(emit_slippage_update))
        .route("/emit/failure", post(emit_transaction_failure))
}

/// WebSocket handler for real-time trade event streaming
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    path: Option<Path<String>>,
    Query(params): Query<SubscriptionQuery>,
    State(state): State<TradeStreamingApiState>,
) -> Response {
    let subscription_type = path.map(|Path(s)| s).unwrap_or_else(|| "all".to_string());
    info!("🔌 WebSocket connection request for user {} (type: {})", params.user_id, subscription_type);
    
    ws.on_upgrade(move |socket| handle_websocket(socket, params.user_id, subscription_type, state))
}

/// Handle WebSocket connection for trade events
async fn handle_websocket(
    mut socket: WebSocket,
    user_id: Uuid,
    subscription_type: String,
    state: TradeStreamingApiState,
) {
    info!("🔗 WebSocket connected for user {} ({})", user_id, subscription_type);

    // Subscribe to trade events
    let mut receiver = match state.trade_streaming.handle_trade_websocket_connection(user_id, &subscription_type).await {
        Ok(rx) => rx,
        Err(e) => {
            error!("❌ Failed to subscribe user {}: {}", user_id, e);
            let _ = socket.send(Message::Text(format!("{{\"error\": \"Failed to subscribe: {}\"}}", e))).await;
            return;
        }
    };

    // Send initial acknowledgment
    let ack = serde_json::json!({
        "user_id": user_id,
        "event_types": vec![subscription_type.clone()],
        "subscription_id": Uuid::new_v4(),
        "timestamp": chrono::Utc::now(),
        "status": "subscribed"
    });
    
    if let Ok(ack_json) = serde_json::to_string(&ack) {
        let _ = socket.send(Message::Text(ack_json)).await;
    }

    // Handle incoming messages and forward events
    loop {
        tokio::select! {
            // Receive events from trade streamer
            event = receiver.recv() => {
                match event {
                    Some(trade_event) => {
                        if let Ok(json) = serde_json::to_string(&trade_event) {
                            if socket.send(Message::Text(json)).await.is_err() {
                                warn!("🔌 WebSocket connection closed for user {}", user_id);
                                break;
                            }
                        }
                    }
                    None => {
                        debug!("📡 Event channel closed for user {}", user_id);
                        break;
                    }
                }
            }
            
            // Handle incoming WebSocket messages
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!("📨 Received message from user {}: {}", user_id, text);
                        // Handle ping/pong or other control messages
                        if text == "ping" {
                            let _ = socket.send(Message::Text("pong".to_string())).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        info!("🔌 WebSocket closed by user {}", user_id);
                        break;
                    }
                    Some(Ok(Message::Binary(_))) | Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                        // Handle binary, ping, pong messages (ignore or respond appropriately)
                        continue;
                    }
                    Some(Err(e)) => {
                        warn!("❌ WebSocket error for user {}: {}", user_id, e);
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    // Cleanup subscription
    if let Err(e) = state.trade_streaming.disconnect_user(user_id).await {
        warn!("⚠️ Failed to cleanup subscription for user {}: {}", user_id, e);
    }
    
    info!("🔌 WebSocket disconnected for user {}", user_id);
}

/// Subscribe to trade events via HTTP POST
pub async fn subscribe_to_events(
    State(state): State<TradeStreamingApiState>,
    Json(params): Json<SubscriptionQuery>,
) -> Result<Json<SubscriptionResponse>, StatusCode> {
    info!("📝 Subscription request for user {}", params.user_id);

    let event_types = params.event_types.unwrap_or_else(|| "all_trade_events".to_string());
    
    match state.trade_streaming.handle_trade_websocket_connection(params.user_id, &event_types).await {
        Ok(_) => {
            let response = SubscriptionResponse {
                success: true,
                message: format!("Successfully subscribed to {}", event_types),
                subscription_id: Some(Uuid::new_v4()),
            };
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ Subscription failed for user {}: {}", params.user_id, e);
            let response = SubscriptionResponse {
                success: false,
                message: format!("Subscription failed: {}", e),
                subscription_id: None,
            };
            Ok(Json(response))
        }
    }
}

/// Unsubscribe from trade events
pub async fn unsubscribe_from_events(
    Path(user_id): Path<Uuid>,
    State(state): State<TradeStreamingApiState>,
) -> Result<Json<SubscriptionResponse>, StatusCode> {
    info!("🗑️ Unsubscribe request for user {}", user_id);

    match state.trade_streaming.disconnect_user(user_id).await {
        Ok(_) => {
            let response = SubscriptionResponse {
                success: true,
                message: "Successfully unsubscribed from all events".to_string(),
                subscription_id: None,
            };
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ Unsubscribe failed for user {}: {}", user_id, e);
            let response = SubscriptionResponse {
                success: false,
                message: format!("Unsubscribe failed: {}", e),
                subscription_id: None,
            };
            Ok(Json(response))
        }
    }
}

/// Get streaming statistics
pub async fn get_streaming_stats(
    State(state): State<TradeStreamingApiState>,
) -> Result<Json<StreamingStatsResponse>, StatusCode> {
    let stats = state.trade_streaming.get_streaming_stats().await;
    let is_healthy = state.trade_streaming.is_healthy().await;
    
    let response = StreamingStatsResponse {
        stats,
        health_status: if is_healthy { "healthy".to_string() } else { "degraded".to_string() },
    };
    
    Ok(Json(response))
}

/// Get streaming system health
pub async fn get_streaming_health(
    State(state): State<TradeStreamingApiState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let is_healthy = state.trade_streaming.is_healthy().await;
    let stats = state.trade_streaming.get_streaming_stats().await;
    
    let health = serde_json::json!({
        "status": if is_healthy { "healthy" } else { "degraded" },
        "active_subscriptions": stats.active_subscriptions,
        "events_processed": stats.events_emitted_total,
        "uptime_seconds": stats.uptime_seconds,
        "timestamp": chrono::Utc::now()
    });
    
    Ok(Json(health))
}

/// Emit trade execution event (for testing/integration)
pub async fn emit_trade_execution(
    State(state): State<TradeStreamingApiState>,
    Json(event): Json<TradeExecutionEvent>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.trade_streaming.emit_trade_execution(event).await {
        Ok(_) => {
            let response = serde_json::json!({
                "success": true,
                "message": "Trade execution event emitted successfully"
            });
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ Failed to emit trade execution: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Emit routing decision event (for testing/integration)
pub async fn emit_routing_decision(
    State(state): State<TradeStreamingApiState>,
    Json(event): Json<RoutingDecisionEvent>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.trade_streaming.emit_routing_decision(event).await {
        Ok(_) => {
            let response = serde_json::json!({
                "success": true,
                "message": "Routing decision event emitted successfully"
            });
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ Failed to emit routing decision: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Emit slippage update event (for testing/integration)
pub async fn emit_slippage_update(
    State(state): State<TradeStreamingApiState>,
    Json(event): Json<SlippageUpdateEvent>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.trade_streaming.emit_slippage_update(event).await {
        Ok(_) => {
            let response = serde_json::json!({
                "success": true,
                "message": "Slippage update event emitted successfully"
            });
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ Failed to emit slippage update: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Emit transaction failure event (for testing/integration)
pub async fn emit_transaction_failure(
    State(state): State<TradeStreamingApiState>,
    Json(event): Json<FailedTransactionEvent>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.trade_streaming.emit_transaction_failure(event).await {
        Ok(_) => {
            let response = serde_json::json!({
                "success": true,
                "message": "Transaction failure event emitted successfully"
            });
            Ok(Json(response))
        }
        Err(e) => {
            error!("❌ Failed to emit transaction failure: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
