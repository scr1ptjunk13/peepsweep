use crate::websocket::{PnLWebSocketState, TradeWebSocketState};
use axum::{
    extract::{ws::WebSocketUpgrade, State},
    response::Response,
    routing::get,
    Router,
};
use std::sync::Arc;
use tracing::info;

/// Combined WebSocket manager state
#[derive(Clone)]
pub struct WebSocketManagerState {
    pub pnl_state: PnLWebSocketState,
    pub trade_state: TradeWebSocketState,
}

impl WebSocketManagerState {
    pub fn new(pnl_state: PnLWebSocketState, trade_state: TradeWebSocketState) -> Self {
        Self {
            pnl_state,
            trade_state,
        }
    }

    /// Start all background tasks
    pub async fn start_background_tasks(&self) {
        info!("Starting WebSocket background tasks");
        self.pnl_state.start_periodic_updates().await;
    }
}

/// Create WebSocket router
pub fn create_websocket_router() -> Router<WebSocketManagerState> {
    Router::new()
        .route("/ws/pnl", get(handle_pnl_websocket_upgrade))
        .route("/ws/trades", get(handle_trade_websocket_upgrade))
}

/// Handle P&L WebSocket upgrade
async fn handle_pnl_websocket_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketManagerState>,
) -> Response {
    crate::websocket::pnl_websocket::handle_pnl_websocket(ws, State(state.pnl_state)).await
}

/// Handle Trade WebSocket upgrade
async fn handle_trade_websocket_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketManagerState>,
) -> Response {
    crate::websocket::trade_websocket::handle_trade_websocket(ws, State(state.trade_state)).await
}
