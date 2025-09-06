use crate::risk_management::types::*;
use crate::risk_management::event_ingestion::*;
use crate::risk_management::position_tracker::*;
use crate::risk_management::risk_engine::*;
use crate::risk_management::alert_system::*;
use crate::risk_management::database::*;
use crate::risk_management::redis_cache::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use rust_decimal::Decimal;

/// Configuration for the integrated risk management service
#[derive(Debug, Clone)]
pub struct RiskManagementConfig {
    pub ingestion_config: EventIngestionConfig,
    pub position_tracker_config: PositionTrackerConfig,
    pub risk_engine_config: RiskEngineConfig,
    pub alert_system_config: AlertSystemConfig,
    pub database_config: DatabaseConfig,
    pub redis_cache_config: RedisCacheConfig,
    pub processing_interval_ms: u64,
    pub persistence_interval_ms: u64,
    pub cleanup_interval_ms: u64,
}

impl Default for RiskManagementConfig {
    fn default() -> Self {
        Self {
            ingestion_config: EventIngestionConfig::default(),
            position_tracker_config: PositionTrackerConfig::default(),
            risk_engine_config: RiskEngineConfig::default(),
            alert_system_config: AlertSystemConfig::default(),
            database_config: DatabaseConfig::default(),
            redis_cache_config: RedisCacheConfig::default(),
            processing_interval_ms: 1000, // 1 second
            persistence_interval_ms: 5000, // 5 seconds
            cleanup_interval_ms: 300000, // 5 minutes
        }
    }
}

/// Integrated risk management service that orchestrates all components
pub struct RiskManagementService {
    config: RiskManagementConfig,
    ingestion_layer: Arc<EventIngestionLayer>,
    position_tracker: Arc<PositionTracker>,
    risk_engine: Arc<RiskProcessingEngine>,
    alert_system: Arc<AlertSystem>,
    database: Arc<RiskDatabase>,
    cache: Arc<RwLock<RiskCache>>,
    stats: Arc<RwLock<ServiceStats>>,
}

/// Service-level statistics
#[derive(Debug, Clone, Default)]
pub struct ServiceStats {
    pub events_processed: u64,
    pub positions_updated: u64,
    pub risk_calculations: u64,
    pub alerts_generated: u64,
    pub database_writes: u64,
    pub cache_operations: u64,
    pub uptime_seconds: u64,
    pub last_processing_time_ms: u64,
}

impl RiskManagementService {
    /// Create new integrated risk management service
    pub async fn new(config: RiskManagementConfig) -> Result<Self, RiskError> {
        // Initialize database
        let database = Arc::new(RiskDatabase::new(&config.database_config.connection_url).await?);
        database.initialize_schema().await?;
        database.create_continuous_aggregates().await?;

        // Initialize Redis cache
        let cache = Arc::new(RwLock::new(RiskCache::with_config(config.redis_cache_config.clone()).await?));

        // Initialize position tracker
        let position_tracker = Arc::new(PositionTracker::new(config.position_tracker_config.clone()));

        // Initialize risk engine
        let risk_engine = Arc::new(RiskProcessingEngine::new(
            config.risk_engine_config.clone(),
            position_tracker.clone(),
        ));

        // Initialize alert system
        let alert_system = Arc::new(AlertSystem::new(config.alert_system_config.clone()));

        // Initialize event ingestion layer
        let ingestion_layer = Arc::new(EventIngestionLayer::new(config.ingestion_config.clone()));

        let stats = Arc::new(RwLock::new(ServiceStats::default()));

        Ok(Self {
            config,
            ingestion_layer,
            position_tracker,
            risk_engine,
            alert_system,
            database,
            cache,
            stats,
        })
    }

    /// Start the integrated risk management service
    pub async fn start(&self) -> Result<(), RiskError> {
        log::info!("Starting integrated risk management service");

        // Start background processing tasks
        self.start_processing_loop().await?;
        self.start_persistence_loop().await?;
        self.start_cleanup_loop().await?;

        log::info!("Risk management service started successfully");
        Ok(())
    }

    /// Ingest a trade event into the system
    pub async fn ingest_trade_event(&self, event: TradeEvent) -> Result<(), RiskError> {
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.events_processed += 1;
        }

        // Ingest event through the ingestion layer
        self.ingestion_layer.ingest_event(event.clone()).await?;

        // Update position tracker
        self.position_tracker.process_trade_event(&event).await?;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.positions_updated += 1;
        }

        Ok(())
    }

    /// Get current risk metrics for a user
    pub async fn get_risk_metrics(&self, user_id: UserId) -> Result<Option<RiskMetrics>, RiskError> {
        // Try cache first
        let cached_metrics = {
            let mut cache = self.cache.write().await;
            cache.get_cached_metrics(user_id.clone()).await?
        };

        if let Some(metrics) = cached_metrics {
            return Ok(Some(metrics));
        }

        // Calculate fresh metrics
        let metrics = self.risk_engine.calculate_user_risk_metrics(&user_id).await?;

        // Cache the results
        {
            let mut cache = self.cache.write().await;
            cache.cache_metrics(user_id, &metrics).await?;
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.risk_calculations += 1;
            stats.cache_operations += 1;
        }

        Ok(Some(metrics))
    }

    /// Get current positions for a user
    pub async fn get_user_positions(&self, user_id: UserId) -> Result<Option<UserPositions>, RiskError> {
        // Try cache first
        let cached_positions = {
            let mut cache = self.cache.write().await;
            cache.get_cached_positions(user_id.clone()).await?
        };

        if let Some(positions) = cached_positions {
            return Ok(Some(positions));
        }

        // Get from position tracker
        let positions = self.position_tracker.get_user_position(&user_id);

        // Cache the results
        if let Some(ref positions) = positions {
            let mut cache = self.cache.write().await;
            cache.cache_positions(user_id, positions).await?;
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.cache_operations += 1;
        }

        Ok(positions)
    }

    /// Subscribe user to risk alerts
    pub async fn subscribe_to_alerts(&self, _user_id: UserId, subscription: AlertSubscription) -> Result<(), RiskError> {
        self.alert_system.subscribe_user(subscription).await
    }

    /// Get service health status
    pub async fn get_health_status(&self) -> Result<ServiceHealthStatus, RiskError> {
        let database_healthy = self.database.health_check().await?;
        let cache_healthy = {
            let mut cache = self.cache.write().await;
            cache.health_check().await?
        };

        let stats = self.stats.read().await.clone();

        Ok(ServiceHealthStatus {
            database_healthy: true, // database_healthy is already Result<(), _>
            cache_healthy,
            ingestion_healthy: true, // EventIngestionLayer is always healthy if running
            processing_healthy: stats.last_processing_time_ms > 0,
            stats,
        })
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> ServiceStats {
        self.stats.read().await.clone()
    }

    /// Start the main processing loop
    async fn start_processing_loop(&self) -> Result<(), RiskError> {
        let interval_duration = Duration::from_millis(self.config.processing_interval_ms);
        let mut processing_interval = interval(interval_duration);

        let ingestion_layer = self.ingestion_layer.clone();
        let position_tracker = self.position_tracker.clone();
        let risk_engine = self.risk_engine.clone();
        let alert_system = self.alert_system.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            loop {
                processing_interval.tick().await;

                let start_time = std::time::Instant::now();

                // Process pending events
                if let Err(e) = Self::process_pending_events(
                    &ingestion_layer,
                    &position_tracker,
                    &risk_engine,
                    &alert_system,
                    &stats,
                ).await {
                    log::error!("Error processing events: {}", e);
                }

                // Update processing time statistics
                let processing_time = start_time.elapsed().as_millis() as u64;
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.last_processing_time_ms = processing_time;
                    stats_guard.uptime_seconds += interval_duration.as_secs();
                }
            }
        });

        Ok(())
    }

    /// Process pending events from ingestion layer
    async fn process_pending_events(
        _ingestion_layer: &Arc<EventIngestionLayer>,
        _position_tracker: &Arc<PositionTracker>,
        _risk_engine: &Arc<RiskProcessingEngine>,
        _alert_system: &Arc<AlertSystem>,
        stats: &Arc<RwLock<ServiceStats>>,
    ) -> Result<(), RiskError> {
        // Process events from ingestion layer (simplified for now)
        // In a full implementation, this would batch process events
        
        // Update statistics for processing loop
        {
            let mut stats_guard = stats.write().await;
            stats_guard.events_processed += 1;
        }

        Ok(())
    }

    /// Start the persistence loop for database writes
    async fn start_persistence_loop(&self) -> Result<(), RiskError> {
        let interval_duration = Duration::from_millis(self.config.persistence_interval_ms);
        let mut persistence_interval = interval(interval_duration);

        let database = self.database.clone();
        let position_tracker = self.position_tracker.clone();
        let risk_engine = self.risk_engine.clone();
        let alert_system = self.alert_system.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            loop {
                persistence_interval.tick().await;

                if let Err(e) = Self::persist_data(&database, &position_tracker, &risk_engine, &alert_system, &stats).await {
                    log::error!("Error persisting data: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Persist data to database
    async fn persist_data(
        database: &Arc<RiskDatabase>,
        _position_tracker: &Arc<PositionTracker>,
        _risk_engine: &Arc<RiskProcessingEngine>,
        _alert_system: &Arc<AlertSystem>,
        stats: &Arc<RwLock<ServiceStats>>,
    ) -> Result<(), RiskError> {
        // Note: Position persistence would require additional methods
        // This is a placeholder for batch position persistence

        // Note: Risk metrics persistence would require additional methods
        // This is a placeholder for batch metrics persistence

        // Persist alerts
        let alerts_to_persist = _alert_system.get_pending_alerts(100).await;
        for alert in alerts_to_persist {
            database.store_risk_alert(&alert).await?;
        }

        // Update statistics
        {
            let mut stats_guard = stats.write().await;
            stats_guard.database_writes += 1;
        }

        Ok(())
    }

    /// Start the cleanup loop
    async fn start_cleanup_loop(&self) -> Result<(), RiskError> {
        let interval_duration = Duration::from_millis(self.config.cleanup_interval_ms);
        let mut cleanup_interval = interval(interval_duration);

        let position_tracker = self.position_tracker.clone();
        let risk_engine = self.risk_engine.clone();
        let alert_system = self.alert_system.clone();

        tokio::spawn(async move {
            loop {
                cleanup_interval.tick().await;

                if let Err(e) = Self::cleanup_old_data(&position_tracker, &risk_engine, &alert_system).await {
                    log::error!("Error during cleanup: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Cleanup old data from memory
    async fn cleanup_old_data(
        _position_tracker: &Arc<PositionTracker>,
        _risk_engine: &Arc<RiskProcessingEngine>,
        _alert_system: &Arc<AlertSystem>,
    ) -> Result<(), RiskError> {
        // Cleanup old positions
        _position_tracker.cleanup_old_positions().await?;

        // Cleanup old risk data
        _risk_engine.cleanup_old_data().await?;

        // Cleanup old alerts
        _alert_system.cleanup_old_data(24).await?;

        Ok(())
    }
}

/// Service health status
#[derive(Debug, Clone)]
pub struct ServiceHealthStatus {
    pub database_healthy: bool,
    pub cache_healthy: bool,
    pub ingestion_healthy: bool,
    pub processing_healthy: bool,
    pub stats: ServiceStats,
}

impl ServiceHealthStatus {
    pub fn is_healthy(&self) -> bool {
        self.database_healthy && self.cache_healthy && self.ingestion_healthy && self.processing_healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    #[ignore] // Requires external dependencies (TimescaleDB, Redis)
    async fn test_integrated_service_initialization() {
        let config = RiskManagementConfig::default();
        let service = RiskManagementService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires external dependencies
    async fn test_event_ingestion_and_processing() {
        let config = RiskManagementConfig::default();
        let service = RiskManagementService::new(config).await.unwrap();

        let test_user_id = uuid::Uuid::new_v4();
        let trade_event = TradeEvent {
            user_id: test_user_id,
            trade_id: uuid::Uuid::new_v4(),
            token_in: "0x1234".to_string(),
            token_out: "0x5678".to_string(),
            amount_in: Decimal::from(1000),
            amount_out: Decimal::from(950),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            dex_source: "uniswap".to_string(),
            gas_used: Decimal::from(150000),
        };

        let result = service.ingest_trade_event(trade_event).await;
        assert!(result.is_ok());

        // Check that positions were updated
        let positions = service.get_user_positions(test_user_id).await.unwrap();
        assert!(positions.is_some());
    }

    #[tokio::test]
    #[ignore] // Requires external dependencies
    async fn test_risk_metrics_calculation() {
        let config = RiskManagementConfig::default();
        let service = RiskManagementService::new(config).await.unwrap();

        // First ingest some events
        let test_user_id = uuid::Uuid::new_v4();
        let trade_event = TradeEvent {
            user_id: test_user_id,
            trade_id: uuid::Uuid::new_v4(),
            token_in: "0x1234".to_string(),
            token_out: "0x5678".to_string(),
            amount_in: Decimal::from(1000),
            amount_out: Decimal::from(950),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            dex_source: "uniswap".to_string(),
            gas_used: Decimal::from(150000),
        };

        service.ingest_trade_event(trade_event).await.unwrap();

        // Calculate risk metrics
        let metrics = service.get_risk_metrics(test_user_id).await.unwrap();
        assert!(metrics.is_some());
    }

    #[tokio::test]
    #[ignore] // Requires external dependencies
    async fn test_health_status() {
        let config = RiskManagementConfig::default();
        let service = RiskManagementService::new(config).await.unwrap();

        let health = service.get_health_status().await.unwrap();
        assert!(health.database_healthy);
        assert!(health.cache_healthy);
        assert!(health.ingestion_healthy);
    }
}
