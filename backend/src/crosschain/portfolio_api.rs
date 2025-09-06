use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use super::{
    portfolio_manager::PortfolioManager,
    portfolio_websocket::{PortfolioWebSocketManager, WebSocketQuery},
};

#[derive(Clone)]
pub struct PortfolioApiState {
    pub portfolio_manager: Arc<PortfolioManager>,
    pub websocket_manager: Arc<PortfolioWebSocketManager>,
}

impl PortfolioApiState {
    pub fn new(portfolio_manager: Arc<PortfolioManager>) -> Self {
        let websocket_manager = Arc::new(PortfolioWebSocketManager::new(Arc::clone(&portfolio_manager)));
        Self {
            portfolio_manager,
            websocket_manager,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PortfolioQuery {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct ChainBalanceQuery {
    pub address: String,
    pub chain_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: u64,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

pub fn create_portfolio_router() -> Router<PortfolioApiState> {
    Router::new()
        .route("/", get(get_portfolio))
        .route("/summary", get(get_portfolio_summary))
        .route("/balances", get(get_chain_balance))
        .route("/ws", get(portfolio_websocket))
        .route("/connections", get(get_websocket_connections))
        .route("/health", get(portfolio_health))
}

async fn portfolio_health() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse::success("Portfolio API is healthy"))
}

async fn get_portfolio(
    State(state): State<PortfolioApiState>,
    Query(query): Query<PortfolioQuery>,
) -> Result<Json<ApiResponse<super::portfolio_manager::Portfolio>>, StatusCode> {
    info!("Getting portfolio for address: {}", query.address);

    match state.portfolio_manager.get_portfolio(&query.address).await {
        Ok(portfolio) => Ok(Json(ApiResponse::success(portfolio))),
        Err(e) => {
            error!("Failed to get portfolio: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to get portfolio: {}", e
            ))))
        }
    }
}

async fn get_portfolio_summary(
    State(state): State<PortfolioApiState>,
    Query(query): Query<PortfolioQuery>,
) -> Result<Json<ApiResponse<super::portfolio_manager::PortfolioSummary>>, StatusCode> {
    info!("Getting portfolio summary for address: {}", query.address);

    match state.portfolio_manager.get_portfolio_summary(&query.address).await {
        Ok(summary) => Ok(Json(ApiResponse::success(summary))),
        Err(e) => {
            error!("Failed to get portfolio summary: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to get portfolio summary: {}", e
            ))))
        }
    }
}

async fn get_chain_balance(
    State(state): State<PortfolioApiState>,
    Query(query): Query<ChainBalanceQuery>,
) -> Result<Json<ApiResponse<super::portfolio_manager::ChainBalanceResponse>>, StatusCode> {
    info!("Getting chain balance for address: {} on chain: {}", query.address, query.chain_id);

    match state.portfolio_manager.get_chain_balance_detailed(&query.address, query.chain_id).await {
        Ok(balance) => Ok(Json(ApiResponse::success(balance))),
        Err(e) => {
            error!("Failed to get chain balance: {}", e);
            Ok(Json(ApiResponse::error(format!(
                "Failed to get chain balance: {}", e
            ))))
        }
    }
}


async fn portfolio_websocket(
    Query(query): Query<WebSocketQuery>,
    State(state): State<PortfolioApiState>,
    ws: WebSocketUpgrade,
) -> Response {
    info!("WebSocket connection request for address: {}", query.address);
    state.websocket_manager.handle_websocket(ws, Query(query)).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketStats {
    pub active_connections: u32,
    pub messages_sent: u64,
    pub last_update: u64,
}

async fn get_websocket_connections(
    State(state): State<PortfolioApiState>,
) -> Result<Json<ApiResponse<WebSocketStats>>, StatusCode> {
    let active_connections = state.websocket_manager.get_active_connections().await;
    let messages_sent = 0; // Mock implementation
    let last_update = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let stats = WebSocketStats {
        active_connections: active_connections as u32,
        messages_sent,
        last_update,
    };

    Ok(Json(ApiResponse::success(stats)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crosschain::portfolio_manager::PortfolioManager;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use std::sync::Arc;

    async fn create_test_state() -> PortfolioApiState {
        let portfolio_manager = Arc::new(PortfolioManager::new());
        PortfolioApiState::new(portfolio_manager)
    }

    #[tokio::test]
    async fn test_portfolio_api_routes() {
        let state = create_test_state().await;
        let app = create_portfolio_router().with_state(state);
        let server = TestServer::new(app).unwrap();

        // Test portfolio endpoint
        let response = server
            .get("/")
            .add_query_param("address", "0x742d35Cc6634C0532925a3b8D4C9db1C4C5C5C5C")
            .await;
        
        assert_eq!(response.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_portfolio_summary_endpoint() {
        let state = create_test_state().await;
        let app = create_portfolio_router().with_state(state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/summary")
            .add_query_param("address", "0x742d35Cc6634C0532925a3b8D4C9db1C4C5C5C5C")
            .await;
        
        assert_eq!(response.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_chain_balance_endpoint() {
        let state = create_test_state().await;
        let app = create_portfolio_router().with_state(state);
        let server = TestServer::new(app).unwrap();

        let response = server
            .get("/balances")
            .add_query_param("address", "0x742d35Cc6634C0532925a3b8D4C9db1C4C5C5C5C")
            .add_query_param("chain_id", "1")
            .await;
        
        assert_eq!(response.status_code(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_websocket_connections_endpoint() {
        let state = create_test_state().await;
        let app = create_portfolio_router().with_state(state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/connections").await;
        assert_eq!(response.status_code(), StatusCode::OK);
        
        let body: ApiResponse<WebSocketStats> = response.json();
        assert!(body.success);
        assert_eq!(body.data.unwrap().active_connections, 0);
    }
}
