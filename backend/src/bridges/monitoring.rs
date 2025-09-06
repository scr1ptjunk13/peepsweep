use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::interval;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use super::{BridgeIntegration, BridgeError, CrossChainParams};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeMetrics {
    pub bridge_name: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f64,
    pub average_response_time_ms: f64,
    pub last_24h_requests: u64,
    pub last_24h_success_rate: f64,
    pub uptime_percentage: f64,
    pub last_health_check: u64,
    pub is_healthy: bool,
    pub error_breakdown: HashMap<String, u64>,
    pub performance_trend: PerformanceTrend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrend {
    pub response_time_trend: Vec<ResponseTimePoint>,
    pub success_rate_trend: Vec<SuccessRatePoint>,
    pub volume_trend: Vec<VolumePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseTimePoint {
    pub timestamp: u64,
    pub avg_response_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessRatePoint {
    pub timestamp: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumePoint {
    pub timestamp: u64,
    pub request_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub total_bridges: usize,
    pub healthy_bridges: usize,
    pub unhealthy_bridges: usize,
    pub system_uptime_hours: f64,
    pub total_system_requests: u64,
    pub system_success_rate: f64,
    pub average_system_response_time_ms: f64,
    pub active_routes: usize,
    pub bridge_metrics: Vec<BridgeMetrics>,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub bridge_name: Option<String>,
    pub message: String,
    pub timestamp: u64,
    pub resolved: bool,
    pub resolved_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AlertType {
    LowSuccessRate,
    HighErrorRate,
    SlowResponse,
    BridgeDown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone)]
struct RequestRecord {
    pub bridge_name: String,
    pub timestamp: Instant,
    pub success: bool,
    pub response_time_ms: u64,
    pub error: Option<String>,
}

pub struct BridgeMonitor {
    bridges: HashMap<String, Box<dyn BridgeIntegration + Send + Sync>>,
    metrics: Arc<RwLock<HashMap<String, BridgeMetrics>>>,
    system_start_time: Instant,
    request_history: Arc<RwLock<Vec<RequestRecord>>>,
    alerts: Arc<RwLock<Vec<Alert>>>,
    alert_thresholds: AlertThresholds,
}

#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub min_success_rate: f64,
    pub max_response_time_ms: u64,
    pub max_error_rate: f64,
    pub health_check_interval_seconds: u64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            min_success_rate: 95.0,
            max_response_time_ms: 5000,
            max_error_rate: 5.0,
            health_check_interval_seconds: 60,
        }
    }
}

impl BridgeMonitor {
    pub fn new(thresholds: Option<AlertThresholds>) -> Self {
        Self {
            bridges: HashMap::new(),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            system_start_time: Instant::now(),
            request_history: Arc::new(RwLock::new(Vec::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            alert_thresholds: thresholds.unwrap_or_default(),
        }
    }

    pub fn add_bridge(&mut self, bridge: Box<dyn BridgeIntegration + Send + Sync>) {
        let name = bridge.name().to_string();
        self.bridges.insert(name.clone(), bridge);
        
        // Initialize metrics for the bridge
        tokio::spawn({
            let metrics = self.metrics.clone();
            let bridge_name = name.clone();
            async move {
                let mut metrics_guard = metrics.write().await;
                metrics_guard.insert(bridge_name.clone(), BridgeMetrics {
                    bridge_name,
                    total_requests: 0,
                    successful_requests: 0,
                    failed_requests: 0,
                    success_rate: 100.0,
                    average_response_time_ms: 0.0,
                    last_24h_requests: 0,
                    last_24h_success_rate: 100.0,
                    uptime_percentage: 100.0,
                    last_health_check: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    is_healthy: true,
                    error_breakdown: HashMap::new(),
                    performance_trend: PerformanceTrend {
                        response_time_trend: Vec::new(),
                        success_rate_trend: Vec::new(),
                        volume_trend: Vec::new(),
                    },
                });
            }
        });
    }

    pub async fn record_request(&self, bridge_name: &str, success: bool, response_time_ms: u64, error: Option<String>) {
        let record = RequestRecord {
            bridge_name: bridge_name.to_string(),
            timestamp: Instant::now(),
            success,
            response_time_ms,
            error: error.clone(),
        };

        // Add to request history
        {
            let mut history = self.request_history.write().await;
            history.push(record);
            
            // Keep only last 10000 records to prevent memory bloat
            if history.len() > 10000 {
                history.drain(0..1000);
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            if let Some(bridge_metrics) = metrics.get_mut(bridge_name) {
                bridge_metrics.total_requests += 1;
                
                if success {
                    bridge_metrics.successful_requests += 1;
                } else {
                    bridge_metrics.failed_requests += 1;
                    
                    if let Some(err) = error {
                        *bridge_metrics.error_breakdown.entry(err).or_insert(0) += 1;
                    }
                }

                bridge_metrics.success_rate = if bridge_metrics.total_requests > 0 {
                    (bridge_metrics.successful_requests as f64 / bridge_metrics.total_requests as f64) * 100.0
                } else {
                    100.0
                };

                // Update average response time
                let total_response_time = bridge_metrics.average_response_time_ms * (bridge_metrics.total_requests - 1) as f64 + response_time_ms as f64;
                bridge_metrics.average_response_time_ms = total_response_time / bridge_metrics.total_requests as f64;
            }
        }

        // Check for alerts
        self.check_alerts(bridge_name).await;
    }

    async fn check_alerts(&self, bridge_name: &str) {
        let metrics = self.metrics.read().await;
        if let Some(bridge_metrics) = metrics.get(bridge_name) {
            let mut alerts_to_add = Vec::new();

            // Check success rate
            if bridge_metrics.success_rate < self.alert_thresholds.min_success_rate && bridge_metrics.total_requests >= 10 {
                alerts_to_add.push(Alert {
                    id: format!("low_success_rate_{}_{}", bridge_name, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
                    alert_type: AlertType::LowSuccessRate,
                    severity: AlertSeverity::Warning,
                    bridge_name: Some(bridge_name.to_string()),
                    message: format!("Bridge {} has low success rate: {:.1}%", bridge_name, bridge_metrics.success_rate),
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    resolved: false,
                    resolved_at: None,
                });
            }

            // Check response time
            if bridge_metrics.average_response_time_ms > self.alert_thresholds.max_response_time_ms as f64 {
                alerts_to_add.push(Alert {
                    id: format!("slow_response_{}_{}", bridge_name, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
                    alert_type: AlertType::SlowResponse,
                    severity: AlertSeverity::Warning,
                    bridge_name: Some(bridge_name.to_string()),
                    message: format!("Bridge {} has slow response time: {:.1}ms", bridge_name, bridge_metrics.average_response_time_ms),
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    resolved: false,
                    resolved_at: None,
                });
            }

            // Check if bridge is down
            if !bridge_metrics.is_healthy {
                alerts_to_add.push(Alert {
                    id: format!("bridge_down_{}_{}", bridge_name, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
                    alert_type: AlertType::BridgeDown,
                    severity: AlertSeverity::Critical,
                    bridge_name: Some(bridge_name.to_string()),
                    message: format!("Bridge {} is down or unhealthy", bridge_name),
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    resolved: false,
                    resolved_at: None,
                });
            }

            // Add alerts
            if !alerts_to_add.is_empty() {
                let mut alerts = self.alerts.write().await;
                for alert in alerts_to_add {
                    // Check if similar alert already exists
                    let similar_exists = alerts.iter().any(|a| {
                        a.alert_type as u8 == alert.alert_type as u8 &&
                        a.bridge_name == alert.bridge_name &&
                        !a.resolved &&
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - a.timestamp < 300 // 5 minutes
                    });

                    if !similar_exists {
                        match alert.severity {
                            AlertSeverity::Critical => error!("🚨 CRITICAL ALERT: {}", alert.message),
                            AlertSeverity::Warning => warn!("⚠️ WARNING ALERT: {}", alert.message),
                            AlertSeverity::Info => info!("ℹ️ INFO ALERT: {}", alert.message),
                        }
                        alerts.push(alert);
                    }
                }
            }
        }
    }

    pub async fn start_health_monitoring(&self) {
        let bridges_clone = self.bridges.keys().cloned().collect::<Vec<_>>();
        let metrics = self.metrics.clone();
        let interval_duration = Duration::from_secs(self.alert_thresholds.health_check_interval_seconds);

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                for bridge_name in &bridges_clone {
                    // Simulate health check (in real implementation, this would call bridge.health_check())
                    let is_healthy = true; // Placeholder
                    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

                    let mut metrics_guard = metrics.write().await;
                    if let Some(bridge_metrics) = metrics_guard.get_mut(bridge_name) {
                        bridge_metrics.is_healthy = is_healthy;
                        bridge_metrics.last_health_check = timestamp;
                        
                        // Update uptime percentage
                        if is_healthy {
                            bridge_metrics.uptime_percentage = 
                                ((bridge_metrics.uptime_percentage * 0.99) + 1.0).min(100.0);
                        } else {
                            bridge_metrics.uptime_percentage *= 0.99;
                        }
                    }
                }

                debug!("Health check completed for {} bridges", bridges_clone.len());
            }
        });
    }

    pub async fn start_metrics_aggregation(&self) {
        let metrics = self.metrics.clone();
        let request_history = self.request_history.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // Every 5 minutes
            
            loop {
                interval.tick().await;
                
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
                let history = request_history.read().await;
                let mut metrics_guard = metrics.write().await;

                for (bridge_name, bridge_metrics) in metrics_guard.iter_mut() {
                    // Calculate 24h metrics
                    let day_ago = now.saturating_sub(86400);
                    let recent_requests: Vec<_> = history.iter()
                        .filter(|r| r.bridge_name == *bridge_name && 
                               r.timestamp.elapsed().as_secs() <= 86400)
                        .collect();

                    bridge_metrics.last_24h_requests = recent_requests.len() as u64;
                    
                    if !recent_requests.is_empty() {
                        let recent_successes = recent_requests.iter().filter(|r| r.success).count();
                        bridge_metrics.last_24h_success_rate = 
                            (recent_successes as f64 / recent_requests.len() as f64) * 100.0;
                    }

                    // Update performance trends
                    let avg_response_time = if !recent_requests.is_empty() {
                        recent_requests.iter().map(|r| r.response_time_ms).sum::<u64>() as f64 / recent_requests.len() as f64
                    } else {
                        0.0
                    };

                    bridge_metrics.performance_trend.response_time_trend.push(ResponseTimePoint {
                        timestamp: now,
                        avg_response_time_ms: avg_response_time,
                    });

                    bridge_metrics.performance_trend.success_rate_trend.push(SuccessRatePoint {
                        timestamp: now,
                        success_rate: bridge_metrics.last_24h_success_rate,
                    });

                    bridge_metrics.performance_trend.volume_trend.push(VolumePoint {
                        timestamp: now,
                        request_count: bridge_metrics.last_24h_requests,
                    });

                    // Keep only last 24 hours of trend data (288 points for 5-minute intervals)
                    if bridge_metrics.performance_trend.response_time_trend.len() > 288 {
                        bridge_metrics.performance_trend.response_time_trend.drain(0..1);
                    }
                    if bridge_metrics.performance_trend.success_rate_trend.len() > 288 {
                        bridge_metrics.performance_trend.success_rate_trend.drain(0..1);
                    }
                    if bridge_metrics.performance_trend.volume_trend.len() > 288 {
                        bridge_metrics.performance_trend.volume_trend.drain(0..1);
                    }
                }

                debug!("Metrics aggregation completed");
            }
        });
    }

    pub async fn get_system_metrics(&self) -> SystemMetrics {
        let metrics = self.metrics.read().await;
        let alerts = self.alerts.read().await;
        
        let total_bridges = metrics.len();
        let healthy_bridges = metrics.values().filter(|m| m.is_healthy).count();
        let unhealthy_bridges = total_bridges - healthy_bridges;

        let system_uptime_hours = self.system_start_time.elapsed().as_secs_f64() / 3600.0;

        let total_system_requests: u64 = metrics.values().map(|m| m.total_requests).sum();
        let total_successful_requests: u64 = metrics.values().map(|m| m.successful_requests).sum();
        
        let system_success_rate = if total_system_requests > 0 {
            (total_successful_requests as f64 / total_system_requests as f64) * 100.0
        } else {
            100.0
        };

        let average_system_response_time_ms = if !metrics.is_empty() {
            metrics.values().map(|m| m.average_response_time_ms).sum::<f64>() / metrics.len() as f64
        } else {
            0.0
        };

        // Count active routes (simplified - in reality would check bridge route support)
        let active_routes = total_bridges * 7; // Assume 7 routes per bridge on average

        SystemMetrics {
            total_bridges,
            healthy_bridges,
            unhealthy_bridges,
            system_uptime_hours,
            total_system_requests,
            system_success_rate,
            average_system_response_time_ms,
            active_routes,
            bridge_metrics: metrics.values().cloned().collect(),
            alerts: alerts.iter().filter(|a| !a.resolved).cloned().collect(),
        }
    }

    pub async fn resolve_alert(&self, alert_id: &str) -> bool {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved = true;
            alert.resolved_at = Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
            info!("✅ Alert resolved: {}", alert.message);
            true
        } else {
            false
        }
    }

    pub async fn get_bridge_metrics(&self, bridge_name: &str) -> Option<BridgeMetrics> {
        let metrics = self.metrics.read().await;
        metrics.get(bridge_name).cloned()
    }

    pub async fn cleanup_old_data(&self) {
        // Clean up old request history (keep only last 24 hours)
        {
            let mut history = self.request_history.write().await;
            let cutoff = Instant::now() - Duration::from_secs(86400);
            history.retain(|record| record.timestamp > cutoff);
        }

        // Clean up old resolved alerts (keep only last 7 days)
        {
            let mut alerts = self.alerts.write().await;
            let cutoff = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - (7 * 86400);
            alerts.retain(|alert| !alert.resolved || alert.timestamp > cutoff);
        }

        debug!("Old monitoring data cleaned up");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_recording() {
        let monitor = BridgeMonitor::new(None);
        
        // Record some requests
        monitor.record_request("test_bridge", true, 100, None).await;
        monitor.record_request("test_bridge", false, 200, Some("API Error".to_string())).await;
        monitor.record_request("test_bridge", true, 150, None).await;

        let metrics = monitor.get_bridge_metrics("test_bridge").await;
        assert!(metrics.is_none()); // Bridge not added to monitor yet
    }

    #[test]
    fn test_alert_thresholds() {
        let thresholds = AlertThresholds {
            min_success_rate: 90.0,
            max_response_time_ms: 3000,
            max_error_rate: 10.0,
            health_check_interval_seconds: 30,
        };

        assert_eq!(thresholds.min_success_rate, 90.0);
        assert_eq!(thresholds.max_response_time_ms, 3000);
    }
}
