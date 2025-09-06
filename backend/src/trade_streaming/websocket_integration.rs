use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;
use anyhow::Result;
use tracing::{info, warn, error, debug};
use rust_decimal::prelude::*;

use super::types::*;
use super::trade_event_streamer::TradeEventStreamer;
// use crate::risk_management::websocket_server::{RiskWebSocketServer, WebSocketMessage as RiskWebSocketMessage};

/// Integration layer between Trade Event Streamer and existing WebSocket infrastructure
#[derive(Clone)]
pub struct TradeWebSocketIntegration {
    trade_streamer: Arc<TradeEventStreamer>,
    // Bridge to existing risk management WebSocket server
    // risk_websocket_bridge: Option<Arc<RiskWebSocketServer>>,
}

impl TradeWebSocketIntegration {
    pub fn new(trade_streamer: Arc<TradeEventStreamer>) -> Self {
        Self {
            trade_streamer,
        }
    }

    /// Connect to existing risk management WebSocket server for unified streaming
    // pub fn with_risk_websocket_bridge(mut self, risk_server: Arc<RiskWebSocketServer>) -> Self {
    //     self.risk_websocket_bridge = Some(risk_server);
    //     info!("🔗 Trade streaming integrated with risk management WebSocket server");
    //     self
    // }

    /// Handle WebSocket connection for trade events
    pub async fn handle_trade_websocket_connection(
        &self,
        user_id: Uuid,
        subscription_type: &str,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<TradeEventMessage>> {
        info!("🔌 New trade WebSocket connection for user {} (type: {})", user_id, subscription_type);

        match subscription_type {
            "trade_executions" => {
                self.trade_streamer.subscribe_to_trade_events(user_id).await
            }
            "routing_decisions" => {
                self.trade_streamer.subscribe_to_routing_events(user_id).await
            }
            "slippage_updates" => {
                self.trade_streamer.subscribe_to_slippage_events(user_id).await
            }
            "transaction_failures" => {
                self.trade_streamer.subscribe_to_failure_events(user_id).await
            }
            "all_trade_events" | "all" => {
                self.trade_streamer.subscribe_to_all_events(user_id).await
            }
            _ => {
                error!("❌ Unknown subscription type: {}", subscription_type);
                Err(anyhow::anyhow!("Unknown subscription type: {}", subscription_type))
            }
        }
    }

    /// Emit trade execution event through the streaming system
    pub async fn emit_trade_execution(&self, event: TradeExecutionEvent) -> Result<()> {
        debug!("📊 Emitting trade execution: {} {} -> {} {}", 
               event.amount_in, event.token_in, event.amount_out, event.token_out);
        
        self.trade_streamer.emit_trade_execution(event).await
    }

    /// Emit routing decision event
    pub async fn emit_routing_decision(&self, event: RoutingDecisionEvent) -> Result<()> {
        debug!("🛣️ Emitting routing decision: {} selected from {} alternatives", 
               event.selected_route.len(), event.alternative_routes.len());
        
        self.trade_streamer.emit_routing_decision(event).await
    }

    /// Emit slippage update event
    pub async fn emit_slippage_update(&self, event: SlippageUpdateEvent) -> Result<()> {
        debug!("📈 Emitting slippage update: {:.3}% on {}", 
               event.slippage_percentage, event.dex_name);
        
        self.trade_streamer.emit_slippage_update(event).await
    }

    /// Emit transaction failure event
    pub async fn emit_transaction_failure(&self, event: FailedTransactionEvent) -> Result<()> {
        warn!("❌ Emitting transaction failure: {} - {}", 
              event.error_code, event.failure_reason);
        
        self.trade_streamer.emit_transaction_failure(event).await
    }

    /// Get streaming statistics
    pub async fn get_streaming_stats(&self) -> TradeStreamingStats {
        self.trade_streamer.get_stats().await
    }

    /// Check if streaming system is healthy
    pub async fn is_healthy(&self) -> bool {
        self.trade_streamer.is_healthy().await
    }

    /// Disconnect user from trade streaming
    pub async fn disconnect_user(&self, user_id: Uuid) -> Result<()> {
        info!("🔌 Disconnecting user {} from trade streaming", user_id);
        self.trade_streamer.unsubscribe_user(user_id).await
    }
}

/// Helper functions for converting between aggregator events and streaming events
pub mod event_converters {
    use super::*;
    use crate::types::QuoteParams;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    /// Convert aggregator quote to routing decision event
    pub fn quote_to_routing_decision(
        quote_id: Uuid,
        user_id: Uuid,
        params: &QuoteParams,
        selected_dex: &str,
        alternatives: Vec<&str>,
        expected_output: &str,
        gas_estimate: u64,
        price_impact: f64,
        selection_reason: &str,
    ) -> Result<RoutingDecisionEvent> {
        let amount_in = Decimal::from_str(&params.amount_in)
            .map_err(|e| anyhow::anyhow!("Invalid amount_in: {}", e))?;
        
        let expected_output = Decimal::from_str(expected_output)
            .map_err(|e| anyhow::anyhow!("Invalid expected_output: {}", e))?;

        let selected_route = vec![(selected_dex.to_string(), Decimal::from(100))];
        
        let alternative_routes: Vec<Vec<(String, Decimal)>> = alternatives
            .iter()
            .map(|dex| vec![(dex.to_string(), Decimal::from(100))])
            .collect();

        Ok(RoutingDecisionEvent {
            quote_id,
            user_id,
            token_in: params.token_in.clone(),
            token_out: params.token_out.clone(),
            amount_in,
            selected_route,
            alternative_routes,
            selection_reason: selection_reason.to_string(),
            expected_output,
            estimated_gas: gas_estimate,
            price_impact: Decimal::from_f64(price_impact).unwrap_or_default(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Convert transaction result to execution event
    pub fn transaction_to_execution_event(
        trade_id: Uuid,
        user_id: Uuid,
        params: &QuoteParams,
        transaction_hash: &str,
        actual_output: &str,
        gas_used: u64,
        gas_price: u64,
        execution_time_ms: u64,
        dex_name: &str,
        status: &str,
    ) -> Result<TradeExecutionEvent> {
        let amount_in = Decimal::from_str(&params.amount_in)
            .map_err(|e| anyhow::anyhow!("Invalid amount_in: {}", e))?;
        
        let amount_out = Decimal::from_str(actual_output)
            .map_err(|e| anyhow::anyhow!("Invalid actual_output: {}", e))?;

        Ok(TradeExecutionEvent {
            trade_id,
            user_id,
            token_in: params.token_in.clone(),
            token_out: params.token_out.clone(),
            amount_in,
            amount_out,
            dex_name: dex_name.to_string(),
            transaction_hash: transaction_hash.to_string(),
            gas_used,
            gas_price: Decimal::from(gas_price),
            execution_time_ms,
            status: status.to_string(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Convert slippage data to slippage event
    pub fn slippage_to_event(
        trade_id: Uuid,
        user_id: Uuid,
        token_in: &str,
        token_out: &str,
        expected_price: f64,
        actual_price: f64,
        liquidity_depth: f64,
        dex_name: &str,
    ) -> Result<SlippageUpdateEvent> {
        let slippage_percentage = if expected_price > 0.0 {
            ((expected_price - actual_price) / expected_price * 100.0).abs()
        } else {
            0.0
        };

        let price_impact = slippage_percentage * 0.7; // Estimate price impact as 70% of slippage

        let market_conditions = if slippage_percentage > 1.0 {
            "volatile"
        } else if liquidity_depth < 1000000.0 {
            "low_liquidity"
        } else {
            "normal"
        };

        Ok(SlippageUpdateEvent {
            trade_id,
            user_id,
            token_pair: (token_in.to_string(), token_out.to_string()),
            expected_price: Decimal::from_f64(expected_price).unwrap_or_default(),
            actual_price: Decimal::from_f64(actual_price).unwrap_or_default(),
            slippage_percentage: Decimal::from_f64(slippage_percentage).unwrap_or_default(),
            price_impact: Decimal::from_f64(price_impact).unwrap_or_default(),
            liquidity_depth: Decimal::from_f64(liquidity_depth).unwrap_or_default(),
            market_conditions: market_conditions.to_string(),
            dex_name: dex_name.to_string(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Convert transaction failure to failure event
    pub fn failure_to_event(
        trade_id: Uuid,
        user_id: Uuid,
        params: &QuoteParams,
        transaction_hash: Option<&str>,
        error_message: &str,
        gas_used: u64,
        gas_limit: u64,
        gas_price: u64,
        dex_name: &str,
    ) -> Result<FailedTransactionEvent> {
        let amount_in = Decimal::from_str(&params.amount_in)
            .map_err(|e| anyhow::anyhow!("Invalid amount_in: {}", e))?;

        // Categorize error
        let (error_code, retry_possible, suggested_gas_limit) = if error_message.contains("gas") {
            ("GAS_LIMIT_EXCEEDED", true, Some(gas_limit * 130 / 100)) // Suggest 30% more gas
        } else if error_message.contains("slippage") {
            ("SLIPPAGE_EXCEEDED", true, None)
        } else if error_message.contains("insufficient") {
            ("INSUFFICIENT_FUNDS", false, None)
        } else if error_message.contains("revert") {
            ("CONTRACT_REVERT", true, None)
        } else {
            ("UNKNOWN_ERROR", false, None)
        };

        Ok(FailedTransactionEvent {
            trade_id,
            user_id,
            transaction_hash: transaction_hash.map(|s| s.to_string()),
            failure_reason: error_message.to_string(),
            error_code: error_code.to_string(),
            gas_used,
            gas_limit,
            gas_price: Decimal::from(gas_price),
            token_in: params.token_in.clone(),
            token_out: params.token_out.clone(),
            amount_in,
            dex_name: dex_name.to_string(),
            retry_possible,
            suggested_gas_limit,
            timestamp: chrono::Utc::now(),
        })
    }
}
