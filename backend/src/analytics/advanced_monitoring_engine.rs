use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::{debug, error, info, warn};

use crate::risk_management::RiskError;
use uuid::Uuid as UserId;

/// Advanced monitoring engine for operational excellence
#[derive(Debug)]
pub struct AdvancedMonitoringEngine {
    /// Active monitors
    monitors: Arc<RwLock<HashMap<String, Box<dyn Monitor + Send + Sync>>>>,
    /// Metrics collectors
    collectors: Arc<RwLock<HashMap<String, Box<dyn MetricsCollector + Send + Sync>>>>,
    /// Alert manager
    alert_manager: Arc<AlertManager>,
    /// Monitoring configuration
    config: MonitoringConfig,
    /// System metrics
    system_metrics: Arc<RwLock<SystemMetrics>>,
}

/// Monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub collection_interval_seconds: u64,
    pub alert_threshold_seconds: u64,
    pub enable_health_checks: bool,
    pub enable_performance_monitoring: bool,
    pub enable_error_tracking: bool,
    pub enable_resource_monitoring: bool,
    pub max_metrics_history: usize,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            collection_interval_seconds: 30,
            alert_threshold_seconds: 300,
            enable_health_checks: true,
            enable_performance_monitoring: true,
            enable_error_tracking: true,
            enable_resource_monitoring: true,
            max_metrics_history: 1000,
        }
    }
}

/// Monitor trait for different monitoring types
pub trait Monitor: std::fmt::Debug {
    fn name(&self) -> &str;
    fn check(&self) -> MonitorResult;
    fn is_critical(&self) -> bool;
    fn get_thresholds(&self) -> MonitorThresholds;
}

/// Metrics collector trait
pub trait MetricsCollector: std::fmt::Debug {
    fn name(&self) -> &str;
    fn collect(&self) -> Vec<Metric>;
    fn collection_interval(&self) -> Duration;
}

/// Monitor result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorResult {
    pub monitor_name: String,
    pub status: MonitorStatus,
    pub message: String,
    pub metrics: HashMap<String, f64>,
    pub timestamp: DateTime<Utc>,
    pub response_time_ms: u64,
}

/// Monitor status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MonitorStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

/// Monitor thresholds
#[derive(Debug, Clone)]
pub struct MonitorThresholds {
    pub warning_threshold: f64,
    pub critical_threshold: f64,
    pub timeout_seconds: u64,
}

/// Individual metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub tags: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// System metrics aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub disk_usage_percent: f64,
    pub network_io_bytes_per_sec: u64,
    pub active_connections: u32,
    pub request_rate_per_sec: f64,
    pub error_rate_percent: f64,
    pub average_response_time_ms: f64,
    pub cache_hit_rate_percent: f64,
    pub database_connections: u32,
    pub last_updated: DateTime<Utc>,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_usage_percent: 0.0,
            disk_usage_percent: 0.0,
            network_io_bytes_per_sec: 0,
            active_connections: 0,
            request_rate_per_sec: 0.0,
            error_rate_percent: 0.0,
            average_response_time_ms: 0.0,
            cache_hit_rate_percent: 0.0,
            database_connections: 0,
            last_updated: Utc::now(),
        }
    }
}

/// Alert manager for handling monitoring alerts
#[derive(Debug)]
pub struct AlertManager {
    /// Active alerts
    active_alerts: Arc<RwLock<HashMap<String, Alert>>>,
    /// Alert history
    alert_history: Arc<RwLock<Vec<Alert>>>,
    /// Alert configuration
    config: AlertConfig,
}

/// Alert configuration
#[derive(Debug, Clone)]
pub struct AlertConfig {
    pub enable_email_alerts: bool,
    pub enable_slack_alerts: bool,
    pub enable_webhook_alerts: bool,
    pub alert_cooldown_seconds: u64,
    pub max_alert_history: usize,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enable_email_alerts: true,
            enable_slack_alerts: true,
            enable_webhook_alerts: true,
            alert_cooldown_seconds: 300,
            max_alert_history: 1000,
        }
    }
}

/// Alert structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub monitor_name: String,
    pub severity: AlertSeverity,
    pub title: String,
    pub description: String,
    pub metrics: HashMap<String, f64>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub status: AlertStatus,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Alert status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertStatus {
    Active,
    Acknowledged,
    Resolved,
}

/// Health check monitor
#[derive(Debug)]
pub struct HealthCheckMonitor {
    name: String,
    endpoint: String,
    thresholds: MonitorThresholds,
}

impl HealthCheckMonitor {
    pub fn new(name: String, endpoint: String) -> Self {
        Self {
            name,
            endpoint,
            thresholds: MonitorThresholds {
                warning_threshold: 1000.0, // 1 second
                critical_threshold: 5000.0, // 5 seconds
                timeout_seconds: 10,
            },
        }
    }
}

impl Monitor for HealthCheckMonitor {
    fn name(&self) -> &str {
        &self.name
    }

    fn check(&self) -> MonitorResult {
        let start_time = std::time::Instant::now();
        
        // Simulate health check
        let response_time = start_time.elapsed().as_millis() as u64;
        let is_healthy = response_time < self.thresholds.critical_threshold as u64;
        
        let status = if !is_healthy {
            MonitorStatus::Critical
        } else if response_time > self.thresholds.warning_threshold as u64 {
            MonitorStatus::Warning
        } else {
            MonitorStatus::Healthy
        };

        let mut metrics = HashMap::new();
        metrics.insert("response_time_ms".to_string(), response_time as f64);
        metrics.insert("availability".to_string(), if is_healthy { 1.0 } else { 0.0 });

        MonitorResult {
            monitor_name: self.name.clone(),
            status,
            message: format!("Health check completed in {}ms", response_time),
            metrics,
            timestamp: Utc::now(),
            response_time_ms: response_time,
        }
    }

    fn is_critical(&self) -> bool {
        true
    }

    fn get_thresholds(&self) -> MonitorThresholds {
        self.thresholds.clone()
    }
}

/// Performance monitor
#[derive(Debug)]
pub struct PerformanceMonitor {
    name: String,
    thresholds: MonitorThresholds,
}

impl PerformanceMonitor {
    pub fn new(name: String) -> Self {
        Self {
            name,
            thresholds: MonitorThresholds {
                warning_threshold: 80.0, // 80% CPU/Memory
                critical_threshold: 95.0, // 95% CPU/Memory
                timeout_seconds: 30,
            },
        }
    }
}

impl Monitor for PerformanceMonitor {
    fn name(&self) -> &str {
        &self.name
    }

    fn check(&self) -> MonitorResult {
        let start_time = std::time::Instant::now();
        
        // Simulate performance metrics collection
        let cpu_usage = 45.2;
        let memory_usage = 67.8;
        let disk_usage = 23.1;
        
        let status = if cpu_usage > self.thresholds.critical_threshold || memory_usage > self.thresholds.critical_threshold {
            MonitorStatus::Critical
        } else if cpu_usage > self.thresholds.warning_threshold || memory_usage > self.thresholds.warning_threshold {
            MonitorStatus::Warning
        } else {
            MonitorStatus::Healthy
        };

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage_percent".to_string(), cpu_usage);
        metrics.insert("memory_usage_percent".to_string(), memory_usage);
        metrics.insert("disk_usage_percent".to_string(), disk_usage);

        MonitorResult {
            monitor_name: self.name.clone(),
            status,
            message: format!("CPU: {:.1}%, Memory: {:.1}%, Disk: {:.1}%", cpu_usage, memory_usage, disk_usage),
            metrics,
            timestamp: Utc::now(),
            response_time_ms: start_time.elapsed().as_millis() as u64,
        }
    }

    fn is_critical(&self) -> bool {
        true
    }

    fn get_thresholds(&self) -> MonitorThresholds {
        self.thresholds.clone()
    }
}

/// Database monitor
#[derive(Debug)]
pub struct DatabaseMonitor {
    name: String,
    thresholds: MonitorThresholds,
}

impl DatabaseMonitor {
    pub fn new(name: String) -> Self {
        Self {
            name,
            thresholds: MonitorThresholds {
                warning_threshold: 100.0, // 100ms query time
                critical_threshold: 1000.0, // 1s query time
                timeout_seconds: 5,
            },
        }
    }
}

impl Monitor for DatabaseMonitor {
    fn name(&self) -> &str {
        &self.name
    }

    fn check(&self) -> MonitorResult {
        let start_time = std::time::Instant::now();
        
        // Simulate database health check
        let query_time = 45.0;
        let active_connections = 15;
        let connection_pool_usage = 30.0;
        
        let status = if query_time > self.thresholds.critical_threshold {
            MonitorStatus::Critical
        } else if query_time > self.thresholds.warning_threshold {
            MonitorStatus::Warning
        } else {
            MonitorStatus::Healthy
        };

        let mut metrics = HashMap::new();
        metrics.insert("query_time_ms".to_string(), query_time);
        metrics.insert("active_connections".to_string(), active_connections as f64);
        metrics.insert("connection_pool_usage_percent".to_string(), connection_pool_usage);

        MonitorResult {
            monitor_name: self.name.clone(),
            status,
            message: format!("Query time: {:.1}ms, Connections: {}", query_time, active_connections),
            metrics,
            timestamp: Utc::now(),
            response_time_ms: start_time.elapsed().as_millis() as u64,
        }
    }

    fn is_critical(&self) -> bool {
        true
    }

    fn get_thresholds(&self) -> MonitorThresholds {
        self.thresholds.clone()
    }
}

/// System metrics collector
#[derive(Debug)]
pub struct SystemMetricsCollector {
    name: String,
}

impl SystemMetricsCollector {
    pub fn new() -> Self {
        Self {
            name: "system_metrics".to_string(),
        }
    }
}

impl MetricsCollector for SystemMetricsCollector {
    fn name(&self) -> &str {
        &self.name
    }

    fn collect(&self) -> Vec<Metric> {
        let now = Utc::now();
        let mut tags = HashMap::new();
        tags.insert("source".to_string(), "system".to_string());

        vec![
            Metric {
                name: "cpu_usage_percent".to_string(),
                value: 45.2,
                unit: "percent".to_string(),
                tags: tags.clone(),
                timestamp: now,
            },
            Metric {
                name: "memory_usage_percent".to_string(),
                value: 67.8,
                unit: "percent".to_string(),
                tags: tags.clone(),
                timestamp: now,
            },
            Metric {
                name: "disk_usage_percent".to_string(),
                value: 23.1,
                unit: "percent".to_string(),
                tags: tags.clone(),
                timestamp: now,
            },
            Metric {
                name: "network_io_bytes_per_sec".to_string(),
                value: 1024000.0,
                unit: "bytes_per_sec".to_string(),
                tags,
                timestamp: now,
            },
        ]
    }

    fn collection_interval(&self) -> Duration {
        Duration::seconds(30)
    }
}

/// Application metrics collector
#[derive(Debug)]
pub struct ApplicationMetricsCollector {
    name: String,
}

impl ApplicationMetricsCollector {
    pub fn new() -> Self {
        Self {
            name: "application_metrics".to_string(),
        }
    }
}

impl MetricsCollector for ApplicationMetricsCollector {
    fn name(&self) -> &str {
        &self.name
    }

    fn collect(&self) -> Vec<Metric> {
        let now = Utc::now();
        let mut tags = HashMap::new();
        tags.insert("source".to_string(), "application".to_string());

        vec![
            Metric {
                name: "request_rate_per_sec".to_string(),
                value: 125.5,
                unit: "requests_per_sec".to_string(),
                tags: tags.clone(),
                timestamp: now,
            },
            Metric {
                name: "error_rate_percent".to_string(),
                value: 0.5,
                unit: "percent".to_string(),
                tags: tags.clone(),
                timestamp: now,
            },
            Metric {
                name: "average_response_time_ms".to_string(),
                value: 45.2,
                unit: "milliseconds".to_string(),
                tags: tags.clone(),
                timestamp: now,
            },
            Metric {
                name: "cache_hit_rate_percent".to_string(),
                value: 92.3,
                unit: "percent".to_string(),
                tags,
                timestamp: now,
            },
        ]
    }

    fn collection_interval(&self) -> Duration {
        Duration::seconds(60)
    }
}

impl AlertManager {
    pub fn new(config: AlertConfig) -> Self {
        Self {
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    pub async fn create_alert(&self, monitor_result: &MonitorResult) -> Option<Alert> {
        if monitor_result.status == MonitorStatus::Healthy {
            return None;
        }

        let severity = match monitor_result.status {
            MonitorStatus::Warning => AlertSeverity::Warning,
            MonitorStatus::Critical => AlertSeverity::Critical,
            _ => AlertSeverity::Info,
        };

        let alert = Alert {
            id: Uuid::new_v4().to_string(),
            monitor_name: monitor_result.monitor_name.clone(),
            severity,
            title: format!("Monitor Alert: {}", monitor_result.monitor_name),
            description: monitor_result.message.clone(),
            metrics: monitor_result.metrics.clone(),
            created_at: Utc::now(),
            resolved_at: None,
            acknowledged_at: None,
            status: AlertStatus::Active,
        };

        // Check for existing alert
        let mut active_alerts = self.active_alerts.write().await;
        if !active_alerts.contains_key(&monitor_result.monitor_name) {
            active_alerts.insert(monitor_result.monitor_name.clone(), alert.clone());
            
            // Add to history
            let mut history = self.alert_history.write().await;
            history.push(alert.clone());
            
            // Trim history if needed
            if history.len() > self.config.max_alert_history {
                history.remove(0);
            }
            
            info!("Created alert: {} - {}", alert.title, alert.description);
            Some(alert)
        } else {
            None
        }
    }

    pub async fn resolve_alert(&self, monitor_name: &str) {
        let mut active_alerts = self.active_alerts.write().await;
        if let Some(mut alert) = active_alerts.remove(monitor_name) {
            alert.resolved_at = Some(Utc::now());
            alert.status = AlertStatus::Resolved;
            
            info!("Resolved alert: {}", alert.title);
        }
    }

    pub async fn get_active_alerts(&self) -> Vec<Alert> {
        let active_alerts = self.active_alerts.read().await;
        active_alerts.values().cloned().collect()
    }

    pub async fn get_alert_history(&self, limit: Option<usize>) -> Vec<Alert> {
        let history = self.alert_history.read().await;
        let limit = limit.unwrap_or(100);
        history.iter().rev().take(limit).cloned().collect()
    }
}

impl AdvancedMonitoringEngine {
    pub fn new(config: MonitoringConfig) -> Self {
        let alert_manager = Arc::new(AlertManager::new(AlertConfig::default()));
        
        Self {
            monitors: Arc::new(RwLock::new(HashMap::new())),
            collectors: Arc::new(RwLock::new(HashMap::new())),
            alert_manager,
            config,
            system_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
        }
    }

    pub async fn initialize_default_monitors(&self) -> Result<(), RiskError> {
        info!("Initializing default monitors");

        let mut monitors = self.monitors.write().await;
        
        // Add health check monitors
        monitors.insert(
            "api_health".to_string(),
            Box::new(HealthCheckMonitor::new("API Health".to_string(), "/health".to_string()))
        );
        
        // Add performance monitor
        monitors.insert(
            "system_performance".to_string(),
            Box::new(PerformanceMonitor::new("System Performance".to_string()))
        );
        
        // Add database monitor
        monitors.insert(
            "database_health".to_string(),
            Box::new(DatabaseMonitor::new("Database Health".to_string()))
        );

        // Initialize metrics collectors
        let mut collectors = self.collectors.write().await;
        collectors.insert(
            "system_metrics".to_string(),
            Box::new(SystemMetricsCollector::new())
        );
        collectors.insert(
            "application_metrics".to_string(),
            Box::new(ApplicationMetricsCollector::new())
        );

        info!("Initialized {} monitors and {} collectors", monitors.len(), collectors.len());
        Ok(())
    }

    pub async fn run_all_monitors(&self) -> Vec<MonitorResult> {
        let monitors = self.monitors.read().await;
        let mut results = Vec::new();

        for (_, monitor) in monitors.iter() {
            let result = monitor.check();
            
            // Create alert if needed
            if result.status != MonitorStatus::Healthy {
                self.alert_manager.create_alert(&result).await;
            } else {
                // Resolve alert if monitor is healthy
                self.alert_manager.resolve_alert(&result.monitor_name).await;
            }
            
            results.push(result);
        }

        info!("Completed monitoring check for {} monitors", results.len());
        results
    }

    pub async fn collect_all_metrics(&self) -> Vec<Metric> {
        let collectors = self.collectors.read().await;
        let mut all_metrics = Vec::new();

        for (_, collector) in collectors.iter() {
            let metrics = collector.collect();
            all_metrics.extend(metrics);
        }

        // Update system metrics
        self.update_system_metrics(&all_metrics).await;

        info!("Collected {} metrics", all_metrics.len());
        all_metrics
    }

    pub async fn get_system_health(&self) -> SystemHealthReport {
        let monitor_results = self.run_all_monitors().await;
        let metrics = self.collect_all_metrics().await;
        let active_alerts = self.alert_manager.get_active_alerts().await;
        let system_metrics = (*self.system_metrics.read().await).clone();

        let overall_status = if active_alerts.iter().any(|a| a.severity == AlertSeverity::Critical) {
            MonitorStatus::Critical
        } else if active_alerts.iter().any(|a| a.severity == AlertSeverity::Warning) {
            MonitorStatus::Warning
        } else {
            MonitorStatus::Healthy
        };

        SystemHealthReport {
            overall_status,
            monitor_results,
            active_alerts,
            system_metrics,
            metrics_summary: self.summarize_metrics(&metrics),
            generated_at: Utc::now(),
        }
    }

    pub async fn get_monitoring_stats(&self) -> MonitoringStats {
        let monitors = self.monitors.read().await;
        let collectors = self.collectors.read().await;
        let active_alerts = self.alert_manager.get_active_alerts().await;
        let alert_history = self.alert_manager.get_alert_history(Some(100)).await;

        MonitoringStats {
            total_monitors: monitors.len(),
            total_collectors: collectors.len(),
            active_alerts_count: active_alerts.len(),
            total_alerts_24h: alert_history.iter()
                .filter(|a| a.created_at > Utc::now() - Duration::hours(24))
                .count(),
            uptime_percentage: 99.5, // Mock calculation
            last_check: Utc::now(),
        }
    }

    async fn update_system_metrics(&self, metrics: &[Metric]) {
        let mut system_metrics = self.system_metrics.write().await;
        
        for metric in metrics {
            match metric.name.as_str() {
                "cpu_usage_percent" => system_metrics.cpu_usage_percent = metric.value,
                "memory_usage_percent" => system_metrics.memory_usage_percent = metric.value,
                "disk_usage_percent" => system_metrics.disk_usage_percent = metric.value,
                "network_io_bytes_per_sec" => system_metrics.network_io_bytes_per_sec = metric.value as u64,
                "request_rate_per_sec" => system_metrics.request_rate_per_sec = metric.value,
                "error_rate_percent" => system_metrics.error_rate_percent = metric.value,
                "average_response_time_ms" => system_metrics.average_response_time_ms = metric.value,
                "cache_hit_rate_percent" => system_metrics.cache_hit_rate_percent = metric.value,
                _ => {}
            }
        }
        
        system_metrics.last_updated = Utc::now();
    }

    fn summarize_metrics(&self, metrics: &[Metric]) -> MetricsSummary {
        let mut summary = MetricsSummary {
            total_metrics: metrics.len(),
            metrics_by_category: HashMap::new(),
            average_values: HashMap::new(),
        };

        for metric in metrics {
            let category = metric.tags.get("source").unwrap_or(&"unknown".to_string()).clone();
            *summary.metrics_by_category.entry(category).or_insert(0) += 1;
            
            summary.average_values.insert(metric.name.clone(), metric.value);
        }

        summary
    }
}

/// System health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthReport {
    pub overall_status: MonitorStatus,
    pub monitor_results: Vec<MonitorResult>,
    pub active_alerts: Vec<Alert>,
    pub system_metrics: SystemMetrics,
    pub metrics_summary: MetricsSummary,
    pub generated_at: DateTime<Utc>,
}

/// Metrics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub total_metrics: usize,
    pub metrics_by_category: HashMap<String, usize>,
    pub average_values: HashMap<String, f64>,
}

/// Monitoring statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringStats {
    pub total_monitors: usize,
    pub total_collectors: usize,
    pub active_alerts_count: usize,
    pub total_alerts_24h: usize,
    pub uptime_percentage: f64,
    pub last_check: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check_monitor() {
        let monitor = HealthCheckMonitor::new("test".to_string(), "/health".to_string());
        let result = monitor.check();
        
        assert_eq!(result.monitor_name, "test");
        assert!(result.metrics.contains_key("response_time_ms"));
    }

    #[tokio::test]
    async fn test_monitoring_engine() {
        let engine = AdvancedMonitoringEngine::new(MonitoringConfig::default());
        engine.initialize_default_monitors().await.unwrap();
        
        let results = engine.run_all_monitors().await;
        assert!(!results.is_empty());
        
        let metrics = engine.collect_all_metrics().await;
        assert!(!metrics.is_empty());
    }

    #[tokio::test]
    async fn test_alert_manager() {
        let alert_manager = AlertManager::new(AlertConfig::default());
        
        let monitor_result = MonitorResult {
            monitor_name: "test_monitor".to_string(),
            status: MonitorStatus::Critical,
            message: "Test alert".to_string(),
            metrics: HashMap::new(),
            timestamp: Utc::now(),
            response_time_ms: 100,
        };
        
        let alert = alert_manager.create_alert(&monitor_result).await;
        assert!(alert.is_some());
        
        let active_alerts = alert_manager.get_active_alerts().await;
        assert_eq!(active_alerts.len(), 1);
    }
}
