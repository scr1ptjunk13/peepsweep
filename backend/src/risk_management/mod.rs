pub mod alert_management;
pub mod alert_system;
pub mod config;
pub mod database;
pub mod event_ingestion;
pub mod integrated_service;
pub mod metrics_aggregation;
pub mod performance_tracker;
pub mod position_tracker;
pub mod redis_cache;
pub mod risk_engine;
pub mod types;
pub mod websocket_server;

pub use alert_management::*;
pub use alert_system::*;
pub use config::*;
pub use database::*;
pub use event_ingestion::*;
pub use integrated_service::*;
pub use metrics_aggregation::*;
pub use performance_tracker::*;
pub use position_tracker::*;
pub use redis_cache::*;
pub use risk_engine::*;
pub use types::*;
pub use websocket_server::*;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_module_imports() {
        // Basic smoke test to ensure all modules compile
        assert!(true);
    }
}
