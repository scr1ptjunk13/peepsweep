pub mod data_models;
pub mod live_pnl_engine;
pub mod pnl_integration;
pub mod pnl_persistence;
pub mod timescaledb_persistence;
pub mod multi_currency_pnl;
pub mod pnl_compression;
pub mod pnl_aggregation;
pub mod pnl_api;
pub mod pnl_websocket;
pub mod performance_metrics;
pub mod benchmark_integration;
pub mod performance_comparison;
pub mod performance_api;
pub mod simple_cache;
pub mod gas_usage_tracker;
pub mod gas_optimization_analyzer;
pub mod gas_reports_generator;
pub mod pnl_calculator;
pub mod trade_history;
pub mod data_aggregation_engine;
pub mod competitive_benchmark_engine;
pub mod advanced_cache_manager;
// Production enhancement modules
pub mod data_compression_engine;
pub mod advanced_monitoring_engine;

// API modules
pub mod trade_history_api;
pub mod trade_streaming;
pub mod api_tests;
pub mod simple_trade_api;
pub mod performance_tests;
pub mod load_test;
pub mod performance_benchmark;

#[cfg(test)]
pub mod tests;


pub use data_models::*;
pub use live_pnl_engine::*;
pub use pnl_integration::*;
pub use pnl_persistence::*;
pub use timescaledb_persistence::*;
pub use multi_currency_pnl::*;
pub use pnl_compression::*;
pub use pnl_aggregation::*;
pub use pnl_api::*;
pub use pnl_websocket::*;
pub use performance_metrics::*;
pub use benchmark_integration::*;
pub use performance_comparison::*;
pub use performance_api::*;
pub use simple_cache::*;
pub use gas_usage_tracker::*;
pub use gas_optimization_analyzer::*;
pub use gas_reports_generator::*;
pub use pnl_calculator::*;
pub use trade_history::*;
pub use data_aggregation_engine::*;
pub use competitive_benchmark_engine::*;
pub use advanced_cache_manager::*;
pub use data_compression_engine::*;
pub use advanced_monitoring_engine::*;
