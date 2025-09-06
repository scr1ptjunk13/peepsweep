use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub performance_metrics: PerformanceMetrics,
    pub dex_liquidity_metrics: DexLiquidityMetrics,
    pub bridge_status_metrics: BridgeStatusMetrics,
    pub system_health_metrics: SystemHealthMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub active_connections: usize,
    pub total_processed: u64,
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub p99_response_time_ms: f64,
    pub requests_per_second: f64,
    pub error_rate: f64,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexLiquidityMetrics {
    pub total_liquidity_usd: u64,
    pub active_pools: usize,
    pub total_volume_24h: u64,
    pub average_pool_size: u64,
    pub top_pools: Vec<PoolMetrics>,
    pub dex_breakdown: HashMap<String, DexMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetrics {
    pub pool_address: String,
    pub dex: String,
    pub token_pair: String,
    pub liquidity_usd: u64,
    pub volume_24h: u64,
    pub fee_tier: u32,
    pub price_impact_1k: f64,
    pub price_impact_10k: f64,
    pub price_impact_100k: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexMetrics {
    pub name: String,
    pub total_liquidity: u64,
    pub pool_count: usize,
    pub volume_24h: u64,
    pub avg_fee: f64,
    pub status: DexStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DexStatus {
    Online,
    Degraded,
    Offline,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatusMetrics {
    pub total_bridges: usize,
    pub active_bridges: usize,
    pub total_volume_24h: u64,
    pub average_bridge_time: f64,
    pub bridge_breakdown: HashMap<String, BridgeMetrics>,
    pub cross_chain_routes: Vec<CrossChainRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMetrics {
    pub name: String,
    pub status: BridgeStatus,
    pub volume_24h: u64,
    pub success_rate: f64,
    pub avg_completion_time: f64,
    pub fee_percentage: f64,
    pub supported_chains: Vec<String>,
    pub last_successful_tx: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeStatus {
    Active,
    Congested,
    Maintenance,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainRoute {
    pub from_chain: String,
    pub to_chain: String,
    pub bridge_name: String,
    pub estimated_time: f64,
    pub fee_percentage: f64,
    pub liquidity_available: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthMetrics {
    pub overall_health_score: f64,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_latency: f64,
    pub database_health: DatabaseHealth,
    pub redis_health: RedisHealth,
    pub external_api_health: HashMap<String, ApiHealth>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    pub connection_pool_size: usize,
    pub active_connections: usize,
    pub query_avg_time_ms: f64,
    pub slow_queries_count: u64,
    pub status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisHealth {
    pub connected: bool,
    pub memory_usage_mb: u64,
    pub keys_count: u64,
    pub hit_rate: f64,
    pub avg_response_time_ms: f64,
    pub status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiHealth {
    pub name: String,
    pub status: HealthStatus,
    pub response_time_ms: f64,
    pub success_rate: f64,
    pub last_check: DateTime<Utc>,
    pub error_count_24h: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsAggregationConfig {
    pub collection_interval_seconds: u64,
    pub retention_hours: u64,
    pub max_snapshots: usize,
    pub performance_monitoring_enabled: bool,
    pub dex_monitoring_enabled: bool,
    pub bridge_monitoring_enabled: bool,
    pub system_monitoring_enabled: bool,
    pub real_time_updates: bool,
}

impl Default for MetricsAggregationConfig {
    fn default() -> Self {
        Self {
            collection_interval_seconds: 30, // Collect every 30 seconds
            retention_hours: 168, // 7 days
            max_snapshots: 20160, // 7 days * 24 hours * 60 minutes * 2 (30-second intervals)
            performance_monitoring_enabled: true,
            dex_monitoring_enabled: true,
            bridge_monitoring_enabled: true,
            system_monitoring_enabled: true,
            real_time_updates: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsQuery {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub metric_types: Vec<MetricType>,
    pub aggregation: AggregationType,
    pub interval: Option<u64>, // seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Performance,
    DexLiquidity,
    BridgeStatus,
    SystemHealth,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationType {
    Raw,
    Average,
    Min,
    Max,
    Sum,
    Percentile(u8), // e.g., Percentile(95) for P95
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub query: MetricsQuery,
    pub data_points: Vec<MetricsDataPoint>,
    pub summary: MetricsSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsDataPoint {
    pub timestamp: DateTime<Utc>,
    pub values: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_data_points: usize,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    pub key_insights: Vec<String>,
    pub anomalies_detected: Vec<MetricsAnomaly>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsAnomaly {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub expected_value: f64,
    pub actual_value: f64,
    pub severity: AnomalySeverity,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}
