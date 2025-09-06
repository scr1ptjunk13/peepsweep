use crate::risk_management::performance_tracker::PortfolioPerformanceTracker;
use crate::routing::liquidity_tracker::LiquidityTracker;
use crate::bridges::BridgeManager;
use crate::risk_management::metrics_aggregation::{
    collectors::*, types::*
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Mutex};
use tokio::time::{interval, Duration};
use tracing::{info, warn, error};
use uuid::Uuid;

pub struct MetricsAggregator {
    config: MetricsAggregationConfig,
    snapshots: Arc<RwLock<VecDeque<MetricsSnapshot>>>,
    performance_collector: PerformanceMetricsCollector,
    dex_collector: DexLiquidityCollector,
    bridge_collector: BridgeStatusCollector,
    system_collector: SystemHealthCollector,
    metrics_sender: broadcast::Sender<MetricsSnapshot>,
    anomaly_detector: Arc<Mutex<AnomalyDetector>>,
}

impl MetricsAggregator {
    pub fn new(
        config: MetricsAggregationConfig,
        performance_monitor: Arc<PortfolioPerformanceTracker>,
        liquidity_tracker: Arc<LiquidityTracker>,
        bridge_manager: Option<Arc<BridgeManager>>,
    ) -> Self {
        let (metrics_sender, _) = broadcast::channel(1000);
        
        Self {
            config,
            snapshots: Arc::new(RwLock::new(VecDeque::new())),
            performance_collector: PerformanceMetricsCollector::new(performance_monitor),
            dex_collector: DexLiquidityCollector::new(liquidity_tracker),
            bridge_collector: BridgeStatusCollector::new(bridge_manager),
            system_collector: SystemHealthCollector::new(),
            anomaly_detector: Arc::new(Mutex::new(AnomalyDetector::new())),
            metrics_sender,
        }
    }


    pub async fn start_collection(&self) -> Result<()> {
        info!("Starting metrics aggregation with {} second intervals", self.config.collection_interval_seconds);
        
        let mut interval = interval(Duration::from_secs(self.config.collection_interval_seconds));
        
        loop {
            interval.tick().await;
            
            match self.collect_snapshot().await {
                Ok(snapshot) => {
                    self.store_snapshot(snapshot.clone()).await?;
                    
                    if self.config.real_time_updates {
                        if let Err(e) = self.metrics_sender.send(snapshot) {
                            warn!("Failed to broadcast metrics snapshot: {}", e);
                        }
                    }
                    
                    info!("Metrics snapshot collected successfully");
                }
                Err(e) => {
                    error!("Failed to collect metrics snapshot: {}", e);
                }
            }
        }
    }

    pub async fn collect_snapshot(&self) -> Result<MetricsSnapshot> {
        let timestamp = Utc::now();
        let id = Uuid::new_v4();
        
        info!("Collecting metrics snapshot at {}", timestamp);
        
        // Collect all metrics in parallel for better performance
        let (performance_result, dex_result, bridge_result, system_result) = tokio::join!(
            self.collect_performance_metrics(),
            self.collect_dex_metrics(),
            self.collect_bridge_metrics(),
            self.collect_system_metrics()
        );
        
        let performance_metrics = performance_result?;
        let dex_liquidity_metrics = dex_result?;
        let bridge_status_metrics = bridge_result?;
        let system_health_metrics = system_result?;
        
        let snapshot = MetricsSnapshot {
            id,
            timestamp,
            performance_metrics,
            dex_liquidity_metrics,
            bridge_status_metrics,
            system_health_metrics,
        };
        
        // Store snapshot
        {
            let mut snapshots = self.snapshots.write().await;
            snapshots.push_back(snapshot.clone());
            
            // Apply retention policy
            while snapshots.len() > self.config.max_snapshots {
                snapshots.pop_front();
            }
        }
        
        // Detect anomalies
        self.detect_anomalies(&snapshot).await;
        
        // Send real-time update if enabled
        if self.config.real_time_updates {
            let _ = self.metrics_sender.send(snapshot.clone());
        }
        
        Ok(snapshot)
    }

    async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics> {
        if !self.config.performance_monitoring_enabled {
            return Ok(PerformanceMetrics {
                active_connections: 0,
                total_processed: 0,
                avg_response_time_ms: 0.0,
                p95_response_time_ms: 0.0,
                p99_response_time_ms: 0.0,
                requests_per_second: 0.0,
                error_rate: 0.0,
                cache_hit_rate: 0.0,
            });
        }
        
        self.performance_collector.collect().await
    }

    async fn collect_dex_metrics(&self) -> Result<DexLiquidityMetrics> {
        if !self.config.dex_monitoring_enabled {
            return Ok(DexLiquidityMetrics {
                total_liquidity_usd: 0,
                active_pools: 0,
                total_volume_24h: 0,
                average_pool_size: 0,
                top_pools: Vec::new(),
                dex_breakdown: HashMap::new(),
            });
        }
        
        self.dex_collector.collect().await
    }

    async fn collect_bridge_metrics(&self) -> Result<BridgeStatusMetrics> {
        if !self.config.bridge_monitoring_enabled {
            return Ok(BridgeStatusMetrics {
                total_bridges: 0,
                active_bridges: 0,
                total_volume_24h: 0,
                average_bridge_time: 0.0,
                bridge_breakdown: HashMap::new(),
                cross_chain_routes: Vec::new(),
            });
        }
        
        self.bridge_collector.collect().await
    }

    async fn collect_system_metrics(&self) -> Result<SystemHealthMetrics> {
        if !self.config.system_monitoring_enabled {
            return Ok(SystemHealthMetrics {
                overall_health_score: 1.0,
                cpu_usage: 0.0,
                memory_usage: 0.0,
                disk_usage: 0.0,
                network_latency: 0.0,
                database_health: DatabaseHealth {
                    connection_pool_size: 0,
                    active_connections: 0,
                    query_avg_time_ms: 0.0,
                    slow_queries_count: 0,
                    status: HealthStatus::Healthy,
                },
                redis_health: RedisHealth {
                    connected: true,
                    memory_usage_mb: 0,
                    keys_count: 0,
                    hit_rate: 1.0,
                    avg_response_time_ms: 0.0,
                    status: HealthStatus::Healthy,
                },
                external_api_health: HashMap::new(),
                uptime_seconds: 0,
            });
        }
        
        self.system_collector.collect().await
    }

    async fn store_snapshot(&self, snapshot: MetricsSnapshot) -> Result<()> {
        let mut snapshots = self.snapshots.write().await;
        
        snapshots.push_back(snapshot);
        
        // Enforce retention policy
        while snapshots.len() > self.config.max_snapshots {
            snapshots.pop_front();
        }
        
        // Also enforce time-based retention
        let cutoff_time = Utc::now() - chrono::Duration::hours(self.config.retention_hours as i64);
        while let Some(front) = snapshots.front() {
            if front.timestamp < cutoff_time {
                snapshots.pop_front();
            } else {
                break;
            }
        }
        
        Ok(())
    }

    pub async fn query_metrics(&self, query: MetricsQuery) -> Result<AggregatedMetrics> {
        let snapshots = self.snapshots.read().await;
        
        let start_time = query.start_time.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(24));
        let end_time = query.end_time.unwrap_or_else(|| Utc::now());
        
        // Debug logging
        println!("🔍 Query metrics: {} snapshots available", snapshots.len());
        println!("🔍 Time range: {} to {}", start_time, end_time);
        
        let filtered_snapshots: Vec<&MetricsSnapshot> = snapshots
            .iter()
            .filter(|s| {
                let in_range = s.timestamp >= start_time && s.timestamp <= end_time;
                if !in_range {
                    println!("🔍 Snapshot {} outside range", s.timestamp);
                }
                in_range
            })
            .collect();
        
        println!("🔍 Filtered snapshots: {}", filtered_snapshots.len());
        
        let data_points = self.aggregate_data_points(&filtered_snapshots, &query)?;
        let summary = self.generate_summary(&filtered_snapshots, &query, start_time, end_time).await?;
        
        Ok(AggregatedMetrics {
            query,
            data_points,
            summary,
        })
    }

    fn aggregate_data_points(&self, snapshots: &[&MetricsSnapshot], query: &MetricsQuery) -> Result<Vec<MetricsDataPoint>> {
        let mut data_points = Vec::new();
        
        for snapshot in snapshots {
            let mut values = HashMap::new();
            
            for metric_type in &query.metric_types {
                match metric_type {
                    MetricType::Performance => {
                        values.insert("active_connections".to_string(), snapshot.performance_metrics.active_connections as f64);
                        values.insert("avg_response_time_ms".to_string(), snapshot.performance_metrics.avg_response_time_ms);
                        values.insert("requests_per_second".to_string(), snapshot.performance_metrics.requests_per_second);
                        values.insert("error_rate".to_string(), snapshot.performance_metrics.error_rate);
                        values.insert("cache_hit_rate".to_string(), snapshot.performance_metrics.cache_hit_rate);
                    }
                    MetricType::DexLiquidity => {
                        values.insert("total_liquidity_usd".to_string(), snapshot.dex_liquidity_metrics.total_liquidity_usd as f64);
                        values.insert("active_pools".to_string(), snapshot.dex_liquidity_metrics.active_pools as f64);
                        values.insert("total_volume_24h".to_string(), snapshot.dex_liquidity_metrics.total_volume_24h as f64);
                    }
                    MetricType::BridgeStatus => {
                        values.insert("total_bridges".to_string(), snapshot.bridge_status_metrics.total_bridges as f64);
                        values.insert("active_bridges".to_string(), snapshot.bridge_status_metrics.active_bridges as f64);
                        values.insert("bridge_volume_24h".to_string(), snapshot.bridge_status_metrics.total_volume_24h as f64);
                        values.insert("average_bridge_time".to_string(), snapshot.bridge_status_metrics.average_bridge_time);
                    }
                    MetricType::SystemHealth => {
                        values.insert("overall_health_score".to_string(), snapshot.system_health_metrics.overall_health_score);
                        values.insert("cpu_usage".to_string(), snapshot.system_health_metrics.cpu_usage);
                        values.insert("memory_usage".to_string(), snapshot.system_health_metrics.memory_usage);
                        values.insert("disk_usage".to_string(), snapshot.system_health_metrics.disk_usage);
                        values.insert("network_latency".to_string(), snapshot.system_health_metrics.network_latency);
                    }
                    MetricType::All => {
                        // Include all metrics
                        values.insert("active_connections".to_string(), snapshot.performance_metrics.active_connections as f64);
                        values.insert("total_liquidity_usd".to_string(), snapshot.dex_liquidity_metrics.total_liquidity_usd as f64);
                        values.insert("active_bridges".to_string(), snapshot.bridge_status_metrics.active_bridges as f64);
                        values.insert("overall_health_score".to_string(), snapshot.system_health_metrics.overall_health_score);
                    }
                }
            }
            
            data_points.push(MetricsDataPoint {
                timestamp: snapshot.timestamp,
                values,
            });
        }
        
        Ok(data_points)
    }

    async fn generate_summary(&self, snapshots: &[&MetricsSnapshot], query: &MetricsQuery, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Result<MetricsSummary> {
        let total_data_points = snapshots.len();
        let time_range = (start_time, end_time);
        
        let key_insights = self.generate_insights(snapshots);
        let anomalies_detected = self.get_anomalies_in_range(start_time, end_time).await;
        
        Ok(MetricsSummary {
            total_data_points,
            time_range,
            key_insights,
            anomalies_detected,
        })
    }

    fn generate_insights(&self, snapshots: &[&MetricsSnapshot]) -> Vec<String> {
        let mut insights = Vec::new();
        
        if snapshots.is_empty() {
            return insights;
        }
        
        // Performance insights
        let avg_response_times: Vec<f64> = snapshots.iter()
            .map(|s| s.performance_metrics.avg_response_time_ms)
            .collect();
        if let (Some(&min_response), Some(&max_response)) = (avg_response_times.iter().min_by(|a, b| a.partial_cmp(b).unwrap()), avg_response_times.iter().max_by(|a, b| a.partial_cmp(b).unwrap())) {
            insights.push(format!("Response time ranged from {:.2}ms to {:.2}ms", min_response, max_response));
        }
        
        // Liquidity insights
        let total_liquidity: Vec<u64> = snapshots.iter()
            .map(|s| s.dex_liquidity_metrics.total_liquidity_usd)
            .collect();
        if let (Some(&min_liquidity), Some(&max_liquidity)) = (total_liquidity.iter().min(), total_liquidity.iter().max()) {
            insights.push(format!("DEX liquidity ranged from ${:.2}M to ${:.2}M", min_liquidity as f64 / 1_000_000.0, max_liquidity as f64 / 1_000_000.0));
        }
        
        // Health insights
        let health_scores: Vec<f64> = snapshots.iter()
            .map(|s| s.system_health_metrics.overall_health_score)
            .collect();
        let avg_health = health_scores.iter().sum::<f64>() / health_scores.len() as f64;
        insights.push(format!("Average system health score: {:.1}%", avg_health * 100.0));
        
        insights
    }

    async fn detect_anomalies(&self, snapshot: &MetricsSnapshot) {
        let mut detector = self.anomaly_detector.lock().await;
        detector.analyze_snapshot(snapshot).await;
    }

    async fn get_anomalies_in_range(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Vec<MetricsAnomaly> {
        let detector = self.anomaly_detector.lock().await;
        detector.get_anomalies_in_range(start_time, end_time)
    }

    pub fn subscribe_to_metrics(&self) -> broadcast::Receiver<MetricsSnapshot> {
        self.metrics_sender.subscribe()
    }

    pub async fn get_latest_snapshot(&self) -> Option<MetricsSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.back().cloned()
    }

    pub async fn get_health_status(&self) -> HealthStatus {
        if let Some(latest) = self.get_latest_snapshot().await {
            let health_score = latest.system_health_metrics.overall_health_score;
            
            if health_score >= 0.9 {
                HealthStatus::Healthy
            } else if health_score >= 0.7 {
                HealthStatus::Warning
            } else if health_score >= 0.4 {
                HealthStatus::Critical
            } else {
                HealthStatus::Down
            }
        } else {
            HealthStatus::Down
        }
    }
}

pub struct AnomalyDetector {
    historical_data: HashMap<String, VecDeque<f64>>,
    anomalies: VecDeque<MetricsAnomaly>,
    max_history: usize,
}

impl AnomalyDetector {
    pub fn new() -> Self {
        Self {
            historical_data: HashMap::new(),
            anomalies: VecDeque::new(),
            max_history: 100, // Keep last 100 data points for each metric
        }
    }

    pub async fn analyze_snapshot(&mut self, snapshot: &MetricsSnapshot) {
        // Analyze performance metrics
        self.check_metric("response_time", snapshot.performance_metrics.avg_response_time_ms, snapshot.timestamp, 50.0, 200.0);
        self.check_metric("error_rate", snapshot.performance_metrics.error_rate, snapshot.timestamp, 0.05, 0.1);
        self.check_metric("cache_hit_rate", snapshot.performance_metrics.cache_hit_rate, snapshot.timestamp, 0.8, 0.6);
        
        // Analyze system health
        self.check_metric("cpu_usage", snapshot.system_health_metrics.cpu_usage, snapshot.timestamp, 80.0, 95.0);
        self.check_metric("memory_usage", snapshot.system_health_metrics.memory_usage, snapshot.timestamp, 85.0, 95.0);
        self.check_metric("health_score", snapshot.system_health_metrics.overall_health_score, snapshot.timestamp, 0.7, 0.5);
        
        // Clean up old anomalies (keep last 24 hours)
        let cutoff = Utc::now() - chrono::Duration::hours(24);
        while let Some(front) = self.anomalies.front() {
            if front.timestamp < cutoff {
                self.anomalies.pop_front();
            } else {
                break;
            }
        }
    }

    fn check_metric(&mut self, name: &str, value: f64, timestamp: DateTime<Utc>, warning_threshold: f64, critical_threshold: f64) {
        let history = self.historical_data.entry(name.to_string()).or_insert_with(VecDeque::new);
        
        // Add current value to history
        history.push_back(value);
        if history.len() > self.max_history {
            history.pop_front();
        }
        
        // Calculate expected value (simple moving average)
        if history.len() < 5 {
            return; // Need at least 5 data points
        }
        
        let expected_value = history.iter().sum::<f64>() / history.len() as f64;
        let std_dev = Self::calculate_std_dev_static(history, expected_value);
        
        // Detect anomalies based on standard deviation and thresholds
        let deviation = (value - expected_value).abs();
        let severity = if deviation > std_dev * 3.0 || value > critical_threshold {
            AnomalySeverity::Critical
        } else if deviation > std_dev * 2.0 || value > warning_threshold {
            AnomalySeverity::High
        } else if deviation > std_dev * 1.5 {
            AnomalySeverity::Medium
        } else {
            return; // No anomaly
        };
        
        let anomaly = MetricsAnomaly {
            timestamp,
            metric_name: name.to_string(),
            expected_value,
            actual_value: value,
            severity,
            description: format!("{} anomaly detected: expected {:.2}, got {:.2} (deviation: {:.2})", 
                name, expected_value, value, deviation),
        };
        
        self.anomalies.push_back(anomaly);
    }

    fn calculate_std_dev_static(values: &VecDeque<f64>, mean: f64) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        
        let variance = values.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / (values.len() - 1) as f64;
        
        variance.sqrt()
    }

    pub fn get_anomalies_in_range(&self, start_time: DateTime<Utc>, end_time: DateTime<Utc>) -> Vec<MetricsAnomaly> {
        self.anomalies
            .iter()
            .filter(|a| a.timestamp >= start_time && a.timestamp <= end_time)
            .cloned()
            .collect()
    }
}
